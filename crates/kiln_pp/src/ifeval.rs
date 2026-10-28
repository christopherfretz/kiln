//! The `#if` / `#elif` expression evaluator ([cpp.cond]): a port of
//! clang 19's `PPExpressions.cpp`.
//!
//! Values are 64-bit `intmax_t` / `uintmax_t` ([cpp.cond]/12) with clang's
//! signedness rules, short-circuit liveness (errors such as division by
//! zero, `-Wundef` and overflow warnings are only reported for evaluated
//! operands), and clang's diagnostics with clang's wording and locations.
//! The first error ends the evaluation: the expression is `false` and the
//! core discards the rest of the line.
//!
//! See `spec/pp.md` §"Conditional inclusion" for the normative rules and
//! the documented deviations from clang.

use kiln_diag::{Diag, DiagId};
use kiln_lex::literal::{
    EncodingParams, IntSize, LitDiag, LiteralOptions, NumberClass, Radix, classify_number,
    decode_char,
};
use kiln_lex::{Encoding, LexDiagKind, LexOptions, Punct, Std, Symbol, TokKind, Token};
use kiln_source::Span;

use crate::builtin::OPERATOR_NAMES;
use crate::cond::{IfContext, IfEvaluator};

/// clang's `#if` evaluator ([cpp.cond]).
///
/// The target matters only for character literals (signedness of `char`
/// and `wchar_t`, [cpp.cond]/12): construct it with the target's
/// [`EncodingParams`], or with [`IfEval::for_triple`].
///
/// ```
/// use kiln_pp::IfEval;
/// use kiln_lex::literal::EncodingParams;
/// let ev = IfEval::for_triple("aarch64-unknown-linux-gnu");
/// assert_eq!(ev.params(), EncodingParams::LINUX_AARCH64);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IfEval {
    params: EncodingParams,
}

impl IfEval {
    /// An evaluator for a target with the character types `params`.
    pub const fn new(params: EncodingParams) -> Self {
        IfEval { params }
    }

    /// An evaluator for the target `triple`: `aarch64*`/`arm64*` Linux
    /// gets [`EncodingParams::LINUX_AARCH64`], `*-windows-*` gets
    /// [`EncodingParams::WINDOWS`], anything else
    /// [`EncodingParams::LINUX_X86_64`].
    pub fn for_triple(triple: &str) -> Self {
        let arch = triple.split('-').next().unwrap_or("");
        let params = if triple.contains("-windows") || triple.contains("-win32") {
            EncodingParams::WINDOWS
        } else if arch.starts_with("aarch64") || arch.starts_with("arm64") {
            EncodingParams::LINUX_AARCH64
        } else {
            EncodingParams::LINUX_X86_64
        };
        IfEval { params }
    }

    /// The target's character types.
    pub const fn params(&self) -> EncodingParams {
        self.params
    }
}

impl IfEvaluator for IfEval {
    fn eval(&mut self, cx: &mut dyn IfContext) -> bool {
        let mut ev = Ev::new(cx, self.params);
        ev.expression().is_ok_and(|v| v.v != 0)
    }

    fn eval_value(&mut self, cx: &mut dyn IfContext) -> Option<i64> {
        let mut ev = Ev::new(cx, self.params);
        let v = ev.value(true).ok()?;
        let v = if ev.at_end() { v } else { ev.sub_expr(v, PREC_QUESTION, true).ok()? };
        Some(v.v as i64)
    }
}

/// A preprocessor value: 64 bits, signed (`intmax_t`) or unsigned
/// (`uintmax_t`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Val {
    v: u64,
    unsigned: bool,
}

impl Val {
    const fn signed(v: u64) -> Self {
        Val { v, unsigned: false }
    }

    const fn is_negative(self) -> bool {
        (self.v as i64) < 0
    }
}

/// An error was reported; evaluation stops.
#[derive(Debug)]
struct Abort;

type R<T> = Result<T, Abort>;

/// clang `getPrecedence`; `None` for tokens that are not operators.
const PREC_QUESTION: u8 = 4;
const PREC_COMMA: u8 = 3;

