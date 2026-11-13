//! Materialized class-typed results and range arguments
//! (spec/reflection.md §3.5).
//!
//! When a call of an intrinsic whose result (or a `reflection_range`
//! parameter) is of class type is compiled, sema names the op's
//! [`Shape`] for it ([`crate::SemaBridge::meta_shape`]) and the Oven
//! matches it against the class's layout once, into a [`Recipe`]: the
//! offsets and types of the scalar members the shape is built from. The
//! match is by layout, not by header: any `string_view` that holds a
//! character pointer and a size, any `vector` that holds three element
//! pointers, qualifies. A class that does not match is diagnosed at the
//! call site when it is compiled. At run time the bridge's [`MetaOut`]
//! is stored through the recipe into the call's result object.

use kiln_ast::{ConstValueId, DeclId, ExprId, ReflectionId};

use super::{MetaError, MetaErrorKind, MetaOut, OvenFault};
use crate::kbc::{ak, new_aux};
use crate::mem::{Origin, Ptr};
use crate::ty::{OTy, TyKind, Types};
use crate::vm::Fault;
use crate::{Oven, SemaBridge};

/// What a materialized class is built from: sema's contract for the op
/// (spec/reflection.md §3.5, the recipe table).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    /// `basic_string_view<C>`: a pointer to the characters and a size,
    /// in either order (filled from [`MetaOut::Str`]).
    Str,
    /// `vector<info>`: begin, end and end-of-capacity `info` pointers in
    /// that offset order; the elements are allocated as by
    /// `std::allocator<info>::allocate` (filled from [`MetaOut::Infos`]).
    Vector,
    /// `source_location`: one pointer to its `__impl` (filled from
    /// [`MetaOut::SrcLoc`]).
    SrcLoc,
    /// Two scalar members in offset order, such as `member_offset`
    /// (`bytes`, `bits`) or `access_context` (scope, designating class)
    /// (filled from [`MetaOut::Pair`]).
    Pair,
    /// A `reflection_range` argument: a contiguous range of `info`
    /// (`vector<info>`, `span<const info>`, `array<info, N>`,
    /// `initializer_list<info>`), read as a (data, size) span.
    Range,
    /// A contiguous range of any element type (`reflect_constant_array`,
    /// `reflect_constant_string`; WP-479): a built-in array, a class
    /// holding one array, a pointer and a size, or three element
    /// pointers, read as a (data, size) span
    /// (`super::value::elems_recipe`).
    Elems,
    /// The whole result object, written by copying the object a value op
    /// thaws ([`MetaOut::Frozen`], [`MetaOut::Deref`]; `extract<T>` of a
    /// class `T`).
    Whole,
}

/// A scalar member of a recipe's class: its byte offset in the complete
/// object and its type.
#[derive(Clone, Copy, Debug, kiln_snap::Snap)]
pub(crate) struct Leaf {
    pub off: u64,
    pub ty: OTy,
}

/// A [`Shape`] matched against one class's layout.
#[derive(Clone, Copy, Debug, kiln_snap::Snap)]
pub(crate) enum Recipe {
    /// A pointer and a size (`Shape::Str`, or a span-like range).
    Span { ptr: Leaf, len: Leaf },
    /// Begin, end and capacity pointers to `elem`.
    Vector { begin: Leaf, end: Leaf, cap: Leaf, elem: OTy },
    /// One pointer.
    Ptr(Leaf),
    /// Two scalars.
    Pair(Leaf, Leaf),
    /// An array of `n` `info` at `off` (`std::array<info, N>`).
    Inline { off: u64, elem: OTy, n: u64 },
    /// The whole object ([`Shape::Whole`]).
    Whole,
}

/// The exception path of a call site (spec/reflection.md §8.2): the
/// register the VM sets when the call throws, and the recipes of the
/// `std::meta::exception` constructor's message and location parameters.
#[derive(Clone, Copy, Debug, kiln_snap::Snap)]
pub(crate) struct ExcSite {
    pub flag: u32,
    pub what: Recipe,
    pub at: Recipe,
}

