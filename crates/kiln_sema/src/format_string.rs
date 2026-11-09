//! The `format` attribute's call checks (spec/sema.md §5.4 "Format
//! strings"; clang `Sema::CheckFormatArguments`, `checkFormatStringExpr`,
//! `CheckPrintfHandler`), a deliberately small part of `-Wformat`: the
//! format argument is classified (literal, unchecked literal, not a
//! literal: `-Wformat-security` when there are no data arguments), and
//! for `printf` each literal's `%[flags][width][.precision][l|ll|z]conv`
//! conversions with `conv` one of `d i u o x X c s p f F e E g G a A`
//! are matched against the data arguments (wrong type, `*` not `int`,
//! more conversions than arguments, incomplete and invalid specifiers,
//! unused arguments across conditional arms; the walk stops where
//! clang's does). Any other conversion, length modifier, positional
//! argument or embedded NUL makes the whole string unchecked (clang has
//! a diagnostic for each; they are not modelled). WP-1073: a `scanf`
//! literal is walked the same way against pointer arguments
//! (`CheckScanfHandler`, [`parse_scan`]).

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

/// How far a format string covered the data arguments (clang
/// `UncoveredArgHandler`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cover {
    /// The walk stopped early; the string does not take part.
    Stopped,
    /// Every data argument is used (or the string is not checked).
    All,
    /// The first data argument the string leaves unused.
    First(usize),
}

/// What a format-string expression is (clang `StringLiteralCheckType`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Source {
    /// Not a literal: `-Wformat-security` when there are no data
    /// arguments.
    NotLiteral,
    /// A literal kiln does not check (a `const` variable's initializer,
    /// `__func__`, a null pointer, a literal plus an offset).
    Unchecked,
    /// Only checked string literals.
    Checked,
}

impl Sema<'_, '_> {
    /// Checks a call's arguments against the callee's `format(printf, F,
    /// A)` attribute (`args` exclude an implicit object argument, which
    /// `F` and `A` count). Each string literal the format argument can
    /// be (through a conditional operator's arms) is matched against the
    /// variadic data arguments (`A` > 0); a format that is not a literal
    /// with no data arguments is `-Wformat-security`'s. Only warns.
    pub(crate) fn check_format_attr(&mut self, callee: DeclId, args: &[ExprId]) {
        let Some(attr) = self.find_attr(callee, AttrKind::FORMAT) else { return };
        let node = *self.ast.attrs.get(attr);
        if node.a == crate::vector::FORMAT_STRFTIME {
            return;
        }
        let this = usize::from(self.is_implicit_object_member(callee));
        let (fmt, first) = ((node.b & 0xffff) as usize, (node.b >> 16) as usize);
        let (Some(fmt), Some(first)) =
            (fmt.checked_sub(this), if first == 0 { Some(0) } else { first.checked_sub(this) })
        else {
            return;
        };
        if fmt == 0 || fmt > args.len() || first > args.len() + 1 {
            return;
        }
        let data = if first == 0 { &[][..] } else { &args[first - 1..] };
        if data.iter().any(|&a| self.expr_is_type_dependent(a)) {
            return;
        }
        let f = args[fmt - 1];
        if self.expr_is_type_dependent(f) || self.expr_is_value_dependent(f) {
            return;
        }
        let mut lits = Vec::new();
        let source = self.format_sources(f, &mut lits, 0);
        if source == Source::NotLiteral {
            if first != 0 && data.is_empty() {
                let at = self.begin_loc(f);
                let mut msg = self.diag(DiagId::sema_format_nonliteral_noargs, at);
                if node.a == crate::vector::FORMAT_PRINTF {
                    msg = msg.with_note(self.diag(DiagId::note_format_security_fixit, at));
                }
                self.emit(msg);
            }
            return;
        }
        let scanf = node.a == crate::vector::FORMAT_SCANF;
        if (node.a != crate::vector::FORMAT_PRINTF && !scanf) || first == 0 {
            return;
        }
        // clang `UncoveredArgHandler`: the first data argument no
        // completely walked string uses, unless one string uses them all.
        let mut uncovered: Option<usize> = None;
        let mut all = source == Source::Unchecked;
        for lit in lits {
            match self.check_format_literal(lit, data, scanf) {
                Cover::Stopped => {}
                Cover::All => all = true,
                Cover::First(i) => uncovered = Some(uncovered.map_or(i, |u| u.max(i))),
            }
        }
        if !all && let Some(i) = uncovered {
            let msg = self.diag(DiagId::sema_format_extra_args, self.begin_loc(data[i]));
            self.emit(msg);
        }
    }

