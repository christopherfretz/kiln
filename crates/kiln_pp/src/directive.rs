//! Directive dispatch and the directives the core implements ([cpp.pre],
//! [cpp.replace], [cpp.cond] structure, [cpp.include] requests,
//! [cpp.line], [cpp.error], [cpp.null]). Pragmas live in `pragma.rs`.

use std::rc::Rc;

use kiln_diag::{Diag, DiagId, SCRATCH_BUFFER_NAME};
use kiln_lex::{Encoding, Interner, LexMode, Punct, Std, Symbol, TokFlags, TokKind, Token};
use kiln_source::{FileId, SourceManager, Span};

use crate::builtin::OPERATOR_NAMES;
use crate::callbacks::FileChange;
use crate::cond::{IfContext, IfEvaluator, StubIfEvaluator, stub_value};
use crate::include::{EmbedRequest, IncludeKind, IncludeRequest};
use crate::macros::MacroDef;
use crate::preprocessor::{Cond, Frame, Marker, Preprocessor};
use crate::tok::{PpFlags, PpTok};

/// The directive names the core dispatches on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Dir {
    If,
    Ifdef,
    Ifndef,
    Elif,
    Elifdef,
    Elifndef,
    Else,
    Endif,
    Include,
    IncludeNext,
    Import,
    Embed,
    Define,
    Undef,
    Line,
    Error,
    Warning,
    Pragma,
    Ident,
    Sccs,
}

impl Dir {
    /// Every directive, in the order of [`Dir::NAMES`].
    const ALL: [Dir; 20] = [
        Dir::If,
        Dir::Ifdef,
        Dir::Ifndef,
        Dir::Elif,
        Dir::Elifdef,
        Dir::Elifndef,
        Dir::Else,
        Dir::Endif,
        Dir::Include,
        Dir::IncludeNext,
        Dir::Import,
        Dir::Embed,
        Dir::Define,
        Dir::Undef,
        Dir::Line,
        Dir::Error,
        Dir::Warning,
        Dir::Pragma,
        Dir::Ident,
        Dir::Sccs,
    ];
    /// The directive names, in the order of [`Dir::ALL`].
    const NAMES: [&'static str; 20] = [
        "if",
        "ifdef",
        "ifndef",
        "elif",
        "elifdef",
        "elifndef",
        "else",
        "endif",
        "include",
        "include_next",
        "import",
        "embed",
        "define",
        "undef",
        "line",
        "error",
        "warning",
        "pragma",
        "ident",
        "sccs",
    ];

    pub(crate) fn from_name(s: &str) -> Option<Dir> {
        Dir::NAMES.iter().position(|n| *n == s).map(|i| Dir::ALL[i])
    }
}

