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
use crate::tokens::{Event, LoopHintInfo};

/// Pragmas clang knows and kiln accepts without effect on the
/// translation unit (codegen hints and `#pragma comment`).
const IGNORED: &[&str] = &["comment", "unused", "omp"];

/// Pragmas clang knows that change the meaning of the translation unit
/// and that kiln does not implement yet.
const UNSUPPORTED: &[&str] = &["align", "options", "redefine_extname"];

/// `#pragma clang ...` pragmas accepted without effect.
const CLANG_IGNORED: &[&str] =
    &["diagnostic", "optimize", "max_tokens_here", "max_tokens_total", "__debug"];

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
    /// The macros defined where the pragma is (a macro-expanded operand).
    pub macros: &'x kiln_pp::MacroTable,
}

impl PragmaCx<'_, '_> {
    /// The identifier spelling of `t`, if it is an identifier (keywords
    /// included).
    pub(crate) fn word(&self, t: Option<&Token>) -> Option<&str> {
        match t?.kind {
            TokKind::Identifier(s) => Some(self.interner.as_str(s)),
            _ => None,
        }
    }

    pub(crate) fn report(&self, id: DiagId, loc: Loc) {
        Diag::new(id, Span::empty(loc)).emit(self.diags);
    }

    pub(crate) fn report_what(&self, id: DiagId, what: &str, loc: Loc) {
        Diag::new(id, Span::empty(loc)).arg(what).emit(self.diags);
    }
}

