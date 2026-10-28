//! Conditional inclusion ([cpp.cond]): the `#if` expression hook.
//!
//! The core parses the directive structure (`#if` .. `#endif` nesting,
//! skipping of excluded groups) itself. The *value* of a `#if` / `#elif`
//! expression comes from an [`IfEvaluator`]; the full evaluator
//! ([expr.const] integer arithmetic, `__has_*`, diagnostics with clang
//! wording) is WP-015. [`StubIfEvaluator`] covers the common subset.

use kiln_diag::{Diag, DiagId};
use kiln_lex::{Interner, Punct, Std, Symbol, TokKind, Token};
use kiln_source::{ExpansionKind, SourceManager, Span};

/// What an [`IfEvaluator`] can do with the directive it evaluates.
///
/// Tokens end with `Eod` (repeated if read again). The core discards
/// whatever the evaluator leaves unread on the line.
pub trait IfContext {
    /// The next token, macro-expanded ([cpp.cond]/13). Builtins such as
    /// `__has_include(...)` are already replaced by their values.
    fn next(&mut self) -> Token;
    /// The next token without macro expansion (for the operand of
    /// `defined`, [cpp.cond]/10).
    fn next_unexpanded(&mut self) -> Token;
    /// `defined name` (marks the macro used).
    fn is_defined(&mut self, name: Symbol) -> bool;
    /// The spelling of a token (cleaned of line splices).
    fn spelling(&self, tok: &Token) -> String;
    /// The interner the token symbols belong to.
    fn interner(&self) -> &Interner;
    /// The source manager the token spans belong to.
    fn source_manager(&self) -> &SourceManager;
    /// Emits a diagnostic.
    fn emit(&mut self, diag: Diag);
    /// True if `id` would be reported (to skip building a diagnostic).
    fn is_enabled(&self, id: DiagId) -> bool;
    /// The language standard.
    fn std(&self) -> Std;
}

/// Evaluates the controlling expression of `#if` / `#elif`.
pub trait IfEvaluator {
    /// Reads the expression from `cx` and returns whether the group is
    /// included. Errors are reported through `cx`; an erroneous
    /// expression evaluates to `false`.
    fn eval(&mut self, cx: &mut dyn IfContext) -> bool;

    /// Evaluates an integer constant expression in preprocessor arithmetic
    /// for an `#embed` / `__has_embed` `limit(...)` / `clang::offset(...)`
    /// parameter (C23 6.10.4.2). The tokens end with `Eod` at the closing
    /// parenthesis. Returns `None` after reporting an error. The default
    /// uses the [`StubIfEvaluator`] grammar; WP-015 overrides it.
    fn eval_value(&mut self, cx: &mut dyn IfContext) -> Option<i64> {
        stub_value(cx)
    }
}

/// A minimal evaluator: integer literals (any base, `'` separators,
/// `u`/`l` suffixes), `true`/`false`, identifiers (0),
/// `defined X` / `defined(X)`, parentheses, unary `!` `-` `+` `~`, binary
/// `*` `/` `%` `+` `-` `<<` `>>` `<` `>` `<=` `>=` `==` `!=` `&` `^` `|`
/// `&&` `||` and `?:`, in `intmax_t` arithmetic with wrapping.
/// Anything else reports `pp_expr_unsupported` (the full evaluator is
/// WP-015).
#[derive(Clone, Copy, Debug, Default)]
pub struct StubIfEvaluator;

struct Stub<'c> {
    cx: &'c mut dyn IfContext,
    tok: Token,
    failed: bool,
    defined: Symbol,
    true_: Symbol,
}

impl IfEvaluator for StubIfEvaluator {
    fn eval(&mut self, cx: &mut dyn IfContext) -> bool {
        stub_value(cx).is_some_and(|v| v != 0)
    }
}

/// The [`StubIfEvaluator`] grammar: the value of the whole line, or `None`
/// after reporting an error.
pub(crate) fn stub_value(cx: &mut dyn IfContext) -> Option<i64> {
    {
        let defined = cx.interner().intern("defined");
        let true_ = cx.interner().intern("true");
        let tok = cx.next();
        if tok.kind == TokKind::Eod {
            cx.emit(Diag::new(DiagId::pp_expected_value_in_expr, tok.span));
            return None;
        }
        let mut s = Stub { cx, tok, failed: false, defined, true_ };
        let v = s.cond(true);
        if !s.failed && s.tok.kind != TokKind::Eod {
            let span = s.tok.span;
            s.fail(DiagId::pp_expr_bad_token_binop, span);
        }
        (!s.failed).then_some(v)
    }
}

