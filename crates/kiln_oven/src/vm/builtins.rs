//! Library builtins evaluated natively (`Op::Builtin`; spec/oven.md
//! "Builtins"): the bit operations, floating-point classification and the
//! string/memory functions, as clang 19's `VisitBuiltinCallExpr` evaluates
//! them ([library.c]: the C library's semantics, with clang's restrictions
//! to typed, element-wise access).
//!
//! The string/memory functions take a byte fast path when the elements are
//! one byte (or scalars, for `memcpy`) and the allocations hold no union;
//! any irregularity (an uninitialized byte, the end of the array) falls
//! back to the element-wise loop, which reports the exact fault.

use kiln_ast::Ast;
use kiln_ast::decl::DefinitionBits;
use kiln_ast::types::BuiltinType;

use super::{Fault, write_fault};
use crate::compile::float_of;
use crate::kbc::{ak, bi, cmp};
use crate::mem::{MemFault, Origin, Ptr, flags};
use crate::softfloat::{self, Class};
use crate::ty::{OTy, TyKind};
use crate::{Oven, SemaBridge};

/// A builtin-specific failure (rendered by `notes`; the types are the
/// designated objects', re-read from the call's arguments when static).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BiFault {
    /// `note_constexpr_memcpy_null`: the source (`src`) or destination
    /// of `memcpy`/`memmove` (`mv`) is null.
    CopyNull { mv: bool, src: bool },
    /// `note_constexpr_memcpy_type_pun`.
    CopyPun { mv: bool, src: OTy, dst: OTy },
    /// `note_constexpr_memcpy_nontrivial`.
    CopyNontrivial { mv: bool, t: OTy },
    /// `note_constexpr_memcpy_overlap` (only `memcpy`).
    CopyOverlap,
    /// `note_constexpr_memcpy_unsupported` (size not a multiple).
    CopySize { mv: bool, t: OTy, n: u64, size: u64 },
    /// `note_constexpr_memcpy_unsupported` (`src`: the source, else the
    /// destination, has fewer than `n` elements left).
    CopyRange { mv: bool, src: bool, t: OTy, n: u64 },
    /// `note_constexpr_memcmp_unsupported`.
    CmpType { t1: OTy, t2: OTy },
    /// `note_constexpr_memchr_unsupported`.
    ChrType { t: OTy },
    /// `note_constexpr_bit_cast_unsupported_bitfield` (a destination
    /// bit-field; a source one is rejected statically).
    CastBitfield,
    /// `note_constexpr_bit_cast_indet_dest`.
    CastIndet { t: OTy },
    /// `note_constexpr_bit_cast_unrepresentable_value` (a `bool` byte).
    CastUnrep { v: u8 },
    /// `__builtin_assume_aligned` to `align` bytes (WP-292): the base
    /// object's alignment `v` is less (`kind` 0,
    /// `note_constexpr_baa_insufficient_alignment`), or the offset `v`
    /// from it is not a multiple (1, the same note), or the value `v` of
    /// a pointer to no object is not (2,
    /// `note_constexpr_baa_value_insufficient_alignment`).
    Align { kind: u8, v: i64, align: u64 },
    /// `err_invalid_is_within_lifetime` (WP-418): `is_within_lifetime`
    /// of a null (`kind` 0), function (1) or one-past-the-end (2)
    /// pointer, or one into the variable being initialized (3, WP-445).
    Lifetime { kind: u8 },
    /// WP-416: the alignment `hi:lo` (`signed`) of an align builtin on a
    /// `w`-bit integer is not a positive power of two
    /// (`note_constexpr_invalid_alignment`), or (`big`) exceeds 2^(w-1)
    /// (`note_constexpr_alignment_too_big`).
    AlignArg { lo: u64, hi: u64, w: u8, signed: bool, big: bool },
    /// WP-432: the alignment of a pointer to `align` bytes is not known
    /// at compile time: whether it is at least that (`is_aligned`,
    /// `note_constexpr_alignment_compute`) or (`adjust`) the result of
    /// `align_up` / `align_down` (`note_constexpr_alignment_adjust`).
    AlignRun { adjust: bool, align: u64 },
}

/// `bi::ALIGN` (WP-416; clang `getBuiltinAlignArguments` and
/// `IntExprEvaluator` `BI__builtin_align_up` & co.): `a` rounded up or
/// down to the alignment `b`, wrapping in the source type, or whether it
/// is a multiple of it; operation 3 only checks the alignment (clang
/// evaluates and checks it before the source).
fn align(sub: u32, a: u128, b: u128, t2: u32) -> Result<u128, Fault> {
    let (w, s, op) = (sub & 0xFF, sub >> 8 & 1 != 0, sub >> 9 & 3);
    let signed = t2 >> 8 & 1 != 0;
    let fault = |big| {
        Fault::Bi(BiFault::AlignArg {
            lo: b as u64,
            hi: (b >> 64) as u64,
            w: u8::try_from(w).unwrap_or(u8::MAX),
            signed,
            big,
        })
    };
    if (signed && (b as i128) < 0) || !b.is_power_of_two() {
        return Err(fault(false));
    }
    if (1..=128).contains(&w) && b > 1u128 << (w - 1) {
        return Err(fault(true));
    }
    let m = b - 1;
    Ok(match op {
        3 => 0,
        0 => crate::arith::canon(a.wrapping_add(m) & !m, w, s),
        1 => crate::arith::canon(a & !m, w, s),
        _ => u128::from(a & m == 0),
    })
}