    /// The string literals format argument `e` can be (clang
    /// `checkFormatStringExpr`), collected into `lits`, and what it is
    /// overall: a conditional operator is each arm (only the selected
    /// one when the condition is a constant), the least literal of the
    /// two; a `const` variable with an initializer, `__func__`, a null
    /// pointer constant, a literal plus a constant or the address of a
    /// literal's element are literals kiln does not check; anything else
    /// is not a literal.
    fn format_sources(&mut self, e: ExprId, lits: &mut Vec<ExprId>, depth: u32) -> Source {
        if depth > 32 {
            return Source::Unchecked;
        }
        let e = self.ignore_paren_casts(e);
        if conv::is_null_pointer_constant(self, e) {
            return Source::Unchecked;
        }
        match self.ast.exprs.get(e).kind() {
            ExprKind::StringLiteral => {
                lits.push(e);
                Source::Checked
            }
            ExprKind::ConditionalOperator => {
                let cond = self.ast.exprs.children(e, &self.ast.lists)[0];
                let [l, r] = self.ast.exprs.cond_arms(e);
                if let Some(v) = self.fold_int(cond) {
                    return self.format_sources(if v != 0 { l } else { r }, lits, depth + 1);
                }
                let a = self.format_sources(l, lits, depth + 1);
                if a == Source::NotLiteral {
                    return a;
                }
                a.min(self.format_sources(r, lits, depth + 1))
            }
            ExprKind::PredefinedExpr => Source::Unchecked,
            ExprKind::DeclRefExpr => {
                let d = self.ast.exprs.decl(e);
                let t = &self.ast.types;
                let ty = self.ety(e);
                let constant = if t.is_pointer(ty) {
                    ty.quals().is_const() && t.pointee(ty).is_some_and(|p| p.quals().is_const())
                } else {
                    t.is_array(ty) && t.element(ty).is_some_and(|el| el.quals().is_const())
                };
                let init = self
                    .ast
                    .redecls_of(d)
                    .any(|r| self.ast.decls.var(r).is_some_and(|v| v.init.is_some()));
                if constant && init && self.ast.decls.var(d).is_some() {
                    Source::Unchecked
                } else {
                    Source::NotLiteral
                }
            }
            ExprKind::BinaryOperator | ExprKind::UnaryOperator => {
                let kids = self.ast.exprs.children(e, &self.ast.lists);
                let lit = kids.iter().any(|&k| {
                    let k = self.ignore_paren_casts(k);
                    matches!(
                        self.ast.exprs.get(k).kind(),
                        ExprKind::StringLiteral | ExprKind::ArraySubscriptExpr
                    )
                });
                if lit { Source::Unchecked } else { Source::NotLiteral }
            }
            _ => Source::NotLiteral,
        }
    }

    /// Walks string literal `lit` against `data`, emitting what it finds.
    fn check_format_literal(&mut self, lit: ExprId, data: &[ExprId], scanf: bool) -> Cover {
        if self.ast.exprs.string_kind(lit) != StringKind::Ordinary {
            return Cover::All;
        }
        let start = self.ast.exprs.get(lit).loc();
        let bytes = self.ast.strings.bytes(self.ast.exprs.string_lit(lit)).to_vec();
        // The stored bytes end in the terminating NUL.
        let body = bytes.strip_suffix(&[0]).unwrap_or(&bytes);
        if body.contains(&0) {
            return Cover::All;
        }
        let walked = if scanf {
            self.walk_scanf(body, start, data)
        } else {
            self.walk_printf(body, start, data)
        };
        let Some((diags, cover)) = walked else {
            return Cover::All;
        };
        for p in diags {
            let mut d = self.diag(p.id, p.at);
            for a in p.args {
                d = d.arg(a);
            }
            self.emit(d);
        }
        cover
    }