impl Stub<'_> {
    fn bump(&mut self) {
        self.tok = self.cx.next();
    }

    fn fail(&mut self, id: DiagId, span: Span) {
        if !self.failed {
            self.failed = true;
            self.cx.emit(Diag::new(id, span));
        }
    }

    fn punct(&self) -> Option<Punct> {
        match self.tok.kind {
            TokKind::Punct(p) => Some(p),
            _ => None,
        }
    }

    /// conditional-expression; `live` is false in unevaluated operands
    /// (division by zero is not reported there).
    fn cond(&mut self, live: bool) -> i64 {
        let c = self.binary(0, live);
        if self.failed || self.punct() != Some(Punct::Question) {
            return c;
        }
        self.bump();
        let a = self.cond(live && c != 0);
        if self.failed {
            return 0;
        }
        if self.punct() != Some(Punct::Colon) {
            let span = self.tok.span;
            self.fail(DiagId::pp_expr_unsupported, span);
            return 0;
        }
        self.bump();
        let b = self.cond(live && c == 0);
        if c != 0 { a } else { b }
    }

    fn prec(p: Punct) -> Option<u8> {
        Some(match p {
            Punct::PipePipe => 1,
            Punct::AmpAmp => 2,
            Punct::Pipe => 3,
            Punct::Caret => 4,
            Punct::Amp => 5,
            Punct::EqualEqual | Punct::ExclaimEqual => 6,
            Punct::Less | Punct::Greater | Punct::LessEqual | Punct::GreaterEqual => 7,
            Punct::LessLess | Punct::GreaterGreater => 8,
            Punct::Plus | Punct::Minus => 9,
            Punct::Star | Punct::Slash | Punct::Percent => 10,
            _ => return None,
        })
    }

    fn binary(&mut self, min: u8, live: bool) -> i64 {
        let mut lhs = self.unary(live);
        loop {
            if self.failed {
                return 0;
            }
            let Some(op) = self.punct() else { return lhs };
            let Some(p) = Self::prec(op) else { return lhs };
            if p <= min {
                return lhs;
            }
            let op_span = self.tok.span;
            self.bump();
            let rlive = match op {
                Punct::AmpAmp => live && lhs != 0,
                Punct::PipePipe => live && lhs == 0,
                _ => live,
            };
            let rhs = self.binary(p, rlive);
            if self.failed {
                return 0;
            }
            lhs = match op {
                Punct::PipePipe => i64::from(lhs != 0 || rhs != 0),
                Punct::AmpAmp => i64::from(lhs != 0 && rhs != 0),
                Punct::Pipe => lhs | rhs,
                Punct::Caret => lhs ^ rhs,
                Punct::Amp => lhs & rhs,
                Punct::EqualEqual => i64::from(lhs == rhs),
                Punct::ExclaimEqual => i64::from(lhs != rhs),
                Punct::Less => i64::from(lhs < rhs),
                Punct::Greater => i64::from(lhs > rhs),
                Punct::LessEqual => i64::from(lhs <= rhs),
                Punct::GreaterEqual => i64::from(lhs >= rhs),
                Punct::LessLess => lhs.wrapping_shl(rhs as u32),
                Punct::GreaterGreater => lhs.wrapping_shr(rhs as u32),
                Punct::Plus => lhs.wrapping_add(rhs),
                Punct::Minus => lhs.wrapping_sub(rhs),
                Punct::Star => lhs.wrapping_mul(rhs),
                Punct::Slash | Punct::Percent => {
                    if rhs == 0 {
                        if live {
                            self.failed = true;
                            self.cx.emit(
                                Diag::new(DiagId::pp_division_by_zero, op_span)
                                    .arg(i32::from(op == Punct::Percent)),
                            );
                        }
                        0
                    } else if op == Punct::Slash {
                        lhs.wrapping_div(rhs)
                    } else {
                        lhs.wrapping_rem(rhs)
                    }
                }
                _ => unreachable!("not a binary operator"),
            };
        }
    }

    fn unary(&mut self, live: bool) -> i64 {
        match self.tok.kind {
            TokKind::Punct(Punct::Exclaim) => {
                self.bump();
                i64::from(self.unary(live) == 0)
            }
            TokKind::Punct(Punct::Minus) => {
                self.bump();
                self.unary(live).wrapping_neg()
            }
            TokKind::Punct(Punct::Plus) => {
                self.bump();
                self.unary(live)
            }
            TokKind::Punct(Punct::Tilde) => {
                self.bump();
                !self.unary(live)
            }
            TokKind::Punct(Punct::LParen) => {
                let open = self.tok.span;
                self.bump();
                let v = self.cond(live);
                if self.failed {
                    return 0;
                }
                if self.punct() != Some(Punct::RParen) {
                    self.failed = true;
                    self.cx.emit(
                        Diag::new(DiagId::pp_expected_rparen_in_expr, self.tok.span)
                            .with_note(Diag::new(DiagId::note_matching, open).arg("'('")),
                    );
                    return 0;
                }
                self.bump();
                v
            }
            TokKind::PpNumber => {
                let text = self.cx.spelling(&self.tok);
                let span = self.tok.span;
                self.bump();
                if let Some(v) = parse_int(&text) {
                    v
                } else {
                    self.fail(DiagId::pp_expr_unsupported, span);
                    0
                }
            }
            TokKind::Identifier(s) if s == self.defined => self.defined(),
            TokKind::Identifier(s) => {
                self.bump();
                // `false` and other identifiers ([cpp.cond]/13) are 0.
                i64::from(s == self.true_)
            }
            TokKind::Eod => {
                let span = self.tok.span;
                self.fail(DiagId::pp_expected_value_in_expr, span);
                0
            }
            _ => {
                let span = self.tok.span;
                self.fail(DiagId::pp_expr_unsupported, span);
                0
            }
        }
    }

    fn defined(&mut self) -> i64 {
        let kw = self.tok.span;
        if let Some(info) = self.cx.source_manager().expansion_info(kw.lo) {
            // clang `isFunctionMacroExpansion`: a warning for an object-like
            // macro (easily rewritten), an extension otherwise.
            let id = match info.kind {
                ExpansionKind::Macro { name } if info.expansion == name => {
                    DiagId::pp_expansion_to_defined
                }
                _ => DiagId::pp_expansion_to_defined_fn,
            };
            if self.cx.is_enabled(id) {
                self.cx.emit(Diag::new(id, kw));
            }
        }
        let mut t = self.cx.next_unexpanded();
        let paren = t.kind == TokKind::Punct(Punct::LParen);
        if paren {
            t = self.cx.next_unexpanded();
        }
        let TokKind::Identifier(name) = t.kind else {
            self.fail(DiagId::pp_defined_requires_identifier, t.span);
            return 0;
        };
        let v = i64::from(self.cx.is_defined(name));
        if paren {
            let close = self.cx.next_unexpanded();
            if close.kind != TokKind::Punct(Punct::RParen) {
                self.failed = true;
                self.cx.emit(
                    Diag::new(DiagId::pp_expected_after, close.span).arg("'defined'").arg("')'"),
                );
                return 0;
            }
        }
        self.bump();
        v
    }
}