impl<'a> Preprocessor<'a, '_> {
    /// The directive name of identifier token `t`, if it is one.
    pub(crate) fn dir_of(&self, t: &PpTok) -> Option<Dir> {
        t.ident().and_then(|s| self.dirs.get(s, self.interner))
    }

    /// `#` at the start of a line was lexed from the top file frame
    /// (clang `HandleDirective`).
    pub(crate) fn handle_directive(&mut self, _hash: Token) {
        crate::profile::time!(t_directive_ns);
        crate::profile::count!(directives);
        self.start_directive();
        // For include-guard detection: were tokens read before this
        // directive? (Its own tokens count as read from here on.)
        let read_before = self.cur_file_ref().is_none_or(|f| f.mi.read_any);
        let name = self.lex_unexpanded();
        let dir = self.dir_of(&name);
        if let Some((mname, mspan)) = self.in_args {
            if matches!(
                dir,
                Some(Dir::Include | Dir::Import | Dir::IncludeNext | Dir::Pragma | Dir::Embed)
            ) {
                let n = self.spelling(&name.tok).into_owned();
                self.emit(Diag::new(DiagId::pp_embedded_directive, name.tok.span).arg(n));
                self.emit(
                    Diag::new(DiagId::note_macro_expansion_here, mspan)
                        .arg(self.interner.as_str(mname)),
                );
                self.finish_directive();
                return;
            }
            self.emit(Diag::new(DiagId::pp_embedded_directive_ext, name.tok.span));
        }
        match name.kind() {
            TokKind::Eod => {}
            TokKind::PpNumber => self.do_line_marker(name),
            _ => match dir {
                Some(Dir::If) => self.do_if(name, read_before),
                Some(d @ (Dir::Ifdef | Dir::Ifndef)) => {
                    self.do_ifdef(name, d == Dir::Ifndef, read_before);
                }
                Some(d @ (Dir::Elif | Dir::Elifdef | Dir::Elifndef)) => self.do_elif(name, d),
                Some(Dir::Else) => self.do_else(name),
                Some(Dir::Endif) => self.do_endif(name),
                Some(d @ (Dir::Include | Dir::IncludeNext | Dir::Import)) => {
                    self.do_include(name, d);
                }
                Some(Dir::Embed) => self.do_embed(name),
                Some(Dir::Define) => {
                    crate::profile::time!(t_define_ns);
                    self.do_define();
                }
                Some(Dir::Undef) => self.do_undef(),
                Some(Dir::Line) => self.do_line(),
                Some(Dir::Error) => self.do_error_warning(name, true),
                Some(Dir::Warning) => self.do_error_warning(name, false),
                Some(Dir::Pragma) => self.handle_pragma_directive(name),
                Some(Dir::Ident | Dir::Sccs) => self.do_ident(name),
                None => {
                    self.emit(Diag::new(DiagId::pp_invalid_directive, name.tok.span));
                }
            },
        }
        self.finish_directive();
    }

    /// Reads the macro name of `#define` / `#undef` / `#ifdef` / ...
    /// (clang `ReadMacroName` + `CheckMacroName`). On failure the line is
    /// discarded and `None` returned.
    pub(crate) fn read_macro_name(&mut self, def_undef: bool) -> Option<(Symbol, PpTok)> {
        let t = self.lex_unexpanded();
        let id = match t.kind() {
            TokKind::Eod => DiagId::pp_macro_name_missing,
            TokKind::Identifier(s) => {
                let name = self.interner.as_str(s);
                if let Some((_, op)) = OPERATOR_NAMES.iter().find(|(n, _)| *n == name) {
                    self.emit(
                        Diag::new(DiagId::pp_operator_macro_name, t.tok.span).arg(name).arg(op),
                    );
                    return Some((s, t));
                }
                if def_undef && s == self.syms.defined {
                    DiagId::pp_defined_macro_name
                } else {
                    return Some((s, t));
                }
            }
            _ => DiagId::pp_macro_name_not_identifier,
        };
        self.emit(Diag::new(id, t.tok.span));
        self.finish_directive();
        None
    }

    /// Diagnoses extra tokens before the end of the directive (clang
    /// `CheckEndOfDirective`) and discards them. `expand`: lex with macro
    /// expansion (as clang does after `#include`).
    pub(crate) fn check_eod(&mut self, dir: &str, expand: bool) {
        let mut t = if expand { self.lex() } else { self.lex_unexpanded() };
        while t.kind() == TokKind::Comment {
            t = self.lex_unexpanded();
        }
        if !t.is_eod_or_eof() {
            self.emit(Diag::new(DiagId::pp_expected_eol, t.tok.span).arg(dir));
        }
        self.finish_directive();
    }

    /// The text of the rest of the directive line after `name`, with
    /// line splices removed and leading spaces trimmed (clang
    /// `ReadToEndOfLine` for `#error` / `#warning`). Consumes the line.
    pub(crate) fn rest_of_line_text(&mut self, name: &PpTok) -> String {
        let src: &'a str = match self.sm.offset_of(name.tok.span.hi) {
            Some((f, off)) => self.sm.text(f).get(off as usize..).unwrap_or(""),
            None => "",
        };
        let b = src.as_bytes();
        let mut out = String::new();
        let mut i = 0;
        let mut seg = 0;
        while i < b.len() {
            match b[i] {
                b'\\' => {
                    // A splice: backslash, horizontal whitespace, newline.
                    let mut j = i + 1;
                    while j < b.len() && matches!(b[j], b' ' | b'\t' | b'\x0b' | b'\x0c') {
                        j += 1;
                    }
                    let nl = match b.get(j) {
                        Some(b'\n') => 1,
                        Some(b'\r') => {
                            if b.get(j + 1) == Some(&b'\n') {
                                2
                            } else {
                                1
                            }
                        }
                        _ => 0,
                    };
                    if nl == 0 {
                        i += 1;
                        continue;
                    }
                    out.push_str(&src[seg..i]);
                    i = j + nl;
                    seg = i;
                }
                b'\n' | b'\r' => break,
                _ => i += 1,
            }
        }
        out.push_str(&src[seg..i]);
        let old = std::mem::replace(&mut self.quiet_lex, true);
        self.finish_directive();
        self.quiet_lex = old;
        let trimmed = out.trim_start_matches(' ');
        if trimmed.len() == out.len() { out } else { trimmed.to_string() }
    }

    /// `#error` / `#warning` ([cpp.error]).
    fn do_error_warning(&mut self, name: PpTok, error: bool) {
        let msg = self.rest_of_line_text(&name);
        let id = if error { DiagId::pp_error_directive } else { DiagId::pp_warning_directive };
        self.emit(Diag::new(id, name.tok.span).arg(msg));
    }

    /// How clang prints a token kind in a diagnostic (`'+'`,
    /// `identifier`, `<numeric_constant>`, ...).
    pub(crate) fn tok_desc(t: &Token) -> String {
        let enc = |e: Encoding| match e {
            Encoding::Ordinary => "",
            Encoding::Wide => "wide_",
            Encoding::Utf8 => "utf8_",
            Encoding::Utf16 => "utf16_",
            Encoding::Utf32 => "utf32_",
        };
        match t.kind {
            TokKind::Punct(p) => format!("'{}'", p.as_str()),
            TokKind::Identifier(_) => "identifier".to_string(),
            TokKind::PpNumber => "<numeric_constant>".to_string(),
            TokKind::CharLit(e) => format!("<{}char_constant>", enc(e)),
            TokKind::StringLit(e) | TokKind::RawStringLit(e) => {
                format!("<{}string_literal>", enc(e))
            }
            TokKind::HeaderName => "<header_name>".to_string(),
            TokKind::Eod => "<eod>".to_string(),
            TokKind::Eof => "<eof>".to_string(),
            TokKind::Comment => "<comment>".to_string(),
            TokKind::Other(_) | TokKind::Unknown => "<unknown>".to_string(),
        }
    }

    /// `#define` ([cpp.replace], clang `HandleDefineDirective`).
    fn do_define(&mut self) {
        let Some((name, ntok)) = self.read_macro_name(true) else { return };
        let old_scope = self.va_scope;
        let def = self.read_definition(name, &ntok);
        self.va_scope = old_scope;
        let Some(def) = def else { return };
        let in_system = self.cur_file_ref().is_some_and(|f| f.system);
        if !in_system && let Some(old) = self.macros.get_rc(name).cloned() {
            if self.is_language_builtin(&old) {
                self.emit(Diag::new(DiagId::pp_builtin_macro_redefined, ntok.tok.span));
            } else if !old.allow_redef.get() && !self.identical(&old, &def) {
                self.emit(
                    Diag::new(DiagId::pp_macro_redefined, def.name_span)
                        .arg(self.interner.as_str(name))
                        .note(DiagId::note_previous_definition, old.name_span),
                );
            }
        }
        let def = Rc::new(def);
        self.callbacks.macro_defined(name, &def);
        self.macros.map.insert(name, def);
    }

    /// clang `isLanguageDefinedBuiltin`.
    fn is_language_builtin(&self, d: &MacroDef) -> bool {
        if d.builtin.is_some() {
            return true;
        }
        let in_builtin = self
            .sm
            .offset_of(d.name_span.lo)
            .is_some_and(|(f, _)| self.sm.file_name(f) == "<built-in>");
        if !in_builtin {
            return false;
        }
        let n = self.interner.as_str(d.name);
        n.starts_with("__STDC") || n == "__cplusplus" || n.starts_with("__cpp")
    }

    /// clang `MacroInfo::isIdenticalTo` (lexical): [cpp.replace]/2.
    fn identical(&self, a: &MacroDef, b: &MacroDef) -> bool {
        if a.body.len() != b.body.len()
            || a.params != b.params
            || a.function_like != b.function_like
            || a.variadic != b.variadic
            || a.named_variadic != b.named_variadic
        {
            return false;
        }
        for (i, (x, y)) in a.body.iter().zip(b.body.iter()).enumerate() {
            if x.tok.kind != y.tok.kind {
                return false;
            }
            if i != 0
                && (x.has(TokFlags::START_OF_LINE) != y.has(TokFlags::START_OF_LINE)
                    || x.has(TokFlags::LEADING_SPACE) != y.has(TokFlags::LEADING_SPACE))
            {
                return false;
            }
            if x.ident().is_some() {
                continue;
            }
            if self.spelling(&x.tok) != self.spelling(&y.tok) {
                return false;
            }
        }
        true
    }

    /// `#undef` ([cpp.scope], clang `HandleUndefDirective`).
    fn do_undef(&mut self) {
        let Some((name, ntok)) = self.read_macro_name(true) else { return };
        self.check_eod("undef", false);
        let old = self.macros.get_rc(name).cloned();
        self.callbacks.macro_undefined(name, ntok.tok.span, old.as_deref());
        if let Some(old) = old {
            if self.is_language_builtin(&old) {
                self.emit(Diag::new(DiagId::pp_builtin_macro_undefined, ntok.tok.span));
            }
            self.macros.map.remove(&name);
        }
    }

    /// Reads a `#define`'s parameter list and replacement list (clang
    /// `ReadOptionalMacroParameterListAndBody`). `None`: an error was
    /// reported; the rest of the line is discarded by the caller.
    fn read_definition(&mut self, name: Symbol, ntok: &PpTok) -> Option<MacroDef> {
        let mut params: Vec<Symbol> = Vec::new();
        let mut function_like = false;
        let (mut variadic, mut named_variadic) = (false, false);
        let mut tok = self.lex_unexpanded();
        if tok.kind() == TokKind::Eod {
        } else if tok.has(TokFlags::LEADING_SPACE) {
            tok.set_flag(TokFlags::LEADING_SPACE, false);
        } else if tok.is_punct(Punct::LParen) {
            function_like = true;
            if !self.read_params(&mut params, &mut variadic, &mut named_variadic) {
                return None;
            }
            if variadic && !named_variadic {
                self.va_scope = true;
            }
            tok = self.lex_unexpanded();
        } else {
            self.emit(Diag::new(DiagId::pp_missing_whitespace_after_name, tok.tok.span));
        }
        let mut body: Vec<PpTok> = Vec::new();
        let mut has_comma_paste = false;
        if function_like {
            if !self.read_fn_body(tok, &params, &mut body, &mut has_comma_paste) {
                return None;
            }
        } else {
            while tok.kind() != TokKind::Eod {
                body.push(tok);
                tok = self.lex_unexpanded();
            }
        }
        if let Some(first) = body.first()
            && first.is_punct(Punct::HashHash)
        {
            self.emit(Diag::new(DiagId::pp_paste_at_edge, first.tok.span).arg(0));
            return None;
        }
        if let Some(last) = body.last()
            && last.is_punct(Punct::HashHash)
        {
            self.emit(Diag::new(DiagId::pp_paste_at_edge, last.tok.span).arg(1));
            return None;
        }
        let (def_start, def_len, rel) = match (body.first(), body.last()) {
            (Some(f), Some(l)) => {
                let start = f.tok.span.lo;
                let rel: Box<[(u32, u32)]> = body
                    .iter()
                    .map(|t| {
                        let lo = self.loc_diff(start, t.tok.span.lo).unwrap_or(0);
                        let hi = self.loc_diff(start, t.tok.span.hi).unwrap_or(lo);
                        (lo, hi)
                    })
                    .collect();
                (start, self.loc_diff(start, l.tok.span.hi).unwrap_or(0), rel)
            }
            _ => (ntok.tok.span.lo, 0, Box::default()),
        };
        Some(MacroDef {
            name,
            name_span: ntok.tok.span,
            params: params.into_boxed_slice(),
            function_like,
            variadic,
            named_variadic,
            body: body.into_boxed_slice(),
            rel,
            def_start,
            def_len,
            builtin: None,
            enabled: std::cell::Cell::new(true),
            used: std::cell::Cell::new(false),
            allow_redef: std::cell::Cell::new(false),
        })
    }

    /// The parameter list after `(` (clang `ReadMacroParameterList`).
    fn read_params(
        &mut self,
        params: &mut Vec<Symbol>,
        variadic: &mut bool,
        named: &mut bool,
    ) -> bool {
        loop {
            let t = self.lex_unexpanded();
            match t.kind() {
                TokKind::Punct(Punct::RParen) => {
                    if params.is_empty() {
                        return true;
                    }
                    self.emit(Diag::new(DiagId::pp_expected_ident_in_params, t.tok.span));
                    return false;
                }
                TokKind::Punct(Punct::Ellipsis) => {
                    let r = self.lex_unexpanded();
                    if !r.is_punct(Punct::RParen) {
                        self.emit(Diag::new(DiagId::pp_missing_rparen_in_params, r.tok.span));
                        return false;
                    }
                    params.push(self.syms.va_args);
                    *variadic = true;
                    return true;
                }
                TokKind::Eod => {
                    self.emit(Diag::new(DiagId::pp_missing_rparen_in_params, t.tok.span));
                    return false;
                }
                TokKind::Identifier(s) => {
                    if params.contains(&s) {
                        self.emit(
                            Diag::new(DiagId::pp_duplicate_param, t.tok.span)
                                .arg(self.interner.as_str(s)),
                        );
                        return false;
                    }
                    params.push(s);
                    let n = self.lex_unexpanded();
                    match n.kind() {
                        TokKind::Punct(Punct::RParen) => return true,
                        TokKind::Punct(Punct::Comma) => {}
                        TokKind::Punct(Punct::Ellipsis) => {
                            self.emit(Diag::new(DiagId::pp_named_variadic, n.tok.span));
                            let r = self.lex_unexpanded();
                            if !r.is_punct(Punct::RParen) {
                                self.emit(Diag::new(
                                    DiagId::pp_missing_rparen_in_params,
                                    r.tok.span,
                                ));
                                return false;
                            }
                            *variadic = true;
                            *named = true;
                            return true;
                        }
                        _ => {
                            self.emit(Diag::new(DiagId::pp_expected_comma_in_params, n.tok.span));
                            return false;
                        }
                    }
                }
                _ => {
                    self.emit(Diag::new(DiagId::pp_invalid_token_in_params, t.tok.span));
                    return false;
                }
            }
        }
    }

    /// The replacement list of a function-like macro: `#` operands,
    /// `__VA_OPT__` structure, GNU comma pasting ([cpp.replace.general],
    /// [cpp.stringize], [cpp.subst]/3).
    fn read_fn_body(
        &mut self,
        mut tok: PpTok,
        params: &[Symbol],
        body: &mut Vec<PpTok>,
        comma_paste: &mut bool,
    ) -> bool {
        // Unmatched `(` of an open `__VA_OPT__` (innermost last).
        let mut va: Vec<Span> = Vec::new();
        let mut last = tok;
        while tok.kind() != TokKind::Eod {
            last = tok;
            if tok.is_punct(Punct::HashHash) {
                let next = self.lex_unexpanded();
                if next.kind() == TokKind::Eod {
                    body.push(tok);
                    tok = next;
                    break;
                }
                if next.ident() == Some(self.syms.va_args)
                    && body.last().is_some_and(|t| t.is_punct(Punct::Comma))
                {
                    *comma_paste = true;
                }
                body.push(tok);
                tok = next;
                continue;
            }
            if tok.is_punct(Punct::Hash) {
                let next = self.lex_unexpanded();
                let is_va_opt = next.ident() == Some(self.syms.va_opt);
                if !is_va_opt && !next.ident().is_some_and(|s| params.contains(&s)) {
                    self.emit(Diag::new(DiagId::pp_hash_not_followed_by_param, next.tok.span));
                    return false;
                }
                body.push(tok);
                if is_va_opt {
                    tok = next;
                } else {
                    body.push(next);
                    tok = self.lex_unexpanded();
                }
                continue;
            }
            body.push(tok);
            if tok.ident() == Some(self.syms.va_opt) {
                if !va.is_empty() {
                    self.emit(Diag::new(DiagId::pp_va_opt_nested, tok.tok.span));
                    return false;
                }
                let lp = self.lex_unexpanded();
                if !lp.is_punct(Punct::LParen) {
                    self.emit(Diag::new(DiagId::pp_va_opt_missing_lparen, lp.tok.span));
                    return false;
                }
                body.push(lp);
                va.push(lp.tok.span);
                tok = self.lex_unexpanded();
                if tok.is_punct(Punct::HashHash) {
                    self.emit(Diag::new(DiagId::pp_paste_at_va_opt_edge, tok.tok.span).arg(0));
                    return false;
                }
                continue;
            }
            if !va.is_empty() {
                if tok.is_punct(Punct::RParen) {
                    va.pop();
                    if va.is_empty()
                        && body.len() >= 2
                        && body[body.len() - 2].is_punct(Punct::HashHash)
                    {
                        self.emit(Diag::new(DiagId::pp_paste_at_va_opt_edge, tok.tok.span).arg(1));
                        return false;
                    }
                } else if tok.is_punct(Punct::LParen) {
                    va.push(tok.tok.span);
                }
            }
            tok = self.lex_unexpanded();
        }
        if let Some(&open) = va.last() {
            self.emit(
                Diag::new(DiagId::pp_expected_after, tok.tok.span)
                    .arg(Self::tok_desc(&last.tok))
                    .arg("')'")
                    .with_note(Diag::new(DiagId::note_matching, open).arg("'('")),
            );
            return false;
        }
        true
    }
}

