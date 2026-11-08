//! Floating-point pragmas (spec/parse.md §2.3, spec/codegen.md
//! "Floating-point semantics"; clang's `PragmaFPContractHandler`,
//! `PragmaFPHandler`, `PragmaFloatControlHandler`, the `STDC` handlers and
//! `Sema::ActOnPragmaFP*`).
//!
//! [`crate::pragma`] reads a pragma's syntax here when the token source
//! reads it (clang's handlers) and queues it, keyed by the token after it.
//! The parser takes the queued pragmas where clang's annotation tokens are
//! consumed: at namespace scope and at the start of a compound statement
//! they change the FP state sema records in each compound statement
//! ([`kiln_ast::fp::FpOverride`]); before any other statement or in a
//! class they are errors. The FENV pragmas kiln does not implement
//! (`FENV_ACCESS ON`, a rounding mode other than `FE_TONEAREST`,
//! `float_control(except, on)`, `clang fp exceptions(maytrap|strict)`,
//! `clang fp eval_method(double|extended)`) are errors, never ignored.

use kiln_ast::fp::{FpContract, fp_bit};
use kiln_diag::{Diag, DiagId};
use kiln_lex::{Punct, TokKind, Token};
use kiln_source::{Loc, Span};

use crate::parser::Parser;
use crate::pragma::PragmaCx;
use crate::tokens::{Event, FpAction, FpPragma};

/// Where the parser takes queued FP pragmas.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FpAt {
    /// At namespace scope (file, namespace or language linkage).
    File,
    /// At the start of a compound statement.
    Compound,
    /// Before another statement, or before the `}` after statements.
    Statement,
    /// In a class (the class-key: `struct`, `class`, `union`).
    Class(&'static str),
}

fn event(loc: Loc, what: &'static str, actions: Vec<FpAction>) -> Event {
    Event::FpPragma { before: Loc::INVALID, pragma: FpPragma { loc, what, actions } }
}

/// `#pragma STDC FP_CONTRACT|FENV_ACCESS|CX_LIMITED_RANGE|FENV_ROUND`
/// (C23 7.12.2, 7.6.1, 7.3.4, 7.6.2); `loc` is the pragma-name token and
/// `rest` the tokens after it.
pub(crate) fn stdc(
    cx: &PragmaCx<'_, '_>,
    name: &str,
    loc: Loc,
    rest: &[Token],
    end: Loc,
) -> Option<Event> {
    if name == "FENV_ROUND" {
        return fenv_round(cx, rest, end);
    }
    // clang's `Preprocessor::LexOnOffSwitch` (unexpanded tokens).
    let at = |i: usize| rest.get(i).map_or(end, |t| t.span.lo);
    let switch = match cx.word(rest.first()) {
        Some("ON") => Some(true),
        Some("OFF") => Some(false),
        Some("DEFAULT") => None,
        _ => {
            cx.report(DiagId::parse_pragma_stdc_on_off, at(0));
            return None;
        }
    };
    if rest.len() > 1 {
        cx.report(DiagId::parse_pragma_stdc_expected_eod, at(1));
    }
    match name {
        "FP_CONTRACT" => {
            let c = switch.map(|on| if on { FpContract::On } else { FpContract::Off });
            Some(event(loc, "fp_contract", vec![FpAction::Contract(c)]))
        }
        "FENV_ACCESS" => {
            Some(event(loc, "STDC FENV_ACCESS", vec![FpAction::FenvAccess(switch == Some(true))]))
        }
        _ => Some(event(loc, "STDC CX_LIMITED_RANGE", vec![FpAction::CxLimitedRange(switch)])),
    }
}