fn precedence(p: Option<Punct>, end: bool) -> Option<u8> {
    if end {
        return Some(0);
    }
    Some(match p? {
        Punct::RParen => 0,
        Punct::Colon => 2,
        Punct::Comma => PREC_COMMA,
        Punct::Question => PREC_QUESTION,
        Punct::PipePipe => 5,
        Punct::AmpAmp => 6,
        Punct::Pipe => 7,
        Punct::Caret => 8,
        Punct::Amp => 9,
        Punct::EqualEqual | Punct::ExclaimEqual => 10,
        Punct::Less | Punct::Greater | Punct::LessEqual | Punct::GreaterEqual => 11,
        Punct::LessLess | Punct::GreaterGreater => 12,
        Punct::Plus | Punct::Minus => 13,
        Punct::Star | Punct::Slash | Punct::Percent => 14,
        _ => return None,
    })
}

struct Ev<'c> {
    cx: &'c mut dyn IfContext,
    tok: Token,
    params: EncodingParams,
    std: Std,
    defined: Symbol,
    true_: Symbol,
    false_: Symbol,
    /// The alternative tokens ([lex.digraph]) and their punctuators.
    op_names: [(Symbol, Punct); OPERATOR_NAMES.len()],
}

impl<'c> Ev<'c> {
    fn new(cx: &'c mut dyn IfContext, params: EncodingParams) -> Self {
        let i = cx.interner();
        let defined = i.intern("defined");
        let true_ = i.intern("true");
        let false_ = i.intern("false");
        let op_names = std::array::from_fn(|k| {
            let (name, spelling) = OPERATOR_NAMES[k];
            let p = Punct::ALL.iter().copied().find(|p| p.as_str() == spelling);
            (i.intern(name), p.unwrap_or(Punct::Amp))
        });
        let std = cx.std();
        let tok = cx.next();
        Ev { cx, tok, params, std, defined, true_, false_, op_names }
    }

    fn bump(&mut self) {
        self.tok = self.cx.next();
    }

    fn at_end(&self) -> bool {
        matches!(self.tok.kind, TokKind::Eod | TokKind::Eof)
    }

    /// The operator name `sym` stands for, if it is one ([lex.digraph]/2).
    fn op_name(&self, sym: Symbol) -> Option<Punct> {
        self.op_names.iter().find(|(s, _)| *s == sym).map(|&(_, p)| p)
    }

    /// The punctuator the current token is (or stands for).
    fn punct(&self) -> Option<Punct> {
        match self.tok.kind {
            TokKind::Punct(p) => Some(p),
            TokKind::Identifier(s) => self.op_name(s),
            _ => None,
        }
    }

    fn prec(&self) -> Option<u8> {
        precedence(self.punct(), self.at_end())
    }

    fn error(&mut self, id: DiagId, span: Span) -> Abort {
        self.cx.emit(Diag::new(id, span));
        Abort
    }

    /// `EvaluateDirectiveExpression`: the whole line. Tokens left after
    /// the expression (`,`, `:` and `)` stop the top-level
    /// subexpression) are an error, but the value stands, as in clang.
    fn expression(&mut self) -> R<Val> {
        let v = self.value(true)?;
        let v = if self.at_end() { v } else { self.sub_expr(v, PREC_QUESTION, true)? };
        if !self.at_end() {
            let span = self.tok.span;
            self.cx.emit(Diag::new(DiagId::pp_expected_eol_in_expr, span));
        }
        Ok(v)
    }

    /// `EvaluateValue`: a unary expression or primary.
    fn value(&mut self, live: bool) -> R<Val> {
        let tok = self.tok;
        let punct = match tok.kind {
            TokKind::Identifier(s) if s == self.defined => return self.defined(),
            TokKind::Identifier(s) => match self.op_name(s) {
                Some(p) => Some(p),
                None => return Ok(self.identifier(s, live)),
            },
            TokKind::Punct(p) => Some(p),
            TokKind::PpNumber => return self.number(live),
            TokKind::CharLit(_) => return self.char_lit(),
            TokKind::Eod | TokKind::Eof => {
                return Err(self.error(DiagId::pp_expected_value_in_expr, tok.span));
            }
            _ => None,
        };
        match punct {
            Some(Punct::RParen) => Err(self.error(DiagId::pp_expected_value_in_expr, tok.span)),
            Some(Punct::LParen) => self.paren(live),
            Some(Punct::Plus) => {
                self.bump();
                self.value(live)
            }
            Some(Punct::Minus) => {
                self.bump();
                let mut v = self.value(live)?;
                v.v = v.v.wrapping_neg();
                if !v.unsigned && v.v == i64::MIN as u64 && live {
                    self.cx.emit(Diag::new(DiagId::pp_expr_overflow, tok.span));
                }
                Ok(v)
            }
            Some(Punct::Tilde) => {
                self.bump();
                let v = self.value(live)?;
                Ok(Val { v: !v.v, unsigned: v.unsigned })
            }
            Some(Punct::Exclaim) => {
                self.bump();
                let v = self.value(live)?;
                Ok(Val::signed(u64::from(v.v == 0)))
            }
            _ => Err(self.error(DiagId::pp_invalid_token_in_if, tok.span)),
        }
    }