/// Checks the pragma whose `pragma` / `_Pragma` token is at `span` (and
/// which starts at `start`, its `#` or `_Pragma`) and whose tokens are
/// `tokens`, reporting syntax warnings and unknown pragmas; returns the
/// event the parser acts on, if any.
pub(crate) fn classify(
    cx: &PragmaCx<'_, '_>,
    span: Span,
    start: Loc,
    tokens: &[Token],
) -> Option<Event> {
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
                // clang's `PragmaUnrollHintHandler` under its GCC names.
                ("GCC", "unroll") => unroll_hint(cx, "unroll", start, loc, &rest[1..], end),
                ("GCC", "nounroll") => unroll_hint(cx, "nounroll", start, loc, &rest[1..], end),
                ("clang", "loop") => clang_loop(cx, start, loc, &rest[1..], end),
                ("clang", "fp") => crate::fp_pragma::clang_fp(cx, loc, &rest[1..], end),
                // Validated and applied by the preprocessor, which records
                // the warning state per location in the diagnostic engine.
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
        "STDC" => match cx.word(rest.first()) {
            Some(s) if STDC.contains(&s) => {
                crate::fp_pragma::stdc(cx, s, rest[0].span.lo, &rest[1..], end)
            }
            _ => {
                let at = rest.first().map_or(end, |t| t.span.lo);
                cx.report(DiagId::parse_pragma_stdc_unknown, at);
                None
            }
        },
        "float_control" => crate::fp_pragma::float_control(cx, loc, rest, end),
        "weak" => weak(cx, loc, rest, end),
        "ms_struct" => ms_struct(cx, rest, end),
        "unroll" => unroll_hint(cx, "unroll", start, loc, rest, end),
        "nounroll" => unroll_hint(cx, "nounroll", start, loc, rest, end),
        "unroll_and_jam" => unroll_hint(cx, "unroll_and_jam", start, loc, rest, end),
        "nounroll_and_jam" => unroll_hint(cx, "nounroll_and_jam", start, loc, rest, end),
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

/// `#pragma weak name` (clang's `PragmaWeakHandler`); `loc` is the
/// `weak` token. A missing identifier, or tokens after it, are ignored
/// with a warning; the alias form `#pragma weak name = alias` is not
/// supported yet.
fn weak(cx: &PragmaCx<'_, '_>, loc: Loc, rest: &[Token], end: Loc) -> Option<Event> {
    let at = |i: usize| rest.get(i).map_or(end, |t| t.span.lo);
    let Some(TokKind::Identifier(name)) = rest.first().map(|t| t.kind) else {
        cx.report_what(DiagId::parse_pragma_expected_identifier, "weak", at(0));
        return None;
    };
    if rest.get(1).is_some_and(|t| t.kind == TokKind::Punct(Punct::Equal)) {
        return Some(Event::UnsupportedPragma { what: "weak name = alias".to_owned(), loc });
    }
    if rest.len() > 1 {
        cx.report_what(DiagId::parse_pragma_extra_tokens, "weak", at(1));
        return None;
    }
    Some(Event::PragmaWeak { name, loc: rest[0].span.lo })
}

/// `#pragma ms_struct on|off|reset` (clang's `PragmaMSStructHandler`):
/// anything else, or tokens after it, are ignored with a warning.
fn ms_struct(cx: &PragmaCx<'_, '_>, rest: &[Token], end: Loc) -> Option<Event> {
    let at = |i: usize| rest.get(i).map_or(end, |t| t.span.lo);
    let on = match cx.word(rest.first()) {
        Some("on") => true,
        Some("off" | "reset") => false,
        _ => {
            cx.report(DiagId::parse_pragma_ms_struct_malformed, at(0));
            return None;
        }
    };
    if rest.len() > 1 {
        cx.report_what(DiagId::parse_pragma_extra_tokens, "ms_struct", at(1));
        return None;
    }
    Some(Event::PragmaMsStruct { on })
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

/// The `#pragma clang loop` options (clang's `PragmaLoopHintHandler`).
const LOOP_OPTIONS: &[&str] = &[
    "vectorize",
    "interleave",
    "unroll",
    "distribute",
    "vectorize_predicate",
    "vectorize_width",
    "interleave_count",
    "unroll_count",
    "pipeline",
    "pipeline_initiation_interval",
];

/// A loop hint's value (clang's `ParseLoopHintValue`): the tokens up to
/// the end of the pragma or, `in_parens`, the matching `)`. Returns the
/// value, where it ends (clang's `eof` terminator), and the number of
/// tokens read; `None` after reporting a missing `)`.
fn hint_value(
    cx: &PragmaCx<'_, '_>,
    toks: &[Token],
    in_parens: bool,
    end: Loc,
) -> Option<(Vec<Token>, Loc, usize)> {
    let mut open = usize::from(in_parens);
    let mut i = 0;
    while let Some(t) = toks.get(i) {
        match t.kind {
            TokKind::Punct(Punct::LParen) => open += 1,
            TokKind::Punct(Punct::RParen) => {
                open = open.saturating_sub(1);
                if open == 0 && in_parens {
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let value = toks[..i].to_vec();
    if in_parens {
        if i == toks.len() {
            cx.report_what(DiagId::parse_expected, "')'", end);
            return None;
        }
        i += 1;
    }
    let value_end = toks.get(i).map_or(end, |t| t.span.lo);
    Some((value, value_end, i))
}

/// `#pragma unroll [N | (N)]`, `nounroll`, `unroll_and_jam [N | (N)]`,
/// `nounroll_and_jam`, `GCC unroll` / `GCC nounroll` (clang's
/// `PragmaUnrollHintHandler`); `loc` is the pragma-name token.
fn unroll_hint(
    cx: &PragmaCx<'_, '_>,
    name: &'static str,
    start: Loc,
    loc: Loc,
    rest: &[Token],
    end: Loc,
) -> Option<Event> {
    let (value, value_end) = if let Some(first) = rest.first() {
        if name.starts_with("no") {
            cx.report_what(DiagId::parse_pragma_extra_tokens, name, first.span.lo);
            return None;
        }
        let parens = first.kind == TokKind::Punct(Punct::LParen);
        let toks = if parens { &rest[1..] } else { rest };
        let (value, value_end, n) = hint_value(cx, toks, parens, end)?;
        if let Some(extra) = toks.get(n) {
            cx.report_what(DiagId::parse_pragma_extra_tokens, "unroll", extra.span.lo);
            return None;
        }
        (Some(value), value_end)
    } else {
        (None, end)
    };
    let hint = LoopHintInfo { start, name, name_loc: loc, option: None, value, value_end };
    Some(Event::LoopHints { before: Loc::INVALID, hints: vec![hint] })
}

/// `#pragma clang loop option(value) ...` (clang's
/// `PragmaLoopHintHandler`); `loc` is the `loop` token. Any error drops
/// the whole pragma.
fn clang_loop(
    cx: &PragmaCx<'_, '_>,
    start: Loc,
    loc: Loc,
    rest: &[Token],
    end: Loc,
) -> Option<Event> {
    let at = |i: usize| rest.get(i).map_or(end, |t| t.span.lo);
    if cx.word(rest.first()).is_none() {
        Diag::new(DiagId::parse_pragma_loop_invalid_option, Span::empty(at(0)))
            .arg("1")
            .arg("")
            .emit(cx.diags);
        return None;
    }
    let mut hints = Vec::new();
    let mut i = 0;
    while let Some(word) = cx.word(rest.get(i)) {
        let Some(&option) = LOOP_OPTIONS.iter().find(|o| **o == word) else {
            Diag::new(DiagId::parse_pragma_loop_invalid_option, Span::empty(at(i)))
                .arg("0")
                .arg(format!(" '{word}'"))
                .emit(cx.diags);
            return None;
        };
        let option_loc = at(i);
        i += 1;
        if rest.get(i).is_none_or(|t| t.kind != TokKind::Punct(Punct::LParen)) {
            cx.report_what(DiagId::parse_expected, "'('", at(i));
            return None;
        }
        i += 1;
        let (value, value_end, n) = hint_value(cx, &rest[i..], true, end)?;
        i += n;
        hints.push(LoopHintInfo {
            start,
            name: "loop",
            name_loc: loc,
            option: Some((option, option_loc)),
            value: Some(value),
            value_end,
        });
    }
    if i < rest.len() {
        cx.report_what(DiagId::parse_pragma_extra_tokens, "clang loop", at(i));
        return None;
    }
    Some(Event::LoopHints { before: Loc::INVALID, hints })
}

impl Parser<'_, '_> {
    /// Acts on a preprocessor event (clang's annotation-token handlers).
    pub(crate) fn on_event(&mut self, e: Event) {
        // Module events (and anything queued behind them) wait for the next
        // namespace-scope declaration (spec/modules.md §4).
        let module_event = matches!(
            e,
            Event::ModuleDirective(_) | Event::UnitBegin { .. } | Event::UnitEnd { .. }
        );
        if module_event || self.module_pending() {
            self.module_event(e);
            return;
        }
        match e {
            Event::ModuleDirective(_) | Event::UnitBegin { .. } | Event::UnitEnd { .. } => {}
            Event::Module { span, import } => {
                // [cpp.module], [cpp.import]: clang parses these; kiln
                // does not implement modules yet.
                let what = if import { "a module import" } else { "a module declaration" };
                self.unsupported(what, span.lo);
            }
            Event::PragmaPack { loc, args } => self.s.act_on_pragma_pack(loc, args),
            Event::PragmaMsStruct { on } => self.s.act_on_pragma_ms_struct(on),
            Event::LoopHints { before, hints } => {
                // Applied when the statement at `before` is parsed.
                if let Some((_, v)) = self.loop_hints.iter_mut().find(|(l, _)| *l == before) {
                    v.extend(hints);
                } else {
                    self.loop_hints.push((before, hints));
                }
            }
            Event::FpPragma { before, pragma } => self.queue_fp_pragma(before, pragma),
            Event::UnsupportedPragma { what, loc } => self.s.act_on_unsupported_pragma(&what, loc),
            // clang's `ActOnPragmaVisibility`; sema applies the pushed
            // visibility to the declarations that follow.
            Event::Visibility { loc, push: Some(v) } => {
                if VISIBILITIES.contains(&v.as_str()) {
                    self.s.act_on_pragma_visibility(Some(&v), loc);
                } else {
                    let d = self.s.diag(DiagId::sema_unknown_visibility, loc).arg(v);
                    self.emit(d);
                }
            }
            Event::Visibility { loc, push: None } => self.s.act_on_pragma_visibility(None, loc),
            Event::PragmaWeak { name, loc } => self.s.act_on_pragma_weak(name, loc),
        }
    }
}