/// `#pragma STDC FENV_ROUND mode` (C23 7.6.2; clang's
/// `PragmaSTDC_FENV_ROUNDHandler`). The mode is read macro-expanded, so
/// one `<cfenv>` defines is not an identifier and the pragma is ignored.
fn fenv_round(cx: &PragmaCx<'_, '_>, rest: &[Token], end: Loc) -> Option<Event> {
    const MODES: &[&str] = &[
        "FE_TOWARDZERO",
        "FE_TONEAREST",
        "FE_UPWARD",
        "FE_DOWNWARD",
        "FE_TONEARESTFROMZERO",
        "FE_DYNAMIC",
    ];
    let first = rest.first();
    let mode = cx.word(first).filter(|_| {
        !matches!(first.map(|t| t.kind), Some(TokKind::Identifier(s)) if cx.macros.is_defined(s))
    });
    let Some(mode) = mode else {
        cx.report_what(
            DiagId::parse_pragma_expected_identifier,
            "FENV_ROUND",
            first.map_or(end, |t| t.span.lo),
        );
        return None;
    };
    let Some(&mode) = MODES.iter().find(|m| **m == mode) else {
        cx.report(DiagId::parse_pragma_fenv_round_unknown_mode, rest[0].span.lo);
        return None;
    };
    if let Some(extra) = rest.get(1) {
        cx.report_what(DiagId::parse_pragma_extra_tokens, "STDC FENV_ROUND", extra.span.lo);
        return None;
    }
    // clang reports it, and the pragma, at the end of the directive.
    cx.report(DiagId::parse_pragma_fenv_round_not_supported, end);
    Some(event(end, "STDC FENV_ROUND", vec![FpAction::FenvRound(mode)]))
}

/// `#pragma clang fp option(value) ...` (clang's `PragmaFPHandler`);
/// `loc` is the `fp` token. Any error drops the whole pragma.
pub(crate) fn clang_fp(cx: &PragmaCx<'_, '_>, loc: Loc, rest: &[Token], end: Loc) -> Option<Event> {
    let at = |i: usize| rest.get(i).map_or(end, |t| t.span.lo);
    let invalid_option = |missing: bool, name: &str, at: Loc| {
        Diag::new(DiagId::parse_pragma_fp_invalid_option, Span::empty(at))
            .arg(if missing { "1" } else { "0" })
            .arg(if missing { String::new() } else { format!(" '{name}'") })
            .emit(cx.diags);
    };
    if cx.word(rest.first()).is_none() {
        invalid_option(true, "", at(0));
        return None;
    }
    let mut actions = Vec::new();
    let mut i = 0;
    while let Some(option) = cx.word(rest.get(i)) {
        let expected = match option {
            "contract" => "'fast' or 'on' or 'off'",
            "reassociate" | "reciprocal" => "'on' or 'off'",
            "exceptions" => "'ignore', 'maytrap' or 'strict'",
            "eval_method" => "'source', 'double' or 'extended'",
            _ => {
                invalid_option(false, option, at(i));
                return None;
            }
        };
        i += 1;
        if rest.get(i).is_none_or(|t| t.kind != TokKind::Punct(Punct::LParen)) {
            cx.report_what(DiagId::parse_expected, "'('", at(i));
            return None;
        }
        i += 1;
        let value = cx.word(rest.get(i));
        let action = match (option, value) {
            ("contract", Some("on")) => Some(FpAction::Contract(Some(FpContract::On))),
            ("contract", Some("off")) => Some(FpAction::Contract(Some(FpContract::Off))),
            ("contract", Some("fast")) => Some(FpAction::Contract(Some(FpContract::Fast))),
            ("reassociate", Some(v @ ("on" | "off"))) => Some(FpAction::Reassociate(v == "on")),
            ("reciprocal", Some(v @ ("on" | "off"))) => Some(FpAction::Reciprocal(v == "on")),
            ("exceptions", Some("ignore")) => Some(FpAction::Exceptions("ignore")),
            ("exceptions", Some("maytrap")) => Some(FpAction::Exceptions("maytrap")),
            ("exceptions", Some("strict")) => Some(FpAction::Exceptions("strict")),
            ("eval_method", Some("source")) => Some(FpAction::EvalMethod("source")),
            ("eval_method", Some("double")) => Some(FpAction::EvalMethod("double")),
            ("eval_method", Some("extended")) => Some(FpAction::EvalMethod("extended")),
            _ => None,
        };
        let Some(action) = action else {
            let spelling = rest
                .get(i)
                .map(|t| kiln_lex::spelling(cx.sm, t, cx.lex_opts).into_owned())
                .unwrap_or_default();
            Diag::new(DiagId::parse_pragma_fp_invalid_argument, Span::empty(at(i)))
                .arg(spelling)
                .arg(option)
                .arg(expected)
                .emit(cx.diags);
            return None;
        };
        actions.push(action);
        i += 1;
        if rest.get(i).is_none_or(|t| t.kind != TokKind::Punct(Punct::RParen)) {
            cx.report_what(DiagId::parse_expected, "')'", at(i));
            return None;
        }
        i += 1;
    }
    if i < rest.len() {
        cx.report_what(DiagId::parse_pragma_extra_tokens, "clang fp", at(i));
        return None;
    }
    Some(event(loc, "clang fp", actions))
}