    /// Walks the conversions of `s`, consuming `data`; `None` when the
    /// string holds a construct this check does not model.
    fn walk_printf(
        &mut self,
        s: &[u8],
        start: Loc,
        data: &[ExprId],
    ) -> Option<(Vec<Pending>, Cover)> {
        let mut out = Vec::new();
        let mut covered = vec![false; data.len()];
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
            let pct = i;
            i += 1;
            if s.get(i) == Some(&b'%') {
                i += 1;
                continue;
            }
            let digits = s[i..].iter().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0 && s.get(i + digits) == Some(&b'$') {
                return None;
            }
            let (star_offs, conv_at, want) = match parse_spec(s, &mut i) {
                Spec::Conv(offs, at, want) => (offs, at, want),
                Spec::Unmodelled => return None,
                Spec::Incomplete => {
                    out.push(Pending {
                        id: DiagId::sema_format_incomplete_specifier,
                        at: byte_loc(self, pct),
                        args: Vec::new(),
                    });
                    return Some((out, Cover::Stopped));
                }
                Spec::Invalid(at) => {
                    // clang `HandleInvalidConversionSpecifier`: the next
                    // argument counts as used (without being consumed);
                    // with none left the walk stops.
                    out.push(Pending {
                        id: DiagId::sema_format_invalid_specifier,
                        at: byte_loc(self, at),
                        args: vec![conversion_spelling(s[at])],
                    });
                    match covered.get_mut(next) {
                        Some(c) => *c = true,
                        None => return Some((out, Cover::Stopped)),
                    }
                    continue;
                }
            };
            for (off, precision) in star_offs {
                let &a = data.get(next)?;
                covered[next] = true;
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
                    return Some((out, Cover::Stopped));
                }
            }
            let Some(&a) = data.get(next) else {
                // clang stops at the first conversion without an argument.
                out.push(Pending {
                    id: DiagId::sema_format_insufficient_args,
                    at: byte_loc(self, conv_at),
                    args: Vec::new(),
                });
                return Some((out, Cover::Stopped));
            };
            covered[next] = true;
            next += 1;
            if let Some((underlying, ty)) = self.format_mismatch(want, a) {
                out.push(Pending {
                    id: DiagId::sema_format_conversion_type,
                    at: self.begin_loc(a),
                    args: vec![self.want_name(want), if underlying { "1" } else { "0" }.into(), ty],
                });
            }
        }
        let cover = covered.iter().position(|&c| !c).map_or(Cover::All, Cover::First);
        Some((out, cover))
    }

    /// WP-1073: [`Self::walk_printf`] for a `scanf` format (clang
    /// `CheckScanfHandler`): each `%[*][width][length]conv` with `conv`
    /// one of `d i u o x X b n c s [ p f F e E g G a A` consumes a data
    /// argument (none with `*`) that must point to the type it
    /// specifies; `None` for a construct not modelled (a positional
    /// argument, a zero width, the `j` `t` `q` `L` `m` `I` `w` lengths on
    /// an integer, any length on `c` `s` `[` `p`, an unclosed `%[`, a
    /// mismatch [`scan_mismatch`] cannot spell).
    fn walk_scanf(
        &mut self,
        s: &[u8],
        start: Loc,
        data: &[ExprId],
    ) -> Option<(Vec<Pending>, Cover)> {
        let mut out = Vec::new();
        let mut covered = vec![false; data.len()];
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
            let pct = i;
            i += 1;
            if s.get(i) == Some(&b'%') {
                i += 1;
                continue;
            }
            let digits = s[i..].iter().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0 && s.get(i + digits) == Some(&b'$') {
                return None;
            }
            let want = match parse_scan(s, &mut i) {
                Scan::Conv(at, want) => (at, want),
                Scan::Unmodelled => return None,
                Scan::Incomplete => {
                    out.push(Pending {
                        id: DiagId::sema_format_incomplete_specifier,
                        at: byte_loc(self, pct),
                        args: Vec::new(),
                    });
                    return Some((out, Cover::Stopped));
                }
                Scan::Invalid(at) => {
                    out.push(Pending {
                        id: DiagId::sema_format_invalid_specifier,
                        at: byte_loc(self, at),
                        args: vec![conversion_spelling(s[at])],
                    });
                    match covered.get_mut(next) {
                        Some(c) => *c = true,
                        None => return Some((out, Cover::Stopped)),
                    }
                    continue;
                }
            };
            let (conv_at, Some(want)) = want else { continue };
            let Some(&a) = data.get(next) else {
                out.push(Pending {
                    id: DiagId::sema_format_insufficient_args,
                    at: byte_loc(self, conv_at),
                    args: Vec::new(),
                });
                return Some((out, Cover::Stopped));
            };
            covered[next] = true;
            next += 1;
            if let Some(ty) = self.scan_mismatch(want, a)? {
                out.push(Pending {
                    id: DiagId::sema_format_conversion_type,
                    at: self.begin_loc(a),
                    args: vec![scan_want_name(want), "0".into(), ty],
                });
            }
        }
        let cover = covered.iter().position(|&c| !c).map_or(Cover::All, Cover::First);
        Some((out, cover))
    }

    /// `Some(Some(type))` when argument `a` does not point to what `w`
    /// specifies (the argument's type as clang reports it), `Some(None)`
    /// when it does, `None` when the case is not modelled (a `std::nullptr_t`
    /// argument, whose spelling differs between clang 19 and 22, a `%z`
    /// mismatch, an extended character or floating type, a pointee clang
    /// may treat specially). Signedness is not checked; a `const` pointee
    /// never matches; an unscoped enumeration matches as its underlying
    /// type. Class arguments are not checked.
    fn scan_mismatch(&self, w: ScanWant, a: ExprId) -> Option<Option<String>> {
        use BuiltinType as B;
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
        let ty: QualType = self.ety(e);
        let types = &self.ast.types;
        if types.is_dependent(ty) || types.as_record(ty).is_some() {
            return Some(None);
        }
        if types.builtin(self.ety(self.ignore_paren_imp_casts(a))) == Some(B::NullPtr) {
            return None;
        }
        let bad = Some(Some(self.ty_arg(ty)));
        let Some(p) = types.pointee(ty).filter(|_| types.is_pointer(ty)) else {
            return bad;
        };
        let p = types.canonical(p);
        if p.quals().is_const() {
            return bad;
        }
        if types.is_pointer(p) {
            return if w == ScanWant::VoidPtrPtr { Some(None) } else { bad };
        }
        if types.as_record(p).is_some() || conv::is_scoped_enum(self, p) {
            return bad;
        }
        let p = match conv::rank::enum_data(self, p) {
            Some(en) => en.int_type?,
            None => p,
        };
        let Some(b) = types.builtin(p) else {
            // An array, function or member pointer pointee.
            return None;
        };
        let ok = match w {
            ScanWant::Int { len, size, .. } => {
                let have = match b {
                    B::Char | B::SChar | B::UChar => -2,
                    B::Short | B::UShort => -1,
                    B::Int | B::UInt => 0,
                    B::Long | B::ULong => 1,
                    B::LongLong | B::ULongLong => 2,
                    B::WChar | B::Char8 | B::Char16 | B::Char32 | B::Int128 | B::UInt128 => {
                        return None;
                    }
                    B::Bool if len != 0 => return None,
                    _ => i8::MAX,
                };
                let ok = have == len;
                if size && !ok {
                    return None;
                }
                ok
            }
            ScanWant::Float(k) => match b {
                B::Float | B::Double | B::LongDouble => {
                    k == [B::Float, B::Double, B::LongDouble].iter().position(|&x| x == b)?
                        as u8
                }
                _ if types.is_floating(p) => return None,
                _ => false,
            },
            ScanWant::Char => match b {
                B::Char | B::SChar | B::UChar => true,
                B::Char8 => return None,
                _ => false,
            },
            ScanWant::VoidPtrPtr => false,
        };
        if ok { Some(None) } else { bad }
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
                    if self.clang_compat >= 22 {
                        format!("'signed size_t' (aka '{signed}')")
                    } else {
                        format!("'ssize_t' (aka '{signed}')")
                    }
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

/// What [`parse_spec`] found after a `%`.
enum Spec {
    /// A modelled conversion: the `*` diagnostic offsets (with whether
    /// each is the precision), the conversion's offset and what it wants.
    Conv(Vec<StarOff>, usize, Want),
    /// The string ends inside the specification.
    Incomplete,
    /// A conversion character clang rejects, at this offset.
    Invalid(usize),
    /// Anything else (a length modifier or conversion not modelled).
    Unmodelled,
}

/// The printable conversion characters clang 19/22 reject in a `printf`
/// format besides `"`, `\` and a non-positional `$` (every other
/// printable ASCII character is a flag, a width, a length modifier or a
/// conversion clang knows on some target; `[` and `{` start constructs
/// clang reports as incomplete, not modelled). Control characters are
/// rejected too.
const INVALID_CONVERSIONS: &[u8] = b"krvyDHJKMNOPQRTUVWYZ!&(),/:;<=>?@]^_`|}~";

/// How clang spells conversion character `c` in a diagnostic: control
/// characters as `\xNN`.
fn conversion_spelling(c: u8) -> String {
    if c.is_ascii_control() { format!("\\x{c:02x}") } else { char::from(c).to_string() }
}

/// Parses the flags, width, precision, length and conversion of the
/// specification after a `%` at `*i`, leaving `*i` after the conversion.
fn parse_spec(s: &[u8], i: &mut usize) -> Spec {
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
    // Length modifiers not modelled: still skipped to find the end.
    let mut other_len = false;
    while s.get(*i).is_some_and(|c| b"hjtLq".contains(c)) {
        other_len = true;
        *i += 1;
    }
    let conv_at = *i;
    let Some(&c) = s.get(*i) else { return Spec::Incomplete };
    // `%1$d` is a positional argument (not modelled); bytes >= 0x80
    // are printed by clang as UTF-8 or `<XX>` (not modelled).
    let positional = c == b'$' && conv_at > 0 && s[conv_at - 1].is_ascii_digit();
    let invalid = INVALID_CONVERSIONS.contains(&c)
        || c.is_ascii_control()
        || (b"\"\\$".contains(&c) && !positional);
    if invalid {
        *i += 1;
        return if star_offs.is_empty() { Spec::Invalid(conv_at) } else { Spec::Unmodelled };
    }
    if other_len {
        return Spec::Unmodelled;
    }
    let want = match c {
        b'd' | b'i' => Want::Int { class, unsigned: false, size },
        b'u' | b'o' | b'x' | b'X' => Want::Int { class, unsigned: true, size },
        b'c' if class == 0 => Want::Int { class, unsigned: false, size },
        b's' if class == 0 => Want::CStr,
        b'p' if class == 0 => Want::VoidPtr,
        b'f' | b'F' | b'e' | b'E' | b'g' | b'G' | b'a' | b'A' if class == 0 => Want::Double,
        _ => return Spec::Unmodelled,
    };
    *i += 1;
    Spec::Conv(star_offs, conv_at, want)
}

/// WP-1073: what a `scanf` conversion's argument must point to (clang's
/// `ArgType` of a `ScanfSpecifier`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ScanWant {
    /// An integer of length `len` (-2 `hh`, -1 `h`, 0, 1 `l`, 2 `ll`;
    /// `size`: `z`, whose mismatches are not modelled); `unsigned` and
    /// `n` (`%n`) only pick the spelling.
    Int { len: i8, unsigned: bool, n: bool, size: bool },
    /// `float` (0), `double` (1, `l`) or `long double` (2, `L`).
    Float(u8),
    /// `char` (`c`, `s`, `[`): any character type.
    Char,
    /// `void *` (`p`): any pointer.
    VoidPtrPtr,
}

