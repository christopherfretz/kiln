//! Macro invocation ([cpp.replace]): argument collection ([cpp.invoke]),
//! substitution ([cpp.subst]), `#` ([cpp.stringize]), `##`
//! ([cpp.concat]), `__VA_OPT__` ([cpp.subst]/3, P1042R1) and the builtin
//! macros ([cpp.predefined]). A port of clang's `TokenLexer`,
//! `MacroArgs` and `PPMacroExpansion`.

use std::cell::Cell;
use std::rc::Rc;

use kiln_diag::{Diag, DiagId};
use kiln_lex::{Encoding, Lexer, Punct, Symbol, TokFlags, TokKind, Token};
use kiln_source::{ExpansionKind, Loc, Span};

use crate::builtin::{Builtin, TargetQuery};
use crate::macros::MacroDef;
use crate::preprocessor::{Def, Frame, MacroFrame, Preprocessor, eof_tok};
use crate::tok::{PpFlags, PpTok};

/// The actual arguments of one invocation (clang `MacroArgs`).
pub(crate) struct Args {
    /// All argument tokens; each argument ends with an `Eof`.
    pub(crate) toks: Vec<PpTok>,
    /// Start index of each argument, plus one past the end.
    pub(crate) bounds: Vec<usize>,
    /// Pre-expanded arguments (with their terminating `Eof`), by index.
    pub(crate) pre: Vec<Option<Vec<PpTok>>>,
    /// The variadic argument was omitted entirely (`F(a)` for
    /// `F(x, ...)`).
    pub(crate) elided: bool,
    /// Cached `invokedWithVariadicArgument`.
    pub(crate) va_nonempty: Option<bool>,
}

impl Args {
    /// The unexpanded tokens of argument `i` (without the `Eof`).
    pub(crate) fn raw(&self, i: usize) -> &[PpTok] {
        &self.toks[self.bounds[i]..self.bounds[i + 1] - 1]
    }

    /// The `Eof` ending argument `i`.
    fn eof(&self, i: usize) -> PpTok {
        self.toks[self.bounds[i + 1] - 1]
    }
}

/// Is `t` the `__VA_OPT__` identifier?
fn is_va_opt(t: &PpTok, va_opt: Symbol) -> bool {
    t.ident() == Some(va_opt)
}

/// Where the tokens being pasted live (the expansion being built).
pub(crate) struct PasteCx {
    pub(crate) exp_base: Option<Loc>,
    pub(crate) exp: Span,
    pub(crate) name_span: Span,
}

/// clang `VAOptExpansionContext`.
#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
struct VaOpt {
    /// Unmatched `(` of the `__VA_OPT__` (empty: not in one).
    parens: u32,
    /// Location of the `__VA_OPT__` token (mapped).
    loc: Option<Loc>,
    prior: usize,
    stringify: bool,
    charify: bool,
    ls_for_stringified: bool,
    begins_placeholder: bool,
    ends_placeholder: bool,
}

impl VaOpt {
    fn in_va_opt(&self) -> bool {
        self.parens > 0
    }
    fn reset(&mut self) {
        *self = VaOpt::default();
    }
    /// A `)`: returns true if it closes the `__VA_OPT__`.
    fn saw_closing_paren(&mut self) -> bool {
        if self.parens > 0 {
            self.parens -= 1;
            return self.parens == 0;
        }
        false
    }
    fn placeholder_before_rparen(&mut self) {
        if self.parens == 1 {
            self.ends_placeholder = true;
        }
    }
}

impl Preprocessor<'_, '_> {
    /// `SourceManager::add_expansion`, tolerating invalid inputs (falls
    /// back to the spelling location).
    pub(crate) fn add_exp(&self, spelling: Loc, len: u32, exp: Span, kind: ExpansionKind) -> Loc {
        if !spelling.is_valid() {
            return spelling;
        }
        let exp = if exp.is_valid() { exp } else { Span::new(spelling, spelling) };
        let kind = match kind {
            ExpansionKind::Macro { name } if !name.is_valid() => ExpansionKind::Macro { name: exp },
            k => k,
        };
        self.sm.add_expansion(spelling, len, exp, kind).unwrap_or(spelling)
    }

    /// The length of a token in bytes.
    pub(crate) fn tok_len(&self, t: &Token) -> u32 {
        self.loc_diff(t.span.lo, t.span.hi).unwrap_or(0)
    }

    /// Defines the builtin macros (clang `RegisterBuiltinMacros`).
    pub(crate) fn define_builtins(&mut self) {
        for &(n, b) in Builtin::ALL {
            let name = self.interner.intern(n);
            let def = MacroDef {
                name,
                name_span: Span::INVALID,
                params: Box::new([]),
                function_like: false,
                variadic: false,
                named_variadic: false,
                body: Box::new([]),
                rel: Box::new([]),
                def_start: Loc::INVALID,
                def_len: 0,
                builtin: Some(b),
                enabled: Cell::new(true),
                used: Cell::new(false),
            };
            self.macros.map.insert(name, Rc::new(def));
        }
    }

    /// Emits `pp_disabled_macro_expansion` if enabled.
    fn diag_disabled(&self, span: Span) {
        if self.enabled(DiagId::pp_disabled_macro_expansion) {
            self.emit(Diag::new(DiagId::pp_disabled_macro_expansion, span));
        }
    }

    /// `note: macro 'X' defined here`, if the definition has a location.
    fn note_macro(&self, d: Diag, def: &MacroDef) -> Diag {
        if def.name_span.is_valid() {
            let n = Diag::new(DiagId::note_macro_here, def.name_span)
                .arg(self.interner.as_str(def.name));
            d.with_note(n)
        } else {
            d
        }
    }

