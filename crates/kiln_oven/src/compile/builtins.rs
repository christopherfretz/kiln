//! Lowering of calls of library builtins (spec/oven.md "Builtins"):
//! constants (`huge_val`, `nan`, `nans`) are folded here, the functions
//! clang 19 does not evaluate become a trap at the call, and the rest
//! become one `Op::Builtin` evaluated natively by the VM.

use kiln_ast::consts::FloatKind;
use kiln_ast::decl::builtin as id;
use kiln_ast::expr::ExprKind;
use kiln_ast::{Ast, ExprId, QualType};

use super::{Compiler, float_code, skip_parens};
use crate::kbc::{Op, bi};
use crate::softfloat;
use crate::ty::TyKind;

/// clang's `note_invalid_subexpr_in_const_expr`.
const INVALID: &str = "subexpression not valid in a constant expression";

/// clang's `note_constexpr_new_untyped`.
const UNTYPED_NEW: &str = "cannot allocate untyped memory in a constant expression; use 'std::allocator<T>::allocate' to allocate memory of type 'T'";

/// The type of the object the pointer argument `e` designates, as far as
/// it is known statically: the pointee of `e` with conversions to `void`
/// pointers looked through; `None` for a `void` pointer (the designated
/// object's type is then read at run time).
pub(crate) fn designator_type(ast: &Ast, mut e: ExprId) -> Option<QualType> {
    loop {
        e = skip_parens(ast, e);
        let n = ast.exprs.get(e);
        let pointee = ast.types.pointee(n.ty())?;
        if !ast.types.is_void(pointee) {
            return Some(pointee);
        }
        if !n.kind().is_cast() {
            return None;
        }
        e = ast.exprs.operand(e);
    }
}

/// Whether evaluating `e` would modify an object, allocate, or throw on
/// its own (clang `Expr::HasSideEffects` without its treatment of calls:
/// a called function is evaluated, and a failure in it makes the result
/// 0). `__builtin_constant_p` of such an argument is 0 without evaluating it.
fn has_side_effects(ast: &Ast, e: ExprId) -> bool {
    let exprs = &ast.exprs;
    let mut stack = vec![e];
    while let Some(x) = stack.pop() {
        match exprs.get(x).kind() {
            ExprKind::UnaryOperator if exprs.unary_op(x).is_inc_dec() => return true,
            ExprKind::BinaryOperator | ExprKind::CompoundAssignOperator
                if exprs.binary_op(x).is_assignment() =>
            {
                return true;
            }
            ExprKind::CXXNewExpr
            | ExprKind::CXXDeleteExpr
            | ExprKind::CXXThrowExpr
            | ExprKind::CoawaitExpr
            | ExprKind::CoyieldExpr => return true,
            ExprKind::UnaryExprOrTypeTraitExpr
            | ExprKind::CXXNoexceptExpr
            | ExprKind::LambdaExpr
            | ExprKind::RequiresExpr => {}
            _ => stack.extend(exprs.children(x, &ast.lists)),
        }
    }
    false
}

/// `StringRef::getAsInteger(0, APInt)`: an unsigned integer whose radix
/// is sensed from its prefix (`0x`, `0b`, `0o`, a leading `0`), modulo
/// 2^128 (only the low bits of a NaN payload are kept).
fn parse_auto_radix(s: &[u8]) -> Option<u128> {
    let lower = |i: usize| s.get(i).map(u8::to_ascii_lowercase);
    let (radix, digits) = match (s.first(), lower(1)) {
        (Some(b'0'), Some(b'x')) => (16, &s[2..]),
        (Some(b'0'), Some(b'b')) => (2, &s[2..]),
        (Some(b'0'), Some(b'o')) if s[1] == b'o' => (8, &s[2..]),
        (Some(b'0'), Some(c)) if c.is_ascii_digit() => (8, &s[1..]),
        _ => (10, s),
    };
    if digits.is_empty() {
        return None;
    }
    digits.iter().try_fold(0u128, |v, &c| {
        let d = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'z' => c - b'a' + 10,
            b'A'..=b'Z' => c - b'A' + 10,
            _ => return None,
        };
        (u32::from(d) < radix)
            .then(|| v.wrapping_mul(u128::from(radix)).wrapping_add(u128::from(d)))
    })
}

/// clang's `TryEvaluateBuiltinNaN`: the argument must be a string literal
/// (an empty one is a zero payload).
fn nan_bits(ast: &Ast, mut arg: ExprId, kind: FloatKind, signaling: bool) -> Option<u128> {
    loop {
        arg = skip_parens(ast, arg);
        if !ast.exprs.get(arg).kind().is_cast() {
            break;
        }
        arg = ast.exprs.operand(arg);
    }
    if ast.exprs.get(arg).kind() != ExprKind::StringLiteral {
        return None;
    }
    let s = ast.strings.bytes(ast.exprs.string_lit(arg));
    let fill = if s.is_empty() { 0 } else { parse_auto_radix(s)? };
    Some(softfloat::make_nan(kind, fill, signaling))
}