/// `#pragma float_control(...)` (clang's `PragmaFloatControlHandler`);
/// `loc` is the `float_control` token.
pub(crate) fn float_control(
    cx: &PragmaCx<'_, '_>,
    loc: Loc,
    rest: &[Token],
    end: Loc,
) -> Option<Event> {
    let at = |i: usize| rest.get(i).map_or(end, |t| t.span.lo);
    let is = |i: usize, p: Punct| rest.get(i).is_some_and(|t| t.kind == TokKind::Punct(p));
    let malformed = |i: usize| {
        cx.report(DiagId::parse_pragma_float_control_malformed, at(i));
        None
    };
    if !is(0, Punct::LParen) {
        cx.report_what(DiagId::parse_expected, "'('", at(0));
        return None;
    }
    let Some(kind) = cx.word(rest.get(1)) else { return malformed(1) };
    let mut i = 2;
    let action = match kind {
        "push" | "pop" => {
            if !is(i, Punct::RParen) {
                return malformed(i);
            }
            i += 1;
            if kind == "push" { FpAction::FloatControlPush } else { FpAction::FloatControlPop }
        }
        "precise" | "except" => {
            let precise = kind == "precise";
            let (mut on, mut push) = (true, false);
            if is(i, Punct::RParen) {
                i += 1;
            } else if !is(i, Punct::Comma) {
                return malformed(i);
            } else {
                i += 1;
                match cx.word(rest.get(i)) {
                    Some("on") => {}
                    Some("off") => on = false,
                    Some("push") => push = true,
                    _ => return malformed(i),
                }
                i += 1;
                if is(i, Punct::Comma) {
                    i += 1;
                    if cx.word(rest.get(i)) != Some("push") {
                        return malformed(i);
                    }
                    push = true;
                    i += 1;
                }
                if !is(i, Punct::RParen) {
                    return malformed(i);
                }
                i += 1;
            }
            FpAction::FloatControl { precise, on, push }
        }
        _ => return malformed(2),
    };
    if i < rest.len() {
        cx.report_what(DiagId::parse_pragma_extra_tokens, "float_control", at(i));
        return None;
    }
    Some(event(loc, "float_control", vec![action]))
}

impl Parser<'_, '_> {
    /// Queues an FP pragma for the token at `before`.
    pub(crate) fn queue_fp_pragma(&mut self, before: Loc, p: FpPragma) {
        self.fp_pragmas.push((before, p));
    }

    /// Takes the FP pragmas queued for the current token, in order, and
    /// applies or rejects them as their place `at` allows. Returns whether
    /// there were any.
    pub(crate) fn take_fp_pragmas(&mut self, at: FpAt) -> bool {
        if self.fp_pragmas.is_empty() {
            return false;
        }
        let here = self.loc();
        let mut taken = Vec::new();
        self.fp_pragmas.retain(|(l, p)| {
            if *l == here {
                taken.push(p.clone());
                false
            } else {
                true
            }
        });
        let any = !taken.is_empty();
        for p in taken {
            match at {
                FpAt::File => self.apply_fp_pragma(&p, false),
                FpAt::Compound => self.apply_fp_pragma(&p, true),
                FpAt::Statement => self.misplaced_fp_pragma(&p),
                FpAt::Class(tag) => {
                    let d = self.s.diag(DiagId::parse_pragma_misplaced_in_decl, p.loc).arg(tag);
                    self.emit(d);
                }
            }
        }
        any
    }