    /// An identifier naming a defined macro was lexed (clang
    /// `HandleIdentifier` + `HandleMacroExpandedIdentifier`). Returns the
    /// token to return, or `None` to lex again (an expansion was entered).
    pub(crate) fn handle_identifier(&mut self, mut t: PpTok) -> Option<PpTok> {
        let s = t.ident()?;
        let def: Def = Rc::clone(self.macros.get_rc(s)?);
        if t.pf.contains(PpFlags::NO_EXPAND) {
            return Some(t);
        }
        if !def.enabled.get() {
            t.pf.insert(PpFlags::NO_EXPAND);
            if self.enabled(DiagId::pp_disabled_macro_expansion)
                && (!def.function_like || self.peek_lparen())
            {
                self.diag_disabled(t.tok.span);
            }
            return Some(t);
        }
        if def.function_like && !self.peek_lparen() {
            return Some(t);
        }
        self.expand_macro(t, &def)
    }

    /// Expands an enabled macro invocation whose name is `t` (clang
    /// `HandleMacroExpandedIdentifier`).
    fn expand_macro(&mut self, t: PpTok, def: &Def) -> Option<PpTok> {
        if let Some(b) = def.builtin {
            self.callbacks.macro_expands(&t.tok, def, t.tok.span);
            return self.expand_builtin(t, b);
        }
        let mut args = None;
        let mut end = t.tok.span.hi;
        if def.function_like {
            let old = self.in_args.replace((def.name, t.tok.span));
            let r = self.read_args(&t, def);
            self.in_args = old;
            match r {
                Ok((a, rparen)) => {
                    args = Some(a);
                    end = rparen.hi;
                }
                Err(None) => return Some(t),
                Err(Some(eof)) => return Some(eof),
            }
        }
        def.used.set(true);
        let range = Span::new(t.tok.span.lo, end);
        self.callbacks.macro_expands(&t.tok, def, range);
        let sol = t.has(TokFlags::START_OF_LINE);
        let ls = t.has(TokFlags::LEADING_SPACE);
        if def.body.is_empty() {
            self.free_args(args);
            self.propagate(sol, ls);
            return None;
        }
        if def.body.len() == 1 && self.is_trivial(def) {
            self.free_args(args);
            let mut r = def.body[0];
            let (lo, hi) = def.rel[0];
            let loc = self.add_exp(
                r.tok.span.lo,
                hi - lo,
                range,
                ExpansionKind::Macro { name: t.tok.span },
            );
            r.tok.span = Span::new(loc, loc.with_offset(hi - lo));
            r.set_flag(TokFlags::START_OF_LINE, sol);
            r.set_flag(TokFlags::LEADING_SPACE, ls);
            if let Some(s) = r.ident()
                && let Some(nd) = self.macros.get_rc(s)
                && (!nd.enabled.get() || Rc::ptr_eq(nd, def))
            {
                r.pf.insert(PpFlags::NO_EXPAND);
                if !Rc::ptr_eq(nd, def) || def.function_like {
                    self.diag_disabled(r.tok.span);
                }
            }
            return Some(r);
        }
        let base = self.add_exp(
            def.def_start,
            def.def_len,
            range,
            ExpansionKind::Macro { name: t.tok.span },
        );
        let exp_base = (base.is_valid() && base != def.def_start).then_some(base);
        let mut toks = self.take_buf();
        toks.extend(def.body.iter().zip(def.rel.iter()).map(|(b, &(lo, hi))| {
            let mut b = *b;
            if let Some(base) = exp_base {
                b.tok.span = Span::new(base.with_offset(lo), base.with_offset(hi));
            }
            b
        }));
        let mut next_gets_space = false;
        if let Some(mut a) = args {
            if !def.params.is_empty() {
                let cx = PasteCx { exp_base, exp: range, name_span: t.tok.span };
                let mut out = self.take_buf();
                next_gets_space = self.expand_args(&toks, &mut out, def, &mut a, &cx);
                self.give_buf(std::mem::replace(&mut toks, out));
            }
            self.free_args(Some(a));
        }
        def.enabled.set(false);
        self.frames.push(Frame::Macro(MacroFrame {
            toks,
            idx: 0,
            def: Some(Rc::clone(def)),
            at_sol: sol,
            has_ls: ls,
            next_gets_space,
            exp_base,
            exp: range,
            name_span: t.tok.span,
            no_expand: false,
        }));
        None
    }

    /// clang `isTrivialSingleTokenExpansion`.
    fn is_trivial(&self, def: &MacroDef) -> bool {
        let Some(s) = def.body[0].ident() else { return true };
        if let Some(nd) = self.macros.get(s)
            && nd.enabled.get()
            && s != def.name
        {
            return false;
        }
        !def.function_like || def.param_index(s).is_none()
    }

    /// Returns argument buffers to the pool.
    fn free_args(&mut self, args: Option<Args>) {
        if let Some(a) = args {
            self.give_buf(a.toks);
            for p in a.pre.into_iter().flatten() {
                self.give_buf(p);
            }
        }
    }