/// clang's spelling of the pointer type a `scanf` conversion specifies.
fn scan_want_name(w: ScanWant) -> String {
    let base = match w {
        ScanWant::Int { len: -2, n: true, .. } => "signed char",
        ScanWant::Int { len, unsigned, .. } => {
            let i = usize::try_from(len + 2).unwrap_or(2).min(4);
            if unsigned {
                ["unsigned char", "unsigned short", "unsigned int", "unsigned long", "unsigned long long"]
                    [i]
            } else {
                ["char", "short", "int", "long", "long long"][i]
            }
        }
        ScanWant::Float(k) => ["float", "double", "long double"][usize::from(k.min(2))],
        ScanWant::Char => "char",
        ScanWant::VoidPtrPtr => return "'void **'".to_owned(),
    };
    format!("'{base} *'")
}

/// What [`parse_scan`] found after a `%`.
enum Scan {
    /// A conversion at this offset: what its argument must point to, or
    /// `None` when assignment is suppressed (`%*d`, no argument).
    Conv(usize, Option<ScanWant>),
    /// The string ends inside the specification.
    Incomplete,
    /// A conversion character clang rejects, at this offset.
    Invalid(usize),
    /// Anything else not modelled.
    Unmodelled,
}

/// The printable conversion characters clang 19/22 reject in a `scanf`
/// format besides a non-positional `$` (the others are conversions,
/// lengths or digits; `"` and `\` are not modelled).
const SCAN_INVALID: &[u8] = b" !#&'()*+,-./:;<=>?@BDHJKMNOPQRTUVWYZ]^_`krvy{|}~";