/// What the VM knows of one compiled intrinsic call (operand 3 of its
/// `bi::META` instruction is its index + 1).
#[derive(Clone, Copy, Debug, kiln_snap::Snap)]
pub(crate) struct MetaSite {
    /// The call expression (the origin of the storage it allocates).
    pub e: ExprId,
    /// The intrinsic's declaration (`exception::from()`).
    pub callee: DeclId,
    /// The result's recipe (the result register holds the address of the
    /// object it initializes).
    pub result: Option<Recipe>,
    /// The recipes of `reflection_range` arguments, by position.
    pub ranges: [Option<Recipe>; super::MAX_META_ARGS],
    pub exc: Option<ExcSite>,
}

/// A `std::meta::exception` being thrown by the site that failed: its
/// constructor's arguments (bridge: [`crate::SemaBridge::meta_throw`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MetaThrow {
    /// The message: a frozen pointer to its characters, and its length.
    pub what: ConstValueId,
    pub len: u64,
    /// `^^fn` of the intrinsic that threw.
    pub from: ReflectionId,
    /// The `const __impl *` of the call's source location (None: null).
    pub at: Option<ConstValueId>,
}

/// The constructor `std::meta::exception` is built with when an
/// intrinsic throws ([`crate::SemaBridge::meta_exception`]).
#[derive(Clone, Copy, Debug)]
pub struct ExcCtor {
    /// The exception class.
    pub class: kiln_ast::QualType,
    /// Its constructor `(string_view-like what, info from,
    /// source_location where)`.
    pub ctor: DeclId,
    /// The types of its first and third parameters.
    pub what: kiln_ast::QualType,
    pub at: kiln_ast::QualType,
}

/// The materialization state of an Oven.
#[derive(Debug, Default, kiln_snap::Snap)]
pub(crate) struct MetaState {
    pub sites: Vec<MetaSite>,
    /// The elements of a [`MetaOut::Infos`] result (reused: no
    /// allocation per call in steady state).
    #[snap(skip)]
    pub infos: Vec<ReflectionId>,
    /// The exception the last failing site throws, until its
    /// constructor's arguments are filled (`bi::META_EXC`).
    #[snap(skip)]
    pub pending: Option<MetaThrow>,
    /// The `std::meta::info` type, once a range argument was compiled
    /// ([`Oven::meta_range_at`] reads its elements).
    pub info_ty: Option<OTy>,
}

/// At most this many scalar members in a recipe's class.
const MAX_LEAVES: usize = 4;
/// How deep a recipe's class nests subobjects at most.
const MAX_DEPTH: u32 = 8;

/// The non-empty scalar subobjects of `t` at `base` (bases and members,
/// empty ones skipped), in offset order.
fn leaves(
    types: &Types,
    t: OTy,
    base: u64,
    depth: u32,
    out: &mut Vec<Leaf>,
) -> Result<(), &'static str> {
    if depth > MAX_DEPTH {
        return Err("nests too deeply");
    }
    match types.kind(t) {
        TyKind::Record { union: true, .. } => Err("is a union"),
        TyKind::Record { .. } => {
            for f in types.fields(t) {
                if f.width.is_some() {
                    return Err("has a bit-field");
                }
                if types.size(f.ty) == 0 || types.holds_no_data(f.ty) {
                    continue;
                }
                leaves(types, f.ty, base + f.offset, depth + 1, out)?;
            }
            Ok(())
        }
        TyKind::Int { .. } | TyKind::Ptr(_) | TyKind::Reflection => {
            if out.len() == MAX_LEAVES {
                return Err("has too many members");
            }
            out.push(Leaf { off: base, ty: t });
            Ok(())
        }
        TyKind::Array { elem, .. } if types.kind(elem) == TyKind::Reflection => {
            if out.len() == MAX_LEAVES {
                return Err("has too many members");
            }
            out.push(Leaf { off: base, ty: t });
            Ok(())
        }
        _ => Err("has a member of another kind"),
    }
}

