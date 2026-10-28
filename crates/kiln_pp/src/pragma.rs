//! `#pragma` ([cpp.pragma]) and `_Pragma` ([cpp.pragma.op]): the pragmas
//! the core executes itself (clang's builtin pragma handlers), and the
//! pass-through of everything else as [`Marker::Pragma`].
//!
//! Both forms read the pragma's tokens through [`Preprocessor::lex`] /
//! [`Preprocessor::lex_unexpanded`] up to an `Eod`: a `#pragma` from the
//! directive line, a `_Pragma` from a token stream lexed out of the
//! destringized literal (one scratch buffer per distinct string, spelled
//! in an expansion of the `_Pragma ( ... )` range, like clang's pragma
//! lexer).

use kiln_diag::{Diag, DiagId, Group};
use kiln_lex::{Encoding, Lexer, Punct, TokFlags, TokKind, Token};
use kiln_source::{ExpansionKind, Span};

use crate::callbacks::{FileChange, PragmaInfo, PragmaIntroducer, PragmaMessageKind};
use crate::preprocessor::{Frame, Marker, Preprocessor};
use crate::tok::PpTok;

/// True for a token clang calls `tok::string_literal` (an ordinary,
/// non-raw string literal).
fn is_plain_string(t: &PpTok) -> bool {
    t.kind() == TokKind::StringLit(Encoding::Ordinary)
}

/// Any string literal (clang `tok::isStringLiteral`).
fn is_any_string(t: &PpTok) -> bool {
    matches!(t.kind(), TokKind::StringLit(_) | TokKind::RawStringLit(_))
}

/// Destringizes the spelling of a string literal ([cpp.pragma.op]/1;
/// clang `prepare_PragmaString`): drops the encoding prefix and the
/// quotes (and a raw string's delimiters), replaces `\\` and `\"` by the
/// character they escape, and puts a space in front (so the first token
/// has leading whitespace). The trailing newline is added by the scratch
/// buffer.
pub(crate) fn destringize(sp: &str) -> String {
    let mut s = sp;
    if let Some(r) = s.strip_prefix("u8") {
        s = r;
    } else if s.starts_with(['L', 'U', 'u']) {
        s = &s[1..];
    }
    let mut out = String::with_capacity(s.len());
    out.push(' ');
    if let Some(r) = s.strip_prefix('R') {
        // R"delim( ... )delim"
        let body = r.strip_prefix('"').and_then(|r| r.strip_suffix('"')).unwrap_or(r);
        let open = body.find('(').unwrap_or(0);
        let delim = &body[..open];
        let inner = body.get(open + 1..body.len().saturating_sub(delim.len() + 1)).unwrap_or("");
        out.push_str(inner);
        return out;
    }
    let inner = s.strip_prefix('"').and_then(|r| r.strip_suffix('"')).unwrap_or(s);
    let mut it = inner.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\'
            && let Some(&n) = it.peek()
            && (n == '\\' || n == '"')
        {
            out.push(n);
            it.next();
            continue;
        }
        out.push(c);
    }
    out
}

