//! Pragmas and module directives the preprocessor passes to the parser
//! (spec/parse.md §2.3; clang's `ParsePragma.cpp`).
//!
//! The preprocessor handles `once`, `push_macro`, `poison`,
//! `system_header`, `message`, `GCC warning` and friends itself and hands
//! the rest over as `Marker::Pragma`. Like clang's pragma handlers,
//! [`classify`] checks a pragma's syntax when the token source reads it
//! (so its warnings come in clang's order) and turns the ones with a
//! semantic effect into an [`Event`] the parser acts on when it reaches
//! the next token (clang's annotation tokens). Unknown pragmas are
//! ignored with `-Wunknown-pragmas` ([cpp.pragma]/1: "Any pragma that is
//! not recognized by the implementation is ignored").

use kiln_diag::{Diag, DiagEngine, DiagId};
use kiln_lex::{LexOptions, Punct, TokKind, Token};
use kiln_source::symbol::Interner;
use kiln_source::{Loc, SourceManager, Span};

use crate::parser::Parser;
use crate::tokens::Event;

/// Pragmas clang knows and kiln accepts without effect on the
/// translation unit (codegen hints and `#pragma comment`).
const IGNORED: &[&str] = &[
    "comment",
    "float_control",
    "unused",
    "omp",
    "unroll",
    "nounroll",
    "unroll_and_jam",
    "nounroll_and_jam",
];

/// Pragmas clang knows that change the meaning of the translation unit
/// and that kiln does not implement yet.
const UNSUPPORTED: &[&str] = &["align", "options", "ms_struct", "weak", "redefine_extname"];

/// `#pragma clang ...` pragmas accepted without effect.
const CLANG_IGNORED: &[&str] =
    &["diagnostic", "loop", "fp", "optimize", "max_tokens_here", "max_tokens_total", "__debug"];

/// `#pragma clang ...` pragmas kiln does not implement yet.
const CLANG_UNSUPPORTED: &[&str] = &["attribute", "section"];

/// The `#pragma STDC` pragmas ([cpp.pragma]/2; C23 7.12.2, 7.6.1,
/// 7.3.4, 7.6.2).
const STDC: &[&str] = &["FP_CONTRACT", "FENV_ACCESS", "CX_LIMITED_RANGE", "FENV_ROUND"];

/// The visibilities of `#pragma GCC visibility push(...)`.
const VISIBILITIES: &[&str] = &["default", "hidden", "internal", "protected"];

/// What [`classify`] reads pragmas with.
pub(crate) struct PragmaCx<'x, 'sm> {
    pub diags: &'x DiagEngine<'sm>,
    pub interner: &'x Interner,
    pub sm: &'sm SourceManager,
    pub lex_opts: &'x LexOptions,
}

impl PragmaCx<'_, '_> {
    /// The identifier spelling of `t`, if it is an identifier (keywords
    /// included).
    fn word(&self, t: Option<&Token>) -> Option<&str> {
        match t?.kind {
            TokKind::Identifier(s) => Some(self.interner.as_str(s)),
            _ => None,
        }
    }

    fn report(&self, id: DiagId, loc: Loc) {
        Diag::new(id, Span::empty(loc)).emit(self.diags);
    }

    fn report_what(&self, id: DiagId, what: &str, loc: Loc) {
        Diag::new(id, Span::empty(loc)).arg(what).emit(self.diags);
    }
}

/// Checks the pragma whose `pragma` / `_Pragma` token is at `span` and
/// whose tokens are `tokens`, reporting syntax warnings and unknown
/// pragmas; returns the event the parser acts on, if any.
pub(crate) fn classify(cx: &PragmaCx<'_, '_>, span: Span, tokens: &[Token]) -> Option<Event> {
    // Where clang reports a missing token: the end of the last one.
    let end = tokens.last().map_or(span.hi, |t| t.span.hi);
    let first = tokens.first();
    let Some(name) = cx.word(first) else {
        cx.report(DiagId::pp_unknown_pragma, first.map_or(end, |t| t.span.lo));
        return None;
    };
    let loc = tokens[0].span.lo;
    let rest = &tokens[1..];
    match name {
        "pack" => pack(cx, loc, rest, end),
        "GCC" | "clang" => {
            let second = rest.first();
            let Some(sub) = cx.word(second) else {
                cx.report(DiagId::pp_unknown_pragma, second.map_or(end, |t| t.span.lo));
                return None;
            };
            let loc = rest[0].span.lo;
            match (name, sub) {
                ("GCC", "visibility") => visibility(cx, loc, &rest[1..], end),
                // Validated by the preprocessor; the warning state is not
                // applied per location yet (spec/parse.md §2.3).
                ("GCC", "diagnostic") => None,
                ("clang", s) if CLANG_IGNORED.contains(&s) => None,
                ("clang", s) if CLANG_UNSUPPORTED.contains(&s) => {
                    Some(Event::UnsupportedPragma { what: format!("clang {s}"), loc })
                }
                _ => {
                    cx.report(DiagId::pp_unknown_pragma, loc);
                    None
                }
            }
        }
        "STDC" => {
            if !cx.word(rest.first()).is_some_and(|s| STDC.contains(&s)) {
                let at = rest.first().map_or(end, |t| t.span.lo);
                cx.report(DiagId::parse_pragma_stdc_unknown, at);
            }
            None
        }
        s if IGNORED.contains(&s) => None,
        s if UNSUPPORTED.contains(&s) => Some(Event::UnsupportedPragma { what: s.to_owned(), loc }),
        _ => {
            cx.report(DiagId::pp_unknown_pragma, loc);
            None
        }
    }
}

