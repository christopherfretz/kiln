//! Values and static storage ([meta.reflection.extract],
//! [meta.reflection.result], [meta.define.static]; P3491; WP-479). The
//! results are interned by content (spec/reflection.md §2), so each op is
//! a pure function of its operand: `Stable`. `reflect_constant_string`
//! and `reflect_constant_array` are library functions over the
//! `__kiln_` intrinsics of the same name, which they hand a built-in
//! array as is and any other range as a span; `define_static_string`,
//! `define_static_array` and `define_static_object` are library functions
//! over these (as libstdc++ defines them; `tests/support/kiln_meta.h`),
//! not intrinsics; `is_string_literal` is one over
//! `__kiln_is_string_literal` (libstdc++: `__builtin_is_string_literal`).
//!
//! The Oven half (spec/reflection-values.md): freezing an operand into the
//! constant store ([`Oven::meta_freeze`], [`Oven::meta_freeze_elems`]),
//! reading a range of any element type ([`elems_recipe`]), and thawing a
//! value op's result into the call's result ([`Oven::meta_value_out`]):
//! a scalar into the result register, a class by a copy into the result
//! object, a reference as the address of the object it designates.

use kiln_ast::consts::{ByteImage, Scalar};
use kiln_ast::{ConstValueId, QualType};
use kiln_source::Loc;

use super::materialize::{Leaf, Recipe};
use super::{MetaCtx, MetaError, MetaErrorKind, MetaOut, OvenFault};
use crate::kbc::ak;
use crate::mem::{Origin, Ptr};
use crate::ty::{OTy, TyKind, Types};
use crate::value::Freeze;
use crate::vm::Fault;
use crate::{Oven, SemaBridge};

super::meta_family! {
    /// The value family.
    pub enum ValueOp {
        /// `extract<T>(r)` (a specialization's signature).
        Extract = ("extract", "i", Stable, 479);
        ReflectConstant = ("reflect_constant", "x", Stable, 479);
        ReflectObject = ("reflect_object", "x", Stable, 479);
        ReflectFunction = ("reflect_function", "x", Stable, 479);
        /// `reflect_constant_string(r)` is a library function over
        /// `std::meta::__kiln_reflect_constant_string`, which reads a
        /// built-in array or a contiguous range by layout (the library
        /// passes it a span of any other range).
        ReflectConstantString = ("__kiln_reflect_constant_string", "x", Stable, 479);
        /// `reflect_constant_array(r)`: likewise.
        ReflectConstantArray = ("__kiln_reflect_constant_array", "x", Stable, 479);
        /// `reflect_constant(info)`: `T` is `std::meta::info`.
        ReflectConstantInfo = ("reflect_constant", "i", Stable, 479);
        /// `std::is_string_literal(p)` ([meta.string.literal]) is
        /// `std::meta::__kiln_is_string_literal(p)`.
        IsStringLiteral = ("__kiln_is_string_literal", "x", Stable, 479);
        /// `reflect_constant_string(r)` as libstdc++ 16.1 declares it
        /// (no body; WP-484): the intrinsic itself.
        ReflectConstantStringStd = ("reflect_constant_string", "x", Stable, 484);
        /// `reflect_constant_array(r)`: likewise.
        ReflectConstantArrayStd = ("reflect_constant_array", "x", Stable, 484);
        /// libstdc++'s `exception::_S_exception_cvt_from_utf8(r)`: the
        /// UTF-8 range `r` as an ordinary-encoding `string_view` (kiln's
        /// ordinary literal encoding is UTF-8: the same characters, or an
        /// empty view when `r` is not valid UTF-8).
        ExcCvtFromUtf8 = ("exception::_S_exception_cvt_from_utf8", "x", Stable, 484);
        /// `exception::_S_exception_cvt_to_utf8(r)`: the converse.
        ExcCvtToUtf8 = ("exception::_S_exception_cvt_to_utf8", "x", Stable, 484);
    }
}

/// How deep a range class nests subobjects at most.
const MAX_DEPTH: u32 = 8;
/// At most this many non-empty members in a range class.
const MAX_LEAVES: usize = 4;