    /// An identifier other than `defined` and the operator names: `true`
    /// and `false`, or 0 ([cpp.cond]/13).
    fn identifier(&mut self, s: Symbol, live: bool) -> Val {
        let span = self.tok.span;
        self.bump();
        if s == self.true_ || s == self.false_ {
            return Val::signed(u64::from(s == self.true_));
        }
        if live && self.cx.is_enabled(DiagId::pp_undef_identifier) {
            let name = self.cx.interner().as_str(s).to_owned();
            self.cx.emit(Diag::new(DiagId::pp_undef_identifier, span).arg(name));
        }
        Val::signed(0)
    }

    fn paren(&mut self, live: bool) -> R<Val> {
        let open = self.tok.span;
        self.bump();
        let mut v = self.value(live)?;
        if self.punct() != Some(Punct::RParen) {
            v = self.sub_expr(v, 1, live)?;
            if self.punct() != Some(Punct::RParen) {
                let span = self.tok.span;
                self.cx.emit(
                    Diag::new(DiagId::pp_expected_rparen_in_expr, span)
                        .with_note(Diag::new(DiagId::note_matching, open).arg("'('")),
                );
                return Err(Abort);
            }
        }
        self.bump();
        Ok(v)
    }

    /// `EvaluateDefined` ([cpp.cond]/10).
    fn defined(&mut self) -> R<Val> {
        let kw = self.tok.span;
        let mut t = self.cx.next_unexpanded();
        let open = (t.kind == TokKind::Punct(Punct::LParen)).then_some(t.span);
        if open.is_some() {
            t = self.cx.next_unexpanded();
        }
        // Preprocessor::CheckMacroName(MU_Other).
        let name = match t.kind {
            TokKind::Eod | TokKind::Eof => {
                return Err(self.error(DiagId::pp_macro_name_missing, t.span));
            }
            TokKind::Identifier(name) => name,
            _ => return Err(self.error(DiagId::pp_macro_name_not_identifier, t.span)),
        };
        if let Some(p) = self.op_name(name) {
            let spelled = self.cx.interner().as_str(name).to_owned();
            self.cx.emit(
                Diag::new(DiagId::pp_operator_macro_name, t.span).arg(spelled).arg(p.as_str()),
            );
        }
        let v = Val::signed(u64::from(self.cx.is_defined(name)));
        if let Some(open) = open {
            let close = self.cx.next_unexpanded();
            if close.kind != TokKind::Punct(Punct::RParen) {
                self.cx.emit(
                    Diag::new(DiagId::pp_expected_after, close.span)
                        .arg("'defined'")
                        .arg("')'")
                        .with_note(Diag::new(DiagId::note_matching, open).arg("'('")),
                );
                return Err(Abort);
            }
        }
        self.bump();
        if let Some(info) = self.cx.source_manager().expansion_info(kw.lo) {
            // clang `isFunctionMacroExpansion`: a warning for an object-like
            // macro (easily rewritten), an extension otherwise.
            let id = match info.kind {
                kiln_source::ExpansionKind::Macro { name } if info.expansion == name => {
                    DiagId::pp_expansion_to_defined
                }
                _ => DiagId::pp_expansion_to_defined_fn,
            };
            if self.cx.is_enabled(id) {
                self.cx.emit(Diag::new(id, kw));
            }
        }
        Ok(v)
    }