/// Parses the `*`, width, length and conversion of the `scanf`
/// specification after a `%` at `*i`, leaving `*i` after the conversion.
fn parse_scan(s: &[u8], i: &mut usize) -> Scan {
    let suppress = s.get(*i) == Some(&b'*');
    if suppress {
        *i += 1;
    }
    if s.get(*i) == Some(&b'0') {
        // clang: "zero field width in scanf format string is unused".
        return Scan::Unmodelled;
    }
    *i += s[*i..].iter().take_while(|c| c.is_ascii_digit()).count();
    let (len, size, other) = match (s.get(*i), s.get(*i + 1)) {
        (Some(b'h'), Some(b'h')) => (-2i8, false, false),
        (Some(b'h'), _) => (-1, false, false),
        (Some(b'l'), Some(b'l')) => (2, false, false),
        (Some(b'l'), _) => (1, false, false),
        (Some(b'z'), _) => (1, true, false),
        (Some(b'L'), _) => (3, false, false),
        (Some(c), _) if b"jtqmIw".contains(c) => (0, false, true),
        _ => (0, false, false),
    };
    *i += match len {
        -2 | 2 => 2,
        0 if !other => 0,
        _ => 1,
    };
    let has_len = len != 0 || other;
    let conv_at = *i;
    let Some(&c) = s.get(*i) else { return Scan::Incomplete };
    if other {
        return Scan::Unmodelled;
    }
    let positional = c == b'$' && conv_at > 0 && s[conv_at - 1].is_ascii_digit();
    if (SCAN_INVALID.contains(&c) || (c == b'$' && !positional)) && !has_len {
        *i += 1;
        return Scan::Invalid(conv_at);
    }
    let int = |unsigned: bool, n: bool| match len {
        3 => None,
        len => Some(ScanWant::Int { len, unsigned, n, size }),
    };
    let want = match c {
        b'd' | b'i' => int(false, false),
        b'u' | b'o' | b'x' | b'X' | b'b' => int(true, false),
        b'n' if !suppress => int(false, true),
        b'c' | b's' if !has_len => Some(ScanWant::Char),
        b'[' if !has_len => {
            // The scanset: `]` right after `[` or `[^` is a member.
            let mut j = *i + 1;
            if s.get(j) == Some(&b'^') {
                j += 1;
            }
            if s.get(j) == Some(&b']') {
                j += 1;
            }
            match s[j..].iter().position(|&c| c == b']') {
                Some(k) => *i = j + k,
                None => return Scan::Unmodelled,
            }
            Some(ScanWant::Char)
        }
        b'p' if !has_len => Some(ScanWant::VoidPtrPtr),
        b'f' | b'F' | b'e' | b'E' | b'g' | b'G' | b'a' | b'A' => match len {
            0 => Some(ScanWant::Float(0)),
            1 if !size => Some(ScanWant::Float(1)),
            3 => Some(ScanWant::Float(2)),
            _ => None,
        },
        _ => None,
    };
    let Some(want) = want else { return Scan::Unmodelled };
    *i += 1;
    Scan::Conv(conv_at, (!suppress).then_some(want))
}