/// The non-empty scalar and array subobjects of `t` at `base`; a union
/// member is skipped and counted in `unions` (the small-string buffer of
/// libstdc++'s `basic_string`, whose pointer and length come first).
fn range_leaves(
    types: &Types,
    t: OTy,
    base: u64,
    depth: u32,
    out: &mut Vec<Leaf>,
    unions: &mut u32,
) -> Result<(), &'static str> {
    if depth > MAX_DEPTH {
        return Err("nests too deeply");
    }
    match types.kind(t) {
        TyKind::Record { union: true, .. } if depth == 0 => Err("is a union"),
        TyKind::Record { union: true, .. } => {
            *unions += 1;
            Ok(())
        }
        TyKind::Record { .. } => {
            for f in types.fields(t) {
                if f.width.is_some() {
                    return Err("has a bit-field");
                }
                if types.size(f.ty) == 0 || types.holds_no_data(f.ty) {
                    continue;
                }
                range_leaves(types, f.ty, base + f.offset, depth + 1, out, unions)?;
            }
            Ok(())
        }
        TyKind::Int { .. } | TyKind::Ptr(_) | TyKind::Array { .. } => {
            if out.len() == MAX_LEAVES {
                return Err("has too many members");
            }
            out.push(Leaf { off: base, ty: t });
            Ok(())
        }
        _ => Err("has a member of another kind"),
    }
}

/// Matches a contiguous range of any element type against `t`'s layout
/// ([`super::materialize::Shape::Elems`]): a built-in array; a class
/// holding one array (`std::array`), a pointer and a size (`span`,
/// `string_view`, `initializer_list`), or two or three element pointers
/// (`vector`).
///
/// # Errors
/// Why `t` is no such range (for the call-site note).
pub(crate) fn elems_recipe(types: &Types, t: OTy) -> Result<Recipe, String> {
    match types.kind(t) {
        TyKind::Array { elem, n } => return Ok(Recipe::Inline { off: 0, elem, n }),
        TyKind::Record { .. } => {}
        _ => return Err("is not a contiguous range".into()),
    }
    let mut ls = Vec::with_capacity(MAX_LEAVES);
    let mut unions = 0;
    range_leaves(types, t, 0, 0, &mut ls, &mut unions).map_err(str::to_owned)?;
    ls.sort_by_key(|l| l.off);
    let ptr_to = |l: &Leaf| match types.kind(l.ty) {
        TyKind::Ptr(e) => Some(e),
        _ => None,
    };
    let is_int = |l: &Leaf| matches!(types.kind(l.ty), TyKind::Int { .. });
    let same = |a: &Leaf, b: &Leaf| {
        ptr_to(a).is_some_and(|x| ptr_to(b).is_some_and(|y| types.info(x).q == types.info(y).q))
    };
    let found = match ls[..] {
        // A union only after a pointer and a length (a string's buffer).
        _ if unions > 0 && !(ls.len() == 2 && ptr_to(&ls[0]).is_some() && is_int(&ls[1])) => None,
        [] => Some(Recipe::Inline { off: 0, elem: t, n: 0 }),
        [l] => match types.kind(l.ty) {
            TyKind::Array { elem, n } => Some(Recipe::Inline { off: l.off, elem, n }),
            _ => None,
        },
        [a, b] if ptr_to(&a).is_some() && is_int(&b) => Some(Recipe::Span { ptr: a, len: b }),
        [a, b] if is_int(&a) && ptr_to(&b).is_some() => Some(Recipe::Span { ptr: b, len: a }),
        [b, e] if same(&b, &e) => {
            Some(Recipe::Vector { begin: b, end: e, cap: e, elem: ptr_to(&b).unwrap_or(t) })
        }
        [b, e, c] if same(&b, &e) && same(&b, &c) => {
            Some(Recipe::Vector { begin: b, end: e, cap: c, elem: ptr_to(&b).unwrap_or(t) })
        }
        _ => None,
    };
    found.ok_or_else(|| "is not a contiguous range kiln reads (an array, a pointer and a size, or element pointers)".into())
}

/// The element type of a range read through `rc` (an [`elems_recipe`]).
pub(crate) fn recipe_elem(types: &Types, rc: Recipe) -> Option<OTy> {
    match rc {
        Recipe::Inline { elem, .. } | Recipe::Vector { elem, .. } => Some(elem),
        Recipe::Span { ptr, .. } => match types.kind(ptr.ty) {
            TyKind::Ptr(e) => Some(e),
            _ => None,
        },
        _ => None,
    }
}

