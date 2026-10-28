//! Pragmas and module directives the preprocessor passes to the parser
//! (spec/parse.md §2.3; clang's `ParsePragma.cpp`).
//!
//! The preprocessor handles `once`, `push_macro`, `poison`,
//! `system_header`, `message`, `GCC warning` and friends itself and hands
//! the rest over as [`Event::Pragma`]s, which the parser handles when it
//! reaches the token after the pragma. Unknown pragmas are ignored with
//! `-Wunknown-pragmas` ([cpp.pragma]/1: "Any pragma that is not
//! recognized by the implementation is ignored").

use kiln_diag::DiagId;
use kiln_lex::{Punct, TokKind, Token};
use kiln_source::{Loc, Span};

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

/// The `#pragma STDC` pragmas ([cpp.pragma]/2, C's 7.12.2, 7.6.1, 7.3.4).
const STDC: &[&str] = &["FP_CONTRACT", "FENV_ACCESS", "CX_LIMITED_RANGE", "FENV_ROUND"];

/// The visibilities of `#pragma GCC visibility push(...)`.
const VISIBILITIES: &[&str] = &["default", "hidden", "internal", "protected"];

impl Parser<'_, '_> {
    /// Handles a preprocessor event.
    pub(crate) fn on_event(&mut self, e: Event) {
        match e {
            Event::Module { span, import } => {
                // [cpp.module], [cpp.import]: clang parses these; kiln
                // does not implement modules yet.
                let what = if import { "a module import" } else { "a module declaration" };
                self.unsupported(what, span.lo);
            }
            Event::Pragma { span, tokens } => self.on_pragma(span, &tokens),
        }
    }

    /// The identifier spelling of `t`, if it is an identifier (keywords
    /// included).
    fn pragma_word(&self, t: Option<&Token>) -> Option<&str> {
        match t?.kind {
            TokKind::Identifier(s) => Some(self.s.ast.symbols().as_str(s)),
            _ => None,
        }
    }

    /// Handles `#pragma` / `_Pragma` at `span` with `tokens`.
    fn on_pragma(&mut self, span: Span, tokens: &[Token]) {
        // Where clang reports a missing token: the end of the last one.
        let end = tokens.last().map_or(span.hi, |t| t.span.hi);
        let first = tokens.first();
        let Some(name) = self.pragma_word(first).map(str::to_owned) else {
            self.unknown_pragma(first.map_or(end, |t| t.span.lo));
            return;
        };
        let rest = &tokens[1..];
        match name.as_str() {
            "pack" => self.pragma_pack(rest, end),
            "GCC" | "clang" => {
                let second = rest.first();
                let Some(sub) = self.pragma_word(second).map(str::to_owned) else {
                    self.unknown_pragma(second.map_or(end, |t| t.span.lo));
                    return;
                };
                let loc = rest[0].span.lo;
                match (name.as_str(), sub.as_str()) {
                    ("GCC", "visibility") => self.pragma_visibility(loc, &rest[1..], end),
                    // Validated by the preprocessor; the warning state
                    // is not applied per location yet.
                    ("GCC", "diagnostic") => {}
                    ("clang", s) if CLANG_IGNORED.contains(&s) => {}
                    ("clang", s) if CLANG_UNSUPPORTED.contains(&s) => {
                        self.s.act_on_unsupported_pragma(&format!("clang {s}"), loc);
                    }
                    _ => self.unknown_pragma(loc),
                }
            }
            "STDC" => match self.pragma_word(rest.first()) {
                Some(s) if STDC.contains(&s) => {}
                _ => {
                    let at = rest.first().map_or(end, |t| t.span.lo);
                    self.error_at(DiagId::parse_pragma_stdc_unknown, at);
                }
            },
            s if IGNORED.contains(&s) => {}
            s if UNSUPPORTED.contains(&s) => {
                let loc = tokens[0].span.lo;
                self.s.act_on_unsupported_pragma(s, loc);
            }
            _ => self.unknown_pragma(tokens[0].span.lo),
        }
    }

    /// `-Wunknown-pragmas` at `loc`.
    fn unknown_pragma(&self, loc: Loc) {
        self.error_at(DiagId::pp_unknown_pragma, loc);
    }

    /// Reports the argument-less diagnostic `id` at `loc`.
    fn error_at(&self, id: DiagId, loc: Loc) {
        let d = self.s.diag(id, loc);
        self.emit(d);
    }

    /// Reports `id` with the pragma name `what` at `loc`.
    fn pragma_warning(&self, id: DiagId, what: &str, loc: Loc) {
        let d = self.s.diag(id, loc).arg(what);
        self.emit(d);
    }

    /// `#pragma pack ( ... )` (clang's `HandlePragmaPack`): recorded and
    /// reported as not supported yet.
    fn pragma_pack(&mut self, rest: &[Token], end: Loc) {
        let Some(open) = rest.first().filter(|t| t.kind == TokKind::Punct(Punct::LParen)) else {
            let at = rest.first().map_or(end, |t| t.span.lo);
            self.pragma_warning(DiagId::parse_pragma_expected_lparen, "pack", at);
            return;
        };
        let loc = open.span.lo;
        let mut args = String::new();
        for t in &rest[1..] {
            if t.kind == TokKind::Punct(Punct::RParen) {
                break;
            }
            args.push_str(&self.spelling_of(t));
        }
        self.s.act_on_pragma_pack(loc, args);
    }

    /// `#pragma GCC visibility push(v)` / `pop` (clang's
    /// `HandlePragmaVisibility`); `loc` is the `visibility` token. The
    /// visibility itself is not applied yet: everything has default
    /// visibility.
    fn pragma_visibility(&mut self, loc: Loc, rest: &[Token], end: Loc) {
        const WHAT: &str = "visibility";
        let at = |i: usize| rest.get(i).map_or(end, |t| t.span.lo);
        let after = match self.pragma_word(rest.first()) {
            Some("push") => {
                if rest.get(1).is_none_or(|t| t.kind != TokKind::Punct(Punct::LParen)) {
                    self.pragma_warning(DiagId::parse_pragma_expected_lparen, WHAT, at(1));
                    return;
                }
                let Some(v) = self.pragma_word(rest.get(2)).map(str::to_owned) else {
                    self.pragma_warning(DiagId::parse_pragma_expected_identifier, WHAT, at(2));
                    return;
                };
                if rest.get(3).is_none_or(|t| t.kind != TokKind::Punct(Punct::RParen)) {
                    self.pragma_warning(DiagId::parse_pragma_expected_rparen, WHAT, at(3));
                    return;
                }
                if let Some(extra) = rest.get(4) {
                    self.pragma_warning(DiagId::parse_pragma_extra_tokens, WHAT, extra.span.lo);
                    return;
                }
                if !VISIBILITIES.contains(&v.as_str()) {
                    let d = self.s.diag(DiagId::sema_unknown_visibility, loc).arg(v);
                    self.emit(d);
                }
                self.visibility_depth += 1;
                return;
            }
            Some("pop") => 1,
            _ => {
                self.pragma_warning(DiagId::parse_pragma_expected_identifier, WHAT, at(0));
                return;
            }
        };
        if let Some(extra) = rest.get(after) {
            self.pragma_warning(DiagId::parse_pragma_extra_tokens, WHAT, extra.span.lo);
            return;
        }
        if self.visibility_depth == 0 {
            self.error_at(DiagId::sema_visibility_pop_mismatch, loc);
        } else {
            self.visibility_depth -= 1;
        }
    }
}