    /// An integer literal ([cpp.cond]/12, [lex.icon]).
    fn number(&mut self, live: bool) -> R<Val> {
        let tok = self.tok;
        let spelling = self.cx.spelling(&tok);
        let n = classify_number(&spelling, self.std);
        // clang's `NumericLiteralParser` reads `1ui`, `1il`, ... as
        // imaginary literals where kiln sees an invalid suffix.
        let imaginary = n.diags.iter().any(|d| {
            matches!(d.kind, LexDiagKind::InvalidSuffix { .. })
                && is_imaginary_suffix(spelling.get(d.range.start as usize..d.range.end as usize))
        });
        if imaginary {
            return Err(self.error(DiagId::pp_illegal_floating_literal, tok.span));
        }
        self.lit_diags(&tok, &spelling, &n.diags, None);
        if n.has_error {
            return Err(Abort);
        }
        let lit = match n.class {
            NumberClass::Integer(lit) if !is_imaginary_suffix(lit.ud_suffix) => lit,
            NumberClass::Invalid => return Err(Abort),
            _ => return Err(self.error(DiagId::pp_illegal_floating_literal, tok.span)),
        };
        if lit.ud_suffix.is_some() {
            self.cx.emit(Diag::new(DiagId::pp_invalid_udl, tok.span).arg(1));
        }
        if lit.suffix.size == IntSize::LongLong && self.std < Std::Cxx11 {
            self.cx.emit(Diag::new(DiagId::pp_long_long_ext, tok.span));
        }
        let radix = lit.radix.value();
        let mut v: u64 = 0;
        let mut overflow = false;
        for d in lit.digits.chars().filter_map(|c| c.to_digit(radix)) {
            let (m, o1) = v.overflowing_mul(u64::from(radix));
            let (a, o2) = m.overflowing_add(u64::from(d));
            overflow |= o1 || o2;
            v = a;
        }
        let val = if overflow {
            if live {
                self.cx.emit(Diag::new(DiagId::sema_int_literal_too_large, tok.span));
            }
            Val { v, unsigned: true }
        } else if !lit.suffix.unsigned && (v as i64) < 0 {
            if live && lit.radix == Radix::Decimal {
                self.cx.emit(Diag::new(DiagId::sema_int_literal_implicitly_unsigned, tok.span));
            }
            Val { v, unsigned: true }
        } else {
            Val { v, unsigned: lit.suffix.unsigned }
        };
        self.bump();
        Ok(val)
    }

    /// A character literal ([cpp.cond]/12, [lex.ccon]).
    fn char_lit(&mut self) -> R<Val> {
        let tok = self.tok;
        let spelling = self.cx.spelling(&tok);
        let lit = decode_char(&spelling, LiteralOptions::new(self.std, self.params));
        if lit.ud_suffix.is_some() {
            self.cx.emit(Diag::new(DiagId::pp_invalid_udl, tok.span).arg(0));
        }
        self.lit_diags(&tok, &spelling, &lit.diags, Some(lit.encoding));
        // clang's `CharLiteralParser::hadError()` ignores the errors it
        // reports without setting `HadError`; the value stands.
        if lit.has_error && lit.diags.iter().any(|d| !is_soft_error(d, &spelling)) {
            return Err(Abort);
        }
        self.bump();
        Ok(Val { v: lit.pp_value as u64, unsigned: lit.pp_unsigned })
    }