    /// Reads the arguments of a function-like macro invocation; the next
    /// token is the `(` (clang `ReadMacroCallArgumentList`). Returns the
    /// arguments and the `)`, or the token to return instead (`None`: the
    /// macro name).
    fn read_args(&mut self, name: &PpTok, def: &MacroDef) -> Result<(Args, Span), Option<PpTok>> {
        let nparams = def.params.len();
        let mut left = nparams;
        let mut tok = self.lex_unexpanded();
        while tok.kind() == TokKind::Comment {
            tok = self.lex_unexpanded();
        }
        let mut toks = self.take_buf();
        let mut bounds = Vec::with_capacity(nparams + 1);
        let mut found_elided_comma = false;
        let mut too_many_at: Option<Span> = None;
        let mut actuals = 0usize;
        while !tok.is_punct(Punct::RParen) {
            let arg_start = toks.len();
            let arg_start_span = tok.tok.span;
            let mut parens = 0u32;
            loop {
                tok = self.lex_unexpanded();
                match tok.kind() {
                    TokKind::Eof | TokKind::Eod => {
                        let d = Diag::new(DiagId::pp_unterminated_macro_args, name.tok.span);
                        self.emit(self.note_macro(d, def));
                        self.give_buf(toks);
                        return Err(Some(tok));
                    }
                    TokKind::Punct(Punct::RParen) => {
                        if parens == 0 {
                            if toks
                                .last()
                                .is_some_and(|t| t.pf.contains(PpFlags::COMMA_AFTER_ELIDED))
                            {
                                found_elided_comma = true;
                            }
                            break;
                        }
                        parens -= 1;
                    }
                    TokKind::Punct(Punct::LParen) => parens += 1,
                    TokKind::Punct(Punct::Comma) => {
                        if parens == 0 && (!def.variadic || left > 1) {
                            break;
                        }
                    }
                    TokKind::Comment if !self.opts.keep_macro_comments => continue,
                    TokKind::Identifier(s)
                        if self.macros.get(s).is_some_and(|m| !m.enabled.get()) =>
                    {
                        tok.pf.insert(PpFlags::NO_EXPAND);
                    }
                    _ => {}
                }
                toks.push(tok);
            }
            if toks.is_empty() && tok.is_punct(Punct::RParen) {
                break;
            }
            if !def.variadic && left == 0 && too_many_at.is_none() {
                too_many_at = Some(toks.get(arg_start).map_or(arg_start_span, |t| t.tok.span));
            }
            bounds.push(arg_start);
            toks.push(PpTok::new(eof_tok(Span::empty(tok.tok.span.lo))));
            actuals += 1;
            left = left.saturating_sub(1);
        }
        let mismatch = |pp: &Self, many: u32, at: Span| {
            let d = Diag::new(DiagId::pp_macro_args_mismatch, at).arg(many);
            pp.emit(pp.note_macro(d, def));
        };
        if !def.variadic && actuals > nparams {
            mismatch(self, 1, too_many_at.unwrap_or(name.tok.span));
            self.give_buf(toks);
            return Err(None);
        }
        let mut elided = false;
        if actuals < nparams {
            if actuals == 0 && nparams == 1 {
                elided = def.variadic;
            } else if (found_elided_comma || def.variadic)
                && (actuals + 1 == nparams || (actuals == 0 && nparams == 2))
            {
                elided = true;
            } else {
                mismatch(self, 0, tok.tok.span);
                self.give_buf(toks);
                return Err(None);
            }
            let e = PpTok::new(eof_tok(Span::empty(tok.tok.span.lo)));
            bounds.push(toks.len());
            toks.push(e);
            if actuals == 0 && nparams == 2 {
                bounds.push(toks.len());
                toks.push(e);
            }
        }
        bounds.push(toks.len());
        let pre = (0..nparams).map(|_| None).collect();
        Ok((Args { toks, bounds, pre, elided, va_nonempty: None }, tok.tok.span))
    }

    /// Does the argument contain an identifier naming a macro (clang
    /// `ArgNeedsPreexpansion`)?
    fn needs_preexpansion(&self, toks: &[PpTok]) -> bool {
        toks.iter().any(|t| t.ident().is_some_and(|s| self.macros.is_defined(s)))
    }

    /// Fully macro-expands argument `i` ([cpp.subst]/1; clang
    /// `getPreExpArgument`), caching the result (ending with `Eof`).
    fn pre_expand(&mut self, a: &mut Args, i: usize) {
        if a.pre[i].is_some() {
            return;
        }
        let mut buf = self.take_buf();
        buf.extend_from_slice(a.raw(i));
        buf.push(a.eof(i));
        let depth = self.frames.len();
        self.push_stream(buf, false);
        self.arg_preexpand += 1;
        let mut out = self.take_buf();
        loop {
            let t = self.lex();
            out.push(t);
            if t.is_eof() {
                break;
            }
        }
        self.arg_preexpand -= 1;
        while self.frames.len() > depth {
            match self.frames.pop() {
                Some(Frame::Macro(m)) => {
                    if let Some(d) = &m.def {
                        d.enabled.set(true);
                    }
                    self.give_buf(m.toks);
                }
                Some(f @ Frame::File(_)) => {
                    self.frames.push(f);
                    break;
                }
                None => break,
            }
        }
        a.pre[i] = Some(out);
    }

    /// clang `invokedWithVariadicArgument`: the variadic argument is
    /// non-empty after pre-expansion.
    fn invoked_with_va(&mut self, def: &MacroDef, a: &mut Args) -> bool {
        if !def.variadic {
            return false;
        }
        if let Some(v) = a.va_nonempty {
            return v;
        }
        let i = def.params.len() - 1;
        self.pre_expand(a, i);
        let v = a.pre[i].as_ref().is_some_and(|p| p.first().is_some_and(|t| !t.is_eof()));
        a.va_nonempty = Some(v);
        v
    }

    /// GNU `, ## __VA_ARGS__` with an omitted variadic argument (clang
    /// `MaybeRemoveCommaBeforeVaArgs`, C++ / paste-operator path).
    fn remove_comma_before_va(
        &self,
        out: &mut Vec<PpTok>,
        def: &MacroDef,
        argno: usize,
        ngs: &mut bool,
    ) -> bool {
        if !def.variadic || argno + 1 != def.params.len() {
            return false;
        }
        let Some(c) = out.last().filter(|t| t.is_punct(Punct::Comma)).copied() else {
            return false;
        };
        self.emit(Diag::new(DiagId::pp_paste_comma, c.tok.span));
        out.pop();
        if out.last().is_some_and(PpTok::is_paste_op) {
            out.pop();
        }
        if let Some(b) = out.last_mut() {
            b.pf.insert(PpFlags::COMMA_AFTER_ELIDED);
        }
        *ngs = false;
        true
    }

    /// Appends argument tokens `src` for the parameter token `param`:
    /// `##` loses its operator meaning and the tokens are given macro-arg
    /// locations. Returns the index of the first appended token.
    fn append_arg(&self, out: &mut Vec<PpTok>, src: &[PpTok], param: Span) -> usize {
        let first = out.len();
        out.extend_from_slice(src);
        for t in &mut out[first..] {
            if t.is_punct(Punct::HashHash) {
                t.pf.insert(PpFlags::NOT_PASTE);
            }
        }
        self.relocate(param, &mut out[first..]);
        first
    }