impl Preprocessor<'_, '_> {
    /// `_Pragma ( string-literal )` (clang `Handle_Pragma`); `t` is the
    /// `_Pragma` token. Returns the token to produce in its place: on a
    /// syntax error the token after the operand (already lexed with
    /// expansion, as clang does), during argument pre-expansion the
    /// `_Pragma` itself (the pragma is executed when the argument is
    /// rescanned, [cpp.pragma.op] as clang reads it), else `None` (the
    /// pragma ran; lexing continues after the `)`).
    pub(crate) fn handle_pragma_op(&mut self, t: PpTok) -> Option<PpTok> {
        let at = t.tok.span;
        let lp = self.lex();
        if !lp.is_punct(Punct::LParen) {
            self.emit(Diag::new(DiagId::pp_pragma_operator_malformed, at));
            return Some(lp);
        }
        let s = self.lex();
        if !is_any_string(&s) {
            self.emit(Diag::new(DiagId::pp_pragma_operator_malformed, at));
            let mut cur = s;
            if !cur.is_punct(Punct::RParen) && !cur.is_eod_or_eof() {
                cur = self.lex();
            }
            while !cur.is_punct(Punct::RParen)
                && !cur.has(TokFlags::START_OF_LINE)
                && !cur.is_eod_or_eof()
            {
                cur = self.lex();
            }
            if cur.is_punct(Punct::RParen) {
                cur = self.lex();
            }
            return Some(cur);
        }
        if s.has(TokFlags::UD_SUFFIX) {
            self.emit(Diag::new(DiagId::pp_string_udl, s.tok.span));
            let mut cur = self.lex();
            if cur.is_punct(Punct::RParen) {
                cur = self.lex();
            }
            return Some(cur);
        }
        let rp = self.lex();
        if !rp.is_punct(Punct::RParen) {
            self.emit(Diag::new(DiagId::pp_pragma_operator_malformed, at));
            return Some(rp);
        }
        if self.arg_preexpand > 0 {
            let mut v = self.take_buf();
            v.extend([lp, s, rp]);
            self.push_stream(v, true);
            return Some(t);
        }
        let text = destringize(&self.spelling(&s.tok));
        let toks = self.lex_pragma_string(&text, Span::new(at.lo, rp.tok.span.hi), at);
        self.push_stream(toks, false);
        let depth = self.frames.len();
        let saved = (self.in_directive, self.dir_eod.take());
        self.in_directive = true;
        self.do_pragma(PragmaIntroducer::Operator, at);
        while !self.lex_unexpanded().is_eod_or_eof() {}
        // Everything above the (exhausted) pragma stream was popped on
        // the way to its `Eod`; drop the stream without propagating its
        // flags to the next token.
        while self.frames.len() > depth {
            self.pop_macro();
        }
        if self.frames.len() == depth
            && let Some(Frame::Macro(m)) = self.frames.pop()
        {
            self.give_buf(m.toks);
        }
        (self.in_directive, self.dir_eod) = saved;
        // clang's pragma buffer ends with a newline, so its end-of-file
        // token is at the start of a line and the next token inherits that
        // (`PropagateLineStartLeadingSpaceInfo`); it shows in `-E` output
        // only for the first line of a file (`IsFirstInFile`).
        self.propagate(true, false);
        None
    }

    /// Lexes `text` (destringized pragma text) as a directive line in a
    /// scratch buffer expanded at `exp` (clang `Create_PragmaLexer`): the
    /// tokens followed by an `Eod`.
    fn lex_pragma_string(&mut self, text: &str, exp: Span, name: Span) -> Vec<PpTok> {
        let mut out = self.take_buf();
        let len = u32::try_from(text.len() + 1).unwrap_or(u32::MAX);
        let eod = Token { kind: TokKind::Eod, span: exp, flags: TokFlags::empty() };
        let Some((file, loc)) = self.scratch_loc(text) else {
            out.push(PpTok::new(eod));
            return out;
        };
        let base = self.add_exp(loc, len, exp, ExpansionKind::Macro { name });
        let mut lx = Lexer::new_in_range(self.sm, self.interner, file, 0..len, base, self.lex_opts);
        lx.set_in_directive(true);
        lx.set_keep_comments(false);
        loop {
            let mut t = lx.next();
            for d in lx.take_diagnostics() {
                if let Some(diag) = crate::diag_map::lex_diag(self.sm, &d) {
                    self.emit(diag);
                }
            }
            if t.kind == TokKind::Eof {
                t = eod;
            }
            t.flags.remove(TokFlags::START_OF_LINE);
            out.push(PpTok::new(t));
            if t.kind == TokKind::Eod {
                return out;
            }
        }
    }

    /// `#pragma` (clang `HandlePragmaDirective`); `name` is the `pragma`
    /// token. The caller discards the rest of the line.
    pub(crate) fn handle_pragma_directive(&mut self, name: PpTok) {
        self.do_pragma(PragmaIntroducer::Directive, name.tok.span);
    }