/// The error of a value that cannot be frozen.
fn freeze_err(f: Freeze) -> MetaError {
    match f {
        Freeze::Notes(notes) => MetaError { notes, kind: MetaErrorKind::Failed },
        Freeze::Fault(k) => {
            let f = if k == crate::FailKind::MemoryLimit { Fault::Memory } else { Fault::Quiet };
            MetaError { notes: Vec::new(), kind: MetaErrorKind::Fault(OvenFault(f)) }
        }
    }
}

fn fault(f: Fault) -> MetaError {
    MetaError { notes: Vec::new(), kind: MetaErrorKind::Fault(OvenFault(f)) }
}

impl MetaError {
    /// The constant store is full: the evaluation exceeded the memory
    /// limit.
    #[must_use]
    pub fn memory() -> Self {
        fault(Fault::Memory)
    }
}

impl Oven {
    /// The element type of the range argument of the value op running
    /// with `ctx` (`range_value_t`, cv-unqualified).
    #[must_use]
    pub fn meta_elem(&self, ctx: &MetaCtx) -> QualType {
        self.types.info(ctx.elem).q
    }

    /// Freezes argument value `v` of type `q` (a scalar's register value;
    /// a class's or array's address) into the constant store: the value a
    /// constant template parameter of type `q` would hold
    /// ([temp.arg.nontype]; the result check of a constant expression).
    ///
    /// # Errors
    /// The notes of a value that is no constant (a pointer to an
    /// automatic object, an uninitialized member).
    pub fn meta_freeze(
        &mut self,
        b: &mut dyn SemaBridge,
        v: u128,
        q: QualType,
        loc: Loc,
    ) -> Result<ConstValueId, MetaError> {
        let ty = self.types.intern(b, q);
        let r = if self.types.is_aggregate(ty) {
            self.freeze_aggregate(b, ty, v as u64, loc)
        } else if self.types.wide(ty).is_some() {
            self.freeze_object(b, v as u64, loc)
        } else {
            self.freeze_scalar(b, ty, v, loc)
        };
        r.map_err(freeze_err)
    }

    /// The value of the object stored pointer `p` designates, frozen as
    /// [`Oven::meta_freeze`] freezes it: `constant_of` of an object
    /// reflection, a subobject of a variable usable in constant
    /// expressions or of a static constant ([meta.reflection.queries];
    /// WP-503). `None`: no object of a type the Oven reads (a wide
    /// integer subobject, a null pointer).
    ///
    /// # Errors
    /// The notes of a value that is no constant; the evaluator's fault
    /// reading it.
    pub fn meta_freeze_at(
        &mut self,
        b: &mut dyn SemaBridge,
        p: ConstValueId,
        ctx: &MetaCtx,
    ) -> Result<Option<ConstValueId>, MetaError> {
        let Some(Scalar::Pointer { ty, .. }) = b.consts().scalar(p) else {
            return Ok(None);
        };
        let pt = self.types.intern(b, ty);
        let TyKind::Ptr(o) = self.types.kind(pt) else { return Ok(None) };
        let addr = self.thaw_ptr(b, p, pt, ctx.top) as u64;
        if addr == 0 || self.types.wide(o).is_some() {
            return Ok(None);
        }
        let r = if self.types.is_aggregate(o) {
            self.freeze_aggregate(b, o, addr, ctx.loc)
        } else {
            let v = self.load(addr, o, ak::READ).map_err(fault)?;
            self.freeze_scalar(b, o, v, ctx.loc)
        };
        r.map(Some).map_err(freeze_err)
    }