    /// Substitutes the arguments into the replacement list `body` (whose
    /// spans are already in the expansion), writing the result to `out`
    /// (clang `ExpandFunctionArguments`). Returns `NextTokGetsSpace`.
    #[allow(clippy::too_many_lines, clippy::many_single_char_names)]
    fn expand_args(
        &mut self,
        body: &[PpTok],
        out: &mut Vec<PpTok>,
        def: &MacroDef,
        a: &mut Args,
        cx: &PasteCx,
    ) -> bool {
        let va_opt = self.syms.va_opt;
        let mut ngs = false;
        let mut vc = VaOpt::default();
        let e = body.len();
        let mut i = 0;
        while i < e {
            let cur = body[i];
            if i != 0 && !body[i - 1].is_paste_op() && cur.has(TokFlags::LEADING_SPACE) {
                ngs = true;
            }
            if def.variadic
                && is_va_opt(&cur, va_opt)
                && !vc.in_va_opt()
                && body.get(i + 1).is_some_and(|t| t.is_punct(Punct::LParen))
            {
                let (s, c, ls) = (vc.stringify, vc.charify, vc.ls_for_stringified);
                vc.reset();
                (vc.stringify, vc.charify, vc.ls_for_stringified) = (s, c, ls);
                vc.parens = 1;
                vc.loc = Some(cur.tok.span.lo);
                vc.prior = out.len();
                i += 2;
                continue;
            }
            if vc.in_va_opt() {
                if cur.is_punct(Punct::LParen) {
                    vc.parens += 1;
                }
                if !cur.is_punct(Punct::RParen) || !vc.saw_closing_paren() {
                    if !self.invoked_with_va(def, a) {
                        i += 1;
                        continue;
                    }
                } else {
                    if vc.stringify || vc.charify {
                        self.stringify_va_opt(out, &vc, cur.tok.span, cx);
                    } else if out.len() == vc.prior {
                        if out.last().is_some_and(PpTok::is_paste_op) {
                            out.pop();
                        } else if i + 1 != e && body[i + 1].is_paste_op() {
                            i += 1;
                        }
                    } else {
                        if vc.begins_placeholder && vc.prior > 0 {
                            out.remove(vc.prior - 1);
                        }
                        if vc.ends_placeholder && i + 1 != e && body[i + 1].is_paste_op() {
                            i += 1;
                        }
                    }
                    vc.reset();
                    i += 1;
                    continue;
                }
            }
            if cur.is_punct(Punct::Hash) && i + 1 < e {
                let next = body[i + 1];
                let argno = next.ident().and_then(|s| def.param_index(s));
                if argno.is_none() && def.variadic && is_va_opt(&next, va_opt) {
                    vc.stringify = true;
                    vc.charify = false;
                    vc.ls_for_stringified = ngs;
                    i += 1;
                    continue;
                }
                if let Some(argno) = argno {
                    let span = Span::new(cur.tok.span.lo, next.tok.span.hi);
                    let mut r = self.stringify(a.raw(argno), false, span, cx.name_span);
                    r.pf.insert(PpFlags::STRINGIFIED);
                    if ngs {
                        r.tok.flags.insert(TokFlags::LEADING_SPACE);
                    }
                    out.push(r);
                    i += 2;
                    ngs = false;
                    continue;
                }
            }
            let nonempty_paste_before = out.last().is_some_and(PpTok::is_paste_op);
            let paste_before = i != 0 && body[i - 1].is_paste_op();
            let paste_after = i + 1 != e && body[i + 1].is_paste_op();
            let rparen_after = i + 1 != e && body[i + 1].is_punct(Punct::RParen);
            let Some(argno) = cur.ident().and_then(|s| def.param_index(s)) else {
                let mut c = cur;
                if ngs {
                    c.tok.flags.insert(TokFlags::LEADING_SPACE);
                    ngs = false;
                } else if paste_before && !nonempty_paste_before {
                    c.tok.flags.remove(TokFlags::LEADING_SPACE);
                }
                out.push(c);
                i += 1;
                continue;
            };
            if !paste_before && !paste_after {
                let use_pre = self.needs_preexpansion(a.raw(argno));
                if use_pre {
                    self.pre_expand(a, argno);
                }
                let src: &[PpTok] = match &a.pre[argno] {
                    Some(p) if use_pre => &p[..p.len() - 1],
                    _ => a.raw(argno),
                };
                if src.is_empty() {
                    if nonempty_paste_before {
                        vc.begins_placeholder = true;
                    } else if rparen_after {
                        vc.placeholder_before_rparen();
                    }
                } else {
                    let first = self.append_arg(out, src, cur.tok.span);
                    out[first].set_flag(TokFlags::LEADING_SPACE, ngs);
                    out[first].set_flag(TokFlags::START_OF_LINE, false);
                    ngs = false;
                }
                i += 1;
                continue;
            }
            let raw = a.raw(argno);
            if !raw.is_empty() {
                let mut pseudo = false;
                if nonempty_paste_before
                    && out.len() >= 2
                    && out[out.len() - 2].is_punct(Punct::Comma)
                    && argno + 1 == def.params.len()
                    && def.variadic
                {
                    pseudo = true;
                    if let Some(op) = out.pop() {
                        self.emit(Diag::new(DiagId::pp_paste_comma, op.tok.span));
                    }
                }
                let first = self.append_arg(out, raw, cur.tok.span);
                if !pseudo {
                    out[first].set_flag(TokFlags::START_OF_LINE, false);
                    out[first].set_flag(TokFlags::LEADING_SPACE, ngs);
                }
                ngs = false;
                i += 1;
                continue;
            }
            if paste_after {
                i += 2;
                continue;
            }
            if rparen_after && !nonempty_paste_before {
                vc.placeholder_before_rparen();
            }
            if nonempty_paste_before {
                if !vc.in_va_opt() || out.len() > vc.prior {
                    out.pop();
                } else {
                    vc.begins_placeholder = true;
                }
            }
            if a.elided {
                self.remove_comma_before_va(out, def, argno, &mut ngs);
            }
            i += 1;
        }
        ngs
    }