/// How many leading `#if` tokens [`IfCx`] records for guard detection.
const IF_REC: usize = 8;

/// The [`IfContext`] the core hands to the [`IfEvaluator`].
struct IfCx<'p, 'a, 'd> {
    pp: &'p mut Preprocessor<'a, 'd>,
    /// For an `#embed` parameter expression: the parenthesis depth; the
    /// `)` closing depth 0 reads as `Eod`.
    paren: Option<u32>,
    /// The token that ended a parameter expression (`)` or `Eod`).
    end: Option<Token>,
    /// `defined` was evaluated.
    used_defined: bool,
    /// The first tokens read (for `#if !defined(X)` guard detection).
    rec: [Token; IF_REC],
    /// How many of `rec` are filled; `None` once more than
    /// [`IF_REC`] tokens were read.
    rec_len: Option<usize>,
}

impl<'p, 'a, 'd> IfCx<'p, 'a, 'd> {
    fn new(pp: &'p mut Preprocessor<'a, 'd>, paren: bool) -> Self {
        IfCx {
            pp,
            paren: paren.then_some(0),
            end: None,
            used_defined: false,
            rec: [crate::preprocessor::eof_tok(Span::INVALID); IF_REC],
            rec_len: Some(0),
        }
    }

    /// The `X` if the tokens read were exactly `!defined X` or
    /// `!defined(X)`, possibly parenthesized, none from a macro expansion
    /// (clang's `DefinedTracker::NotDefinedMacro` with no
    /// `DidMacroExpansion`).
    fn ifndef_macro(&self) -> Option<Symbol> {
        let rec = self.rec.get(..self.rec_len?)?;
        let (last, rest) = rec.split_last()?;
        if last.kind != TokKind::Eod || rest.iter().any(|t| self.pp.sm.is_macro_loc(t.span.lo)) {
            return None;
        }
        let mut depth = 0i32;
        let mut core = [TokKind::Eod; IF_REC];
        let mut n = 0;
        for t in rest {
            match t.kind {
                TokKind::Punct(Punct::LParen) => depth += 1,
                TokKind::Punct(Punct::RParen) => depth -= 1,
                k => {
                    core[n] = k;
                    n += 1;
                }
            }
            if depth < 0 {
                return None;
            }
        }
        match &core[..n] {
            [TokKind::Punct(Punct::Exclaim), TokKind::Identifier(d), TokKind::Identifier(x)]
                if depth == 0 && *d == self.pp.syms.defined =>
            {
                Some(*x)
            }
            _ => None,
        }
    }

    fn record(&mut self, t: Token) -> Token {
        if let Some(n) = self.rec_len {
            if let Some(slot) = self.rec.get_mut(n) {
                *slot = t;
                self.rec_len = Some(n + 1);
            } else {
                self.rec_len = None;
            }
        }
        t
    }

    fn filter(&mut self, t: Token) -> Token {
        let Some(depth) = self.paren.as_mut() else { return t };
        match t.kind {
            TokKind::Punct(Punct::LParen) => *depth += 1,
            TokKind::Punct(Punct::RParen) if *depth == 0 => {
                self.end = Some(t);
                return Token { kind: TokKind::Eod, span: t.span, flags: TokFlags::empty() };
            }
            TokKind::Punct(Punct::RParen) => *depth -= 1,
            TokKind::Eod | TokKind::Eof => self.end = Some(t),
            _ => {}
        }
        t
    }

    fn ended(&self) -> Option<Token> {
        self.end.map(|e| Token { kind: TokKind::Eod, span: e.span, flags: TokFlags::empty() })
    }
}

impl IfContext for IfCx<'_, '_, '_> {
    fn next(&mut self) -> Token {
        if let Some(e) = self.ended() {
            return e;
        }
        let t = self.pp.lex().tok;
        let t = self.record(t);
        self.filter(t)
    }

    fn next_unexpanded(&mut self) -> Token {
        if let Some(e) = self.ended() {
            return e;
        }
        let t = self.pp.lex_unexpanded().tok;
        let t = self.record(t);
        self.filter(t)
    }

    fn is_defined(&mut self, name: Symbol) -> bool {
        self.used_defined = true;
        match self.pp.macros.get(name) {
            Some(d) => {
                d.used.set(true);
                true
            }
            None => false,
        }
    }

    fn spelling(&self, tok: &Token) -> String {
        self.pp.spelling(tok).into_owned()
    }

    fn interner(&self) -> &Interner {
        self.pp.interner
    }

    fn source_manager(&self) -> &SourceManager {
        self.pp.sm
    }

    fn emit(&mut self, diag: Diag) {
        self.pp.emit(diag);
    }

    fn is_enabled(&self, id: DiagId) -> bool {
        self.pp.enabled(id)
    }

    fn std(&self) -> Std {
        self.pp.opts.std
    }
}