    /// Dispatches a pragma whose tokens are next in the stream (clang's
    /// `PragmaNamespace` tree with the builtin handlers, and the `-E`
    /// printer's catch-all handlers for the rest). `span` is the `pragma`
    /// / `_Pragma` token.
    fn do_pragma(&mut self, intro: PragmaIntroducer, span: Span) {
        let first = self.lex_unexpanded();
        let name = first.ident().map(|s| self.interner.as_str(s));
        match name {
            Some("once") => {
                self.pragma_eod("pragma once");
                self.pragma_once(&first);
            }
            // Consumed without effect (`mark` is not even lexed by clang).
            Some("mark" | "region" | "endregion") => {}
            Some(n @ ("push_macro" | "pop_macro")) => {
                self.pragma_push_pop(&first, n == "push_macro");
            }
            Some("message") => self.pragma_message(&first, None, PragmaMessageKind::Message),
            Some(ns @ ("GCC" | "clang")) => {
                let second = self.lex_unexpanded();
                let sub = second.ident().map(|s| self.interner.as_str(s));
                match sub {
                    Some("poison") => self.pragma_poison(),
                    Some("system_header") => {
                        self.pragma_system_header(&second);
                        self.pragma_eod("pragma");
                    }
                    // Consumed; kiln does not check the file's date.
                    Some("dependency") => {}
                    Some(k @ ("deprecated" | "final" | "restrict_expansion")) if ns == "clang" => {
                        self.pragma_macro_annotation(k);
                    }
                    Some("diagnostic") => {
                        if let Some(rest) = self.pragma_diagnostic() {
                            let mut toks = vec![first.tok, second.tok];
                            toks.extend(rest);
                            self.pass_pragma(intro, span, toks);
                        }
                    }
                    Some(k @ ("warning" | "error")) if ns == "GCC" => {
                        let kind = if k == "warning" {
                            PragmaMessageKind::Warning
                        } else {
                            PragmaMessageKind::Error
                        };
                        self.pragma_message(&second, Some("GCC"), kind);
                    }
                    _ => {
                        let mut toks = vec![first.tok];
                        if !second.is_eod_or_eof() {
                            toks.push(second.tok);
                            self.rest_of_pragma(&mut toks, false);
                        }
                        self.pass_pragma(intro, span, toks);
                    }
                }
            }
            _ => {
                let mut toks = Vec::new();
                if !first.is_eod_or_eof() {
                    toks.push(first.tok);
                    // clang's `-E` printer expands `#pragma omp` (its
                    // handler is registered with token expansion).
                    let expand = name == Some("omp");
                    self.rest_of_pragma(&mut toks, expand);
                }
                self.pass_pragma(intro, span, toks);
            }
        }
    }

    /// Appends the pragma's remaining tokens (without the `Eod`).
    fn rest_of_pragma(&mut self, toks: &mut Vec<Token>, expand: bool) {
        loop {
            let t = if expand { self.lex() } else { self.lex_unexpanded() };
            if t.is_eod_or_eof() {
                return;
            }
            toks.push(t.tok);
        }
    }

    /// Offers an unconsumed pragma to [`crate::PpCallbacks::pragma`];
    /// queues a [`Marker::Pragma`] unless the callback consumed it.
    fn pass_pragma(&mut self, introducer: PragmaIntroducer, span: Span, tokens: Vec<Token>) {
        let info = PragmaInfo { introducer, span, tokens: &tokens };
        if !self.callbacks.pragma(&info) {
            self.queue(Marker::Pragma { introducer, span, tokens });
        }
    }

    /// clang `HandleMacroAnnotationPragma` (`#pragma clang deprecated`,
    /// `final` and `restrict_expansion`): validates `( macro [, "msg"] )`
    /// and consumes the pragma (clang's `-E` does not print it). The
    /// annotations' effects on later uses of the macro are not
    /// implemented (spec/pp.md deviation 3).
    fn pragma_macro_annotation(&mut self, kind: &str) {
        let t = self.lex();
        if !t.is_punct(Punct::LParen) {
            self.emit(Diag::new(DiagId::pp_expected, t.tok.span).arg("("));
            return;
        }
        let t = self.lex_unexpanded();
        let Some(name) = t.ident() else {
            self.emit(Diag::new(DiagId::pp_expected, t.tok.span).arg("identifier"));
            return;
        };
        if !self.macros.is_defined(name) {
            let n = self.interner.as_str(name).to_owned();
            self.emit(Diag::new(DiagId::pp_pragma_non_macro, t.tok.span).arg(n));
            return;
        }
        let mut t = self.lex();
        if kind != "final" && t.is_punct(Punct::Comma) {
            let first = self.lex();
            let tag = format!("#pragma clang {kind}");
            let mut strs = Vec::new();
            let Some((_, next)) = self.finish_string(first, &tag, true, &mut strs) else { return };
            t = next;
        }
        if !t.is_punct(Punct::RParen) {
            self.emit(Diag::new(DiagId::pp_expected, t.tok.span).arg(")"));
        }
    }

    /// clang `CheckEndOfDirective` inside a pragma (the rest of the line
    /// is discarded by the caller).
    fn pragma_eod(&mut self, what: &str) {
        let t = self.lex_unexpanded();
        if !t.is_eod_or_eof() {
            self.emit(Diag::new(DiagId::pp_expected_eol, t.tok.span).arg(what));
        }
    }

    /// clang `HandlePragmaOnce`.
    fn pragma_once(&mut self, t: &PpTok) {
        let Some(file) = self.cur_file_ref().map(|f| f.file) else { return };
        if Some(file) == self.main_file {
            self.emit(Diag::new(DiagId::pp_pragma_once_in_main, t.tok.span));
            return;
        }
        self.once_files.insert(file);
    }