    /// Reports the FP pragmas no rule took (clang finds the annotation
    /// token where no statement or declaration may start).
    pub(crate) fn flush_fp_pragmas(&mut self) {
        for (_, p) in std::mem::take(&mut self.fp_pragmas) {
            self.misplaced_fp_pragma(&p);
        }
    }

    fn misplaced_fp_pragma(&mut self, p: &FpPragma) {
        let d = self.s.diag(DiagId::parse_pragma_file_or_compound_scope, p.loc).arg(p.what);
        self.emit(d);
    }

    /// Applies `p` to sema's FP state (clang's `ActOnPragmaFPContract`,
    /// `ActOnPragmaFPValueChangingOption`, `ActOnPragmaFloatControl`,
    /// ...); `in_function`: at the start of a compound statement.
    fn apply_fp_pragma(&mut self, p: &FpPragma, in_function: bool) {
        for &a in &p.actions {
            let mut cur = self.s.fp_pragma_state();
            match a {
                FpAction::Contract(c) => cur.set_contract(c),
                FpAction::Reassociate(on) => cur.set_flag(fp_bit::REASSOC, on),
                FpAction::Reciprocal(on) => cur.set_flag(fp_bit::ARCP, on),
                FpAction::CxLimitedRange(v) => cur.cx_limited = v,
                // kiln's floating-point environment is the default one
                // (spec/codegen.md): these keep it.
                FpAction::Exceptions("ignore")
                | FpAction::EvalMethod("source")
                | FpAction::FenvAccess(false)
                | FpAction::FenvRound("FE_TONEAREST")
                | FpAction::FloatControl { precise: false, on: false, push: false } => {}
                FpAction::Exceptions(v) => {
                    self.fenv_unsupported(&format!("'#pragma clang fp exceptions({v})'"), p.loc);
                }
                FpAction::EvalMethod(v) => {
                    self.fenv_unsupported(&format!("'#pragma clang fp eval_method({v})'"), p.loc);
                }
                FpAction::FenvAccess(true) => {
                    self.fenv_unsupported("'#pragma STDC FENV_ACCESS ON'", p.loc);
                }
                FpAction::FenvRound(m) => {
                    self.fenv_unsupported(&format!("'#pragma STDC FENV_ROUND {m}'"), p.loc);
                }
                FpAction::FloatControlPush
                | FpAction::FloatControlPop
                | FpAction::FloatControl { push: true, .. }
                    if in_function =>
                {
                    let d = self.s.diag(DiagId::parse_pragma_float_control_scope, p.loc);
                    self.emit(d);
                    continue;
                }
                FpAction::FloatControlPush => self.s.fp_pragma_push(),
                FpAction::FloatControlPop => {
                    if !self.s.fp_pragma_pop() {
                        let d = self.s.diag(DiagId::parse_pragma_float_control_pop_failed, p.loc);
                        self.emit(d);
                    }
                    continue;
                }
                FpAction::FloatControl { precise, on, push } => {
                    if push {
                        self.s.fp_pragma_push();
                    }
                    if !precise {
                        if on {
                            self.fenv_unsupported("'#pragma float_control(except, on)'", p.loc);
                        }
                        continue;
                    }
                    cur.set_precise(on);
                }
            }
            self.s.set_fp_pragma_state(cur);
        }
    }

    fn fenv_unsupported(&mut self, what: &str, loc: Loc) {
        self.s.unsupported(&format!("{what} (the floating-point environment)"), loc);
    }
}