impl Preprocessor<'_, '_> {
    /// Evaluates the rest of the directive line as a `#if` expression and
    /// finishes the directive (clang `EvaluateDirectiveExpression`; macro
    /// expansion is on even inside macro arguments).
    pub(crate) fn eval_if(&mut self) -> bool {
        crate::profile::time!(t_if_ns);
        crate::profile::count!(if_evals);
        let old_de = std::mem::replace(&mut self.disable_expand, false);
        let old_pi = std::mem::replace(&mut self.parsing_if, true);
        let old_q = std::mem::replace(&mut self.quiet_lex, false);
        let mut ev = self.evaluator.take();
        let v = {
            let mut cx = IfCx::new(self, false);
            let v = match ev.as_mut() {
                Some(e) => e.eval(&mut cx),
                None => StubIfEvaluator.eval(&mut cx),
            };
            let ndef = cx.ifndef_macro();
            cx.pp.if_ndef = ndef;
            v
        };
        if self.evaluator.is_none() {
            self.evaluator = ev;
        }
        self.finish_directive();
        self.parsing_if = old_pi;
        self.disable_expand = old_de;
        self.quiet_lex = old_q;
        v
    }

    fn push_cond(&mut self, c: Cond) {
        if let Some(f) = self.cur_file() {
            f.conds.push(c);
        }
    }

    fn pop_cond(&mut self) -> Option<Cond> {
        self.cur_file().and_then(|f| f.conds.pop())
    }

    /// Include-guard detection for a conditional directive at the top
    /// level of the file: `guard` is the controlling macro it may
    /// introduce (`#ifndef X` / `#if !defined(X)` with `X` undefined and
    /// no tokens before).
    fn mi_enter(&mut self, guard: Option<Symbol>) {
        if let Some(f) = self.cur_file()
            && f.conds.is_empty()
        {
            match guard {
                Some(m) => f.mi.enter_ifndef(m),
                None => f.mi.invalidate(),
            }
        }
    }

    /// `#else` / `#elif` / `#endif` at the top level of the file.
    fn mi_exit(&mut self, endif: bool) {
        if let Some(f) = self.cur_file()
            && f.conds.is_empty()
        {
            if endif {
                f.mi.exit_top_level();
            } else {
                f.mi.invalidate();
            }
        }
    }

    /// `#if` ([cpp.cond]).
    fn do_if(&mut self, name: PpTok, read_before: bool) {
        let v = self.eval_if();
        let ndef = self.if_ndef.take();
        self.mi_enter(if v && !read_before { ndef } else { None });
        if v {
            self.push_cond(Cond {
                span: name.tok.span,
                found_non_skip: true,
                found_else: false,
                was_skipping: false,
            });
        } else {
            self.skip_excluded(name.tok.span, false, false);
        }
    }

    /// `#ifdef` / `#ifndef`.
    fn do_ifdef(&mut self, name: PpTok, ndef: bool, read_before: bool) {
        let Some((s, _)) = self.read_macro_name(false) else {
            self.mi_enter(None);
            self.skip_excluded(name.tok.span, false, false);
            return;
        };
        self.check_eod(if ndef { "ifndef" } else { "ifdef" }, false);
        let defined = match self.macros.get(s) {
            Some(d) => {
                d.used.set(true);
                true
            }
            None => false,
        };
        self.mi_enter((ndef && !defined && !read_before).then_some(s));
        if defined == ndef {
            self.skip_excluded(name.tok.span, false, false);
        } else {
            self.push_cond(Cond {
                span: name.tok.span,
                found_non_skip: true,
                found_else: false,
                was_skipping: false,
            });
        }
    }

    /// The `#elifdef` / `#elifndef` extension warning before C++23.
    fn elifdef_ext(&mut self, name: &PpTok, d: Dir) {
        if d != Dir::Elif && self.opts.std < Std::Cxx23 {
            let n = if d == Dir::Elifdef { "elifdef" } else { "elifndef" };
            self.emit(Diag::new(DiagId::pp_elifdef_ext, name.tok.span).arg(n));
        }
    }

    /// `#elif` / `#elifdef` / `#elifndef` after an included group: the
    /// rest of the conditional is skipped.
    fn do_elif(&mut self, name: PpTok, d: Dir) {
        let n = match d {
            Dir::Elifdef => "elifdef",
            Dir::Elifndef => "elifndef",
            _ => "elif",
        };
        self.elifdef_ext(&name, d);
        self.finish_directive();
        let Some(c) = self.pop_cond() else {
            if d == Dir::Elif {
                self.emit(Diag::new(DiagId::pp_elif_without_if, name.tok.span));
            } else {
                self.emit(Diag::new(DiagId::pp_elifdef_without_if, name.tok.span).arg(n));
            }
            return;
        };
        if c.found_else {
            self.emit(Diag::new(DiagId::pp_elif_after_else, name.tok.span).arg(n));
        }
        self.mi_exit(false);
        self.skip_excluded(c.span, true, c.found_else);
    }

    /// `#else` after an included group.
    fn do_else(&mut self, name: PpTok) {
        self.check_eod("else", false);
        let Some(c) = self.pop_cond() else {
            self.emit(Diag::new(DiagId::pp_else_without_if, name.tok.span));
            return;
        };
        if c.found_else {
            self.emit(Diag::new(DiagId::pp_else_after_else, name.tok.span));
        }
        self.mi_exit(false);
        self.skip_excluded(c.span, true, true);
    }

    /// `#endif`.
    fn do_endif(&mut self, name: PpTok) {
        self.check_eod("endif", false);
        if self.pop_cond().is_none() {
            self.emit(Diag::new(DiagId::pp_endif_without_if, name.tok.span));
        } else {
            self.mi_exit(true);
        }
    }

    /// Skips a group that is not included, up to the directive that ends
    /// it (clang `SkipExcludedConditionalBlock`). Tokens are lexed
    /// without diagnostics or macro lookups; nested conditionals are
    /// tracked; `#elif` conditions are evaluated only when they could
    /// select a group.
    pub(crate) fn skip_excluded(&mut self, if_span: Span, found_non_skip: bool, found_else: bool) {
        crate::profile::time!(t_skip_ns);
        self.push_cond(Cond { span: if_span, found_non_skip, found_else, was_skipping: false });
        let start = if_span.lo;
        let old_q = std::mem::replace(&mut self.quiet_lex, true);
        loop {
            if !self.in_file() {
                break;
            }
            let top = self.frames.len() - 1;
            let t = self.lex_raw_from(top);
            crate::profile::count!(skip_tokens);
            match t.kind {
                TokKind::Eof => {
                    self.unlex_file_tok(t);
                    break;
                }
                TokKind::Punct(Punct::Hash) if t.flags.contains(TokFlags::START_OF_LINE) => {}
                _ => {
                    // The rest of a non-directive line cannot matter
                    // unless it could open a comment or a literal.
                    if let Frame::File(f) = &mut self.frames[top]
                        && f.back.is_empty()
                    {
                        f.lx.skip_plain_line_rest();
                    }
                    continue;
                }
            }
            self.start_directive();
            let name = PpTok::new(self.lex_raw_from(top));
            let Some(d) = self.dir_of(&name) else {
                self.finish_directive();
                continue;
            };
            let done = match d {
                Dir::If | Dir::Ifdef | Dir::Ifndef => {
                    self.finish_directive();
                    self.push_cond(Cond {
                        span: name.tok.span,
                        found_non_skip: false,
                        found_else: false,
                        was_skipping: true,
                    });
                    false
                }
                Dir::Endif => {
                    let c = self.pop_cond();
                    if c.is_some_and(|c| !c.was_skipping) {
                        self.quiet_lex = false;
                        self.check_eod("endif", false);
                        true
                    } else {
                        self.finish_directive();
                        false
                    }
                }
                Dir::Else => {
                    let Some(c) = self.cur_file().and_then(|f| f.conds.last().copied()) else {
                        break;
                    };
                    if c.found_else {
                        self.emit(Diag::new(DiagId::pp_else_after_else, name.tok.span));
                    }
                    let enter = !c.was_skipping && !c.found_non_skip;
                    if let Some(f) = self.cur_file()
                        && let Some(c) = f.conds.last_mut()
                    {
                        c.found_else = true;
                        c.found_non_skip |= enter;
                    }
                    if enter {
                        self.quiet_lex = false;
                        self.check_eod("else", false);
                    } else {
                        self.finish_directive();
                    }
                    enter
                }
                Dir::Elif | Dir::Elifdef | Dir::Elifndef => self.skipped_elif(&name, d),
                _ => {
                    self.finish_directive();
                    false
                }
            };
            if done {
                break;
            }
        }
        self.quiet_lex = old_q;
        let end = self.cur_file_ref().map_or(start, |f| f.base.with_offset(f.lx.offset()));
        self.callbacks.source_range_skipped(Span::new(start, end));
    }