    /// The number of elements [`Oven::meta_freeze_elems`] takes from
    /// range argument `r` (data | length << 64): its length, less the
    /// terminator when `string` and the range is a whole string literal
    /// ([meta.reflection.result]: `reflect_constant_string`).
    ///
    /// # Errors
    /// The evaluator's fault reading the last element.
    pub fn meta_elems_len(&self, r: u128, string: bool, ctx: &MetaCtx) -> Result<u64, MetaError> {
        let (p, n) = (r as u64, (r >> 64) as u64);
        if !string || n == 0 {
            return Ok(n);
        }
        let et = ctx.elem;
        let lit = self.alloc_any(Ptr::decode(p)).is_some_and(|a| {
            matches!(a.origin, Origin::Str(..))
                && Ptr::decode(p).off == 0
                && u64::from(a.size) == n * self.types.size(et)
        });
        if !lit {
            return Ok(n);
        }
        let last = self.ptr_add(p, i128::from(n - 1), et).map_err(fault)?;
        let v = self.load(last, et, ak::READ).map_err(fault)?;
        Ok(if v == 0 { n - 1 } else { n })
    }

    /// Freezes the first `m` elements of range argument `r` as an array of
    /// type `arr` (`T[m]`, or `CharT[m + 1]` with the terminator appended
    /// when `string`): the content-interned object of
    /// `reflect_constant_array` / `reflect_constant_string`.
    ///
    /// # Errors
    /// The notes of an element that is no constant; the evaluator's
    /// fault reading one.
    pub fn meta_freeze_elems(
        &mut self,
        b: &mut dyn SemaBridge,
        (r, m): (u128, u64),
        arr: QualType,
        string: bool,
        ctx: &MetaCtx,
    ) -> Result<ConstValueId, MetaError> {
        let p = r as u64;
        let at = self.types.intern(b, arr);
        if !string {
            let cv = self.freeze_aggregate(b, at, p, ctx.loc).map_err(freeze_err)?;
            // The image is typed `arr` itself, not the Oven type's first
            // spelling (`const T[N]` when a const array came first): the
            // same content is one object whatever reached it first.
            return retype(b, cv, arr);
        }
        // Characters are plain integers: their bytes, little-endian as
        // the Oven stores them, then the terminator.
        let et = ctx.elem;
        let w = self.types.size(et);
        let mut bytes = Vec::with_capacity(((m + 1) * w) as usize);
        let mut q = p;
        for i in 0..m {
            if i > 0 {
                q = self.ptr_add(q, 1, et).map_err(fault)?;
            }
            let v = self.load(q, et, ak::READ).map_err(fault)?;
            bytes.extend_from_slice(&v.to_le_bytes()[..w as usize]);
        }
        bytes.resize(((m + 1) * w) as usize, 0);
        let img = ByteImage { ty: arr, bytes: &bytes, relocs: &[], padding: &[], unions: &[] };
        b.consts().push_aggregate(img).map_err(|_| fault(Fault::Memory))
    }

    /// The value of class `q` that holds no data (every byte padding:
    /// libstdc++'s `array<T, 0>`, whose one member is an empty struct),
    /// frozen: `reflect_constant_array` of an empty range (P3491R3).
    /// `None` when `q` holds data.
    ///
    /// # Errors
    /// The constant store's exhaustion.
    pub fn meta_freeze_blank(
        &mut self,
        b: &mut dyn SemaBridge,
        q: QualType,
    ) -> Result<Option<ConstValueId>, MetaError> {
        let t = self.types.intern(b, q);
        if !self.types.holds_no_data(t) {
            return Ok(None);
        }
        let n = self.types.size(t);
        let bytes = vec![0; usize::try_from(n).unwrap_or(0)];
        let len = u32::try_from(n).map_err(|_| fault(Fault::Memory))?;
        let padding = [kiln_ast::consts::ByteRange { start: 0, len }];
        let padding = if n == 0 { &padding[..0] } else { &padding[..] };
        let img = ByteImage { ty: q, bytes: &bytes, relocs: &[], padding, unions: &[] };
        b.consts().push_aggregate(img).map(Some).map_err(|_| fault(Fault::Memory))
    }

    /// The first `m` characters of range argument `r` (data | length <<
    /// 64) of one-byte characters, as bytes (`None`: wider characters).
    ///
    /// # Errors
    /// The evaluator's fault reading one.
    pub fn meta_elems_bytes(
        &self,
        r: u128,
        m: u64,
        ctx: &MetaCtx,
    ) -> Result<Option<Vec<u8>>, MetaError> {
        let et = ctx.elem;
        if self.types.size(et) != 1 {
            return Ok(None);
        }
        let mut q = r as u64;
        let mut bytes = Vec::with_capacity(usize::try_from(m).unwrap_or(0).min(1 << 20));
        for i in 0..m {
            if i > 0 {
                q = self.ptr_add(q, 1, et).map_err(fault)?;
            }
            bytes.push(self.load(q, et, ak::READ).map_err(fault)? as u8);
        }
        Ok(Some(bytes))
    }