/// `bi::OVERFLOW` (WP-270; clang `BI__builtin_*_overflow`): the
/// overflow flag of `a op b` computed in infinite precision against the
/// result type, or (`bi::VALUE`) the result wrapped to that type. The
/// canonical operands are the two's-complement images of their values,
/// so the wrapped result is the modulo-2^128 operation re-canonicalized.
fn checked(sub: u32, a: u128, b: u128) -> u128 {
    let (op, sr, w) = (sub & 3, sub & 16 != 0, (sub >> 6) & 0xFF);
    if sub & bi::VALUE != 0 {
        let v = match op {
            0 => a.wrapping_add(b),
            1 => a.wrapping_sub(b),
            _ => a.wrapping_mul(b),
        };
        return crate::arith::canon(v, w, sr);
    }
    // Sign and magnitude of each operand.
    let sm = |v: u128, signed: bool| {
        if signed && (v as i128) < 0 { (true, (v as i128).unsigned_abs()) } else { (false, v) }
    };
    let (na, ma) = sm(a, sub & 4 != 0);
    let (nb, mb) = sm(b, sub & 8 != 0);
    let nb = if op == 1 { !nb } else { nb };
    let r = if op == 2 {
        ma.checked_mul(mb).map(|m| (na != nb, m))
    } else if na == nb {
        ma.checked_add(mb).map(|m| (na, m))
    } else if ma >= mb {
        Some((na, ma - mb))
    } else {
        Some((nb, mb - ma))
    };
    let Some((neg, m)) = r else { return 1 };
    let fits = match (sr, neg && m != 0) {
        (false, true) => false,
        (false, false) => m <= mask(w),
        (true, true) => m <= 1u128 << (w - 1),
        (true, false) => m <= mask(w - 1),
    };
    u128::from(!fits)
}

/// `bi::CARRY` (WP-557): the wrapped `a + b + c` (`a - b - c`) in `w`
/// bits, or the carry-out (clang `uadd_ov` / `usub_ov` twice, the flags
/// OR-ed).
fn carry(sub: u32, a: u128, b: u128, c: u128) -> u128 {
    let (w, minus) = (sub & 0xFF, sub >> 8 & 1 != 0);
    let m = mask(w);
    let c = if sub & bi::CARRY_BOOL_IN != 0 { u128::from(c != 0) } else { c };
    let (a, b, c) = (a & m, b & m, c & m);
    // A wrapping step overflows when its result is below (above, for a
    // subtraction) its first operand.
    let (r, carry) = if minus {
        let r1 = a.wrapping_sub(b) & m;
        let r2 = r1.wrapping_sub(c) & m;
        (r2, r1 > a || r2 > r1)
    } else {
        let r1 = a.wrapping_add(b) & m;
        let r2 = r1.wrapping_add(c) & m;
        (r2, r1 < a || r2 < r1)
    };
    if sub & bi::CARRY_VALUE != 0 { r } else { u128::from(carry) }
}

/// `bi::FP_MATH` (WP-365, spec/oven.md "Floating-point math builtins"):
/// `None` where the operation is not a constant (an IEEE exception other
/// than inexact, or a signaling NaN operand).
fn fp_math(sub: u32, a: u128, b: u128, c: u128, t1: u32) -> Option<u128> {
    use softfloat::math::{self, Rounding};
    let k = float_of(sub & 15);
    let second = sub & bi::SECOND != 0;
    let int = |v: i32| i128::from(v) as u128;
    let round = |m| math::round_int(k, a, m);
    Some(match sub >> 4 & 31 {
        bi::CEIL => round(Rounding::Up)?,
        bi::FLOOR => round(Rounding::Down)?,
        bi::TRUNC => round(Rounding::Zero)?,
        bi::ROUND => round(Rounding::Away)?,
        bi::ROUNDEVEN | bi::RINT => round(Rounding::Even)?,
        bi::LROUND => math::to_int(k, a, Rounding::Away, t1)?,
        bi::LRINT => math::to_int(k, a, Rounding::Even, t1)?,
        op @ (bi::FMOD | bi::REMAINDER | bi::REMQUO) => {
            let (r, q) = math::rem(k, a, b, op != bi::FMOD)?;
            if second { int(q) } else { r }
        }
        bi::FDIM => math::fdim(k, a, b)?,
        bi::FMA => math::fma(k, a, b, c)?,
        bi::FREXP => {
            let (f, e) = math::frexp(k, a)?;
            if second { int(e) } else { f }
        }
        bi::LDEXP => math::ldexp(k, a, b as i128)?,
        bi::ILOGB => int(math::ilogb(k, a)?),
        bi::LOGB => math::logb(k, a)?,
        bi::MODF => {
            let (f, i) = math::modf(k, a)?;
            if second { i } else { f }
        }
        bi::NEXTAFTER => math::nextafter(k, a, float_of(t1), b)?,
        bi::SQRT => math::sqrt(k, a)?,
        bi::SIGNBIT => u128::from(a & softfloat::sign_bit(k) != 0),
        _ => {
            let o = softfloat::cmp(k, a, b);
            u128::from(match (t1, o) {
                (bi::UNORDERED, o) => o.is_none(),
                (_, None) => false,
                (cmp::GT, Some(o)) => o.is_gt(),
                (cmp::GE, Some(o)) => o.is_ge(),
                (cmp::LT, Some(o)) => o.is_lt(),
                (cmp::LE, Some(o)) => o.is_le(),
                (_, Some(o)) => o.is_ne(),
            })
        }
    })
}

fn mask(w: u32) -> u128 {
    if w >= 128 { u128::MAX } else { (1u128 << w) - 1 }
}

/// clang's `isOneByteCharacterType`: `char`, `signed char`, `unsigned
/// char`, `char8_t`.
fn is_narrow_char(ast: &Ast, oven: &Oven, t: OTy) -> bool {
    let q = oven.types.info(t).q;
    matches!(
        ast.types.builtin(q),
        Some(BuiltinType::Char | BuiltinType::SChar | BuiltinType::UChar | BuiltinType::Char8)
    )
}

