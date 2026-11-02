//! The `printf` `format` attribute's argument checks (spec/sema.md §5.4
//! "Format strings"; clang 19 `Sema::CheckFormatArguments` /
//! `CheckPrintfHandler`), a deliberately small part of `-Wformat`: a
//! literal format string's `%[flags][width][.precision][l|ll|z]conv`
//! conversions with `conv` one of `d i u o x X c s p f F e E g G a A`
//! are matched against the data arguments (wrong type, `*` not `int`,
//! more conversions than arguments, unused arguments; the walk stops,
//! as clang's does, at the first missing or mistyped `*` argument or
//! missing conversion argument). Any other
//! conversion, length modifier, positional argument, embedded NUL or
//! incomplete specifier makes the whole string unchecked (clang has a
//! diagnostic for each; they are not modelled).

use kiln_ast::attr::AttrKind;
use kiln_ast::expr::{CastKind, ExprKind, StringKind};
use kiln_ast::types::BuiltinType;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv;

/// The argument type a conversion specifies (clang's `ArgType`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Want {
    /// An integer of rank class `class` (0 `int`, 1 `long`, 2 `long long`;
    /// signedness is not checked, as `-Wformat-signedness` is off by
    /// default); `size` names `size_t` / `ssize_t`.
    Int { class: u8, unsigned: bool, size: bool },
    /// `double` (a `float` argument is promoted).
    Double,
    /// `char *`: a pointer to any character type.
    CStr,
    /// `void *`: any pointer or `std::nullptr_t`.
    VoidPtr,
}

/// A diagnostic found while walking the string, emitted only if the walk
/// finishes (an unchecked construct discards them).
struct Pending {
    id: DiagId,
    at: Loc,
    args: Vec<String>,
}

/// The rank class of an integer type after the integral promotions.
fn int_class(b: BuiltinType) -> Option<u8> {
    use BuiltinType as B;
    Some(match b {
        B::Bool
        | B::Char
        | B::SChar
        | B::UChar
        | B::WChar
        | B::Char8
        | B::Char16
        | B::Char32
        | B::Short
        | B::UShort
        | B::Int
        | B::UInt => 0,
        B::Long | B::ULong => 1,
        B::LongLong | B::ULongLong => 2,
        B::Int128 | B::UInt128 => 3,
        _ => return None,
    })
}

impl Sema<'_, '_> {
    /// Checks a call's arguments against the callee's `format(printf, F,
    /// A)` attribute when the format string is a literal and the data
    /// arguments are variadic (`A` > 0). Only warns.
    pub(crate) fn check_format_attr(&mut self, callee: DeclId, args: &[ExprId]) {
        let Some(attr) = self.find_attr(callee, AttrKind::FORMAT) else { return };
        let b = self.ast.attrs.get(attr).b;
        let (fmt, first) = ((b & 0xffff) as usize, (b >> 16) as usize);
        if fmt == 0 || first == 0 || fmt > args.len() || first > args.len() + 1 {
            return;
        }
        let data = &args[first - 1..];
        if data.iter().any(|&a| self.expr_is_type_dependent(a)) {
            return;
        }
        let f = args[fmt - 1];
        if self.expr_is_value_dependent(f) {
            return;
        }
        let Some(lit) = self.string_lit_of(self.ignore_paren_imp_casts(f)) else { return };
        if self.ast.exprs.string_kind(lit) != StringKind::Ordinary {
            return;
        }
        let start = self.ast.exprs.get(lit).loc();
        let bytes = self.ast.strings.bytes(self.ast.exprs.string_lit(lit)).to_vec();
        // The stored bytes end in the terminating NUL.
        let body = bytes.strip_suffix(&[0]).unwrap_or(&bytes);
        if body.contains(&0) {
            return;
        }
        if let Some(diags) = self.walk_printf(body, start, data) {
            for p in diags {
                let mut d = self.diag(p.id, p.at);
                for a in p.args {
                    d = d.arg(a);
                }
                self.emit(d);
            }
        }
    }

    /// Walks the conversions of `s`, consuming `data`; `None` when the
    /// string holds a construct this check does not model.
    fn walk_printf(&mut self, s: &[u8], start: Loc, data: &[ExprId]) -> Option<Vec<Pending>> {
        let mut out = Vec::new();
        let mut next = 0usize;
        let mut i = 0usize;
        let byte_loc = |me: &Self, off: usize| {
            me.string_byte_loc(start, u32::try_from(off).unwrap_or(u32::MAX))
        };
        while i < s.len() {
            if s[i] != b'%' {
                i += 1;
                continue;
            }
            i += 1;
            if s.get(i) == Some(&b'%') {
                i += 1;
                continue;
            }
            let digits = s[i..].iter().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0 && s.get(i + digits) == Some(&b'$') {
                return None;
            }
            let (star_offs, conv_at, want) = parse_spec(s, &mut i)?;
            for (off, precision) in star_offs {
                let &a = data.get(next)?;
                next += 1;
                let star = Want::Int { class: 0, unsigned: false, size: false };
                if let Some((_, ty)) = self.format_mismatch(star, a) {
                    out.push(Pending {
                        id: DiagId::sema_format_star_type,
                        at: byte_loc(self, off),
                        args: vec![
                            if precision { "1" } else { "0" }.to_owned(),
                            "'int'".to_owned(),
                            ty,
                        ],
                    });
                    // clang stops at a mistyped `*` argument.
                    return Some(out);
                }
            }
            let Some(&a) = data.get(next) else {
                // clang stops at the first conversion without an argument.
                out.push(Pending {
                    id: DiagId::sema_format_insufficient_args,
                    at: byte_loc(self, conv_at),
                    args: Vec::new(),
                });
                return Some(out);
            };
            next += 1;
            if let Some((underlying, ty)) = self.format_mismatch(want, a) {
                out.push(Pending {
                    id: DiagId::sema_format_conversion_type,
                    at: self.begin_loc(a),
                    args: vec![self.want_name(want), if underlying { "1" } else { "0" }.into(), ty],
                });
            }
        }
        if let Some(&a) = data.get(next) {
            out.push(Pending {
                id: DiagId::sema_format_extra_args,
                at: self.begin_loc(a),
                args: Vec::new(),
            });
        }
        Some(out)
    }