    /// `#elif*` inside a skipped group; true if its group is entered.
    #[allow(clippy::many_single_char_names)]
    fn skipped_elif(&mut self, name: &PpTok, d: Dir) -> bool {
        let Some(c) = self.cur_file().and_then(|f| f.conds.last().copied()) else {
            self.finish_directive();
            return false;
        };
        let n = match d {
            Dir::Elifdef => "elifdef",
            Dir::Elifndef => "elifndef",
            _ => "elif",
        };
        self.elifdef_ext(name, d);
        if c.found_else {
            self.emit(Diag::new(DiagId::pp_elif_after_else, name.tok.span).arg(n));
        }
        if c.was_skipping || c.found_non_skip {
            self.finish_directive();
            return false;
        }
        let enter = if d == Dir::Elif {
            let v = self.eval_if();
            self.quiet_lex = true;
            v
        } else {
            self.quiet_lex = false;
            let r = self.read_macro_name(false);
            let v = match r {
                Some((s, _)) => {
                    self.check_eod(n, false);
                    let def = match self.macros.get(s) {
                        Some(m) => {
                            m.used.set(true);
                            true
                        }
                        None => false,
                    };
                    def == (d == Dir::Elifdef)
                }
                None => false,
            };
            self.quiet_lex = true;
            v
        };
        if enter
            && let Some(f) = self.cur_file()
            && let Some(c) = f.conds.last_mut()
        {
            c.found_non_skip = true;
        }
        enter
    }
}

impl Preprocessor<'_, '_> {
    /// Lexes a header name (clang `LexHeaderName`): from a file, the lexer
    /// tries header-name tokenization first; a macro-expanded `<` ... `>`
    /// sequence is glued into one header-name token (tokens joined by
    /// their spellings, a space where one had leading whitespace); an
    /// ordinary string literal `"..."` is retagged as a header name.
    /// `Err`: an error was reported, holding the token that ended it.
    pub(crate) fn lex_header_name(&mut self) -> Result<PpTok, PpTok> {
        if self.dir_eod.is_none()
            && let Some(Frame::File(f)) = self.frames.last_mut()
            && f.back.is_empty()
        {
            f.lx.set_mode(LexMode::HeaderName);
        }
        let mut t = self.lex();
        while t.kind() == TokKind::Comment {
            t = self.lex();
        }
        if t.is_punct(Punct::Less) {
            let start = t.tok.span;
            let mut end;
            let mut buf = String::from("<");
            loop {
                let n = self.lex();
                if n.is_eod_or_eof() {
                    self.emit(
                        Diag::new(DiagId::pp_expected, n.tok.span)
                            .arg("'>'")
                            .with_note(Diag::new(DiagId::note_matching, start).arg("'<'")),
                    );
                    return Err(n);
                }
                end = n.tok.span;
                if n.has(TokFlags::LEADING_SPACE) {
                    buf.push(' ');
                }
                buf.push_str(&self.spelling(&n.tok));
                if n.is_punct(Punct::Greater) {
                    break;
                }
            }
            let mut h = PpTok::new(self.scratch_tok(
                &buf,
                TokKind::HeaderName,
                Span::new(start.lo, end.hi),
                start,
            ));
            h.set_flag(TokFlags::START_OF_LINE, t.has(TokFlags::START_OF_LINE));
            h.set_flag(TokFlags::LEADING_SPACE, t.has(TokFlags::LEADING_SPACE));
            return Ok(h);
        }
        if t.kind() == TokKind::StringLit(Encoding::Ordinary) && !t.has(TokFlags::UD_SUFFIX) {
            let sp = self.spelling(&t.tok);
            if sp.len() >= 2 && sp.starts_with('"') && sp.ends_with('"') {
                t.tok.kind = TokKind::HeaderName;
            }
        }
        Ok(t)
    }

    /// The name inside a header-name token and whether it is `<...>`
    /// (clang `GetIncludeFilenameSpelling`); diagnostics at `at`.
    pub(crate) fn include_name(&self, t: &Token, at: Span) -> Option<(String, bool)> {
        let sp = self.spelling(t);
        let angled = match sp.as_bytes().first() {
            Some(b'<') if sp.ends_with('>') => true,
            Some(b'"') if sp.len() >= 2 && sp.ends_with('"') => false,
            _ => {
                self.emit(Diag::new(DiagId::pp_include_expects_file, at));
                return None;
            }
        };
        if sp.len() <= 2 {
            self.emit(Diag::new(DiagId::pp_empty_filename, at));
            return None;
        }
        Some((sp[1..sp.len() - 1].to_string(), angled))
    }

    /// The physical file containing the current directive (`None` in the
    /// predefines buffer).
    pub(crate) fn includer(&self) -> Option<FileId> {
        let f = self.cur_file_ref()?.file;
        (self.sm.file_name(f) != "<built-in>").then_some(f)
    }

    /// Where `#include_next` / `__has_include_next` continue the search
    /// (clang `getIncludeNextStart`).
    fn include_next_start(&mut self, at: Span) -> Option<u32> {
        let f = self.cur_file_ref()?;
        if Some(f.file) == self.main_file {
            self.emit(Diag::new(DiagId::pp_include_next_in_primary, at));
            return None;
        }
        let l = f.lookup;
        if l.is_none() {
            self.emit(Diag::new(DiagId::pp_include_next_absolute_path, at));
        }
        l
    }

    /// `#include`, `#include_next`, `#import` ([cpp.include]): asks the
    /// [`IncludeResolver`] and enters the file.
    fn do_include(&mut self, name: PpTok, d: Dir) {
        crate::profile::time!(t_include_ns);
        let (dir, kind) = match d {
            Dir::IncludeNext => ("include_next", IncludeKind::IncludeNext),
            Dir::Import => ("import", IncludeKind::Import),
            _ => ("include", IncludeKind::Include),
        };
        let mut lookup_from = None;
        match d {
            Dir::Import => self.emit(Diag::new(DiagId::pp_import_ext, name.tok.span)),
            Dir::IncludeNext => {
                self.emit(Diag::new(DiagId::pp_include_next_ext, name.tok.span));
                lookup_from = self.include_next_start(name.tok.span);
            }
            _ => {}
        }
        let Ok(ft) = self.lex_header_name() else { return };
        if ft.kind() != TokKind::HeaderName {
            self.emit(Diag::new(DiagId::pp_include_expects_file, ft.tok.span));
            return;
        }
        self.check_eod(dir, true);
        let Some((fname, angled)) = self.include_name(&ft.tok, ft.tok.span) else { return };
        if self.include_depth + 1 >= self.opts.max_include_depth {
            self.emit(Diag::new(DiagId::pp_include_nested_too_deeply, ft.tok.span));
            return;
        }
        let req = IncludeRequest {
            kind,
            name: fname,
            angled,
            includer: self.includer(),
            lookup_from,
            name_span: ft.tok.span,
            directive_span: name.tok.span,
        };
        let res = self.resolver.resolve(self.sm, &req);
        self.callbacks.inclusion_directive(&req, res.as_ref());
        let Some(r) = res else {
            self.emit(Diag::new(DiagId::pp_file_not_found, ft.tok.span).arg(&req.name));
            return;
        };
        if self.once_files.contains(&r.file) {
            return;
        }
        // The multiple-include optimization: re-including a guarded file
        // whose guard macro is defined has no effect.
        if let Some(&m) = self.guards.get(&r.file)
            && self.macros.is_defined(m)
        {
            return;
        }
        if d == Dir::Import {
            self.once_files.insert(r.file);
        }
        // clang: the characteristic is the max of the directory's and the
        // includer's (user < system < extern "C" system).
        let (cur_sys, cur_c) =
            self.cur_file_ref().map_or((false, false), |f| (f.system, f.extern_c));
        let extern_c = r.extern_c || cur_c;
        let system = r.system || cur_sys || extern_c;
        self.include_depth += 1;
        self.push_file(
            r.file,
            Some(name.tok.span.lo),
            Some(ft.tok.span.lo),
            r.lookup_index,
            system,
            extern_c,
        );
    }

    /// The `(` of `__has_include` / `__has_embed` and the header name after
    /// it (clang `EvaluateHasIncludeCommon` / `EvaluateHasEmbed`, up to the
    /// header name). `Ok((lparen, header))`.
    fn has_open(&mut self, t: PpTok, lenient: bool) -> Result<(Span, PpTok), PpTok> {
        let name = self.spelling(&t.tok).into_owned();
        if !self.parsing_if {
            self.emit(Diag::new(DiagId::pp_has_include_outside_directive, t.tok.span).arg(&name));
            return Err(t);
        }
        let quoted = format!("'{name}'");
        let mut h = if lenient {
            self.lex_header_name()?
        } else {
            let mut n = self.lex_unexpanded();
            while n.kind() == TokKind::Comment {
                n = self.lex_unexpanded();
            }
            n
        };
        let lparen = if h.is_punct(Punct::LParen) {
            let l = h.tok.span;
            h = self.lex_header_name()?;
            l
        } else {
            let at = if lenient { Span::new(t.tok.span.hi, t.tok.span.hi) } else { h.tok.span };
            self.emit(Diag::new(DiagId::pp_expected_after, at).arg(&quoted).arg("'('"));
            if !lenient || h.kind() != TokKind::HeaderName {
                return Err(h);
            }
            at
        };
        if h.kind() != TokKind::HeaderName {
            self.emit(Diag::new(DiagId::pp_include_expects_file, h.tok.span));
            return Err(h);
        }
        Ok((lparen, h))
    }