impl Oven {
    /// `bi::LIFETIME` of the pointer `raw` designating an object of type
    /// `t` ([`bi::DYN`]: unknown), `ok` its static union designators'
    /// verdict.
    fn bi_within_lifetime(&self, ast: &Ast, raw: u64, ok: bool, t: u32) -> Result<u128, Fault> {
        let p = Ptr::decode(raw);
        let kind = match () {
            () if raw == 0 => Some(0),
            () if p.is_fn() => Some(1),
            () if p.past => Some(2),
            () => None,
        };
        if let Some(kind) = kind {
            return Err(Fault::Bi(BiFault::Lifetime { kind }));
        }
        let ty = OTy(if t == bi::DYN { 0 } else { t });
        let len = if t == bi::DYN { 1 } else { self.types.size(ty).max(1) };
        let (_, _, a) = self.mem.check(raw, len, false).map_err(|f| Fault::Mem {
            f,
            ak: ak::READ,
            ty,
            p: raw,
        })?;
        // clang: the variable whose initializer is being evaluated
        // (`Info.EvaluatingDecl`) has not begun its lifetime (WP-445): the
        // object it initializes, or the placeholder a namespace-scope
        // variable named in its own initializer reads (`constexpr C
        // c(&c)`, WP-465).
        let evaluating = match (a.origin, self.run.object) {
            (Origin::Var(d), Some(o)) => d == o,
            (Origin::Global(d), Some(o)) => {
                a.flags & flags::RESOLVING != 0 && ast.first(d) == ast.first(o)
            }
            _ => false,
        };
        if evaluating {
            return Err(Fault::Bi(BiFault::Lifetime { kind: 3 }));
        }
        if !ok {
            return Ok(0);
        }
        // An object of a union-holding allocation: every union on its
        // path must have it (or a member holding it) active.
        let live = t == bi::DYN
            || a.flags & flags::HAS_UNION == 0
            || !matches!(self.union_path(raw, ty, ak::READ), Err(Fault::Union { .. }));
        Ok(u128::from(live))
    }

    /// Whether `raw` (designating an object of type `t`, [`bi::DYN`]:
    /// unknown) points into a subobject of an object under construction
    /// whose lifetime has not yet begun, at the `bi::LIFETIME` at `pc`
    /// (clang: the subobject has no value yet, [basic.life]/1): a
    /// subobject after the one a constructor active on the stack is
    /// initializing (`Oven::ctor_steps`), or that one when it is not of
    /// class or array type (its initializer is being evaluated). A
    /// subobject of class type being initialized is left to the
    /// constructor frames above (WP-445). While a complete-object
    /// constructor initializes its virtual bases, its non-virtual
    /// subobjects and later virtual bases have not begun (WP-465).
    pub(super) fn not_yet_begun(&self, pc: usize, raw: u64, t: u32) -> bool {
        if self.ctor_steps.is_empty() {
            return false;
        }
        let p = Ptr::decode(raw);
        let ty = (t != bi::DYN).then_some(OTy(t));
        for (i, fr) in self.stack.iter().enumerate() {
            let Some(steps) = self.ctor_steps.get(&fr.func) else { continue };
            let f = &self.funcs[fr.func as usize];
            let (Some(r), Some(cls)) = (f.this_reg, f.this_ty) else { continue };
            let this = Ptr::decode(self.regs[fr.fb as usize + r as usize] as u64);
            if Ptr::alloc_key(this.encode()) != Ptr::alloc_key(raw) || p.off < this.off {
                continue;
            }
            let rel = u64::from(p.off - this.off);
            let at = self.stack.get(i + 1).map_or(pc, |up| up.call_pc as usize) as u32;
            let k = steps.partition_point(|s| s.pc <= at);
            let Some(&st) = k.checked_sub(1).map(|k| &steps[k]) else { continue };
            let cur = st.cur;
            if cur == u32::MAX {
                continue;
            }
            let tys = &self.types;
            // A virtual-base stage looks at the virtual bases too; after
            // it they have begun, and a base-subobject constructor never
            // initializes them.
            let vstage = cur & crate::CTOR_STEP_VBASE != 0;
            let nv = tys.vbase_start(cls);
            let all = tys.fields(cls);
            let fields = if vstage { all } else { &all[..nv] };
            let holds = |fi: &crate::ty::FieldInfo| {
                fi.width.is_none() && rel >= fi.offset && rel < fi.offset + tys.size(fi.ty).max(1)
            };
            let reaches = |fi: &crate::ty::FieldInfo| {
                ty.is_none_or(|ty| tys.has_subobject(fi.ty, rel - fi.offset, ty))
            };
            let pick = fields.iter().position(|fi| holds(fi) && reaches(fi));
            let Some(idx) = pick.or_else(|| fields.iter().position(holds)) else { continue };
            if vstage {
                // A non-virtual subobject or a later virtual base; the one
                // being initialized is a base, left to the frames above.
                if idx < nv || (idx - nv) as u32 > cur & !crate::CTOR_STEP_VBASE {
                    return true;
                }
                continue;
            }
            let idx = idx as u32;
            if cur & crate::CTOR_STEP_ELEM != 0 {
                let field = cur & !(crate::CTOR_STEP_ELEM | crate::CTOR_STEP_RECORD);
                if idx > field || (idx == field && self.elem_not_begun(st, rel, ty)) {
                    return true;
                }
                continue;
            }
            if idx > cur {
                return true;
            }
            if idx == cur {
                let fi = &fields[idx as usize];
                let whole = !fi.base
                    && !matches!(tys.kind(fi.ty), TyKind::Record { .. } | TyKind::Array { .. });
                if whole {
                    return true;
                }
            }
        }
        false
    }

    /// Whether the subobject `rel` bytes from `this` (of type `ty`, `None`:
    /// unknown) has not begun while element step `st` is current: it ends
    /// before the element (begun), holds it (an aggregate being
    /// list-initialized, begun), lies in an element of class type (left
    /// to the frames above), or is the element or after it (not begun;
    /// clang evaluates a list's elements in order, WP-465).
    fn elem_not_begun(&self, st: crate::CtorStep, rel: u64, ty: Option<OTy>) -> bool {
        let tys = &self.types;
        let (o, n) = (u64::from(st.off), u64::from(st.len));
        let sz = ty.map_or(1, |t| tys.size(t).max(1));
        if rel + sz <= o || rel < o {
            return false;
        }
        let record_leaf = st.cur & crate::CTOR_STEP_RECORD != 0;
        if rel == o
            && let Some(t) = ty
        {
            let holds = match tys.kind(t) {
                TyKind::Record { .. } => sz > n || !record_leaf,
                TyKind::Array { .. } => sz > n,
                _ => false,
            };
            if holds {
                return false;
            }
        }
        !(record_leaf && rel < o + n)
    }