    /// clang's spelling of the type a conversion specifies.
    fn want_name(&self, w: Want) -> String {
        match w {
            Want::Int { size: true, unsigned, .. } => {
                let st = self.size_type();
                let canon = self.ast.types.builtin(st).map_or("unsigned long", BuiltinType::name);
                if unsigned {
                    format!("'size_t' (aka '{canon}')")
                } else {
                    let signed = canon.strip_prefix("unsigned ").unwrap_or(canon);
                    format!("'ssize_t' (aka '{signed}')")
                }
            }
            Want::Int { class, unsigned, .. } => {
                let base = ["int", "long", "long long"][usize::from(class.min(2))];
                if unsigned { format!("'unsigned {base}'") } else { format!("'{base}'") }
            }
            Want::Double => "'double'".to_owned(),
            Want::CStr => "'char *'".to_owned(),
            Want::VoidPtr => "'void *'".to_owned(),
        }
    }

    /// `Some((through_enum, type))` when argument `a` does not match `w`:
    /// the type clang reports (the argument before its default
    /// promotions, or an unscoped enumeration's underlying type).
    /// Class and scoped-enumeration arguments are not checked.
    fn format_mismatch(&self, w: Want, a: ExprId) -> Option<(bool, String)> {
        let mut e = a;
        for _ in 0..8 {
            let n = self.ast.exprs.get(e);
            if n.kind() == ExprKind::ImplicitCastExpr
                && matches!(
                    self.ast.exprs.cast_kind(e),
                    CastKind::IntegralCast | CastKind::FloatingCast
                )
            {
                e = self.ast.exprs.operand(e);
            } else {
                break;
            }
        }
        let mut ty: QualType = self.ety(e);
        let types = &self.ast.types;
        if types.is_dependent(ty) || types.as_record(ty).is_some() || conv::is_scoped_enum(self, ty)
        {
            return None;
        }
        let mut underlying = false;
        if let Some(it) = conv::rank::enum_data(self, ty).and_then(|en| en.int_type) {
            ty = it;
            underlying = true;
        }
        let b = types.builtin(ty);
        let ok = match w {
            Want::Int { class, size, .. } => {
                let want = if size {
                    self.ast.types.builtin(self.size_type()).and_then(int_class).unwrap_or(1)
                } else {
                    class
                };
                b.and_then(int_class) == Some(want)
            }
            Want::Double => matches!(b, Some(BuiltinType::Float | BuiltinType::Double)),
            Want::CStr => types.pointee(ty).filter(|_| types.is_pointer(ty)).is_some_and(|p| {
                matches!(
                    types.builtin(p),
                    Some(BuiltinType::Char | BuiltinType::SChar | BuiltinType::UChar)
                )
            }),
            Want::VoidPtr => types.is_pointer(ty) || b == Some(BuiltinType::NullPtr),
        };
        (!ok).then(|| (underlying, self.ty_arg(ty)))
    }
}

/// A `*` width or precision: the byte offset its diagnostic points at,
/// and whether it is the precision.
type StarOff = (usize, bool);

/// Parses the flags, width, precision, length and conversion of the
/// specification after a `%` at `*i`, leaving `*i` after the conversion:
/// the `*` diagnostic offsets (with whether each is the precision), the
/// conversion's offset and what it wants. `None` for an unmodelled one.
fn parse_spec(s: &[u8], i: &mut usize) -> Option<(Vec<StarOff>, usize, Want)> {
    while s.get(*i).is_some_and(|c| b"-+ #0".contains(c)) {
        *i += 1;
    }
    // Width and precision: (byte offset of the diagnostic, is
    // precision) per `*`. clang reports a width `*` one byte
    // after it and a precision `*` at it.
    let mut star_offs = Vec::new();
    if s.get(*i) == Some(&b'*') {
        star_offs.push((*i + 1, false));
        *i += 1;
    } else {
        *i += s[*i..].iter().take_while(|c| c.is_ascii_digit()).count();
    }
    if s.get(*i) == Some(&b'.') {
        *i += 1;
        if s.get(*i) == Some(&b'*') {
            star_offs.push((*i, true));
            *i += 1;
        } else {
            *i += s[*i..].iter().take_while(|c| c.is_ascii_digit()).count();
        }
    }
    let (class, size) = match (s.get(*i), s.get(*i + 1)) {
        (Some(b'l'), Some(b'l')) => (2, false),
        (Some(b'l'), _) => (1, false),
        (Some(b'z'), _) => (1, true),
        _ => (0, false),
    };
    *i += match (class, size) {
        (2, _) => 2,
        (1, _) => 1,
        _ => 0,
    };
    let conv_at = *i;
    let want = match s.get(*i)? {
        b'd' | b'i' => Want::Int { class, unsigned: false, size },
        b'u' | b'o' | b'x' | b'X' => Want::Int { class, unsigned: true, size },
        b'c' if class == 0 => Want::Int { class, unsigned: false, size },
        b's' if class == 0 => Want::CStr,
        b'p' if class == 0 => Want::VoidPtr,
        b'f' | b'F' | b'e' | b'E' | b'g' | b'G' | b'a' | b'A' if class == 0 => Want::Double,
        _ => return None,
    };
    *i += 1;
    Some((star_offs, conv_at, want))
}
