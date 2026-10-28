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
use kiln_lex::{Encoding, LexDiagKind, LexOptions, Punct, Std, Symbol, TokFlags, TokKind, Token};
use kiln_source::Span;

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
    /// The symbol of `defined`, once seen (symbols are equal exactly when
    /// their strings are, so later checks compare symbols).
    defined: Option<Symbol>,
}

/// The punctuator an alternative token ([lex.digraph]/2) stands for
/// (`crate::builtin::OPERATOR_NAMES`, as a match: names are compared as
/// text so that no symbol is interned per `#if`).
fn op_punct(name: &str) -> Option<Punct> {
    Some(match name {
        "and" => Punct::AmpAmp,
        "and_eq" => Punct::AmpEqual,
        "bitand" => Punct::Amp,
        "bitor" => Punct::Pipe,
        "compl" => Punct::Tilde,
        "not" => Punct::Exclaim,
        "not_eq" => Punct::ExclaimEqual,
        "or" => Punct::PipePipe,
        "or_eq" => Punct::PipeEqual,
        "xor" => Punct::Caret,
        "xor_eq" => Punct::CaretEqual,
        _ => return None,
    })
}

impl<'c> Ev<'c> {
    fn new(cx: &'c mut dyn IfContext, params: EncodingParams) -> Self {
        let std = cx.std();
        let tok = cx.next();
        Ev { cx, tok, params, std, defined: None }
    }

    fn bump(&mut self) {
        self.tok = self.cx.next();
    }

    fn at_end(&self) -> bool {
        matches!(self.tok.kind, TokKind::Eod | TokKind::Eof)
    }

    /// The operator name `sym` stands for, if it is one ([lex.digraph]/2).
    fn op_name(&self, sym: Symbol) -> Option<Punct> {
        op_punct(self.name(sym))
    }

    /// True if `sym` is `defined` ([cpp.cond]/1).
    fn is_defined_keyword(&mut self, sym: Symbol) -> bool {
        if let Some(d) = self.defined {
            return d == sym;
        }
        let yes = self.name(sym) == "defined";
        if yes {
            self.defined = Some(sym);
        }
        yes
    }

    /// The text of an identifier.
    fn name(&self, sym: Symbol) -> &str {
        self.cx.interner().as_str(sym)
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
            TokKind::Identifier(s) if self.is_defined_keyword(s) => return self.defined(),
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
        match self.name(s) {
            "true" => return Val::signed(1),
            "false" => return Val::signed(0),
            _ => {}
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
        if !tok.flags.contains(TokFlags::NEEDS_CLEANING)
            && let Some(v) = self.cx.source_manager().span_text(tok.span).and_then(plain_decimal)
        {
            self.bump();
            return Ok(v);
        }
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
        if diags.is_empty() {
            return;
        }
        let opts = LexOptions::new(self.std);
        let sm = self.cx.source_manager();
        for d in crate::diag_map::literal_diags(sm, tok, spelling, diags, enc, &opts) {
            self.cx.emit(d);
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

/// The value of a plain decimal literal such as `0`, `42`, `201103L` or
/// `1u`: no digit separators, at most 18 digits (so it cannot overflow),
/// and a suffix of at most one `u` and one `l`, which can draw no
/// diagnostic. `None` sends anything else down the full
/// [`classify_number`] path. The `#if` fast path ([cpp.cond]/12).
fn plain_decimal(text: &str) -> Option<Val> {
    let b = text.as_bytes();
    let digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 || digits > 18 || (digits > 1 && b[0] == b'0') {
        return None;
    }
    let (mut unsigned, mut long) = (false, false);
    for &c in &b[digits..] {
        match c {
            b'u' | b'U' if !unsigned => unsigned = true,
            b'l' | b'L' if !long => long = true,
            _ => return None,
        }
    }
    let v = b[..digits].iter().fold(0u64, |v, &c| v * 10 + u64::from(c - b'0'));
    Some(Val { v, unsigned })
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

#[cfg(test)]
mod tests {
    use super::{IntSize, NumberClass, Radix, Std, classify_number, op_punct, plain_decimal};
    use crate::builtin::OPERATOR_NAMES;

    /// [lex.digraph]/2: the evaluator's alternative-token table is the
    /// crate's.
    #[test]
    fn op_punct_matches_operator_names() {
        for (name, spelling) in OPERATOR_NAMES {
            assert_eq!(op_punct(name).map(kiln_lex::Punct::as_str), Some(*spelling), "{name}");
        }
        assert_eq!(op_punct("defined"), None);
        assert_eq!(op_punct("AND"), None);
    }

    /// [cpp.cond]/12, [lex.icon]: the plain-decimal fast path agrees with
    /// the full literal classifier wherever it applies, and declines
    /// everything that could draw a diagnostic or overflow.
    #[test]
    fn cpp_cond_plain_decimal_matches_classify_number() {
        let mut cases: Vec<String> = [
            "0",
            "7",
            "42",
            "201103L",
            "1u",
            "1U",
            "1l",
            "1ul",
            "1LU",
            "0u",
            "999999999999999999",
            "1000000000000000000",
            "08",
            "00",
            "0x1F",
            "1'000",
            "1LL",
            "1ull",
            "1uu",
            "1ll",
            "1i",
            "1z",
            "1.0",
            "1e3",
            "1_x",
            "18446744073709551616",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..2000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let len = 1 + (seed % 21) as usize;
            let s: String = (0..len)
                .map(|i| {
                    let pick = (seed >> (i * 3 % 60)) % 16;
                    b"0123456789uUlLx'"[pick as usize] as char
                })
                .collect();
            cases.push(s);
        }
        for text in &cases {
            let Some(fast) = plain_decimal(text) else { continue };
            let n = classify_number(text, Std::Cxx98);
            assert!(n.diags.is_empty() && !n.has_error, "{text}");
            let NumberClass::Integer(lit) = n.class else { panic!("{text}") };
            let radix = if text.starts_with('0') { Radix::Octal } else { Radix::Decimal };
            assert_eq!(lit.radix, radix, "{text}");
            assert!(lit.ud_suffix.is_none(), "{text}");
            assert_ne!(lit.suffix.size, IntSize::LongLong, "{text}");
            assert_eq!(fast.unsigned, lit.suffix.unsigned, "{text}");
            let slow = if lit.digits.is_empty() {
                0
            } else {
                u64::from_str_radix(&lit.digits, radix.value()).unwrap_or_else(|_| panic!("{text}"))
            };
            assert_eq!(fast.v, slow, "{text}");
            assert!(i64::try_from(slow).is_ok(), "{text}");
        }
        for text in ["08", "1'0", "1LL", "1uu", "0x1", "1i", "1000000000000000000", ""] {
            assert!(plain_decimal(text).is_none(), "{text}");
        }
    }
}