    /// Evaluates builtin `aux` (`bi` kind | sub << 8) on registers `a`..`c`
    /// and types `t1`/`t2`.
    #[allow(clippy::too_many_arguments, reason = "the instruction's operands")]
    pub(super) fn builtin(
        &mut self,
        ast: &Ast,
        aux: u32,
        a: u128,
        b: u128,
        c: u128,
        t1: u32,
        t2: u32,
    ) -> Result<u128, Fault> {
        let sub = aux >> 8;
        match aux & 0xFF {
            k @ (bi::CLZ | bi::CTZ) => {
                let w = sub & 0xFF;
                let v = a & mask(w);
                if v == 0 {
                    // clang: the result of the GCC builtins is undefined
                    // for zero (`Error(E)`), unless `clzg`/`ctzg` were
                    // given a fallback.
                    return if sub >> 8 & 1 != 0 { Ok(b) } else { Err(Fault::Invalid) };
                }
                let n =
                    if k == bi::CLZ { v.leading_zeros() - (128 - w) } else { v.trailing_zeros() };
                Ok(u128::from(n))
            }
            bi::POPCOUNT => Ok(u128::from((a & mask(sub & 0xFF)).count_ones())),
            bi::STRING_LIT => Ok(u128::from(self.meta_is_string_literal(a))),
            bi::LIFETIME => {
                // A `void *` argument's designator is its conversion's
                // tag (`kbc::VOID_TAG`), if any.
                let tag = (a >> 64) as u32;
                let t = if t1 == bi::DYN && tag != 0 { tag - 1 } else { t1 };
                self.bi_within_lifetime(ast, a as u64, b != 0, t)
            }
            bi::UNION_LIVE => {
                let raw = a as u64;
                let key = Ptr::union_key(raw, u64::from(Ptr::decode(raw).off), OTy(t1));
                Ok(u128::from(b != 0 && self.mem.may_be_active(key, sub)))
            }
            bi::LOCK_FREE => {
                // clang 19 `IntExprEvaluator::VisitBuiltinCallExpr`: a
                // power-of-two size within the inline width is lock-free
                // for an `_Atomic` (c11) or null object, a single byte,
                // or an object aligned to the size, or (bit 10) a
                // pointer whose integer value `b` is a multiple of it.
                let max = u128::from(sub & 0xFF);
                let int_aligned =
                    sub >> 10 & 1 != 0 && (b as u64) & (a as u64).wrapping_sub(1) == 0;
                let yes = a.is_power_of_two()
                    && a <= max
                    && (sub >> 8 & 1 != 0 || a == 1 || u128::from(t1) >= a || int_aligned);
                if yes || sub >> 9 & 1 != 0 { Ok(u128::from(yes)) } else { Err(Fault::Invalid) }
            }
            bi::ALIGN => align(sub, a, b, t2),
            bi::BSWAP => {
                let w = sub & 0xFF;
                Ok((a & mask(w)).swap_bytes() >> (128 - w))
            }
            bi::FP_CLASS => {
                let c = softfloat::classify(float_of(sub & 15), a);
                let r = match sub >> 4 {
                    bi::FINITE => !matches!(c, Class::Inf | Class::NaN),
                    bi::INF => c == Class::Inf,
                    bi::NAN => c == Class::NaN,
                    bi::SUBNORMAL => c == Class::Subnormal,
                    bi::ZERO => c == Class::Zero,
                    bi::SIGNALING => softfloat::is_signaling(float_of(sub & 15), a),
                    bi::MASK => {
                        let k = float_of(sub & 15);
                        let neg = a & softfloat::sign_bit(k) != 0;
                        let bit = match c {
                            Class::NaN if softfloat::is_signaling(k, a) => 0x1,
                            Class::NaN => 0x2,
                            Class::Inf => {
                                if neg {
                                    0x4
                                } else {
                                    0x200
                                }
                            }
                            Class::Normal => {
                                if neg {
                                    0x8
                                } else {
                                    0x100
                                }
                            }
                            Class::Subnormal => {
                                if neg {
                                    0x10
                                } else {
                                    0x80
                                }
                            }
                            Class::Zero => {
                                if neg {
                                    0x20
                                } else {
                                    0x40
                                }
                            }
                        };
                        b & bit != 0
                    }
                    _ => c == Class::Normal,
                };
                Ok(u128::from(r))
            }
            bi::FP_OP => {
                let k = float_of(sub & 15);
                let sign = softfloat::sign_bit(k);
                let neg = |x: u128| x & sign != 0;
                let zero = |x: u128| softfloat::classify(k, x) == Class::Zero;
                let nan = |x: u128| softfloat::classify(k, x) == Class::NaN;
                let quiet = |x: u128| softfloat::quiet(k, x);
                let snan = |x: u128| softfloat::is_signaling(k, x);
                let op = sub >> 4;
                Ok(match op {
                    bi::FABS => a & !sign,
                    bi::COPYSIGN => a & !sign | b & sign,
                    bi::FMAX_22 | bi::FMIN_22 if snan(a) => quiet(a),
                    bi::FMAX_22 | bi::FMIN_22 if snan(b) => quiet(b),
                    bi::FMAXIMUM_NUM | bi::FMINIMUM_NUM if nan(a) => {
                        if nan(b) {
                            quiet(b)
                        } else {
                            b
                        }
                    }
                    bi::FMAXIMUM_NUM | bi::FMINIMUM_NUM if nan(b) => a,
                    bi::FMAX | bi::FMAX_22 | bi::FMAXIMUM_NUM => {
                        let gt = softfloat::cmp(k, b, a) == Some(std::cmp::Ordering::Greater);
                        if zero(a) && zero(b) {
                            if neg(a) { b } else { a }
                        } else if nan(a) || gt {
                            b
                        } else {
                            a
                        }
                    }
                    bi::FMIN | bi::FMIN_22 | bi::FMINIMUM_NUM => {
                        let lt = softfloat::cmp(k, b, a) == Some(std::cmp::Ordering::Less);
                        if zero(a) && zero(b) {
                            if neg(b) { b } else { a }
                        } else if nan(a) || lt {
                            b
                        } else {
                            a
                        }
                    }
                    _ => match softfloat::classify(k, a) {
                        Class::Inf if neg(a) => u128::MAX,
                        Class::Inf => 1,
                        _ => 0,
                    },
                })
            }
            bi::FP_MATH => fp_math(sub, a, b, c, t1).ok_or(Fault::Invalid),
            bi::STRLEN => self.bi_strlen(a as u64, OTy(t1)).map(u128::from),
            k @ (bi::MEMCMP | bi::MEMCHR) => {
                let n = if sub & bi::UNBOUNDED != 0 { u64::MAX } else { c as u64 };
                let mode = StrMode { wide: sub & bi::WIDE != 0, str: sub & bi::STR != 0 };
                if k == bi::MEMCMP {
                    self.bi_memcmp(ast, a as u64, b as u64, n, t1, t2, mode)
                } else {
                    self.bi_memchr(ast, a as u64, b, n, t1, mode).map(u128::from)
                }
            }
            bi::MEMCPY => {
                let (mv, wide) = (sub & bi::MOVE != 0, sub & bi::WCHAR != 0);
                self.bi_memcpy(ast, a as u64, b as u64, c as u64, [t1, t2], mv, wide).map(|()| a)
            }
            bi::INT_OP => {
                let w = sub & 0xFF;
                let v = a & mask(w);
                Ok(match sub >> 8 {
                    bi::FFS if v == 0 => 0,
                    bi::FFS => u128::from(v.trailing_zeros() + 1),
                    bi::PARITY => u128::from(v.count_ones() & 1),
                    bi::CLRSB => {
                        // The redundant sign bits: `clz` of `v` (or of
                        // `~v` when negative) in `w` bits, minus one.
                        let s = if v >> (w - 1) & 1 != 0 { !v & mask(w) } else { v };
                        u128::from(s.leading_zeros() - (128 - w) - 1)
                    }
                    bi::BITREVERSE => v.reverse_bits() >> (128 - w),
                    bi::IABS => {
                        // `a` is canonical; the most negative value has no
                        // representable magnitude (undefined, [expr.pre]/4);
                        // clang 22 fails it without a note.
                        let x = a as i128;
                        if v == 1 << (w - 1) {
                            return Err(Fault::Quiet);
                        }
                        x.unsigned_abs()
                    }
                    k @ (bi::SADD_SAT | bi::SSUB_SAT) => {
                        // Canonical signed operands are sign-extended, so
                        // the `i128` result is exact below 128 bits.
                        let (x, y) = (a as i128, b as i128);
                        let r = if k == bi::SADD_SAT {
                            x.saturating_add(y)
                        } else {
                            x.saturating_sub(y)
                        };
                        let sh = 128 - w;
                        r.clamp(i128::MIN >> sh, i128::MAX >> sh) as u128
                    }
                    bi::UADD_SAT => v.saturating_add(b & mask(w)).min(mask(w)),
                    bi::USUB_SAT => v.saturating_sub(b & mask(w)),
                    // WP-630 (clang 19 `IntExprEvaluator`): the x86
                    // BMI / TBM bit-field builtins.
                    bi::BEXTR => {
                        let (start, len) = ((b & 0xff) as u32, (b >> 8 & 0xff) as u32);
                        if len == 0 || start >= w { 0 } else { v >> start & mask(len.min(w)) }
                    }
                    bi::BZHI => {
                        let i = (b & 0xff) as u32;
                        if i < w { v & mask(i) } else { v }
                    }
                    bi::PDEP | bi::PEXT => {
                        let (m, pdep) = (b & mask(w), sub >> 8 == bi::PDEP);
                        let (mut r, mut p) = (0u128, 0u32);
                        for i in 0..w {
                            if m >> i & 1 != 0 {
                                let (from, to) = if pdep { (p, i) } else { (i, p) };
                                r |= (v >> from & 1) << to;
                                p += 1;
                            }
                        }
                        r
                    }
                    k => {
                        // clang: the amount modulo the width.
                        let n = ((b & mask(w)) % u128::from(w)) as u32;
                        let n = if k == bi::ROTL { n } else { (w - n) % w };
                        if n == 0 { v } else { (v << n | v >> (w - n)) & mask(w) }
                    }
                })
            }
            bi::OVERFLOW => Ok(checked(sub, a, b)),
            bi::CARRY => Ok(carry(sub, a, b, c)),
            bi::OBJECT_SIZE => self.bi_object_size(sub, a as u64, b, c, t1),
            bi::BIT_CAST => {
                let (to_mem, read) = (sub == bi::TO_MEM, sub == bi::READ_ONLY);
                self.bit_cast(ast, a as u64, b as u64, OTy(t1), OTy(t2), to_mem, read)
            }
            _ => Err(Fault::Invalid),
        }
    }