/// Parses an integer pp-number (`0x1F`, `0b101`, `017`, `1'000`, with
/// `u`/`l`/`z` suffixes). `None` for floating literals and malformed
/// numbers.
#[allow(clippy::many_single_char_names)]
pub(crate) fn parse_int(text: &str) -> Option<i64> {
    let t = text.as_bytes();
    let (radix, mut i) = if t.len() > 1 && t[0] == b'0' && (t[1] | 0x20) == b'x' {
        (16, 2)
    } else if t.len() > 1 && t[0] == b'0' && (t[1] | 0x20) == b'b' {
        (2, 2)
    } else if t.first() == Some(&b'0') {
        (8, 1)
    } else {
        (10, 0)
    };
    let mut v: u64 = 0;
    let mut digits = radix == 8;
    while i < t.len() {
        let c = t[i];
        if c == b'\'' {
            i += 1;
            continue;
        }
        let Some(d) = (c as char).to_digit(radix) else { break };
        v = v.wrapping_mul(u64::from(radix)).wrapping_add(u64::from(d));
        digits = true;
        i += 1;
    }
    if !digits {
        return None;
    }
    let suffix = text[i..].to_ascii_lowercase();
    match suffix.as_str() {
        "" | "u" | "l" | "ul" | "lu" | "ll" | "ull" | "llu" | "z" | "uz" | "zu" => Some(v as i64),
        _ => None,
    }
}
