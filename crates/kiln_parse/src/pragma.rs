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

/// `#pragma clang ...` pragmas accepted without effect.
const CLANG_IGNORED: &[&str] =
    &["diagnostic", "optimize", "max_tokens_here", "max_tokens_total", "__debug"];

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
                ("clang", "attribute") => crate::attr_pragma::classify(cx, loc, &rest[1..], end),
                // Validated and applied by the preprocessor, which records
                // the warning state per location in the diagnostic engine.
                ("GCC", "diagnostic") => None,
                ("clang", s) if CLANG_IGNORED.contains(&s) => None,
                ("clang", "section") => clang_section(cx, &rest[1..], end),
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
        "redefine_extname" => redefine_extname(cx, rest, end),
        "ms_struct" => ms_struct(cx, rest, end),
        "unroll" => unroll_hint(cx, "unroll", start, loc, rest, end),
        "nounroll" => unroll_hint(cx, "nounroll", start, loc, rest, end),
        "unroll_and_jam" => unroll_hint(cx, "unroll_and_jam", start, loc, rest, end),
        "nounroll_and_jam" => unroll_hint(cx, "nounroll_and_jam", start, loc, rest, end),
        s if IGNORED.contains(&s) => None,
        "options" => options_align(cx, loc, rest, end, true),
        "align" => options_align(cx, loc, rest, end, false),
        _ => {
            cx.report(DiagId::pp_unknown_pragma, loc);
            None
        }
    }
}

/// `#pragma pack ( ... )` (clang's `PragmaPackHandler`); `loc` is the
/// `pack` token. The forms: `()`, `(N)`, `(show)`, `(push|pop [, id]
/// [, N])`; a malformed one is ignored with clang's warning.
fn pack(cx: &PragmaCx<'_, '_>, loc: Loc, rest: &[Token], end: Loc) -> Option<Event> {
    use kiln_sema::pragma_pack::{PackAction, PackAlignArg};
    const WHAT: &str = "pack";
    let at = |i: usize| rest.get(i).map_or(end, |t| t.span.lo);
    let kind = |i: usize| rest.get(i).map(|t| t.kind);
    let num = |i: usize| -> Option<PackAlignArg> {
        let t = rest.get(i).filter(|t| t.kind == TokKind::PpNumber)?;
        let text = kiln_lex::spelling(cx.sm, t, cx.lex_opts);
        let n = kiln_lex::literal::classify_number(&text, cx.lex_opts.std);
        Some(match n.class {
            kiln_lex::literal::NumberClass::Integer(i) if i.ud_suffix.is_none() => {
                PackAlignArg::Int(i.value().and_then(|v| u64::try_from(v).ok()))
            }
            _ => PackAlignArg::NotInteger,
        })
    };
    if kind(0) != Some(TokKind::Punct(Punct::LParen)) {
        cx.report_what(DiagId::parse_pragma_expected_lparen, WHAT, at(0));
        return None;
    }
    let mut action = PackAction::default();
    let mut label = None;
    let mut align = None;
    let mut i = 1;
    if let Some(a) = num(i) {
        align = Some(a);
        action.set = true;
        i += 1;
    } else if let Some(word) = cx.word(rest.get(i)) {
        if word == "show" {
            action.show = true;
            i += 1;
        } else {
            match word {
                "push" => action.push = true,
                "pop" => action.pop = true,
                _ => {
                    cx.report_what(DiagId::parse_pragma_invalid_action, WHAT, at(i));
                    return None;
                }
            }
            i += 1;
            if kind(i) == Some(TokKind::Punct(Punct::Comma)) {
                i += 1;
                if let Some(a) = num(i) {
                    align = Some(a);
                    action.set = true;
                    i += 1;
                } else if let Some(l) = cx.word(rest.get(i)) {
                    label = Some(l.to_owned());
                    i += 1;
                    if kind(i) == Some(TokKind::Punct(Punct::Comma)) {
                        i += 1;
                        let Some(a) = num(i) else {
                            cx.report(DiagId::parse_pragma_pack_malformed, at(i));
                            return None;
                        };
                        align = Some(a);
                        action.set = true;
                        i += 1;
                    }
                } else {
                    cx.report(DiagId::parse_pragma_pack_malformed, at(i));
                    return None;
                }
            }
        }
    }
    if kind(i) != Some(TokKind::Punct(Punct::RParen)) {
        cx.report_what(DiagId::parse_pragma_expected_rparen, WHAT, at(i));
        return None;
    }
    if rest.len() > i + 1 {
        cx.report_what(DiagId::parse_pragma_extra_tokens, WHAT, at(i + 1));
        return None;
    }
    Some(Event::PragmaPack { loc, action, label, align })
}