    /// `__has_include(header)` / `__has_include_next(header)`
    /// ([cpp.cond]/4): `Ok((exists, rparen))`; `Err(tok)` after an error,
    /// `tok` replacing the builtin.
    pub(crate) fn eval_has_include(&mut self, t: PpTok, next: bool) -> Result<(bool, Span), PpTok> {
        let lookup_from = if next { self.include_next_start(t.tok.span) } else { None };
        let (lparen, h) = self.has_open(t, true)?;
        let mut close = self.lex();
        while close.kind() == TokKind::Comment {
            close = self.lex();
        }
        if !close.is_punct(Punct::RParen) {
            let name = format!("'{}'", self.spelling(&t.tok));
            self.emit(
                Diag::new(DiagId::pp_expected_after, Span::new(h.tok.span.hi, h.tok.span.hi))
                    .arg(name)
                    .arg("')'")
                    .with_note(Diag::new(DiagId::note_matching, lparen).arg("'('")),
            );
            return Err(close);
        }
        let Some((fname, angled)) = self.include_name(&h.tok, close.tok.span) else {
            return Ok((false, close.tok.span));
        };
        let req = IncludeRequest {
            kind: if next { IncludeKind::HasIncludeNext } else { IncludeKind::HasInclude },
            name: fname,
            angled,
            includer: self.includer(),
            lookup_from,
            name_span: h.tok.span,
            directive_span: t.tok.span,
        };
        let found = {
            crate::profile::time!(t_has_include_ns);
            self.resolver.resolve(self.sm, &req).is_some()
        };
        Ok((found, close.tok.span))
    }
}

/// Parsed `#embed` / `__has_embed` parameters (clang
/// `LexEmbedParametersResult`).
#[derive(Default)]
struct EmbedParams {
    limit: Option<u64>,
    offset: Option<u64>,
    prefix: Option<Vec<PpTok>>,
    suffix: Option<Vec<PpTok>>,
    if_empty: Option<Vec<PpTok>>,
    unknown: u32,
}

fn bracket_desc(p: Punct) -> &'static str {
    match p {
        Punct::LParen => "'('",
        Punct::RParen => "')'",
        Punct::LSquare => "'['",
        Punct::RSquare => "']'",
        Punct::LBrace => "'{'",
        _ => "'}'",
    }
}

fn closer(p: Punct) -> Punct {
    match p {
        Punct::LParen => Punct::RParen,
        Punct::LSquare => Punct::RSquare,
        _ => Punct::RBrace,
    }
}

impl Preprocessor<'_, '_> {
    /// The next macro-expanded non-comment token (clang `LexNonComment`).
    fn lex_nc(&mut self) -> PpTok {
        loop {
            let t = self.lex();
            if t.kind() != TokKind::Comment {
                return t;
            }
        }
    }

    /// Reads to the end of the directive without ending it (clang
    /// `DiscardUntilEndOfDirective`); returns the `Eod`.
    fn discard_line(&mut self, cur: PpTok) -> PpTok {
        let mut t = cur;
        while !t.is_eod_or_eof() {
            t = self.lex_unexpanded();
        }
        t
    }

    /// `expected <what>` at `cur`, then discards the line.
    fn embed_expected(&mut self, cur: &mut PpTok, what: &str) {
        self.emit(Diag::new(DiagId::pp_expected, cur.tok.span).arg(what));
        *cur = self.discard_line(*cur);
    }

    /// `identifier` or `identifier :: identifier`.
    fn embed_param_name(&mut self, cur: &mut PpTok) -> Option<String> {
        let Some(a) = cur.ident() else {
            self.embed_expected(cur, "identifier");
            return None;
        };
        let mut name = self.interner.as_str(a).to_string();
        *cur = self.lex_nc();
        if cur.is_punct(Punct::ColonColon) {
            *cur = self.lex_nc();
            let Some(b) = cur.ident() else {
                self.embed_expected(cur, "identifier");
                return None;
            };
            name.push_str("::");
            name.push_str(self.interner.as_str(b));
            *cur = self.lex_nc();
        }
        Some(name)
    }

    /// `( constant-expression )` for `limit` / `clang::offset`.
    fn embed_int(&mut self, cur: &mut PpTok) -> Option<u64> {
        if !cur.is_punct(Punct::LParen) {
            self.embed_expected(cur, "'('");
            return None;
        }
        let old_pi = std::mem::replace(&mut self.parsing_if, true);
        let mut ev = self.evaluator.take();
        let (v, used_defined, end) = {
            let mut cx = IfCx::new(self, true);
            let v = match ev.as_mut() {
                Some(e) => e.eval_value(&mut cx),
                None => stub_value(&mut cx),
            };
            (v, cx.used_defined, cx.end)
        };
        if self.evaluator.is_none() {
            self.evaluator = ev;
        }
        self.parsing_if = old_pi;
        *cur = match end {
            Some(t) => PpTok::new(t),
            None => self.lex_nc(),
        };
        let Some(v) = v else {
            *cur = self.discard_line(*cur);
            return None;
        };
        if !cur.is_punct(Punct::RParen) {
            self.embed_expected(cur, "')'");
            return None;
        }
        *cur = self.lex_nc();
        if used_defined {
            self.emit(Diag::new(DiagId::pp_embed_defined, cur.tok.span));
            *cur = self.discard_line(*cur);
            return None;
        }
        if v < 0 {
            self.emit(Diag::new(DiagId::pp_embed_negative_value, cur.tok.span).arg(v));
            *cur = self.discard_line(*cur);
            return None;
        }
        Some(v.unsigned_abs())
    }

    /// `( balanced-token-sequence )` for `prefix` / `suffix` / `if_empty`
    /// (clang `LexParenthesizedBalancedTokenSoup`, including its quirk
    /// that the first `)` closing a nested `(` at depth > 0 does not end
    /// the loop but a later `)` while no `(` is open does).
    fn embed_soup(&mut self, cur: &mut PpTok) -> Option<Vec<PpTok>> {
        if !cur.is_punct(Punct::LParen) {
            self.embed_expected(cur, "'('");
            return None;
        }
        let open = cur.tok.span;
        *cur = self.lex_nc();
        let mut out = Vec::new();
        let mut stack: Vec<(Punct, Span)> = Vec::new();
        let mut waiting = false;
        while !cur.is_eod_or_eof() && (waiting || !cur.is_punct(Punct::RParen)) {
            if let TokKind::Punct(p) = cur.kind() {
                match p {
                    Punct::LParen | Punct::LSquare | Punct::LBrace => {
                        waiting |= p == Punct::LParen;
                        stack.push((p, cur.tok.span));
                    }
                    Punct::RParen | Punct::RSquare | Punct::RBrace => {
                        if p == Punct::RParen {
                            waiting = false;
                        }
                        // An unmatched closer: clang reads an empty stack
                        // (and crashes); kiln matches it against the `(`.
                        let (o, at) = stack.last().copied().unwrap_or((Punct::LParen, open));
                        let want = closer(o);
                        if stack.is_empty() || p != want {
                            self.emit(
                                Diag::new(DiagId::pp_expected, cur.tok.span)
                                    .arg(bracket_desc(want))
                                    .with_note(
                                        Diag::new(DiagId::note_matching, at).arg(bracket_desc(o)),
                                    ),
                            );
                            *cur = self.discard_line(*cur);
                            return None;
                        }
                        stack.pop();
                    }
                    _ => {}
                }
            }
            out.push(*cur);
            *cur = self.lex_nc();
        }
        if !cur.is_punct(Punct::RParen) {
            self.embed_expected(cur, "')'");
            return None;
        }
        *cur = self.lex_nc();
        Some(out)
    }

    /// The embed parameter sequence after the resource name (clang
    /// `LexEmbedParameters`), ending at `Eod` (or at `)` for
    /// `__has_embed`). Returns the parameters (or `None` after an error,
    /// with `cur` at the `Eod`) and the token that ended them.
    fn lex_embed_params(&mut self, for_has: bool) -> (Option<EmbedParams>, PpTok) {
        let mut r = EmbedParams::default();
        let mut cur = self.lex_nc();
        while !cur.is_eod_or_eof() && (!for_has || !cur.is_punct(Punct::RParen)) {
            let Some(raw) = self.embed_param_name(&mut cur) else { return (None, cur) };
            let p = if raw.len() > 4 && raw.starts_with("__") && raw.ends_with("__") {
                &raw[2..raw.len() - 2]
            } else {
                &raw[..]
            };
            let dup = |pp: &mut Self, cur: &PpTok, seen: bool| {
                if seen {
                    pp.emit(Diag::new(DiagId::pp_embed_duplicate_param, cur.tok.span).arg(p));
                }
            };
            match p {
                "limit" | "clang::offset" => {
                    let lim = p == "limit";
                    dup(self, &cur, if lim { r.limit.is_some() } else { r.offset.is_some() });
                    let Some(v) = self.embed_int(&mut cur) else { return (None, cur) };
                    if lim {
                        r.limit = Some(v);
                    } else {
                        r.offset = Some(v);
                    }
                }
                "prefix" | "suffix" | "if_empty" => {
                    let slot = match p {
                        "prefix" => &r.prefix,
                        "suffix" => &r.suffix,
                        _ => &r.if_empty,
                    };
                    dup(self, &cur, slot.is_some());
                    let Some(v) = self.embed_soup(&mut cur) else { return (None, cur) };
                    match p {
                        "prefix" => r.prefix = Some(v),
                        "suffix" => r.suffix = Some(v),
                        _ => r.if_empty = Some(v),
                    }
                }
                _ => {
                    r.unknown += 1;
                    if cur.is_punct(Punct::LParen) && self.embed_soup(&mut cur).is_none() {
                        return (None, cur);
                    }
                    if !for_has {
                        self.emit(Diag::new(DiagId::pp_embed_unknown_param, cur.tok.span).arg(p));
                        cur = self.discard_line(cur);
                        return (None, cur);
                    }
                }
            }
        }
        (Some(r), cur)
    }