    /// Reports literal diagnostics with clang's wording and locations.
    fn lit_diags(&mut self, tok: &Token, spelling: &str, diags: &[LitDiag], enc: Option<Encoding>) {
        use LexDiagKind as K;
        if diags.is_empty() {
            return;
        }
        let opts = LexOptions::new(self.std);
        let text = self.cx.source_manager().span_text(tok.span).map(str::to_owned);
        let text = text.as_deref().unwrap_or(spelling);
        for d in diags {
            let span = d.locate(tok, text, &opts).span;
            let at = Span::new(span.lo, span.lo);
            let start = Span::new(tok.span.lo, tok.span.lo);
            let end = Span::new(span.hi, span.hi);
            // The backslash of the escape sequence the range is in.
            let backslash = || {
                let b = spelling[..(d.range.start as usize).min(spelling.len())]
                    .rfind('\\')
                    .unwrap_or(0);
                let b = u32::try_from(b).unwrap_or(0);
                let l = LitDiag { kind: d.kind, range: b..b };
                let lo = l.locate(tok, text, &opts).span.lo;
                Span::new(lo, lo)
            };
            let range = spelling.get(d.range.start as usize..d.range.end as usize).unwrap_or("");
            let diag = match d.kind {
                K::InvalidDigit { ch, radix } => {
                    Diag::new(DiagId::lit_invalid_digit, at).arg(ch).arg(match radix {
                        2 => 0,
                        8 => 1,
                        _ => 2,
                    })
                }
                K::InvalidSuffix { floating } => {
                    Diag::new(DiagId::lit_invalid_suffix, at).arg(range).arg(i32::from(floating))
                }
                K::ExponentNoDigits => Diag::new(DiagId::lit_exponent_no_digits, at),
                // clang points at the end of the digits read.
                K::HexFloatNoExponent => Diag::new(DiagId::lit_hex_float_requires, end).arg(0),
                K::HexFloatNoSignificand => Diag::new(DiagId::lit_hex_float_requires, end).arg(1),
                K::DigitSeparatorPosition { at_end } => {
                    Diag::new(DiagId::lit_digit_separator, at).arg(i32::from(at_end))
                }
                K::BinaryLiteralExtension => Diag::new(DiagId::lit_binary_ext, start),
                K::HexFloatExtension => Diag::new(DiagId::lit_hex_float_ext, start),
                K::SizeSuffixExtension => Diag::new(DiagId::lit_size_t_ext, start),
                K::IntegerTooLarge => Diag::new(DiagId::sema_int_literal_too_large, start),
                K::UnknownEscape { ch } => Diag::new(DiagId::lit_unknown_escape, at).arg(ch),
                K::NonstandardEscape { ch } => {
                    Diag::new(DiagId::lit_nonstandard_escape, at).arg(ch)
                }
                K::EscapeNoDigits { esc } => Diag::new(DiagId::lit_escape_no_digits, at).arg(esc),
                K::EscapeMissingBrace { esc } => {
                    Diag::new(DiagId::lit_escape_missing_brace, at).arg(esc)
                }
                K::DelimitedEscapeEmpty => Diag::new(DiagId::lit_delimited_escape_empty, at),
                K::DelimitedEscapeUnterminated => Diag::new(DiagId::pp_expected, at).arg("'}'"),
                K::DelimitedEscapeInvalidDigit { ch } => {
                    Diag::new(DiagId::lit_delimited_escape_invalid, backslash()).arg(ch)
                }
                K::DelimitedEscapeExtension | K::UcnDelimitedExtension => {
                    Diag::new(DiagId::lex_delimited_escape_ext, at).arg(0)
                }
                K::UcnNamedExtension => Diag::new(DiagId::lex_delimited_escape_ext, at).arg(1),
                K::HexEscapeTooLarge => Diag::new(DiagId::lit_hex_escape_too_large, at),
                K::OctalEscapeTooLarge => Diag::new(DiagId::lit_octal_escape_too_large, at),
                K::UcnEscapeIncomplete => Diag::new(DiagId::lit_ucn_escape_incomplete, at),
                K::UcnUnknownName => {
                    Diag::new(DiagId::lex_ucn_unknown_name, at).arg(ucn_name(range))
                }
                K::UcnNameLooseMatch { .. } => Diag::new(DiagId::lex_ucn_unknown_name, at)
                    .arg(ucn_name(range))
                    .with_note(Diag::new(DiagId::note_ucn_name_loose_match, at)),
                K::CharTooLarge => Diag::new(DiagId::lit_char_too_large, start),
                K::MultiCharLiteral => Diag::new(DiagId::lit_multichar, start),
                K::FourCharLiteral => Diag::new(DiagId::lit_four_char, start),
                K::CharConstantTooLong => Diag::new(DiagId::lit_char_constant_too_long, start),
                K::MultiCharNotAllowed => Diag::new(DiagId::lit_multichar_not_allowed, start)
                    .arg(i32::from(enc != Some(Encoding::Wide))),
                _ => {
                    let lex = kiln_lex::LexDiag { kind: d.kind, span };
                    match crate::diag_map::lex_diag(self.cx.source_manager(), &lex) {
                        Some(diag) => diag,
                        None => continue,
                    }
                }
            };
            self.cx.emit(diag);
        }
    }