    /// `#__VA_OPT__(...)` (clang `stringifyVAOPTContents`): pastes the
    /// contents, then replaces them with their stringification.
    fn stringify_va_opt(&mut self, out: &mut Vec<PpTok>, vc: &VaOpt, close: Span, cx: &PasteCx) {
        let prior = vc.prior.min(out.len());
        let toks: Vec<PpTok> = out[prior..].to_vec();
        let n = toks.len();
        let mut cat = self.take_buf();
        let mut k = 0;
        while k < n {
            if toks[k].is_paste_op() && k > 0 {
                let lhs = toks[k - 1];
                let r = self.paste(&toks, &mut k, lhs, cx);
                if let Some(b) = cat.last_mut() {
                    *b = r;
                }
                if k == n {
                    break;
                }
            }
            cat.push(toks[k]);
            k += 1;
        }
        let lo = vc.loc.unwrap_or(close.lo);
        let mut r = self.stringify(&cat, vc.charify, Span::new(lo, close.hi), cx.name_span);
        self.give_buf(cat);
        if vc.ls_for_stringified {
            r.tok.flags.insert(TokFlags::LEADING_SPACE);
        }
        r.pf.insert(PpFlags::STRINGIFIED);
        out.truncate(prior);
        out.push(r);
    }

    /// Gives argument tokens macro-arg locations expanded at the parameter
    /// token `param` (clang `updateLocForMacroArgTokens`): runs of nearby
    /// tokens of one entry share one source-manager entry.
    fn relocate(&self, param: Span, toks: &mut [PpTok]) {
        let mut b = 0;
        while b < toks.len() {
            let begin = toks[b].tok.span.lo;
            if !begin.is_valid() {
                b += 1;
                continue;
            }
            if toks.len() - b == 1 {
                let t = &mut toks[b];
                let len = self.tok_len(&t.tok);
                let l = self.add_exp(begin, len, param, ExpansionKind::MacroArg);
                t.tok.span = Span::new(l, l.with_offset(len));
                return;
            }
            let entry = self.entry_of(begin);
            let mut end = b + 1;
            let mut last = begin;
            let mut total = 0u32;
            while end < toks.len() {
                let l = toks[end].tok.span.lo;
                if !l.is_valid() || self.entry_of(l) != entry {
                    break;
                }
                match self.loc_diff(last, l) {
                    Some(d) if d <= 50 => total += d,
                    _ => break,
                }
                last = l;
                end += 1;
            }
            let len = total + self.tok_len(&toks[end - 1].tok);
            let base = self.add_exp(begin, len, param, ExpansionKind::MacroArg);
            if base != begin {
                let mut off = 0u32;
                let mut prev = begin;
                for t in &mut toks[b..end] {
                    off += self.loc_diff(prev, t.tok.span.lo).unwrap_or(0);
                    prev = t.tok.span.lo;
                    let l = self.tok_len(&t.tok);
                    t.tok.span = Span::new(base.with_offset(off), base.with_offset(off + l));
                }
            }
            b = end;
        }
    }

    /// Walks `loc` up its expansion chain until it is in the entry of
    /// `target` (clang `getImmediateExpansionRange` loop in
    /// `pasteTokens`).
    fn walk_to_entry(&self, mut loc: Loc, target: Option<Loc>) -> Option<Loc> {
        let target = target?;
        loop {
            if self.entry_of(loc) == Some(target) {
                return Some(loc);
            }
            loc = self.sm.expansion_info(loc)?.expansion.lo;
        }
    }

