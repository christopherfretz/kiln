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
            id::ISFINITE | id::ISINF | id::ISNAN | id::ISNORMAL => {
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
                    _ => bi::NORMAL,
                };
                op(self, bi::FP_CLASS, float_code(fk) | class << 4, [a, dst, dst], [0, 0], begin);
            }
            id::HUGE_VAL | id::HUGE_VALF | id::HUGE_VALL => {
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
            | id::EXP
            | id::EXPF
            | id::EXPL
            | id::EXP2
            | id::EXP2F
            | id::EXP2L
            | id::EXPM1
            | id::EXPM1F
            | id::EXPM1L
            | id::FREXP
            | id::FREXPF
            | id::FREXPL
            | id::LDEXP
            | id::LDEXPF
            | id::LDEXPL
            | id::POW
            | id::POWF
            | id::POWL
            | id::SCALBN
            | id::SCALBNF
            | id::SCALBNL
            | id::SCALBLN
            | id::SCALBLNF
            | id::SCALBLNL => self.trap_loc(begin, INVALID.into()),
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
            id::MEMCMP | id::WMEMCMP => {
                let (x, y, n) = (arg(0)?, arg(1)?, arg(2)?);
                let t = [self.designator(x), self.designator(y)];
                let r = [self.rv(x), self.rv(y), self.rv(n)];
                op(self, bi::MEMCMP, u32::from(bid == id::WMEMCMP) * bi::WIDE, r, t, begin);
            }
            id::MEMCHR | id::CHAR_MEMCHR | id::WMEMCHR => {
                let (x, v, n) = (arg(0)?, arg(1)?, arg(2)?);
                let t = self.designator(x);
                let r = [self.rv(x), self.rv(v), self.rv(n)];
                op(self, bi::MEMCHR, u32::from(bid == id::WMEMCHR) * bi::WIDE, r, [t, 0], begin);
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