    /// `EvaluateDirectiveSubExpr`: operator-precedence parsing of the
    /// binary operators of precedence `min` and above, `lhs` already read.
    fn sub_expr(&mut self, mut lhs: Val, min: u8, live: bool) -> R<Val> {
        let Some(mut peek) = self.prec() else {
            let span = self.tok.span;
            return Err(self.error(DiagId::pp_expr_bad_token_binop, span));
        };
        loop {
            if peek < min {
                return Ok(lhs);
            }
            let op = self.punct().unwrap_or(Punct::Comma);
            let op_span = self.tok.span;
            let rhs_live = match op {
                Punct::AmpAmp | Punct::Question if lhs.v == 0 => false,
                Punct::PipePipe if lhs.v != 0 => false,
                _ => live,
            };
            self.bump();
            let mut rhs = self.value(rhs_live)?;
            let this = peek;
            let Some(p) = self.prec() else {
                let span = self.tok.span;
                return Err(self.error(DiagId::pp_expr_bad_token_binop, span));
            };
            peek = p;
            let rhs_prec = if op == Punct::Question { PREC_COMMA } else { this + 1 };
            if peek >= rhs_prec {
                rhs = self.sub_expr(rhs, rhs_prec, rhs_live)?;
                peek = self.prec().unwrap_or(0);
            }
            lhs = self.binary(op, op_span, lhs, rhs, this, live, &mut peek)?;
        }
    }

    /// Applies the binary operator `op` (the body of clang's
    /// `EvaluateDirectiveSubExpr` loop).
    // A line-for-line port of clang's EvaluateDirectiveSubExpr operator
    // switch; splitting it would obscure the correspondence.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn binary(
        &mut self,
        op: Punct,
        op_span: Span,
        mut lhs: Val,
        mut rhs: Val,
        this: u8,
        live: bool,
        peek: &mut u8,
    ) -> R<Val> {
        // Usual arithmetic conversions ([expr.arith.conv]): not for the
        // operands of `?`, shifts, `,`, `&&`, `||`.
        let uac = !matches!(
            op,
            Punct::Question
                | Punct::LessLess
                | Punct::GreaterGreater
                | Punct::Comma
                | Punct::PipePipe
                | Punct::AmpAmp
        );
        let mut res_unsigned = true;
        if uac {
            res_unsigned = lhs.unsigned || rhs.unsigned;
            if live && res_unsigned {
                for (side, v) in [(0, lhs), (1, rhs)] {
                    if !v.unsigned && v.is_negative() {
                        self.cx.emit(
                            Diag::new(DiagId::pp_convert_to_positive, op_span)
                                .arg(side)
                                .arg(format!("{} to {}", v.v as i64, v.v)),
                        );
                    }
                }
            }
            lhs.unsigned = res_unsigned;
            rhs.unsigned = res_unsigned;
        }
        let (l, r) = (lhs.v, rhs.v);
        let (sl, sr) = (l as i64, r as i64);
        let mut overflow = false;
        let res = match op {
            Punct::Percent | Punct::Slash => {
                if r != 0 {
                    let v = if lhs.unsigned {
                        if op == Punct::Slash { l.checked_div(r) } else { l.checked_rem(r) }
                            .unwrap_or(0)
                    } else if op == Punct::Slash {
                        let (q, o) = sl.overflowing_div(sr);
                        overflow = o;
                        q as u64
                    } else {
                        sl.wrapping_rem(sr) as u64
                    };
                    Val { v, unsigned: lhs.unsigned }
                } else if live {
                    self.cx.emit(
                        Diag::new(DiagId::pp_division_by_zero, op_span)
                            .arg(i32::from(op == Punct::Percent)),
                    );
                    return Err(Abort);
                } else {
                    Val { v: 0, unsigned: res_unsigned }
                }
            }
            Punct::Star => {
                if res_unsigned {
                    Val { v: l.wrapping_mul(r), unsigned: true }
                } else {
                    let (v, o) = sl.overflowing_mul(sr);
                    overflow = o;
                    Val::signed(v as u64)
                }
            }
            Punct::LessLess => {
                let v = if r >= 64 {
                    overflow = true;
                    0
                } else {
                    if lhs.unsigned {
                        overflow = r > u64::from(l.leading_zeros());
                    } else if sl >= 0 {
                        overflow = r >= u64::from(l.leading_zeros());
                    } else {
                        overflow = r >= u64::from(l.leading_ones());
                    }
                    l << r
                };
                Val { v, unsigned: lhs.unsigned }
            }
            Punct::GreaterGreater => {
                let mut amt = r;
                if amt >= 64 {
                    overflow = true;
                    amt = 63;
                }
                let v = if lhs.unsigned { l >> amt } else { (sl >> amt) as u64 };
                Val { v, unsigned: lhs.unsigned }
            }
            Punct::Plus | Punct::Minus => {
                if lhs.unsigned {
                    let v = if op == Punct::Plus { l.wrapping_add(r) } else { l.wrapping_sub(r) };
                    Val { v, unsigned: true }
                } else {
                    let (v, o) = if op == Punct::Plus {
                        sl.overflowing_add(sr)
                    } else {
                        sl.overflowing_sub(sr)
                    };
                    overflow = o;
                    Val::signed(v as u64)
                }
            }
            Punct::Less | Punct::Greater | Punct::LessEqual | Punct::GreaterEqual => {
                let ord = if res_unsigned { l.cmp(&r) } else { sl.cmp(&sr) };
                let b = match op {
                    Punct::Less => ord.is_lt(),
                    Punct::Greater => ord.is_gt(),
                    Punct::LessEqual => ord.is_le(),
                    _ => ord.is_ge(),
                };
                Val::signed(u64::from(b))
            }
            Punct::EqualEqual => Val::signed(u64::from(l == r)),
            Punct::ExclaimEqual => Val::signed(u64::from(l != r)),
            Punct::Amp => Val { v: l & r, unsigned: res_unsigned },
            Punct::Caret => Val { v: l ^ r, unsigned: res_unsigned },
            Punct::Pipe => Val { v: l | r, unsigned: res_unsigned },
            Punct::AmpAmp => Val::signed(u64::from(l != 0 && r != 0)),
            Punct::PipePipe => Val::signed(u64::from(l != 0 || r != 0)),
            Punct::Comma => {
                // Always diagnosed in C++ (clang: `!C99 || ValueLive`).
                self.cx.emit(Diag::new(DiagId::pp_comma_expr, op_span));
                rhs
            }
            Punct::Question => {
                if self.punct() != Some(Punct::Colon) {
                    let span = self.tok.span;
                    self.cx.emit(
                        Diag::new(DiagId::pp_expected, span)
                            .arg("':'")
                            .with_note(Diag::new(DiagId::note_matching, op_span).arg("'?'")),
                    );
                    return Err(Abort);
                }
                self.bump();
                let else_live = live && l == 0;
                let e = self.value(else_live)?;
                let e = self.sub_expr(e, this, else_live)?;
                *peek = self.prec().unwrap_or(0);
                let v = if l != 0 { rhs.v } else { e.v };
                Val { v, unsigned: rhs.unsigned || e.unsigned }
            }
            Punct::Colon => return Err(self.error(DiagId::pp_colon_without_question, op_span)),
            _ => unreachable!("precedence() admits only binary operators"),
        };
        if overflow && live {
            self.cx.emit(Diag::new(DiagId::pp_expr_overflow, op_span));
        }
        Ok(res)
    }
}