    /// `bi::ASSUME_ALIGNED` (WP-292; clang `PointerExprEvaluator`): the
    /// complete object of `p` must be aligned to `align` (a variable's
    /// declared alignment, else its type's), and `p`'s byte offset in it
    /// minus `off` (when `has_off`) a multiple of `align`.
    pub(super) fn bi_assume_aligned(
        &self,
        b: &mut dyn SemaBridge,
        has_off: bool,
        p: u64,
        align: u128,
        off: u128,
    ) -> Result<(), Fault> {
        let ptr = Ptr::decode(p);
        if ptr.is_fn() {
            return Ok(());
        }
        let align = u64::try_from(align).unwrap_or(u64::MAX).max(1);
        let base = (p != 0).then(|| self.alloc_any(ptr)).flatten();
        if let Some(a) = base {
            let decl = match a.origin {
                Origin::Var(d) | Origin::Global(d) | Origin::External(d) => b.decl_align(d),
                _ => 0,
            };
            let have = if decl != 0 { decl } else { u64::from(self.types.info(a.ty).align) };
            if have < align {
                let v = i64::try_from(have).unwrap_or(i64::MAX);
                return Err(Fault::Bi(BiFault::Align { kind: 0, v, align }));
            }
        }
        let at = if p == 0 { 0 } else { i64::from(ptr.off) };
        // clang: `Offset += -Offset.getZExtValue()` on 64-bit `CharUnits`.
        let v = if has_off { at.wrapping_sub(off as u64 as i64) } else { at };
        if v.rem_euclid(i64::try_from(align).unwrap_or(i64::MAX)) != 0 {
            let kind = if base.is_some() { 1 } else { 2 };
            return Err(Fault::Bi(BiFault::Align { kind, v, align }));
        }
        Ok(())
    }