/// Matches `shape` against class `t`'s layout.
///
/// # Errors
/// Why the class cannot be built by the shape (for the call-site note).
pub(crate) fn recipe(types: &Types, t: OTy, shape: Shape) -> Result<Recipe, String> {
    match shape {
        Shape::Elems => return super::value::elems_recipe(types, t),
        Shape::Whole => return Ok(Recipe::Whole),
        _ => {}
    }
    match types.kind(t) {
        TyKind::Record { .. } => {}
        // A built-in array of `info` is a range too.
        TyKind::Array { elem, n }
            if shape == Shape::Range && types.kind(elem) == TyKind::Reflection =>
        {
            return Ok(Recipe::Inline { off: 0, elem, n });
        }
        _ if shape == Shape::Range => {
            return Err("is not a contiguous range of 'std::meta::info'".into());
        }
        _ => return Err("is not a class".into()),
    }
    let mut ls = Vec::with_capacity(MAX_LEAVES);
    leaves(types, t, 0, 0, &mut ls).map_err(str::to_owned)?;
    ls.sort_by_key(|l| l.off);
    let ptr_to = |l: &Leaf| match types.kind(l.ty) {
        TyKind::Ptr(e) => Some(e),
        _ => None,
    };
    let is_int = |l: &Leaf| matches!(types.kind(l.ty), TyKind::Int { .. });
    let is_info = |e: OTy| types.kind(e) == TyKind::Reflection;
    let is_char = |e: OTy| matches!(types.kind(e), TyKind::Int { width: 8, .. });
    let span = |ls: &[Leaf], elem_ok: &dyn Fn(OTy) -> bool| match ls {
        [a, b] if ptr_to(a).is_some_and(elem_ok) && is_int(b) => {
            Some(Recipe::Span { ptr: *a, len: *b })
        }
        [a, b] if is_int(a) && ptr_to(b).is_some_and(elem_ok) => {
            Some(Recipe::Span { ptr: *b, len: *a })
        }
        _ => None,
    };
    let found = match shape {
        Shape::Str => span(&ls, &is_char),
        Shape::Vector => match ls[..] {
            [b, e, c] if [b, e, c].iter().all(|l| ptr_to(l).is_some_and(is_info)) => {
                let elem = ptr_to(&b).unwrap_or(t);
                Some(Recipe::Vector { begin: b, end: e, cap: c, elem })
            }
            _ => None,
        },
        Shape::SrcLoc => match ls[..] {
            [p] if ptr_to(&p).is_some() => Some(Recipe::Ptr(p)),
            _ => None,
        },
        Shape::Pair => match ls[..] {
            [a, b] if ptr_to(&a).is_none() && ptr_to(&b).is_none() => Some(Recipe::Pair(a, b)),
            _ => None,
        },
        Shape::Range => match ls[..] {
            [] => Some(Recipe::Inline { off: 0, elem: t, n: 0 }),
            [l] => match types.kind(l.ty) {
                TyKind::Array { elem, n } => Some(Recipe::Inline { off: l.off, elem, n }),
                _ => None,
            },
            [b, e, c] if [b, e, c].iter().all(|l| ptr_to(l).is_some_and(is_info)) => {
                let elem = ptr_to(&b).unwrap_or(t);
                Some(Recipe::Vector { begin: b, end: e, cap: c, elem })
            }
            _ => span(&ls, &is_info),
        },
        Shape::Elems | Shape::Whole => None,
    };
    found.ok_or_else(|| {
        let what = match shape {
            Shape::Str => "a character pointer and a size",
            Shape::Vector => "three 'std::meta::info' pointers",
            Shape::SrcLoc => "one pointer",
            Shape::Pair => "two scalar members",
            Shape::Range => "a contiguous range of 'std::meta::info'",
            Shape::Elems | Shape::Whole => "a contiguous range",
        };
        format!("does not hold {what}")
    })
}

/// The address of the subobject at byte `off` of the object at `raw`.
fn field_addr(raw: u64, off: u64) -> u64 {
    Ptr::with_off(raw, u64::from(Ptr::decode(raw).off) + off, false)
}

impl Oven {
    /// The scratch the bridge writes a [`MetaOut::Infos`] result's
    /// elements to (cleared by the caller; its capacity is reused).
    pub fn meta_infos(&mut self) -> &mut Vec<ReflectionId> {
        &mut self.meta.infos
    }

    /// The length of `reflection_range` argument `r` (an argument
    /// value of a range parameter: its data pointer | length << 64).
    #[must_use]
    pub fn meta_range_len(&self, r: u128) -> u64 {
        (r >> 64) as u64
    }

    /// Element `i` of `reflection_range` argument `r`.
    ///
    /// # Errors
    /// The evaluator's fault reading it (an uninitialized element), or
    /// an invalid argument when `i` is out of range.
    pub fn meta_range_at(&self, r: u128, i: u64) -> Result<ReflectionId, MetaError> {
        let fault = |f| MetaError { notes: Vec::new(), kind: MetaErrorKind::Fault(OvenFault(f)) };
        let Some(ty) = self.meta.info_ty.filter(|_| i < self.meta_range_len(r)) else {
            return Err(MetaError::invalid());
        };
        let p = self.ptr_add(r as u64, i128::from(i), ty).map_err(fault)?;
        let v = self.load(p, ty, ak::READ).map_err(fault)?;
        ReflectionId::from_bits(v).ok_or_else(MetaError::invalid)
    }