/// Whether a ud-suffix makes clang's `NumericLiteralParser` read the
/// literal as imaginary (`1i`, `1il`, `1ui`, ...): only `u`/`l`/`z` and
/// exactly one `i`/`j`, or the standard ud-suffix `if`.
fn is_imaginary_suffix(s: Option<&str>) -> bool {
    let Some(s) = s else { return false };
    if s == "if" {
        return true;
    }
    let im = s.bytes().filter(|b| matches!(b, b'i' | b'I' | b'j' | b'J')).count();
    im == 1 && s.bytes().all(|b| b"iIjJuUlLzZ".contains(&b))
}

/// The name in `\N{name}` (or the whole range if it has no braces).
fn ucn_name(range: &str) -> &str {
    match range.find('{') {
        Some(i) => range[i + 1..].split('}').next().unwrap_or(""),
        None => range,
    }
}

/// Whether `d` is an error that clang's `CharLiteralParser` reports
/// without setting `HadError`: `\x` without digits, an empty or
/// unterminated `\x{}` / `\o{}`, a too-large plain octal escape.
fn is_soft_error(d: &LitDiag, spelling: &str) -> bool {
    use kiln_lex::LexSeverity;
    if d.kind.severity() != LexSeverity::Error {
        return true;
    }
    let esc = spelling.get(d.range.start as usize + 1..).and_then(|r| r.chars().next());
    match d.kind {
        LexDiagKind::EscapeNoDigits { esc } => esc == 'x',
        LexDiagKind::DelimitedEscapeEmpty | LexDiagKind::DelimitedEscapeUnterminated => {
            matches!(esc, Some('x' | 'o'))
        }
        LexDiagKind::OctalEscapeTooLarge => esc != Some('o'),
        _ => false,
    }
}