    /// `bi::PTR_ALIGN` (WP-432; clang `getBaseAlignment` and
    /// `CharUnits::alignmentAtOffset`).
    pub(super) fn bi_ptr_align(
        &self,
        b: &mut dyn SemaBridge,
        sub: u32,
        p: u64,
        align: u128,
        elem: OTy,
    ) -> Result<u128, Fault> {
        let align = u64::try_from(align).unwrap_or(u64::MAX).max(1);
        let fault = Fault::Bi(BiFault::AlignRun { adjust: sub != 2, align });
        let ptr = Ptr::decode(p);
        // clang has no base object for a null or integral pointer.
        if p == 0 || ptr.is_fn() || Ptr::is_nowhere(p) {
            return Err(fault);
        }
        let Some(a) = self.alloc_any(ptr) else { return Err(fault) };
        let decl = match a.origin {
            Origin::Var(d) | Origin::Global(d) | Origin::External(d) => b.decl_align(d),
            _ => 0,
        };
        let base = if decl != 0 { decl } else { u64::from(self.types.info(a.ty).align) }.max(1);
        let off = u64::from(ptr.off);
        // `MinAlign(base, off)`: the largest power of two dividing both.
        let at = if off == 0 { base } else { base.min(1 << off.trailing_zeros()) };
        match sub {
            2 if at >= align => Ok(1),
            2 if base >= align => Ok(0),
            _ if sub != 2 && at >= align => Ok(u128::from(p)),
            _ if sub != 2 && base >= align => {
                let m = align - 1;
                let new = if sub == 1 { off & !m } else { (off + m) & !m };
                let es = self.types.size(elem).max(1);
                let d = i128::from(new) - i128::from(off);
                // An element step keeps the pointer's array position
                // (one past the end included); clang does not check the
                // bounds of the adjusted offset.
                if d % i128::from(es) == 0
                    && let Ok(q) = self.ptr_add(p, d / i128::from(es), elem)
                {
                    return Ok(u128::from(q));
                }
                Ok(u128::from(Ptr::with_off(p, new, false)))
            }
            _ => Err(fault),
        }
    }

    /// `bi::OBJECT_SIZE` (WP-282): a pointer that designates no object (a
    /// null or function pointer, an unknown object) is not a constant.
    /// With [`bi::SIDE`], `ok` says whether the speculative evaluation of
    /// the pointer completed.
    fn bi_object_size(&self, sub: u32, p: u64, ty: u128, ok: u128, t: u32) -> Result<u128, Fault> {
        let ty = (ty & 3) as u32;
        if ok == 0 && sub & bi::SIDE != 0 {
            return Ok(if ty & 2 != 0 { 0 } else { u128::from(u64::MAX) });
        }
        if ok == 0 {
            return Err(Fault::Invalid);
        }
        let ptr = Ptr::decode(p);
        if p == 0 || ptr.is_fn() {
            return Err(Fault::Invalid);
        }
        let Some(a) = self.alloc_any(ptr) else { return Err(Fault::Invalid) };
        // An object of incomplete type (`extern char a[];`) has no known size.
        if matches!(self.types.kind(a.ty), TyKind::Opaque) {
            return Err(Fault::Invalid);
        }
        let off = u64::from(ptr.off);
        let size = u64::from(a.size);
        let end = if ty & 1 == 0 {
            size
        } else {
            let elem = self.designated(p, t)?;
            let es = self.types.size(elem);
            match self.types.extent(a.ty, off, ptr.past, elem) {
                Some((start, n, _)) if es > 0 => start + n * es,
                _ => size,
            }
        };
        Ok(u128::from(end.saturating_sub(off)))
    }

    /// The type of the object `p` designates: `t`, or (`bi::DYN`, a
    /// `void` pointer) the innermost object at its offset.
    fn designated(&self, p: u64, t: u32) -> Result<OTy, Fault> {
        if t != bi::DYN {
            return Ok(OTy(t));
        }
        let ptr = Ptr::decode(p);
        let Some(a) = self.alloc_any(ptr) else {
            let f = match self.mem.check(p, 1, false) {
                Err(f) => f,
                Ok(_) => MemFault::Stale,
            };
            return Err(Fault::Mem { f, ak: ak::READ, ty: OTy(0), p });
        };
        let off = u64::from(ptr.off);
        if (ptr.past || off >= u64::from(a.size))
            && let TyKind::Array { elem, .. } = self.types.kind(a.ty)
        {
            return Ok(elem);
        }
        Ok(self.deepest_at(a.ty, off))
    }

    /// The bytes `[p, p + n)` of a one-byte-element run, if they are all
    /// initialized plain data of an allocation without unions; `n` is
    /// clamped to the elements left in `p`'s array.
    pub(crate) fn byte_run(&self, p: u64, elem: OTy, n: u64) -> Option<&[u8]> {
        let (_, len, cur, _) = self.extent_of(p, elem).ok()?;
        let n = n.min(len - cur);
        let (sp, abs, a) = self.mem.check(p, n, false).ok()?;
        let space = self.mem.space(sp);
        (n > 0
            && a.flags & flags::HAS_UNION == 0
            && space.is_init(abs, n)
            && !space.any_ptr(abs, n))
        .then(|| &space.bytes[abs as usize..(abs + n) as usize])
    }