    /// clang `HandlePragmaSystemHeader`: the rest of the current file is
    /// a system header.
    fn pragma_system_header(&mut self, t: &PpTok) {
        let Some(file) = self.cur_file_ref().map(|f| f.file) else { return };
        if Some(file) == self.main_file {
            self.emit(Diag::new(DiagId::pp_pragma_system_header_in_main, t.tok.span));
            return;
        }
        if let Some(f) = self.cur_file() {
            f.system = true;
            f.extern_c = false;
        }
        self.file_change(FileChange::SystemHeader { file, at: t.tok.span.lo });
    }

    /// clang `HandlePragmaPoison` (`#pragma GCC poison` / `clang poison`).
    fn pragma_poison(&mut self) {
        loop {
            // Lexed "raw" (clang `LexingRawMode`) so that poisoning an
            // already poisoned identifier is not an error.
            let old = std::mem::replace(&mut self.quiet_lex, true);
            let t = self.lex_unexpanded();
            self.quiet_lex = old;
            if t.is_eod_or_eof() {
                return;
            }
            let Some(s) = t.ident() else {
                self.emit(Diag::new(DiagId::pp_pragma_poison_malformed, t.tok.span));
                return;
            };
            if self.poisoned.contains_key(&s) {
                continue;
            }
            if self.macros.is_defined(s) {
                self.emit(Diag::new(DiagId::pp_poisoning_existing_macro, t.tok.span));
            }
            self.poisoned.insert(s, t.tok.span);
        }
    }

    /// `#pragma push_macro("name")` / `pop_macro("name")` (clang
    /// `ParsePragmaPushOrPopMacro`, `HandlePragmaPushMacro`,
    /// `HandlePragmaPopMacro`).
    fn pragma_push_pop(&mut self, t: &PpTok, push: bool) {
        let what = if push { "push_macro" } else { "pop_macro" };
        let malformed = DiagId::pp_pragma_push_pop_malformed;
        let lp = self.lex();
        if !lp.is_punct(Punct::LParen) {
            self.emit(Diag::new(malformed, t.tok.span).arg(what));
            return;
        }
        let s = self.lex();
        if !is_plain_string(&s) {
            self.emit(Diag::new(malformed, t.tok.span).arg(what));
            return;
        }
        if s.has(TokFlags::UD_SUFFIX) {
            self.emit(Diag::new(DiagId::pp_string_udl, s.tok.span));
            return;
        }
        let sp = self.spelling(&s.tok);
        let rp = self.lex();
        if !rp.is_punct(Punct::RParen) {
            self.emit(Diag::new(malformed, t.tok.span).arg(what));
            return;
        }
        let text = sp.get(1..sp.len().saturating_sub(1)).unwrap_or("");
        let name = self.interner.intern(text);
        if push {
            let cur = self.macros.get_rc(name).cloned();
            if let Some(d) = &cur {
                // Allow the pushed definition to be redefined later.
                d.allow_redef.set(true);
            }
            self.pushed.entry(name).or_default().push(cur);
            return;
        }
        let Some(stack) = self.pushed.get_mut(&name) else {
            self.emit(Diag::new(DiagId::pp_pragma_pop_without_push, t.tok.span).arg(text));
            return;
        };
        let def = stack.pop().flatten();
        if stack.is_empty() {
            self.pushed.remove(&name);
        }
        match def {
            Some(d) => {
                self.macros.map.insert(name, d);
            }
            None => {
                self.macros.map.remove(&name);
            }
        }
    }

    /// clang `PragmaMessageHandler`: `#pragma message`, `#pragma GCC
    /// warning`, `#pragma GCC error`, with the string in parentheses or
    /// not. `t` is the `message` / `warning` / `error` token.
    fn pragma_message(&mut self, t: &PpTok, namespace: Option<&str>, kind: PragmaMessageKind) {
        let (sel, tag) = match kind {
            PragmaMessageKind::Message => (0, "pragma message"),
            PragmaMessageKind::Warning => (1, "pragma warning"),
            PragmaMessageKind::Error => (2, "pragma error"),
        };
        let malformed = |pp: &Self, at: Span| {
            pp.emit(Diag::new(DiagId::pp_pragma_message_malformed, at).arg(sel));
        };
        let mut cur = self.lex();
        let paren = cur.is_punct(Punct::LParen);
        if paren {
            cur = self.lex();
        } else if !is_plain_string(&cur) {
            malformed(self, t.tok.span);
            return;
        }
        let mut strs = Vec::new();
        let Some((text, next)) = self.finish_string(cur, tag, true, &mut strs) else { return };
        cur = next;
        if paren {
            if !cur.is_punct(Punct::RParen) {
                malformed(self, cur.tok.span);
                return;
            }
            cur = self.lex();
        }
        if !cur.is_eod_or_eof() {
            malformed(self, cur.tok.span);
            return;
        }
        let id = if kind == PragmaMessageKind::Error {
            DiagId::pp_pragma_gcc_error
        } else {
            DiagId::pp_pragma_message
        };
        self.emit(Diag::new(id, t.tok.span).arg(&text));
        self.callbacks.pragma_message(t.tok.span, namespace, kind, &text);
        self.queue(Marker::PragmaMessage {
            span: t.tok.span,
            namespace: namespace.map(str::to_owned),
            kind,
            text,
        });
    }