    /// Pastes `lhs` with the tokens after the `##` at `s[*idx]`, repeatedly
    /// while another `##` follows (clang `pasteTokens`). On return `*idx`
    /// is the index of the first token not consumed.
    pub(crate) fn paste(
        &mut self,
        s: &[PpTok],
        idx: &mut usize,
        mut lhs: PpTok,
        cx: &PasteCx,
    ) -> PpTok {
        let start = lhs.tok.span.lo;
        let mut buf = std::mem::take(&mut self.paste_buf);
        loop {
            let op = s[*idx].tok.span;
            if s[*idx].is_paste_op() {
                *idx += 1;
            }
            let Some(rhs) = s.get(*idx).copied() else { break };
            buf.clear();
            buf.push_str(&self.spelling(&lhs.tok));
            buf.push_str(&self.spelling(&rhs.tok));
            let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);
            let mut res = None;
            if let Some((file, base)) = self.scratch_loc(&buf) {
                let span = Span::new(base, base.with_offset(len));
                if lhs.ident().is_some() && rhs.ident().is_some() && !buf.contains('\\') {
                    let kind = TokKind::Identifier(self.interner.intern(&buf));
                    res = Some(Token { kind, span, flags: TokFlags::empty() });
                } else {
                    let mut o = self.lex_opts;
                    o.keep_comments = false;
                    let mut lx = Lexer::new_in_range(self.sm, self.interner, file, 0..len, base, o);
                    let t = lx.next();
                    if t.kind != TokKind::Eof && lx.offset() == len {
                        res = Some(t);
                    }
                }
            }
            let Some(t) = res else {
                let l = self.add_exp(op.lo, 2, cx.exp, ExpansionKind::Macro { name: cx.name_span });
                self.emit(
                    Diag::new(DiagId::pp_paste_invalid_token, Span::new(l, l.with_offset(2)))
                        .arg(&buf),
                );
                break;
            };
            let mut r = PpTok::new(t);
            if r.is_punct(Punct::HashHash) {
                r.pf.insert(PpFlags::NOT_PASTE);
            }
            r.set_flag(TokFlags::START_OF_LINE, lhs.has(TokFlags::START_OF_LINE));
            r.set_flag(TokFlags::LEADING_SPACE, lhs.has(TokFlags::LEADING_SPACE));
            r.tok.flags.remove(TokFlags::NEEDS_CLEANING);
            *idx += 1;
            lhs = r;
            if !s.get(*idx).is_some_and(PpTok::is_paste_op) {
                break;
            }
        }
        self.paste_buf = buf;
        let end = s[(*idx).max(1) - 1].tok.span.lo;
        let target = cx.exp_base.and_then(|b| self.entry_of(b));
        let lo = self.walk_to_entry(start, target).unwrap_or(cx.exp.lo);
        let hi = self.walk_to_entry(end, target).unwrap_or(cx.exp.hi);
        let len = self.tok_len(&lhs.tok);
        let l = self.add_exp(
            lhs.tok.span.lo,
            len,
            Span::new(lo, hi),
            ExpansionKind::Macro { name: cx.name_span },
        );
        lhs.tok.span = Span::new(l, l.with_offset(len));
        lhs
    }

    /// Pastes the token `t` just read from the top macro frame with what
    /// follows (the frame's index is at the `##`).
    pub(crate) fn paste_in_frame(&mut self, t: PpTok) -> PpTok {
        let Some(Frame::Macro(m)) = self.frames.last_mut() else { return t };
        let toks = std::mem::take(&mut m.toks);
        let mut idx = m.idx;
        let cx = PasteCx { exp_base: m.exp_base, exp: m.exp, name_span: m.name_span };
        let r = self.paste(&toks, &mut idx, t, &cx);
        if let Some(Frame::Macro(m)) = self.frames.last_mut() {
            m.toks = toks;
            m.idx = idx;
        }
        r
    }

    /// `#` applied to `toks` (clang `MacroArgs::StringifyArgument`). The
    /// result is spelled in a scratch buffer and expanded at `span`.
    pub(crate) fn stringify(
        &mut self,
        toks: &[PpTok],
        charify: bool,
        span: Span,
        name: Span,
    ) -> PpTok {
        let quote = if charify { '\'' } else { '"' };
        let mut s = String::with_capacity(16);
        s.push(quote);
        for (k, t) in toks.iter().enumerate() {
            if t.is_eof() {
                break;
            }
            if k > 0 && (t.has(TokFlags::LEADING_SPACE) || t.has(TokFlags::START_OF_LINE)) {
                s.push(' ');
            }
            let sp = self.spelling(&t.tok);
            if matches!(
                t.kind(),
                TokKind::StringLit(_) | TokKind::RawStringLit(_) | TokKind::CharLit(_)
            ) {
                escape_into(&mut s, &sp);
            } else {
                s.push_str(&sp);
            }
        }
        if s.len() > 1 && s.ends_with('\\') {
            let n = s.len() - s.trim_end_matches('\\').len();
            if n % 2 == 1 {
                if let Some(last) = toks.iter().rev().find(|t| !t.is_eof()) {
                    self.emit(Diag::new(DiagId::pp_invalid_string_literal, last.tok.span));
                }
                s.pop();
            }
        }
        s.push(quote);
        let kind = if charify {
            TokKind::CharLit(Encoding::Ordinary)
        } else {
            TokKind::StringLit(Encoding::Ordinary)
        };
        PpTok::new(self.scratch_tok(&s, kind, span, name))
    }

    /// A token spelled `text` in a scratch buffer, expanded at `exp`.
    pub(crate) fn scratch_tok(
        &mut self,
        text: &str,
        kind: TokKind,
        exp: Span,
        name: Span,
    ) -> Token {
        let len = u32::try_from(text.len()).unwrap_or(u32::MAX);
        let span = match self.scratch_loc(text) {
            Some((_, l)) => {
                let l = self.add_exp(l, len, exp, ExpansionKind::Macro { name });
                Span::new(l, l.with_offset(len))
            }
            None => exp,
        };
        Token { kind, span, flags: TokFlags::empty() }
    }

    /// The value of concatenated ordinary string literals (lossy UTF-8),
    /// or `None` if one is malformed.
    pub(crate) fn string_value(&self, toks: &[Token]) -> Option<String> {
        let mut bytes = Vec::new();
        for t in toks {
            let sp = self.spelling(t);
            let lit =
                kiln_lex::literal::decode_string(&sp, kiln_lex::literal::LiteralOptions::default());
            if lit.has_error {
                return None;
            }
            bytes.extend(lit.units.iter().map(|u| u8::try_from(u).unwrap_or(b'?')));
        }
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// Appends `s` with `\` and `"` escaped and newlines turned into `\n`
/// (clang `Lexer::Stringify`).
pub(crate) fn escape_into(out: &mut String, s: &str) {
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\\' | '"' => {
                out.push('\\');
                out.push(c);
            }
            '\n' | '\r' => {
                if let Some(&n) = it.peek()
                    && (n == '\n' || n == '\r')
                    && n != c
                {
                    it.next();
                }
                out.push_str("\\n");
            }
            _ => out.push(c),
        }
    }
}

/// What the operand of a feature-like builtin evaluated to.
struct OpResult {
    value: u32,
    /// The token after the operand was already lexed (clang
    /// `HasLexedNextToken`).
    next: Option<PpTok>,
}