/// `#pragma pack ( ... )` (clang's `PragmaPackHandler`); `loc` is the
/// `pack` token.
fn pack(cx: &PragmaCx<'_, '_>, loc: Loc, rest: &[Token], end: Loc) -> Option<Event> {
    if rest.first().is_none_or(|t| t.kind != TokKind::Punct(Punct::LParen)) {
        let at = rest.first().map_or(end, |t| t.span.lo);
        cx.report_what(DiagId::parse_pragma_expected_lparen, "pack", at);
        return None;
    }
    let mut args = String::new();
    for t in &rest[1..] {
        if t.kind == TokKind::Punct(Punct::RParen) {
            break;
        }
        args.push_str(&kiln_lex::spelling(cx.sm, t, cx.lex_opts));
    }
    Some(Event::PragmaPack { loc, args })
}

/// `#pragma GCC visibility push(v)` / `pop` (clang's
/// `PragmaGCCVisibilityHandler`); `loc` is the `visibility` token.
fn visibility(cx: &PragmaCx<'_, '_>, loc: Loc, rest: &[Token], end: Loc) -> Option<Event> {
    const WHAT: &str = "visibility";
    let at = |i: usize| rest.get(i).map_or(end, |t| t.span.lo);
    let is = |i: usize, p: Punct| rest.get(i).is_some_and(|t| t.kind == TokKind::Punct(p));
    let (push, after) = match cx.word(rest.first()) {
        Some("push") => {
            if !is(1, Punct::LParen) {
                cx.report_what(DiagId::parse_pragma_expected_lparen, WHAT, at(1));
                return None;
            }
            let Some(v) = cx.word(rest.get(2)) else {
                cx.report_what(DiagId::parse_pragma_expected_identifier, WHAT, at(2));
                return None;
            };
            if !is(3, Punct::RParen) {
                cx.report_what(DiagId::parse_pragma_expected_rparen, WHAT, at(3));
                return None;
            }
            (Some(v.to_owned()), 4)
        }
        Some("pop") => (None, 1),
        _ => {
            cx.report_what(DiagId::parse_pragma_expected_identifier, WHAT, at(0));
            return None;
        }
    };
    if let Some(extra) = rest.get(after) {
        cx.report_what(DiagId::parse_pragma_extra_tokens, WHAT, extra.span.lo);
        return None;
    }
    Some(Event::Visibility { loc, push })
}

impl Parser<'_, '_> {
    /// Acts on a preprocessor event (clang's annotation-token handlers).
    pub(crate) fn on_event(&mut self, e: Event) {
        match e {
            Event::Module { span, import } => {
                // [cpp.module], [cpp.import]: clang parses these; kiln
                // does not implement modules yet.
                let what = if import { "a module import" } else { "a module declaration" };
                self.unsupported(what, span.lo);
            }
            Event::PragmaPack { loc, args } => self.s.act_on_pragma_pack(loc, args),
            Event::UnsupportedPragma { what, loc } => self.s.act_on_unsupported_pragma(&what, loc),
            // clang's `ActOnPragmaVisibility`. The visibility itself is
            // not applied yet: everything has default visibility.
            Event::Visibility { loc, push: Some(v) } => {
                if VISIBILITIES.contains(&v.as_str()) {
                    self.visibility_depth += 1;
                } else {
                    let d = self.s.diag(DiagId::sema_unknown_visibility, loc).arg(v);
                    self.emit(d);
                }
            }
            Event::Visibility { loc, push: None } => {
                if self.visibility_depth == 0 {
                    let d = self.s.diag(DiagId::sema_visibility_pop_mismatch, loc);
                    self.emit(d);
                } else {
                    self.visibility_depth -= 1;
                }
            }
        }
    }
}
