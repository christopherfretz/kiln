//! Loop-hint pragmas at the statement they precede (clang's
//! `HandlePragmaLoopHint` and `ParsePragmaLoopHint`; spec/parse.md §2.3).
//!
//! [`crate::pragma`] checks a hint's syntax when the pragma is read and
//! queues it, keyed by the token after it; here each hint's argument is
//! checked and parsed and the hint becomes a `LOOP_HINT` attribute of the
//! following statement.

use kiln_ast::AttrId;
use kiln_ast::attr::{LoopHint, LoopHintOption as O, LoopHintSpelling, LoopHintState as St};
use kiln_diag::DiagId;
use kiln_lex::{Punct, TokKind, Token};
use kiln_source::Loc;

use crate::parser::Parser;
use crate::tokens::LoopHintInfo;

impl Parser<'_, '_> {
    /// The loop hints queued for the statement at the current token, if
    /// any (removed from the queue).
    pub(crate) fn take_loop_hints(&mut self) -> Option<Vec<LoopHintInfo>> {
        if self.loop_hints.is_empty() {
            return None;
        }
        let here = self.loc();
        let i = self.loop_hints.iter().position(|(l, _)| *l == here)?;
        Some(self.loop_hints.swap_remove(i).1)
    }

    /// The attributes of loop hints `hints` (clang `HandlePragmaLoopHint`
    /// for each): the invalid ones are diagnosed and dropped.
    pub(crate) fn loop_hint_attrs(&mut self, hints: Vec<LoopHintInfo>) -> Vec<AttrId> {
        let starts: Vec<Loc> = hints.iter().map(|h| h.start).collect();
        let mut out = Vec::new();
        for (i, h) in hints.into_iter().enumerate() {
            // clang's `Tok` after the annotation token: the next hint's
            // annotation, else the statement.
            let next = starts.get(i + 1).copied().unwrap_or_else(|| self.loc());
            out.extend(self.loop_hint_attr(h, next));
        }
        out
    }

    /// The identifier spelling of `t`.
    fn word_of(&self, t: Option<&Token>) -> Option<String> {
        match t?.kind {
            TokKind::Identifier(s) => Some(self.s.ast.symbols().as_str(s).to_owned()),
            _ => None,
        }
    }

    fn extra_tokens(&self, what: &str, at: Loc) {
        self.emit(self.s.diag(DiagId::parse_pragma_extra_tokens, at).arg(what));
    }

    /// Parses `toks` as a constant-expression ending at `end` (clang's
    /// `EnterTokenStream` of the hint's tokens). `f` runs after the
    /// expression; leftover tokens draw `-Wignored-pragmas` with `what`.
    fn hint_expr<R>(
        &mut self,
        toks: Vec<Token>,
        end: Loc,
        what: &str,
        f: impl FnOnce(&mut Self, Option<kiln_ast::ExprId>) -> R,
    ) -> R {
        self.begin_replay(toks, end);
        let e = self.parse_constant_expr();
        let r = f(self, e);
        if !self.at_eof() {
            self.extra_tokens(what, self.loc());
            while !self.at_eof() {
                self.bump();
            }
        }
        self.end_replay();
        r
    }