impl Preprocessor<'_, '_> {
    /// Expands builtin macro `b` named by `t` (clang `ExpandBuiltinMacro`).
    pub(crate) fn expand_builtin(&mut self, t: PpTok, b: Builtin) -> Option<PpTok> {
        use std::fmt::Write as _;
        let name_span = t.tok.span;
        let mut text = String::new();
        let mut kind = TokKind::PpNumber;
        // The token whose location the result takes (clang reuses `Tok`).
        let mut at = t.tok.span;
        match b {
            Builtin::Pragma => return self.handle_pragma_op(t),
            Builtin::Line => {
                // clang `AdvanceToTokenCharacter(Loc, 0)`: a token spelled
                // after a line splice is on the line after it.
                let mut lo = t.tok.span.lo;
                if let Some((f, mut off)) = self.sm.offset_of(lo) {
                    let b = self.sm.text(f).as_bytes();
                    while b.get(off as usize) == Some(&b'\\') {
                        let mut i = off as usize + 1;
                        while matches!(b.get(i), Some(b' ' | b'\t' | b'\x0b' | b'\x0c')) {
                            i += 1;
                        }
                        match b.get(i) {
                            Some(b'\r') if b.get(i + 1) == Some(&b'\n') => i += 2,
                            Some(b'\n' | b'\r') => i += 1,
                            _ => break,
                        }
                        off = u32::try_from(i).unwrap_or(off);
                    }
                    lo = self.sm.loc(f, off);
                }
                let hi = self.sm.expansion_span(lo).hi;
                let line = self.sm.presumed_line_col(hi).map_or(1, |p| p.line);
                let _ = write!(text, "{line}");
            }
            Builtin::File | Builtin::BaseFile | Builtin::FileName => {
                kind = TokKind::StringLit(Encoding::Ordinary);
                let name: Option<String> = if b == Builtin::BaseFile {
                    self.main_file.map(|f| self.sm.file_name(f).to_owned())
                } else {
                    self.sm.presumed_line_col(t.tok.span.lo).map(|p| p.file_name.to_owned())
                };
                if let Some(mut n) = name {
                    if b == Builtin::FileName
                        && let Some(i) = n.rfind('/')
                    {
                        n.drain(..=i);
                    }
                    text.push('"');
                    escape_into(&mut text, &n);
                    text.push('"');
                }
            }
            Builtin::Date | Builtin::Time => {
                self.emit(Diag::new(DiagId::pp_date_time, t.tok.span));
                kind = TokKind::StringLit(Encoding::Ordinary);
                let (d, tm) = self.date_time_strings();
                text.push('"');
                text.push_str(if b == Builtin::Date { &d } else { &tm });
                text.push('"');
            }
            Builtin::Timestamp => {
                self.emit(Diag::new(DiagId::pp_date_time, t.tok.span));
                kind = TokKind::StringLit(Encoding::Ordinary);
                let ts = self.opts.timestamp.clone();
                let _ = write!(text, "\"{}\"", ts.as_deref().unwrap_or("??? ??? ?? ??:??:?? ????"));
            }
            Builtin::IncludeLevel => {
                let _ = write!(text, "{}", self.include_depth);
            }
            Builtin::Counter => {
                let _ = write!(text, "{}", self.counter);
                self.counter = self.counter.wrapping_add(1);
            }
            Builtin::FltEvalMethod => {
                let _ = write!(text, "{}", self.features.flt_eval_method());
            }
            Builtin::HasInclude | Builtin::HasIncludeNext => {
                match self.eval_has_include(t, b == Builtin::HasIncludeNext) {
                    Ok((v, rparen)) => {
                        let _ = write!(text, "{}", u32::from(v));
                        at = rparen;
                    }
                    Err(r) => return Some(r),
                }
            }
            Builtin::HasEmbed => match self.eval_has_embed(t) {
                Ok((v, rparen)) => {
                    let _ = write!(text, "{v}");
                    at = rparen;
                }
                Err(r) => return Some(r),
            },
            _ => {
                let expand = matches!(
                    b,
                    Builtin::HasAttribute
                        | Builtin::HasCppAttribute
                        | Builtin::HasDeclspecAttribute
                );
                match self.eval_feature_like(&t, b, expand) {
                    Ok((v, rparen)) => {
                        let _ = write!(text, "{v}");
                        if v > 1 {
                            text.push('L');
                        }
                        at = rparen;
                    }
                    Err(r) => return Some(r),
                }
            }
        }
        let mut r = PpTok::new(self.scratch_tok(&text, kind, at, name_span));
        r.set_flag(TokFlags::START_OF_LINE, t.has(TokFlags::START_OF_LINE));
        r.set_flag(TokFlags::LEADING_SPACE, t.has(TokFlags::LEADING_SPACE));
        Some(r)
    }

    /// `__DATE__` / `__TIME__` spellings, computed once.
    fn date_time_strings(&mut self) -> (String, String) {
        if self.date_time.is_none() {
            let dt = self.opts.reproducible_time.clone().unwrap_or_else(|| {
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
                crate::builtin::date_time(secs)
            });
            self.date_time = Some(dt);
        }
        self.date_time.clone().unwrap_or_default()
    }