    /// clang `FinishLexStringLiteral`: concatenates the ordinary string
    /// literals starting at `first` (appending their tokens to `out`);
    /// returns the value and the token after them.
    fn finish_string(
        &mut self,
        first: PpTok,
        tag: &str,
        expand: bool,
        out: &mut Vec<Token>,
    ) -> Option<(String, PpTok)> {
        if !is_plain_string(&first) {
            self.emit(Diag::new(DiagId::pp_expected_string_literal, first.tok.span).arg(tag));
            return None;
        }
        let start = out.len();
        let mut cur = first;
        while is_plain_string(&cur) {
            if cur.has(TokFlags::UD_SUFFIX) {
                self.emit(Diag::new(DiagId::pp_string_udl, cur.tok.span));
            }
            out.push(cur.tok);
            cur = if expand { self.lex() } else { self.lex_unexpanded() };
        }
        let s = self.string_value(&out[start..])?;
        Some((s, cur))
    }

    /// clang `PragmaDiagnosticHandler` (after `GCC diagnostic` / `clang
    /// diagnostic`). kiln's diagnostic engine has no severity mappings
    /// yet, so the pragma is validated (and `push` / `pop` balanced) but
    /// has no effect on kiln's own diagnostics. Returns the tokens after
    /// `diagnostic` if the pragma is valid (it is then passed through).
    fn pragma_diagnostic(&mut self) -> Option<Vec<Token>> {
        let t = self.lex_unexpanded();
        let Some(s) = t.ident() else {
            self.emit(Diag::new(DiagId::pp_pragma_diagnostic_invalid, t.tok.span));
            return None;
        };
        let mut toks = vec![t.tok];
        let name = self.interner.as_str(s);
        let next = self.lex_unexpanded();
        if name == "push" || name == "pop" {
            let ok = if name == "push" {
                self.diag_pushes += 1;
                true
            } else if self.diag_pushes == 0 {
                self.emit(Diag::new(DiagId::pp_pragma_diagnostic_cannot_pop, next.tok.span));
                false
            } else {
                self.diag_pushes -= 1;
                true
            };
            if !next.is_eod_or_eof() {
                self.emit(Diag::new(DiagId::pp_pragma_diagnostic_invalid_token, next.tok.span));
            }
            return ok.then_some(toks);
        }
        if !matches!(name, "ignored" | "warning" | "error" | "fatal") {
            self.emit(Diag::new(DiagId::pp_pragma_diagnostic_invalid, next.tok.span));
            return None;
        }
        let at = next.tok.span;
        let (opt, end) = self.finish_string(next, "pragma diagnostic", false, &mut toks)?;
        if !end.is_eod_or_eof() {
            self.emit(Diag::new(DiagId::pp_pragma_diagnostic_invalid_token, end.tok.span));
            return None;
        }
        let b = opt.as_bytes();
        if b.len() < 3 || b[0] != b'-' || (b[1] != b'W' && b[1] != b'R') {
            self.emit(Diag::new(DiagId::pp_pragma_diagnostic_invalid_option, at));
            return None;
        }
        let group = &opt[2..];
        if group != "everything" && !Group::is_known_name(group) {
            self.emit(Diag::new(DiagId::pp_pragma_diagnostic_unknown_warning, at).arg(&opt));
            return None;
        }
        Some(toks)
    }
}

#[cfg(test)]
mod tests {
    use super::destringize;

    #[test]
    fn destringize_forms() {
        assert_eq!(destringize(r#""a \"b\" \\ c""#), r#" a "b" \ c"#);
        assert_eq!(destringize(r#"L"x""#), " x");
        assert_eq!(destringize(r#"u8"x""#), " x");
        assert_eq!(destringize(r#"R"d(a\"b)d""#), r#" a\"b"#);
        assert_eq!(destringize(r#"uR"(q)""#), " q");
    }
}