    /// Whether `p` points into a string literal object ([lex.string];
    /// `std::is_string_literal`).
    #[must_use]
    pub fn meta_is_string_literal(&self, p: u128) -> bool {
        let p = p as u64;
        p != 0
            && !Ptr::decode(p).is_fn()
            && self.alloc_any(Ptr::decode(p)).is_some_and(|a| matches!(a.origin, Origin::Str(..)))
    }

    /// The register value of value op result `out` of type `ty` (a
    /// [`MetaOut::Frozen`] or [`MetaOut::Deref`]); `whole` is the address
    /// of the result object when the call's result is a class prvalue
    /// (its recipe is [`Recipe::Whole`]). `None`: `out` is not of `ty`.
    pub(crate) fn meta_value_out(
        &mut self,
        b: &mut dyn SemaBridge,
        out: MetaOut,
        ty: OTy,
        whole: Option<u64>,
        top: u32,
    ) -> Result<Option<u128>, Fault> {
        match out {
            MetaOut::Frozen(cv) => {
                if let Some(dst) = whole {
                    if b.consts().aggregate(cv).is_none() {
                        return Ok(None);
                    }
                    let src = self.thaw(b, cv, top);
                    self.copy_obj(dst, src, ty, true, Some(b.ast()))?;
                    return Ok(Some(u128::from(dst)));
                }
                Ok(self.frozen_reg(b, cv, ty, top))
            }
            MetaOut::Deref(cv) => {
                let p = self.thaw_ptr(b, cv, ty, top) as u64;
                if p == 0 {
                    return Ok(None);
                }
                if let Some(dst) = whole {
                    self.copy_obj(dst, p, ty, true, Some(b.ast()))?;
                    return Ok(Some(u128::from(dst)));
                }
                if self.types.is_aggregate(ty) || self.types.wide(ty).is_some() {
                    return Ok(None);
                }
                self.load(p, ty, ak::READ).map(Some)
            }
            _ => Ok(None),
        }
    }

    /// The register value of frozen scalar `cv` as a value of type `ty`
    /// (a reference result: the address of the object it designates).
    fn frozen_reg(
        &mut self,
        b: &mut dyn SemaBridge,
        cv: ConstValueId,
        ty: OTy,
        top: u32,
    ) -> Option<u128> {
        if let TyKind::MemPtr { .. } = self.types.kind(ty) {
            return Some(self.mp_thaw(b, cv));
        }
        Some(match b.consts().scalar(cv)? {
            Scalar::Pointer { .. } => return Some(self.thaw_ptr(b, cv, ty, top)),
            Scalar::Null { .. } => 0,
            Scalar::Int { bits, signed, .. } => {
                let TyKind::Int { width, .. } = self.types.kind(ty) else { return None };
                crate::arith::canon(bits.to_u128()?, u32::from(width), signed)
            }
            Scalar::Bool { value, .. } => u128::from(value),
            Scalar::Float { bits, .. } => bits,
            Scalar::Reflection { id, .. } => u128::from(id.bits()),
            Scalar::MemberPtr { .. } => return None,
        })
    }
}

/// `cv`, an aggregate, with its image typed `ty`.
fn retype(
    b: &mut dyn SemaBridge,
    cv: ConstValueId,
    ty: QualType,
) -> Result<ConstValueId, MetaError> {
    let c = b.consts();
    let Some(img) = c.aggregate(cv) else { return Ok(cv) };
    if img.ty == ty {
        return Ok(cv);
    }
    let (bytes, relocs) = (img.bytes.to_vec(), img.relocs.to_vec());
    let (padding, unions) = (img.padding.to_vec(), img.unions.to_vec());
    let img = ByteImage { ty, bytes: &bytes, relocs: &relocs, padding: &padding, unions: &unions };
    c.push_aggregate(img).map_err(|_| fault(Fault::Memory))
}