    /// `strlen`/`wcslen`: reads elements of type `ct` up to the first
    /// zero.
    pub(super) fn bi_strlen(&self, mut p: u64, ct: OTy) -> Result<u64, Fault> {
        let mut n = 0u64;
        if self.types.size(ct) == 1
            && let Some(run) = self.byte_run(p, ct, u64::MAX)
        {
            if let Some(i) = run.iter().position(|&x| x == 0) {
                return Ok(i as u64);
            }
            n = run.len() as u64;
            p = self.ptr_add(p, i128::from(n), ct)?;
        }
        loop {
            if self.load(p, ct, ak::READ)? == 0 {
                return Ok(n);
            }
            n += 1;
            p = self.ptr_add(p, 1, ct)?;
        }
    }

    /// `bi::MSG_CHARS`: reads the `n` elements of type `ct` from `p` into
    /// the message buffer (`Oven::evaluate_chars`; clang
    /// `EvaluateCharRangeAsString` reads `data[i]` for each `i < n`).
    pub(super) fn bi_msg_chars(&mut self, n: u128, mut p: u64, ct: OTy) -> Result<(), Fault> {
        let mut out = std::mem::take(&mut self.msg_chars);
        out.clear();
        let mut r = Ok(());
        let mut i = 0u128;
        while i < n {
            if i > 0 {
                match self.ptr_add(p, 1, ct) {
                    Ok(q) => p = q,
                    Err(f) => {
                        r = Err(f);
                        break;
                    }
                }
            }
            match self.load(p, ct, ak::READ) {
                Ok(v) => out.push(v as u8),
                Err(f) => {
                    r = Err(f);
                    break;
                }
            }
            i += 1;
        }
        self.msg_chars = out;
        r
    }

    /// `memcmp` (`wide` false: only arrays of narrow character types,
    /// compared as unsigned bytes) and `wmemcmp` (as `wchar_t`); with
    /// `str`, `strcmp`/`strncmp`/`wcscmp`/`wcsncmp`: any element type, and
    /// equal at the first null element (clang `StopAtNull`).
    #[allow(clippy::too_many_arguments, reason = "the instruction's operands")]
    fn bi_memcmp(
        &self,
        ast: &Ast,
        mut p: u64,
        mut q: u64,
        mut n: u64,
        t1: u32,
        t2: u32,
        StrMode { wide, str }: StrMode,
    ) -> Result<u128, Fault> {
        let ret = |o: std::cmp::Ordering| o as i8 as i128 as u128;
        if n == 0 {
            return Ok(0);
        }
        for x in [p, q] {
            if x == 0 {
                let ty = OTy(if t1 == bi::DYN { 0 } else { t1 });
                return Err(Fault::Mem { f: MemFault::Null, ak: ak::READ, ty, p: 0 });
            }
        }
        let (t1, t2) = (self.designated(p, t1)?, self.designated(q, t2)?);
        if !wide {
            if !(str || (is_narrow_char(ast, self, t1) && is_narrow_char(ast, self, t2))) {
                return Err(Fault::Bi(BiFault::CmpType { t1, t2 }));
            }
            if let (Some(x), Some(y)) = (self.byte_run(p, t1, n), self.byte_run(q, t2, n)) {
                let m = x.len().min(y.len());
                if let Some(i) = (0..m).find(|&i| x[i] != y[i] || str && x[i] == 0) {
                    return Ok(ret(x[i].cmp(&y[i])));
                }
                if m as u64 == n {
                    return Ok(0);
                }
                n -= m as u64;
                p = self.ptr_add(p, m as i128, t1)?;
                q = self.ptr_add(q, m as i128, t2)?;
            }
        }
        let signed = wide && matches!(self.types.kind(t1), TyKind::Int { signed: true, .. });
        let bits = if wide { u32::MAX } else { 8 };
        for _ in 0..n {
            let x = self.load(p, t1, ak::READ)?;
            let y = self.load(q, t2, ak::READ)?;
            if x != y {
                let o = if signed {
                    (x as i128).cmp(&(y as i128))
                } else {
                    (x & mask(bits)).cmp(&(y & mask(bits)))
                };
                if o.is_ne() {
                    return Ok(ret(o));
                }
            }
            if str && x & mask(bits) == 0 {
                return Ok(0);
            }
            p = self.ptr_add(p, 1, t1)?;
            q = self.ptr_add(q, 1, t2)?;
        }
        Ok(0)
    }

    /// `memchr`/`char_memchr` (`wide` false: the value converted to
    /// `unsigned char`; `memchr` only over narrow character types) and
    /// `wmemchr`: the first of `n` elements at `p` equal to `v`, else null.
    /// With `str`, `strchr`/`wcschr`: any element type, null after the
    /// first null element, and (`strchr`) null for an `int` value the
    /// element type cannot represent (clang's `HandleIntToIntCast` check).
    fn bi_memchr(
        &self,
        ast: &Ast,
        mut p: u64,
        v: u128,
        mut n: u64,
        t: u32,
        StrMode { wide, str }: StrMode,
    ) -> Result<u64, Fault> {
        if n == 0 {
            return Ok(0);
        }
        if p == 0 {
            let ty = OTy(if t == bi::DYN { 0 } else { t });
            return Err(Fault::Mem { f: MemFault::Null, ak: ak::READ, ty, p: 0 });
        }
        let t = self.designated(p, t)?;
        let w = match self.types.kind(t) {
            TyKind::Int { width, .. } => u32::from(width),
            _ => 8,
        };
        if !wide && !str && !is_narrow_char(ast, self, t) {
            return Err(Fault::Bi(BiFault::ChrType { t }));
        }
        if str && !wide {
            // `strchr`'s `int` argument, sign-extended, must survive the
            // conversion to the element type.
            let sv = i128::from(v as u32 as i32);
            let signed = matches!(self.types.kind(t), TyKind::Int { signed: true, .. });
            let (lo, hi) = if signed {
                (-(1i128 << (w - 1)), (1i128 << (w - 1)) - 1)
            } else {
                (0, (1i128 << w) - 1)
            };
            if sv < lo || sv > hi {
                return Ok(0);
            }
        }
        let want = if wide { v & mask(w) } else { v & 0xFF };
        if !wide && let Some(run) = self.byte_run(p, t, n) {
            if let Some(i) = run.iter().position(|&x| u128::from(x) == want || str && x == 0) {
                if u128::from(run[i]) != want {
                    return Ok(0);
                }
                return self.ptr_add(p, i as i128, t);
            }
            n -= run.len() as u64;
            if n == 0 {
                return Ok(0);
            }
            p = self.ptr_add(p, run.len() as i128, t)?;
        }
        for _ in 0..n {
            let x = self.load(p, t, ak::READ)? & mask(w);
            if x == want {
                return Ok(p);
            }
            if str && x == 0 {
                return Ok(0);
            }
            p = self.ptr_add(p, 1, t)?;
        }
        Ok(0)
    }