/// `#pragma options align=k` (`options`) and `#pragma align=k` (clang's
/// `ParseAlignPragma`); `loc` is the `options` / `align` token.
fn options_align(
    cx: &PragmaCx<'_, '_>,
    loc: Loc,
    rest: &[Token],
    end: Loc,
    options: bool,
) -> Option<Event> {
    use kiln_sema::pragma_pack::OptionsAlign;
    let name = if options { "options" } else { "align" };
    let shown = if options { "options align" } else { "align" };
    let at = |i: usize| rest.get(i).map_or(end, |t| t.span.lo);
    let mut i = 0;
    if options {
        if cx.word(rest.first()) != Some("align") {
            cx.report(DiagId::parse_pragma_options_expected_align, at(0));
            return None;
        }
        i = 1;
    }
    if rest.get(i).is_none_or(|t| t.kind != TokKind::Punct(Punct::Equal)) {
        cx.report_what(DiagId::parse_pragma_align_expected_equal, shown, at(i));
        return None;
    }
    i += 1;
    let Some(word) = cx.word(rest.get(i)) else {
        cx.report_what(DiagId::parse_pragma_expected_identifier, name, at(i));
        return None;
    };
    let kind = match word {
        "native" => OptionsAlign::Native,
        "natural" => OptionsAlign::Natural,
        "packed" => OptionsAlign::Packed,
        "power" => OptionsAlign::Power,
        "mac68k" => OptionsAlign::Mac68k,
        "reset" => OptionsAlign::Reset,
        _ => {
            cx.report_what(DiagId::parse_pragma_align_invalid_option, shown, at(i));
            return None;
        }
    };
    if rest.len() > i + 1 {
        cx.report_what(DiagId::parse_pragma_extra_tokens, name, at(i + 1));
        return None;
    }
    Some(Event::PragmaOptionsAlign { loc, kind })
}

/// `#pragma clang section kind="name" ...` (clang
/// `PragmaClangSectionHandler`): any number of `kind = string-literal`
/// with `kind` one of `bss`, `data`, `rodata`, `text`, `relro`; the
/// string is ordinary, adjacent ones concatenated. The entries before an
/// error take effect; the error ends the pragma.
fn clang_section(cx: &PragmaCx<'_, '_>, rest: &[Token], end: Loc) -> Option<Event> {
    use kiln_sema::section::ClangSectionKind;
    let at = |i: usize| rest.get(i).map_or(end, |t| t.span.lo);
    let mut entries = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        let Some(kind) = cx.word(rest.get(i)).and_then(ClangSectionKind::from_name) else {
            cx.report_what(
                DiagId::parse_pragma_clang_section_expected_kind,
                "clang section",
                at(i),
            );
            break;
        };
        let kind_loc = at(i);
        i += 1;
        if rest.get(i).is_none_or(|t| t.kind != TokKind::Punct(Punct::Equal)) {
            cx.report_what(DiagId::parse_pragma_clang_section_expected_equal, kind.name(), at(i));
            break;
        }
        i += 1;
        // clang `Preprocessor::LexStringLiteral`: ordinary string
        // literals only.
        let ordinary = |t: Option<&Token>| {
            matches!(t.map(|t| t.kind), Some(TokKind::StringLit(kiln_lex::Encoding::Ordinary)))
        };
        if !ordinary(rest.get(i)) {
            cx.report_what(DiagId::parse_expected_string_literal_in, "pragma clang section", at(i));
            break;
        }
        let opts = kiln_lex::literal::LiteralOptions::new(
            cx.lex_opts.std,
            kiln_lex::literal::EncodingParams::default(),
        );
        let mut name = Vec::new();
        let mut bad = false;
        while ordinary(rest.get(i)) {
            let t = &rest[i];
            let sp = kiln_lex::spelling(cx.sm, t, cx.lex_opts);
            let lit = kiln_lex::literal::decode_string(&sp, opts);
            for d in kiln_pp::literal_diags(cx.sm, t, &sp, &lit.diags, None, cx.lex_opts) {
                d.emit(cx.diags);
            }
            if lit.ud_suffix.is_some() {
                cx.report(DiagId::pp_string_udl, t.span.lo);
            }
            bad |= lit.had_error;
            if let kiln_lex::literal::CodeUnits::U8(u) = &lit.units {
                name.extend_from_slice(u);
            }
            i += 1;
        }
        if bad {
            break;
        }
        entries.push((kind, kind_loc, name.into_boxed_slice()));
    }
    (!entries.is_empty()).then_some(Event::PragmaClangSection(entries))
}