    /// `__has_embed(resource params...)` (C23 6.10.2): `Ok((value,
    /// rparen))` with 0 = not found, 1 = found, 2 = empty.
    pub(crate) fn eval_has_embed(&mut self, t: PpTok) -> Result<(u32, Span), PpTok> {
        let (lparen, h) = self.has_open(t, false)?;
        let (params, cur) = self.lex_embed_params(true);
        let Some(params) = params else { return Err(cur) };
        if params.unknown > 0 {
            // clang turns the closing token into the value even when it is
            // the `Eod`; kiln's `Eod` is sticky, so it is still seen.
            return Ok((0, cur.tok.span));
        }
        if !cur.is_punct(Punct::RParen) {
            let name = format!("'{}'", self.spelling(&t.tok));
            self.emit(
                Diag::new(DiagId::pp_expected_after, Span::new(h.tok.span.hi, h.tok.span.hi))
                    .arg(name)
                    .arg("')'")
                    .with_note(Diag::new(DiagId::note_matching, lparen).arg("'('")),
            );
            let e = self.discard_line(cur);
            return Err(e);
        }
        let Some((name, angled)) = self.include_name(&h.tok, h.tok.span) else {
            return Err(cur);
        };
        let req = EmbedRequest {
            name,
            angled,
            includer: self.includer(),
            limit: params.limit,
            offset: params.offset,
            name_span: h.tok.span,
            query_only: true,
        };
        let v = match self.resolver.resolve_embed(self.sm, &req) {
            None => 0,
            Some(d) if d.bytes.is_empty() => 2,
            Some(_) => 1,
        };
        Ok((v, cur.tok.span))
    }

    /// `#embed` ([cpp.embed], a Clang extension in C++): the resource's
    /// bytes become `prefix` tokens, comma-separated integer literals and
    /// `suffix` tokens, or the `if_empty` tokens when it is empty.
    fn do_embed(&mut self, name: PpTok) {
        self.emit(Diag::new(DiagId::pp_embed_ext, name.tok.span));
        let Ok(ft) = self.lex_header_name() else { return };
        if ft.kind() != TokKind::HeaderName {
            self.emit(Diag::new(DiagId::pp_include_expects_file, ft.tok.span));
            return;
        }
        let (params, _) = self.lex_embed_params(false);
        let Some(params) = params else { return };
        let Some((fname, angled)) = self.include_name(&ft.tok, ft.tok.span) else { return };
        let req = EmbedRequest {
            name: fname,
            angled,
            includer: self.includer(),
            limit: params.limit,
            offset: params.offset,
            name_span: ft.tok.span,
            query_only: false,
        };
        let Some(data) = self.resolver.resolve_embed(self.sm, &req) else {
            self.emit(Diag::new(DiagId::pp_file_not_found, ft.tok.span).arg(&req.name));
            return;
        };
        self.finish_directive();
        let mut toks = self.take_buf();
        if data.bytes.is_empty() {
            toks.extend(params.if_empty.unwrap_or_default());
        } else {
            toks.extend(params.prefix.unwrap_or_default());
            self.embed_tokens(&data.bytes, &mut toks);
            toks.extend(params.suffix.unwrap_or_default());
        }
        if toks.is_empty() {
            self.give_buf(toks);
        } else {
            self.queue(Marker::Embed { span: name.tok.span });
            self.push_stream(toks, true);
        }
    }

    /// Appends `b0 , b1 , ...` as tokens spelled in one scratch buffer.
    fn embed_tokens(&mut self, bytes: &[u8], out: &mut Vec<PpTok>) {
        use std::fmt::Write as _;
        let mut text = String::with_capacity(bytes.len() * 5 + 1);
        for (i, b) in bytes.iter().enumerate() {
            if i > 0 {
                text.push_str(", ");
            }
            let _ = write!(text, "{b}");
        }
        text.push('\n');
        let Ok(f) = self.sm.add_virtual(SCRATCH_BUFFER_NAME, text) else { return };
        self.embed_bufs.insert(f);
        let base = self.sm.loc(f, 0);
        let mut off = 0u32;
        out.reserve(bytes.len() * 2);
        for (i, b) in bytes.iter().enumerate() {
            if i > 0 {
                let mut c = PpTok::new(Token {
                    kind: TokKind::Punct(Punct::Comma),
                    span: Span::new(base.with_offset(off), base.with_offset(off + 1)),
                    flags: TokFlags::empty(),
                });
                c.pf.insert(PpFlags::EMBED);
                out.push(c);
                off += 2;
            }
            let len = if *b >= 100 {
                3
            } else if *b >= 10 {
                2
            } else {
                1
            };
            let mut n = PpTok::new(Token {
                kind: TokKind::PpNumber,
                span: Span::new(base.with_offset(off), base.with_offset(off + len)),
                flags: if i > 0 { TokFlags::LEADING_SPACE } else { TokFlags::empty() },
            });
            n.pf.insert(PpFlags::EMBED);
            out.push(n);
            off += len;
        }
    }
}

impl Preprocessor<'_, '_> {
    /// The value of a `#line` / line-marker digit sequence (clang
    /// `GetLineValue`); `None` after reporting `id` or a malformed number.
    fn line_value(&mut self, t: &PpTok, id: DiagId, gnu: bool) -> Option<u32> {
        if t.kind() != TokKind::PpNumber {
            self.emit(Diag::new(id, t.tok.span));
            return None;
        }
        let sp = self.spelling(&t.tok).into_owned();
        let mut v: u32 = 0;
        for (i, c) in sp.bytes().enumerate() {
            if c == b'\'' {
                continue;
            }
            if !c.is_ascii_digit() {
                let at = if t.tok.span.lo.is_macro() || t.has(TokFlags::NEEDS_CLEANING) {
                    t.tok.span
                } else {
                    let l = t.tok.span.lo.with_offset(u32::try_from(i).unwrap_or(0));
                    Span::new(l, l)
                };
                self.emit(Diag::new(DiagId::pp_line_digit_sequence, at).arg(i32::from(gnu)));
                return None;
            }
            let Some(n) = v.checked_mul(10).and_then(|n| n.checked_add(u32::from(c - b'0'))) else {
                self.emit(Diag::new(id, t.tok.span));
                return None;
            };
            v = n;
        }
        if sp.starts_with('0') && v != 0 {
            self.emit(Diag::new(DiagId::pp_line_decimal, t.tok.span).arg(i32::from(gnu)));
        }
        Some(v)
    }

    /// The file name operand of `#line` / a line marker: `Ok(None)` at the
    /// `Eod`, `Err(())` after an error.
    fn line_file_name(&mut self, s: &PpTok, gnu: bool) -> Result<Option<String>, ()> {
        match s.kind() {
            TokKind::Eod => Ok(None),
            TokKind::StringLit(Encoding::Ordinary) if s.has(TokFlags::UD_SUFFIX) => {
                self.emit(Diag::new(DiagId::pp_string_udl, s.tok.span));
                Err(())
            }
            TokKind::StringLit(Encoding::Ordinary) => {
                self.string_value(&[s.tok]).map(Some).ok_or(())
            }
            _ => {
                self.emit(
                    Diag::new(DiagId::pp_line_invalid_filename, s.tok.span).arg(i32::from(gnu)),
                );
                Err(())
            }
        }
    }

    /// Records a presumed-line change for the line after the current
    /// (finished) directive.
    fn add_line_note(
        &mut self,
        line: u32,
        name: Option<&str>,
        kind: Option<(bool, bool)>,
        flag: Option<u8>,
    ) {
        let Some(f) = self.cur_file() else { return };
        if let Some((system, extern_c)) = kind {
            // A line marker sets the characteristic of the rest of the
            // file (flags 3 and 4, or neither: user).
            f.system = system;
            f.extern_c = extern_c;
        }
        let (file, base, off, system, extern_c) =
            (f.file, f.base, f.lx.offset(), f.system, f.extern_c);
        let text = self.sm.text(file).as_bytes();
        let mut at = off as usize;
        if text.get(at) == Some(&b'\r') {
            at += 1;
        }
        if text.get(at) == Some(&b'\n') {
            at += 1;
        }
        let at = u32::try_from(at.min(text.len())).unwrap_or(off);
        self.sm.add_line_directive(file, at, line, name);
        let loc = base.with_offset(at);
        self.file_change(FileChange::Rename { file, at: loc, system, extern_c, flag });
    }