/// The builtins lowered to LLVM rounding / fused intrinsics, which clang
/// 19 does not evaluate.
fn is_rounding(b: u32) -> bool {
    matches!(
        b,
        id::CEIL
            | id::CEILF
            | id::CEILL
            | id::FLOOR
            | id::FLOORF
            | id::FLOORL
            | id::TRUNC
            | id::TRUNCF
            | id::TRUNCL
            | id::ROUND
            | id::ROUNDF
            | id::ROUNDL
            | id::RINT
            | id::RINTF
            | id::RINTL
            | id::NEARBYINT
            | id::NEARBYINTF
            | id::NEARBYINTL
            | id::FMA
            | id::FMAF
            | id::FMAL
    )
}

impl Compiler<'_> {
    fn int_width(&mut self, e: ExprId) -> u32 {
        let t = self.ety(e);
        match self.kind(t) {
            TyKind::Int { width, .. } => u32::from(width),
            TyKind::Bool => 1,
            _ => 64,
        }
    }

    fn float_kind(&mut self, e: ExprId) -> Option<FloatKind> {
        let t = self.ety(e);
        match self.kind(t) {
            TyKind::Float(k) => Some(k),
            _ => None,
        }
    }

    /// The `t` operand of the object pointer argument `e`.
    fn designator(&mut self, e: ExprId) -> u32 {
        match designator_type(self.ast(), e) {
            Some(q) => self.ty(q).0,
            None => bi::DYN,
        }
    }

    /// Lowers call `e` of builtin `bid` into a fresh register; `None` for
    /// a builtin evaluated as an ordinary call.
    pub(super) fn builtin_call(&mut self, e: ExprId, bid: u32, args: &[ExprId]) -> Option<u32> {
        let begin = self.begin(e);
        let dst = self.reg();
        let arg = |i: usize| args.get(i).copied();
        // `Builtin` with kind `k`, `sub`, registers and types.
        let op = |s: &mut Self, k: u32, sub: u32, r: [u32; 3], t: [u32; 2], loc| {
            s.emit_site(Op::Builtin, k | sub << 8, &[dst, r[0], r[1], r[2], t[0], t[1]], e, loc);
        };
        match bid {
            id::CONSTANT_P => {
                let x = arg(0)?;
                let k = self.ety(x);
                let ptr = matches!(self.kind(k), TyKind::Ptr(_));
                let scalar = ptr
                    || matches!(
                        self.kind(k),
                        TyKind::Bool | TyKind::Int { .. } | TyKind::Float(_) | TyKind::NullPtr
                    );
                if !scalar || has_side_effects(self.b.ast(), x) {
                    // clang `EvaluateBuiltinConstantP`: any other type, or
                    // an argument with side effects, is not a constant.
                    self.konst(dst, 0);
                } else {
                    // The argument is evaluated speculatively in this
                    // frame's registers: the region ends in a `Ret` of
                    // its value, after which `Speculate` resumes at `end`.
                    let pc = self.emit(Op::Speculate, u32::from(ptr), &[dst, 0, 0]);
                    let v = self.rv(x);
                    self.emit(Op::Ret, 1, &[v]);
                    let end = self.here();
                    self.patch(pc + 2, v);
                    self.patch(pc + 3, end);
                }
            }
            id::CLZ
            | id::CLZL
            | id::CLZLL
            | id::CLZG
            | id::CTZ
            | id::CTZL
            | id::CTZLL
            | id::CTZG => {
                let x = arg(0)?;
                let w = self.int_width(x);
                let a = self.rv(x);
                let (fb, has) = match arg(1) {
                    Some(f) => (self.rv(f), 1),
                    None => (dst, 0),
                };
                let k = if matches!(bid, id::CLZ | id::CLZL | id::CLZLL | id::CLZG) {
                    bi::CLZ
                } else {
                    bi::CTZ
                };
                op(self, k, w | has << 8, [a, fb, dst], [0, 0], begin);
            }
            id::POPCOUNT
            | id::POPCOUNTL
            | id::POPCOUNTLL
            | id::POPCOUNTG
            | id::BSWAP16
            | id::BSWAP32
            | id::BSWAP64 => {
                let x = arg(0)?;
                let w = self.int_width(x);
                let a = self.rv(x);
                let k = if matches!(bid, id::BSWAP16 | id::BSWAP32 | id::BSWAP64) {
                    bi::BSWAP
                } else {
                    bi::POPCOUNT
                };
                op(self, k, w, [a, dst, dst], [0, 0], begin);
            }
            id::ISFINITE
            | id::ISINF
            | id::ISNAN
            | id::ISNORMAL
            | id::ISSUBNORMAL
            | id::ISZERO
            | id::ISSIGNALING
            | id::ISINF_SIGN => {
                let x = arg(0)?;
                let Some(fk) = self.float_kind(x) else {
                    self.trap_loc(begin, INVALID.into());
                    return Some(self.finish_builtin(dst));
                };
                let a = self.rv(x);
                let class = match bid {
                    id::ISFINITE => bi::FINITE,
                    id::ISINF => bi::INF,
                    id::ISNAN => bi::NAN,
                    id::ISSUBNORMAL => bi::SUBNORMAL,
                    id::ISZERO => bi::ZERO,
                    id::ISSIGNALING => bi::SIGNALING,
                    id::ISINF_SIGN => {
                        let sub = float_code(fk) | bi::ISINF_SIGN << 4;
                        op(self, bi::FP_OP, sub, [a, dst, dst], [0, 0], begin);
                        return Some(self.finish_builtin(dst));
                    }
                    _ => bi::NORMAL,
                };
                op(self, bi::FP_CLASS, float_code(fk) | class << 4, [a, dst, dst], [0, 0], begin);
            }
            // clang: the mask (an integer constant) first, then the value.
            id::ISFPCLASS => {
                let (x, m) = (arg(0)?, arg(1)?);
                let Some(fk) = self.float_kind(x) else {
                    self.trap_loc(begin, INVALID.into());
                    return Some(self.finish_builtin(dst));
                };
                let b = self.rv(m);
                let a = self.rv(x);
                op(self, bi::FP_CLASS, float_code(fk) | bi::MASK << 4, [a, b, dst], [0, 0], begin);
            }
            id::FPCLASSIFY => {
                // clang: the operand, then only the argument its class
                // selects (nan, infinite, normal, subnormal, zero).
                let (Some(x), true) = (arg(5), args.len() == 6) else {
                    self.trap_loc(begin, INVALID.into());
                    return Some(self.finish_builtin(dst));
                };
                let Some(fk) = self.float_kind(x) else {
                    self.trap_loc(begin, INVALID.into());
                    return Some(self.finish_builtin(dst));
                };
                let a = self.rv(x);
                let mut ends = Vec::new();
                for (i, class) in [(0, bi::NAN), (1, bi::INF), (2, bi::NORMAL), (3, bi::SUBNORMAL)]
                {
                    op(
                        self,
                        bi::FP_CLASS,
                        float_code(fk) | class << 4,
                        [a, dst, dst],
                        [0, 0],
                        begin,
                    );
                    let j = self.jmp_if(dst, false);
                    self.rv_into(args[i], dst);
                    ends.push(self.jmp());
                    self.patch_here(j);
                }
                self.rv_into(args[4], dst);
                for j in ends {
                    self.patch_here(j);
                }
            }
            id::FABS
            | id::FABSF
            | id::FABSL
            | id::COPYSIGN
            | id::COPYSIGNF
            | id::COPYSIGNL
            | id::FMAX
            | id::FMAXF
            | id::FMAXL
            | id::FMIN
            | id::FMINF
            | id::FMINL => {
                let Some(fk) = self.float_kind(e) else {
                    self.trap_loc(begin, INVALID.into());
                    return Some(self.finish_builtin(dst));
                };
                let k = match bid {
                    id::FABS | id::FABSF | id::FABSL => bi::FABS,
                    id::COPYSIGN | id::COPYSIGNF | id::COPYSIGNL => bi::COPYSIGN,
                    id::FMAX | id::FMAXF | id::FMAXL => bi::FMAX,
                    _ => bi::FMIN,
                };
                let a = self.rv(arg(0)?);
                let b = match arg(1) {
                    Some(y) => self.rv(y),
                    None => a,
                };
                op(self, bi::FP_OP, float_code(fk) | k << 4, [a, b, dst], [0, 0], begin);
            }
            id::INF | id::INFF | id::INFL | id::HUGE_VAL | id::HUGE_VALF | id::HUGE_VALL => {
                let fk = self.float_kind(e)?;
                self.konst(dst, softfloat::inf(fk));
            }
            id::NAN | id::NANF | id::NANL | id::NANS | id::NANSF | id::NANSL => {
                let fk = self.float_kind(e)?;
                let signaling = matches!(bid, id::NANS | id::NANSF | id::NANSL);
                match arg(0).and_then(|x| nan_bits(self.ast(), x, fk, signaling)) {
                    Some(bits) => self.konst(dst, bits),
                    None => self.trap_loc(begin, INVALID.into()),
                }
            }
            id::LAUNDER => self.rv_into(arg(0)?, dst),
            // clang: the value of the first operand (the expected value is
            // not evaluated).
            id::EXPECT | id::EXPECT_WITH_PROBABILITY => self.rv_into(arg(0)?, dst),
            // clang: the assumption is not evaluated.
            id::ASSUME => self.konst(dst, 0),
            id::OPERATOR_NEW => self.trap_loc(begin, UNTYPED_NEW.into()),
            // Not evaluated by clang 19: the call is not a constant
            // expression, its arguments unevaluated.
            id::OPERATOR_DELETE
            | id::UNREACHABLE
            | id::SIGNBIT
            | id::ISGREATER
            | id::ISGREATEREQUAL
            | id::ISLESS
            | id::ISLESSEQUAL
            | id::ISLESSGREATER
            | id::ISUNORDERED
            | id::ABS
            | id::LABS
            | id::LLABS
            | id::TRAP
            | id::ABORT
            | id::STRRCHR
            | id::STRPBRK
            | id::STRSTR => self.trap_loc(begin, INVALID.into()),
            // The libm functions and the rounding intrinsics.
            _ if id::is_libm(bid) || is_rounding(bid) => self.trap_loc(begin, INVALID.into()),
            id::STRLEN | id::WCSLEN => {
                let x = arg(0)?;
                let q = self.ast().exprs.get(x).ty();
                let ct = self.ast().types.pointee(q)?;
                let t = self.ty(ct).0;
                let a = self.rv(x);
                // clang reads through the argument (notes at its location).
                let loc = self.ast().exprs.get(x).loc();
                op(self, bi::STRLEN, 0, [a, dst, dst], [t, 0], loc);
            }
            id::MEMCMP | id::WMEMCMP | id::STRCMP | id::STRNCMP | id::WCSCMP | id::WCSNCMP => {
                let (x, y) = (arg(0)?, arg(1)?);
                let mut sub = u32::from(matches!(bid, id::WMEMCMP | id::WCSCMP | id::WCSNCMP));
                if matches!(bid, id::STRCMP | id::STRNCMP | id::WCSCMP | id::WCSNCMP) {
                    sub |= bi::STR;
                }
                let n = if matches!(bid, id::STRCMP | id::WCSCMP) {
                    sub |= bi::UNBOUNDED;
                    None
                } else {
                    Some(arg(2)?)
                };
                let t = [self.designator(x), self.designator(y)];
                let (a, b) = (self.rv(x), self.rv(y));
                let c = n.map_or(dst, |n| self.rv(n));
                op(self, bi::MEMCMP, sub, [a, b, c], t, begin);
            }
            id::MEMCHR | id::CHAR_MEMCHR | id::WMEMCHR | id::STRCHR | id::WCSCHR => {
                let (x, v) = (arg(0)?, arg(1)?);
                let mut sub = u32::from(matches!(bid, id::WMEMCHR | id::WCSCHR));
                let n = if matches!(bid, id::STRCHR | id::WCSCHR) {
                    sub |= bi::STR | bi::UNBOUNDED;
                    None
                } else {
                    Some(arg(2)?)
                };
                let t = self.designator(x);
                let (a, b) = (self.rv(x), self.rv(v));
                let c = n.map_or(dst, |n| self.rv(n));
                op(self, bi::MEMCHR, sub, [a, b, c], [t, 0], begin);
            }
            id::MEMCPY | id::MEMMOVE => {
                let (d, s, n) = (arg(0)?, arg(1)?, arg(2)?);
                let t = [self.designator(d), self.designator(s)];
                let r = [self.rv(d), self.rv(s), self.rv(n)];
                op(self, bi::MEMCPY, u32::from(bid == id::MEMMOVE) * bi::MOVE, r, t, begin);
            }
            _ => {
                self.next = dst;
                return None;
            }
        }
        Some(self.finish_builtin(dst))
    }

    fn finish_builtin(&mut self, dst: u32) -> u32 {
        self.next = dst + 1;
        dst
    }
}

#[cfg(test)]
mod tests {
    use super::parse_auto_radix as p;

    /// clang's `__builtin_nan` payload parsing ([library.c]; clang
    /// `TryEvaluateBuiltinNaN`).
    #[test]
    fn nan_payload_radix() {
        assert_eq!(p(b"0"), Some(0));
        assert_eq!(p(b"12"), Some(12));
        assert_eq!(p(b"0x1F"), Some(31));
        assert_eq!(p(b"0X1f"), Some(31));
        assert_eq!(p(b"0b101"), Some(5));
        assert_eq!(p(b"0o17"), Some(15));
        assert_eq!(p(b"017"), Some(15));
        assert_eq!(p(b"08"), None);
        assert_eq!(p(b"0x"), None);
        assert_eq!(p(b"-1"), None);
        assert_eq!(p(b"1a"), None);
    }
}