/// `#pragma weak name` and `#pragma weak name = target` (clang's
/// `PragmaWeakHandler`); `loc` is the `weak` token. A missing
/// identifier, or tokens after the pragma, are ignored with a warning.
fn weak(cx: &PragmaCx<'_, '_>, _loc: Loc, rest: &[Token], end: Loc) -> Option<Event> {
    let at = |i: usize| rest.get(i).map_or(end, |t| t.span.lo);
    let ident = |i: usize| match rest.get(i).map(|t| t.kind) {
        Some(TokKind::Identifier(s)) => Some((s, at(i))),
        _ => None,
    };
    let Some((name, name_loc)) = ident(0) else {
        cx.report_what(DiagId::parse_pragma_expected_identifier, "weak", at(0));
        return None;
    };
    let mut next = 1;
    let mut target = None;
    if rest.get(1).is_some_and(|t| t.kind == TokKind::Punct(Punct::Equal)) {
        let Some(t) = ident(2) else {
            cx.report_what(DiagId::parse_pragma_expected_identifier, "weak", at(2));
            return None;
        };
        target = Some(t);
        next = 3;
    }
    if rest.len() > next {
        cx.report_what(DiagId::parse_pragma_extra_tokens, "weak", at(next));
        return None;
    }
    Some(Event::PragmaWeak { name, loc: name_loc, target })
}

/// `#pragma redefine_extname old new` (clang's
/// `PragmaRedefineExtnameHandler`): a missing identifier, or tokens
/// after the pragma, are ignored with a warning.
fn redefine_extname(cx: &PragmaCx<'_, '_>, rest: &[Token], end: Loc) -> Option<Event> {
    const WHAT: &str = "redefine_extname";
    let at = |i: usize| rest.get(i).map_or(end, |t| t.span.lo);
    let ident = |i: usize| {
        if let Some(TokKind::Identifier(s)) = rest.get(i).map(|t| t.kind) {
            Some((s, at(i)))
        } else {
            cx.report_what(DiagId::parse_pragma_expected_identifier, WHAT, at(i));
            None
        }
    };
    let old = ident(0)?;
    let new = ident(1)?;
    if rest.len() > 2 {
        cx.report_what(DiagId::parse_pragma_extra_tokens, WHAT, at(2));
        return None;
    }
    Some(Event::RedefineExtname { old, new })
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
            Event::PragmaPack { loc, action, label, align } => {
                self.s.act_on_pragma_pack(loc, action, label.as_deref(), align);
            }
            Event::PragmaOptionsAlign { loc, kind } => {
                self.s.act_on_pragma_options_align(kind, loc);
            }
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
            Event::PragmaClangSection(entries) => {
                for (kind, loc, name) in entries {
                    self.s.act_on_pragma_clang_section(kind, loc, &name);
                }
            }
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
            Event::PragmaWeak { name, loc, target: None } => self.s.act_on_pragma_weak(name, loc),
            Event::PragmaWeak { name, loc, target: Some(t) } => {
                self.s.act_on_pragma_weak_alias((name, loc), t);
            }
            Event::RedefineExtname { old, new } => self.s.act_on_pragma_redefine_extname(old, new),
            Event::PragmaAttribute(p) => self.act_on_attr_pragma(p),
        }
    }
}