    /// clang `EvaluateFeatureLikeBuiltinMacro`: parses `( operand )` after
    /// `t`. `Ok((value, span of the closing token))`, or `Err(token)` to
    /// return instead (the `Eof` / `Eod` that ended the invocation).
    fn eval_feature_like(
        &mut self,
        t: &PpTok,
        b: Builtin,
        expand: bool,
    ) -> Result<(u32, Span), PpTok> {
        let name = b.name();
        let lp = self.lex_unexpanded();
        if !lp.is_punct(Punct::LParen) {
            self.emit(
                Diag::new(DiagId::pp_expected_after, lp.tok.span)
                    .arg(format!("'{name}'"))
                    .arg("'('"),
            );
            if lp.is_eod_or_eof() {
                return Err(lp);
            }
            return Ok((0, lp.tok.span));
        }
        let _ = t;
        let mut depth = 1u32;
        let mut result: Option<(u32, PpTok)> = None;
        let mut quiet = false;
        let mut pending: Option<PpTok> = None;
        loop {
            let tok = match pending.take() {
                Some(p) => p,
                None if expand => self.lex(),
                None => self.lex_unexpanded(),
            };
            if tok.is_eod_or_eof() {
                self.emit(Diag::new(DiagId::pp_unterminated_macro_args, tok.tok.span));
                return Err(tok);
            }
            if tok.is_punct(Punct::Comma) {
                if !quiet {
                    self.emit(Diag::new(DiagId::pp_macro_args_mismatch, tok.tok.span).arg(1));
                    quiet = true;
                }
                continue;
            }
            if tok.is_punct(Punct::LParen) {
                depth += 1;
                if result.is_none() {
                    if !quiet {
                        self.emit(Diag::new(DiagId::pp_nested_paren, tok.tok.span).arg(name));
                        quiet = true;
                    }
                    continue;
                }
            } else if tok.is_punct(Punct::RParen) {
                depth -= 1;
                if depth > 0 {
                    continue;
                }
                return Ok(if let Some((v, _)) = result {
                    (v, tok.tok.span)
                } else {
                    if !quiet {
                        self.emit(Diag::new(DiagId::pp_macro_args_mismatch, tok.tok.span).arg(0));
                    }
                    (0, tok.tok.span)
                });
            } else if result.is_none() {
                let r = self.feature_op(b, tok);
                result = Some((r.value, tok));
                pending = r.next;
                continue;
            }
            // A token after the operand: diagnose the missing `)`.
            if !quiet {
                let last = result.map_or(tok, |(_, t)| t);
                let what = match last.ident() {
                    Some(s) => format!("'{}'", self.interner.as_str(s)),
                    None => format!("'{}'", self.spelling(&last.tok)),
                };
                self.emit(
                    Diag::new(DiagId::pp_expected_after, tok.tok.span)
                        .arg(what)
                        .arg("')'")
                        .range(last.tok.span)
                        .with_note(Diag::new(DiagId::note_matching, lp.tok.span).arg("'('")),
                );
                quiet = true;
            }
        }
    }

    /// The operand evaluation of one feature-like builtin, starting at
    /// `tok` (the lambdas passed to `EvaluateFeatureLikeBuiltinMacro`).
    fn feature_op(&mut self, b: Builtin, tok: PpTok) -> OpResult {
        let mut next = None;
        let value = match b {
            Builtin::IsIdentifier => {
                let id =
                    tok.ident().is_some_and(|s| !self.features.is_keyword(self.interner.as_str(s)));
                u32::from(id)
            }
            Builtin::HasWarning => {
                let (v, n) = self.has_warning(tok);
                next = n;
                u32::from(v)
            }
            Builtin::HasCppAttribute => {
                if let Some(first) = self.feature_ident(&tok) {
                    let colons = self.lex_unexpanded();
                    let (scope, attr) = if colons.is_punct(Punct::ColonColon) {
                        let n = self.lex();
                        (Some(first), self.feature_ident(&n))
                    } else {
                        next = Some(colons);
                        (None, Some(first))
                    };
                    match attr {
                        Some(a) => self.features.has_cpp_attribute(
                            scope.map(|s| self.interner.as_str(s)),
                            self.interner.as_str(a),
                        ),
                        None => 0,
                    }
                } else {
                    0
                }
            }
            _ => match self.feature_ident(&tok) {
                None => 0,
                Some(s) => {
                    let n = self.interner.as_str(s);
                    let f = &*self.features;
                    match b {
                        Builtin::HasFeature => u32::from(f.has_feature(n)),
                        Builtin::HasExtension => u32::from(f.has_extension(n)),
                        Builtin::HasBuiltin => f.has_builtin(n),
                        Builtin::HasConstexprBuiltin => u32::from(f.has_constexpr_builtin(n)),
                        Builtin::HasAttribute => f.has_attribute(n),
                        Builtin::HasDeclspecAttribute => u32::from(f.has_declspec_attribute(n)),
                        Builtin::IsTargetArch => u32::from(f.is_target(TargetQuery::Arch, n)),
                        Builtin::IsTargetVendor => u32::from(f.is_target(TargetQuery::Vendor, n)),
                        Builtin::IsTargetOs => u32::from(f.is_target(TargetQuery::Os, n)),
                        Builtin::IsTargetEnvironment => {
                            u32::from(f.is_target(TargetQuery::Environment, n))
                        }
                        Builtin::IsTargetVariantOs => {
                            u32::from(f.is_target(TargetQuery::VariantOs, n))
                        }
                        Builtin::IsTargetVariantEnvironment => {
                            u32::from(f.is_target(TargetQuery::VariantEnvironment, n))
                        }
                        _ => 0,
                    }
                }
            },
        };
        OpResult { value, next }
    }

    /// clang `ExpectFeatureIdentifierInfo`.
    fn feature_ident(&self, t: &PpTok) -> Option<Symbol> {
        let s = t.ident();
        if s.is_none() {
            self.emit(Diag::new(DiagId::pp_feature_check_malformed, t.tok.span));
        }
        s
    }

    /// `__has_warning("-Wname")`: `(value, token lexed after the
    /// strings)` (clang `FinishLexStringLiteral` without expansion).
    fn has_warning(&mut self, first: PpTok) -> (bool, Option<PpTok>) {
        let is_str = |t: &PpTok| {
            matches!(
                t.kind(),
                TokKind::StringLit(Encoding::Ordinary) | TokKind::RawStringLit(Encoding::Ordinary)
            )
        };
        if !is_str(&first) {
            self.emit(
                Diag::new(DiagId::pp_expected_string_literal, first.tok.span)
                    .arg("'__has_warning'"),
            );
            return (false, None);
        }
        let mut toks = Vec::new();
        let mut cur = first;
        while is_str(&cur) {
            if cur.has(TokFlags::UD_SUFFIX) {
                self.emit(Diag::new(DiagId::pp_string_udl, cur.tok.span));
            }
            toks.push(cur.tok);
            cur = self.lex_unexpanded();
        }
        let Some(name) = self.string_value(&toks) else { return (false, Some(cur)) };
        if name.len() < 3 || !name.starts_with("-W") {
            self.emit(Diag::new(DiagId::pp_has_warning_invalid_option, first.tok.span));
            return (false, Some(cur));
        }
        (kiln_diag::Group::is_known_name(&name[2..]), Some(cur))
    }
}