    /// `#line digit-sequence "s-char-sequence"opt` ([cpp.line]; the
    /// operands are macro-expanded).
    fn do_line(&mut self) {
        let d = self.lex_nc();
        let Some(line) = self.line_value(&d, DiagId::pp_line_invalid_number, false) else { return };
        if line == 0 {
            self.emit(Diag::new(DiagId::pp_line_zero, d.tok.span));
        }
        if line >= 2_147_483_648 {
            self.emit(Diag::new(DiagId::pp_line_too_big, d.tok.span).arg(2_147_483_648u32));
        }
        let s = self.lex_nc();
        let Ok(name) = self.line_file_name(&s, false) else { return };
        if name.is_some() {
            self.check_eod("line", true);
        }
        self.finish_directive();
        self.add_line_note(line, name.as_deref(), None, None);
    }

    /// A GNU line marker `# 42 "file" flags...` (clang
    /// `HandleDigitDirective`). The include-stack effect of flags 1 and 2
    /// is not modeled (`kiln_source` has no presumed include stack); flag 3
    /// makes the rest of the file a system header.
    #[allow(clippy::many_single_char_names)]
    fn do_line_marker(&mut self, d: PpTok) {
        let Some(line) = self.line_value(&d, DiagId::pp_line_marker_invalid_number, true) else {
            return;
        };
        let s = self.lex_nc();
        if s.kind() == TokKind::Eod {
            self.emit(Diag::new(DiagId::pp_line_marker_ext, s.tok.span));
            self.finish_directive();
            self.add_line_note(line, None, None, None);
            return;
        }
        let Ok(name) = self.line_file_name(&s, true) else { return };
        let mut enter = false;
        let mut exit = false;
        let mut system = false;
        let mut extern_c = false;
        let mut want = [1u32, 2, 3, 4].as_slice();
        loop {
            let f = self.lex_nc();
            if f.kind() == TokKind::Eod {
                break;
            }
            let Some(v) = self.line_value(&f, DiagId::pp_line_marker_flag, true) else { return };
            let Some(i) = want.iter().position(|&w| w == v) else {
                self.emit(Diag::new(DiagId::pp_line_marker_flag, f.tok.span));
                return;
            };
            // 1 and 2 are exclusive (1 skips 2); 4 needs 3.
            if v == 4 && !system {
                self.emit(Diag::new(DiagId::pp_line_marker_flag, f.tok.span));
                return;
            }
            if v == 2 && self.cur_file_ref().is_some_and(|f| f.marker_depth == 0) {
                // clang: the presumed location has no include location in
                // this physical file.
                self.emit(Diag::new(DiagId::pp_line_marker_pop, f.tok.span));
                return;
            }
            enter |= v == 1;
            exit |= v == 2;
            system |= v >= 3;
            extern_c |= v == 4;
            want = &want[(i + if v == 1 { 2 } else { 1 })..];
        }
        let builtin =
            self.cur_file_ref().is_some_and(|f| self.sm.file_name(f.file) == "<built-in>");
        if !builtin {
            self.emit(Diag::new(DiagId::pp_line_marker_ext, s.tok.span));
        }
        let name = name.filter(|n| !(exit && n.is_empty()));
        if let Some(f) = self.cur_file() {
            if enter {
                f.marker_depth += 1;
            } else if exit {
                f.marker_depth -= 1;
            }
        }
        self.finish_directive();
        let flag = if enter { Some(1) } else { exit.then_some(2) };
        self.add_line_note(line, name.as_deref(), Some((system, extern_c)), flag);
    }

    /// `#ident "text"` / `#sccs "text"`.
    fn do_ident(&mut self, name: PpTok) {
        self.emit(Diag::new(DiagId::pp_ident_ext, name.tok.span));
        let s = self.lex();
        if !matches!(s.kind(), TokKind::StringLit(Encoding::Ordinary | Encoding::Wide)) {
            self.emit(Diag::new(DiagId::pp_malformed_ident, s.tok.span));
            return;
        }
        if s.has(TokFlags::UD_SUFFIX) {
            self.emit(Diag::new(DiagId::pp_string_udl, s.tok.span));
            return;
        }
        self.check_eod("ident", false);
        let text = self.spelling(&s.tok).into_owned();
        self.callbacks.ident(name.tok.span, &text);
        self.queue(Marker::Ident { span: name.tok.span, text });
    }

    /// A `module` / `import` / `export` identifier at the start of a line
    /// ([cpp.pre]/1): if the next tokens on the line make it a module or
    /// import directive, queues [`Marker::Module`]. The tokens themselves
    /// are then returned as normal text ([cpp.module]/3, [cpp.import]/4).
    pub(crate) fn maybe_module_directive(&mut self, t: Token, s: Symbol) {
        let top = self.frames.len() - 1;
        let same = |x: &Token| {
            !x.flags.contains(TokFlags::START_OF_LINE)
                && !matches!(x.kind, TokKind::Eod | TokKind::Eof)
        };
        let n1 = self.lex_raw_from(top);
        let mut n2 = None;
        let mut kw = s;
        let export = s == self.syms.export;
        let mut next = n1;
        if export {
            match n1.kind {
                TokKind::Identifier(k)
                    if same(&n1) && (k == self.syms.module || k == self.syms.import) =>
                {
                    let n = self.lex_raw_from(top);
                    n2 = Some(n);
                    kw = k;
                    next = n;
                }
                _ => {
                    self.unlex_file_tok(n1);
                    return;
                }
            }
        }
        let import = kw == self.syms.import;
        let ok = same(&next)
            && match next.kind {
                TokKind::Identifier(_) | TokKind::Punct(Punct::Colon) => true,
                TokKind::Punct(Punct::Semi) => !import,
                TokKind::Punct(Punct::Less)
                | TokKind::StringLit(Encoding::Ordinary)
                | TokKind::HeaderName => import,
                _ => false,
            };
        if let Some(n) = n2 {
            self.unlex_file_tok(n);
        }
        self.unlex_file_tok(n1);
        if !ok {
            return;
        }
        if import && !matches!(next.kind, TokKind::Identifier(_) | TokKind::Punct(Punct::Colon)) {
            self.emit(Diag::new(DiagId::pp_header_unit_unsupported, next.span));
        }
        if let TokKind::Identifier(n) = next.kind
            && self.macros.get(n).is_some_and(|d| !d.function_like)
        {
            let sp = self.interner.as_str(n).to_string();
            self.emit(Diag::new(DiagId::pp_module_name_is_macro, next.span).arg(sp));
        }
        self.queue(Marker::Module { span: t.span, export, import });
    }
}

/// `Symbol -> Dir` for the directive names, interned once per
/// preprocessor: a table indexed by symbol index (offset by the smallest
/// directive symbol), so the per-directive lookup is one bounds check
/// instead of a string comparison.
pub(crate) struct DirTable {
    base: u32,
    /// `Dir::ALL` index + 1, or 0 for "not a directive name".
    slots: Box<[u8]>,
}

impl DirTable {
    /// Above this many slots the table falls back to string matching.
    const MAX_SLOTS: u32 = 1 << 16;

    pub(crate) fn new(interner: &Interner) -> Self {
        let syms = Dir::NAMES.map(|n| interner.intern(n).index());
        let base = syms.iter().copied().min().unwrap_or(0);
        let span = syms.iter().copied().max().unwrap_or(0) - base + 1;
        if span > Self::MAX_SLOTS {
            return DirTable { base: 0, slots: Box::new([]) };
        }
        let mut slots = vec![0u8; span as usize].into_boxed_slice();
        for (i, s) in syms.iter().enumerate() {
            slots[(s - base) as usize] = i as u8 + 1;
        }
        DirTable { base, slots }
    }

    pub(crate) fn get(&self, s: Symbol, interner: &Interner) -> Option<Dir> {
        if self.slots.is_empty() {
            return Dir::from_name(interner.as_str(s));
        }
        let slot = *self.slots.get(s.index().checked_sub(self.base)? as usize)?;
        Dir::ALL.get(usize::from(slot).checked_sub(1)?).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::{Dir, DirTable};
    use kiln_lex::Interner;

    /// [cpp.pre]/1: the symbol table classifies exactly the directive
    /// names, also when the interner already held other strings (so the
    /// directive symbols are not contiguous).
    #[test]
    fn cpp_pre_dir_table_matches_names() {
        for warm in [0usize, 3, 300] {
            let interner = Interner::new();
            for i in 0..warm {
                interner.intern(&format!("w{i}"));
                if i == 1 {
                    interner.intern("endif");
                }
            }
            let table = DirTable::new(&interner);
            for (name, dir) in Dir::NAMES.iter().zip(Dir::ALL) {
                assert_eq!(Dir::from_name(name), Some(dir));
                assert_eq!(table.get(interner.intern(name), &interner), Some(dir), "{name}");
            }
            for other in ["w0", "IF", "defined", "include_nexts", "x"] {
                let s = interner.intern(other);
                assert_eq!(table.get(s, &interner), None, "{other}");
                assert_eq!(Dir::from_name(other), None);
            }
        }
    }
}