    /// Whether objects of type `t` are trivially copyable
    /// ([basic.types.general]/9).
    fn trivially_copyable(&self, ast: &Ast, t: OTy) -> bool {
        match self.types.kind(t) {
            TyKind::Array { elem, .. } => self.trivially_copyable(ast, elem),
            TyKind::Record { decl, .. } => ast
                .decls
                .record(decl)
                .is_some_and(|r| r.defn.contains(DefinitionBits::TRIVIALLY_COPYABLE)),
            TyKind::Func | TyKind::Opaque | TyKind::Void => false,
            _ => true,
        }
    }

    /// `memcpy`/`memmove` (`mv`) of `n` bytes from `src` to `dst`: whole
    /// elements of one trivially copyable type, within both arrays,
    /// assigned one at a time (backward for an overlapping `memmove`).
    #[allow(clippy::too_many_arguments, reason = "the instruction's operands")]
    fn bi_memcpy(
        &mut self,
        ast: &Ast,
        dst: u64,
        src: u64,
        n: u64,
        [t1, t2]: [u32; 2],
        mv: bool,
        wide: bool,
    ) -> Result<(), Fault> {
        if n == 0 {
            return Ok(());
        }
        if src == 0 || dst == 0 {
            return Err(Fault::Bi(BiFault::CopyNull { mv, src: src == 0 }));
        }
        let t = self.designated(dst, t1)?;
        let st = self.designated(src, t2)?;
        if !self.types.same_unqualified(t, st) {
            return Err(Fault::Bi(BiFault::CopyPun { mv, src: st, dst: t }));
        }
        // A pointer to a whole array (an implicit copy of an array
        // member) copies its elements.
        let t = match self.types.kind(t) {
            TyKind::Array { elem, .. } => elem,
            _ => t,
        };
        if !self.trivially_copyable(ast, t) {
            return Err(Fault::Bi(BiFault::CopyNontrivial { mv, t }));
        }
        let size = self.types.size(t);
        if size == 0 {
            return Err(Fault::Invalid);
        }
        // `wmemcpy`/`wmemmove` count elements (clang: no size check).
        let (n, ne) = if wide {
            (n.saturating_mul(size), n)
        } else if !n.is_multiple_of(size) {
            return Err(Fault::Bi(BiFault::CopySize { mv, t, n, size }));
        } else {
            (n, n / size)
        };
        for (p, is_src) in [(src, true), (dst, false)] {
            let (_, len, cur, _) = self.extent_of(p, t)?;
            if ne > len - cur {
                return Err(Fault::Bi(BiFault::CopyRange { mv, src: is_src, t, n: ne }));
            }
        }
        let mut back = false;
        if Ptr::alloc_key(src) == Ptr::alloc_key(dst) {
            let so = u64::from(Ptr::decode(src).off);
            let dof = u64::from(Ptr::decode(dst).off);
            if dof >= so && dof - so < n {
                if !mv {
                    return Err(Fault::Bi(BiFault::CopyOverlap));
                }
                back = true;
            } else if !mv && so >= dof && so - dof < n {
                return Err(Fault::Bi(BiFault::CopyOverlap));
            }
        }
        let info = *self.types.info(t);
        let plain = !matches!(
            info.kind,
            TyKind::Ptr(_) | TyKind::NullPtr | TyKind::Record { .. } | TyKind::Array { .. }
        );
        if plain && self.bulk_copy(dst, src, n, t) {
            return Ok(());
        }
        for i in 0..ne {
            let i = if back { ne - 1 - i } else { i } as i128;
            let (d, s) = (self.ptr_add(dst, i, t)?, self.ptr_add(src, i, t)?);
            if plain || matches!(info.kind, TyKind::Ptr(_) | TyKind::NullPtr) {
                let v = self.load(s, t, ak::READ)?;
                self.store(d, t, v, ak::ASSIGN, false)?;
            } else {
                self.copy_obj(d, s, t, false, None)?;
            }
        }
        Ok(())
    }

    /// Copies `n` bytes of scalar elements of type `t` at once when the
    /// element-wise copy could not fail: the source fully initialized
    /// plain data, the destination writable, neither in an allocation
    /// with unions. Returns whether it copied.
    fn bulk_copy(&mut self, dst: u64, src: u64, n: u64, t: OTy) -> bool {
        let Ok((ss, sa, s_alloc)) = self.mem.check(src, n, false) else { return false };
        if s_alloc.flags & flags::HAS_UNION != 0 {
            return false;
        }
        let space = self.mem.space(ss);
        if !space.is_init(sa, n) || space.any_ptr(sa, n) {
            return false;
        }
        let Ok((ds, da, d_alloc)) = self.mem.check(dst, n, true) else { return false };
        if d_alloc.flags & flags::HAS_UNION != 0 || write_fault(ds, d_alloc, t).is_some() {
            return false;
        }
        // A refused trial write: the element-wise copy reports it.
        if self.mem.trial_write(dst, n).is_err() {
            return false;
        }
        self.mem.copy(ds, da, ss, sa, n);
        true
    }
}

/// The flavour of a `MEMCMP`/`MEMCHR` builtin (`bi::WIDE`, `bi::STR`).
#[derive(Clone, Copy)]
struct StrMode {
    /// Elements compared as `wchar_t` (the `w` forms).
    wide: bool,
    /// The `str`/`wcs` forms: stop at a null element.
    str: bool,
}