    /// The operand value of the class argument object at `addr`: a
    /// range's (data, length), or a pair's two scalars.
    pub(crate) fn range_arg(&self, rc: Recipe, addr: u64) -> Result<u128, Fault> {
        let get = |l: Leaf| self.load(field_addr(addr, l.off), l.ty, ak::READ);
        let (data, n) = match rc {
            Recipe::Inline { n: 0, .. } => (0, 0),
            Recipe::Inline { off, n, .. } => (field_addr(addr, off), n),
            Recipe::Span { ptr, len } => (get(ptr)? as u64, get(len)? as u64),
            Recipe::Vector { begin, end, elem, .. } => {
                let (b, e) = (get(begin)? as u64, get(end)? as u64);
                let n = if b == e { 0 } else { self.ptr_sub(e, b, elem)? };
                (b, u64::try_from(n).map_err(|_| Fault::Invalid)?)
            }
            // Two scalars (an `access_context`): the first in the low
            // 64 bits, the second in the high.
            Recipe::Pair(a, b) => {
                let (x, y) = (get(a)?, get(b)?);
                let x = u64::try_from(x).map_err(|_| Fault::Invalid)?;
                let y = u64::try_from(y).map_err(|_| Fault::Invalid)?;
                return Ok(u128::from(x) | u128::from(y) << 64);
            }
            Recipe::Ptr(_) | Recipe::Whole => return Err(Fault::Invalid),
        };
        Ok(u128::from(data) | u128::from(n) << 64)
    }

    /// Stores result `out` through recipe `rc` into the object at `addr`
    /// (`e` the call, `top` the first free register above the frame);
    /// false when the result does not fit the recipe (a bridge bug).
    pub(crate) fn materialize_out(
        &mut self,
        b: &mut dyn SemaBridge,
        out: MetaOut,
        rc: Recipe,
        addr: u64,
        (e, top): (ExprId, u32),
    ) -> Result<bool, Fault> {
        let put = |s: &mut Self, l: Leaf, v: u128| {
            s.store(field_addr(addr, l.off), l.ty, v, ak::INIT, true)
        };
        match (out, rc) {
            (MetaOut::Str(cv, len), Recipe::Span { ptr, len: n }) => {
                let p = self.thaw_ptr(b, cv, ptr.ty, top);
                put(self, ptr, p)?;
                put(self, n, u128::from(len))?;
            }
            (MetaOut::SrcLoc(cv), Recipe::Ptr(p)) => {
                let v = cv.map_or(0, |cv| self.thaw_ptr(b, cv, p.ty, top));
                put(self, p, v)?;
            }
            (MetaOut::Pair(x, y), Recipe::Pair(a, c)) => {
                put(self, a, x)?;
                put(self, c, y)?;
            }
            (MetaOut::Infos(n), Recipe::Vector { begin, end, cap, elem }) => {
                let n = n as usize;
                if n > self.meta.infos.len() {
                    return Ok(false);
                }
                let (data, last) = if n == 0 {
                    (0, 0)
                } else {
                    let origin = Origin::Heap(e, self.mem.heap_allocs as u32);
                    let aux = new_aux::ARRAY | new_aux::STD;
                    let data = self.new_alloc(elem, n as u64, aux, origin)?;
                    // The elements are constructed (as by `construct_at`):
                    // `std::allocator` storage begins outside its lifetime.
                    let bytes = self.types.size(elem) * n as u64;
                    self.mem.set_lifetime(data, bytes, true).map_err(|f| Fault::Mem {
                        f,
                        ak: ak::INIT,
                        ty: elem,
                        p: data,
                    })?;
                    let mut q = data;
                    for i in 0..n {
                        let v = u128::from(self.meta.infos[i].bits());
                        self.store(q, elem, v, ak::INIT, true)?;
                        q = self.ptr_add(q, 1, elem)?;
                    }
                    (data, q)
                };
                put(self, begin, u128::from(data))?;
                put(self, end, u128::from(last))?;
                put(self, cap, u128::from(last))?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