    /// One hint's attribute; `next` is where clang's parser stands after
    /// the hint's annotation token.
    fn loop_hint_attr(&mut self, h: LoopHintInfo, next: Loc) -> Option<AttrId> {
        let spelling = match h.name {
            "unroll" => LoopHintSpelling::Unroll,
            "nounroll" => LoopHintSpelling::Nounroll,
            "unroll_and_jam" => LoopHintSpelling::UnrollAndJam,
            "nounroll_and_jam" => LoopHintSpelling::NounrollAndJam,
            _ => LoopHintSpelling::ClangLoop,
        };
        let mk = |option, state| LoopHint { option, state, spelling };
        let Some(toks) = h.value else {
            // A valueless `#pragma unroll` and friends (clang
            // `handleLoopHintAttr`).
            let (option, state) = match spelling {
                LoopHintSpelling::Nounroll => (O::Unroll, St::Disable),
                LoopHintSpelling::UnrollAndJam => (O::UnrollAndJam, St::Enable),
                LoopHintSpelling::NounrollAndJam => (O::UnrollAndJam, St::Disable),
                _ => (O::Unroll, St::Enable),
            };
            return Some(self.s.loop_hint_attr(mk(option, state), h.name_loc, None));
        };
        let opt = h.option.map_or("", |o| o.0);
        let full = opt == "unroll";
        let state_option =
            matches!(opt, "vectorize" | "interleave" | "vectorize_predicate" | "unroll")
                || matches!(opt, "distribute" | "pipeline");
        let assume_safety = !matches!(opt, "unroll" | "distribute" | "pipeline");
        let what = match spelling {
            LoopHintSpelling::ClangLoop => format!("clang loop {opt}"),
            LoopHintSpelling::Unroll => "unroll".to_owned(),
            LoopHintSpelling::UnrollAndJam => "unroll_and_jam".to_owned(),
            _ => String::new(),
        };
        let Some(first) = toks.first().copied() else {
            let expected = if state_option {
                format!(
                    "'enable'{}{} or 'disable'",
                    if full { ", 'full'" } else { "" },
                    if assume_safety { ", 'assume_safety'" } else { "" }
                )
            } else {
                "an integer value".to_owned()
            };
            let d = self.s.diag(DiagId::parse_pragma_loop_missing_argument, h.value_end);
            self.emit(d.arg(expected));
            return None;
        };
        let option = match opt {
            "vectorize" => O::Vectorize,
            "vectorize_width" => O::VectorizeWidth,
            "interleave" => O::Interleave,
            "interleave_count" => O::InterleaveCount,
            "unroll" => O::Unroll,
            "unroll_count" => O::UnrollCount,
            "pipeline" => O::PipelineDisabled,
            "pipeline_initiation_interval" => O::PipelineInitiationInterval,
            "distribute" => O::Distribute,
            "vectorize_predicate" => O::VectorizePredicate,
            _ if spelling == LoopHintSpelling::UnrollAndJam => O::UnrollAndJamCount,
            _ => O::UnrollCount,
        };
        if state_option {
            let state = match self.word_of(Some(&first)).as_deref() {
                Some("disable") => Some(St::Disable),
                Some("enable") if opt != "pipeline" => Some(St::Enable),
                Some("full") if full => Some(St::Full),
                Some("assume_safety") if assume_safety => Some(St::AssumeSafety),
                _ => None,
            };
            let Some(state) = state else {
                let expected = if opt == "pipeline" {
                    "'disable'".to_owned()
                } else {
                    format!(
                        "'enable'{}{} or 'disable'",
                        if full { ", 'full'" } else { "" },
                        if assume_safety { ", 'assume_safety'" } else { "" }
                    )
                };
                let d = self.s.diag(DiagId::parse_pragma_invalid_keyword, first.span.lo);
                self.emit(d.arg(expected));
                return None;
            };
            if toks.len() > 1 {
                self.extra_tokens(&what, next);
            }
            return Some(self.s.loop_hint_attr(mk(option, state), h.name_loc, None));
        }
        if option == O::VectorizeWidth {
            return self.vectorize_width(h.name_loc, h.value_end, toks, &what, mk);
        }
        let e = self.hint_expr(toks, h.value_end, &what, |_, e| e)?;
        self.s.check_loop_hint_expr(e, true)?;
        Some(self.s.loop_hint_attr(mk(option, St::Numeric), h.name_loc, Some(e)))
    }

    /// `vectorize_width(N)`, `(N, fixed|scalable)` or `(fixed|scalable)`.
    fn vectorize_width(
        &mut self,
        name_loc: Loc,
        end: Loc,
        toks: Vec<Token>,
        what: &str,
        mk: impl Fn(O, St) -> LoopHint,
    ) -> Option<AttrId> {
        let first = toks[0];
        let width_state = |w: Option<&str>| match w {
            Some("scalable") => Some(St::ScalableWidth),
            Some("fixed") => Some(St::FixedWidth),
            _ => None,
        };
        if let Some(state) = width_state(self.word_of(Some(&first)).as_deref()) {
            if let Some(t) = toks.get(1) {
                self.extra_tokens(what, t.span.lo);
            }
            return Some(self.s.loop_hint_attr(mk(O::VectorizeWidth, state), name_loc, None));
        }
        let (e, state, arg2_error) = self.hint_expr(toks, end, what, |p, e| {
            if e.is_none() && !p.at(Punct::Comma) {
                p.emit(p.s.diag(DiagId::note_pragma_loop_invalid_vectorize_option, first.span.lo));
            }
            let mut state = St::FixedWidth;
            let mut arg2_error = false;
            if p.at(Punct::Comma) {
                p.bump();
                match width_state(p.word_of(Some(&p.tok())).as_deref()) {
                    Some(s) => state = s,
                    None => {
                        let at = p.loc();
                        p.emit(p.s.diag(DiagId::parse_pragma_loop_invalid_vectorize_option, at));
                        arg2_error = true;
                    }
                }
                p.bump();
            }
            (e, state, arg2_error)
        });
        let e = e.filter(|_| !arg2_error)?;
        self.s.check_loop_hint_expr(e, false)?;
        Some(self.s.loop_hint_attr(mk(O::VectorizeWidth, state), name_loc, Some(e)))
    }
}
