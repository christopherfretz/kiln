//! The `-E` printer: clang's `PrintPreprocessedOutput.cpp` and
//! `TokenConcatenation.cpp`, over [`Preprocessor::next_item`].
//!
//! [`print_preprocessed`] writes the preprocessed translation unit the way
//! `clang -E` (with line markers) or `clang -E -P` (without) does: tokens
//! keep their source line where cheap, the first token of a line is
//! indented to its (expansion) column, and a space is inserted wherever two
//! adjacent tokens would otherwise lex differently. [`print_macros`] is
//! `-dM`.

use kiln_lex::{Encoding, Punct, TokFlags, TokKind, Token};
use kiln_source::{FileId, Loc, SourceManager};

use crate::callbacks::{FileChange, PragmaMessageKind};
use crate::preprocessor::{Marker, PpItem, Preprocessor};

/// Options of [`print_preprocessed`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrintOptions {
    /// Emit line markers (`# 12 "file.h" 2`); `false` is clang's `-P`.
    pub line_markers: bool,
    /// Spell line markers as `#line 12 "file.h"` (clang
    /// `-fuse-line-directives`).
    pub line_directives: bool,
}

/// What the printer remembers of a printed token (clang keeps the whole
/// previous two tokens for `AvoidConcat`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Prev {
    /// Nothing printed yet.
    None,
    /// An annotation (clang's `annot_embed`): always separated.
    Annot,
    /// A token.
    Tok(PrevTok),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PrevTok {
    kind: TokKind,
    /// Carries a ud-suffix.
    ud: bool,
    /// The spelling is a string-literal encoding prefix (`L`, `u8R`, ...).
    str_prefix: bool,
    /// The spelling location just past the token.
    end: Option<Loc>,
}

/// The state of an `#embed` whose tokens are being printed.
#[derive(Clone, Copy, Debug)]
struct EmbedState {
    file: FileId,
    line: u32,
}

#[allow(clippy::struct_excessive_bools)]
struct Printer<'p, 'a, 'd, 'o> {
    pp: &'p mut Preprocessor<'a, 'd>,
    sm: &'a SourceManager,
    out: &'o mut String,
    opts: PrintOptions,
    cur_line: u32,
    cur_filename: String,
    system: bool,
    emitted_tokens: bool,
    emitted_directive: bool,
    initialized: bool,
    first_file_entered: bool,
    prev: Prev,
    prev_prev: Prev,
    embed: Option<EmbedState>,
}

/// Writes the preprocessed translation unit (`clang -E`, or `-E -P`
/// without [`PrintOptions::line_markers`]) to `out`, driving `pp` to the
/// end of the main file. The main file must have been entered.
pub fn print_preprocessed(pp: &mut Preprocessor<'_, '_>, opts: PrintOptions, out: &mut String) {
    let sm = pp.source_manager();
    let mut p = Printer {
        pp,
        sm,
        out,
        opts,
        cur_line: 0,
        cur_filename: String::from("<uninit>"),
        system: false,
        emitted_tokens: false,
        emitted_directive: false,
        initialized: false,
        first_file_entered: false,
        prev: Prev::None,
        prev_prev: Prev::None,
        embed: None,
    };
    p.run();
}

/// `-dM`: drives `pp` to the end of the main file, then writes a
/// `#define` line for every macro still defined (builtins excluded),
/// sorted by name.
pub fn print_macros(pp: &mut Preprocessor<'_, '_>, out: &mut String) {
    while pp.next().kind != TokKind::Eof {}
    let interner = pp.interner();
    let sm = pp.source_manager();
    let mut defs: Vec<_> = pp
        .macros()
        .iter()
        .filter(|(_, d)| !d.is_builtin())
        .map(|(n, d)| (interner.as_str(n), d))
        .collect();
    defs.sort_unstable_by(|a, b| a.0.cmp(b.0));
    for (name, d) in defs {
        out.push_str("#define ");
        out.push_str(name);
        if d.is_function_like() {
            out.push('(');
            let params = d.params();
            for (i, &p) in params.iter().enumerate() {
                let s = interner.as_str(p);
                if i + 1 == params.len() && d.is_variadic() {
                    if s == "__VA_ARGS__" {
                        out.push_str("...");
                    } else {
                        out.push_str(s);
                        out.push_str("...");
                    }
                } else {
                    out.push_str(s);
                    if i + 1 < params.len() {
                        out.push(',');
                    }
                }
            }
            out.push(')');
        }
        let mut body = d.body().peekable();
        if body.peek().is_none_or(|t| !t.flags.contains(TokFlags::LEADING_SPACE)) {
            out.push(' ');
        }
        for t in body {
            if t.flags.contains(TokFlags::LEADING_SPACE) {
                out.push(' ');
            }
            out.push_str(&kiln_lex::spelling(sm, &t, pp.lex_options()));
        }
        out.push('\n');
    }
}

/// Appends `s` with clang's `outputPrintable` escaping (`\`, `"` and
/// non-printable bytes become three-digit octal escapes).
fn push_printable(out: &mut String, s: &str) {
    for b in s.bytes() {
        if (0x20..0x7f).contains(&b) && b != b'\\' && b != b'"' {
            out.push(char::from(b));
        } else {
            push_octal(out, b);
        }
    }
}

fn push_octal(out: &mut String, b: u8) {
    out.push('\\');
    out.push(char::from(b'0' + ((b >> 6) & 7)));
    out.push(char::from(b'0' + ((b >> 3) & 7)));
    out.push(char::from(b'0' + (b & 7)));
}

/// Appends `s` with LLVM's `raw_ostream::write_escaped` escaping (used for
/// file names in line markers).
fn push_escaped(out: &mut String, s: &str) {
    for b in s.bytes() {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'\t' => out.push_str("\\t"),
            b'\n' => out.push_str("\\n"),
            b'"' => out.push_str("\\\""),
            0x20..0x7f => out.push(char::from(b)),
            _ => push_octal(out, b),
        }
    }
}

/// `L`, `u`, `U`, `R`, `LR`, `uR`, `UR`, `u8`, `u8R` (clang
/// `IsStringPrefix` for C++11).
fn is_string_prefix(s: &str) -> bool {
    matches!(s, "L" | "u" | "U" | "R" | "LR" | "uR" | "UR" | "u8" | "u8R")
}

/// `[A-Za-z0-9_.]` (clang `isPreprocessingNumberBody`).
fn is_pp_number_body(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'.'
}

/// Whether a space may be needed after a token of kind `k` (clang's
/// `TokenInfo` table): `(custom or first-char rule, avoid a following =)`.
fn concat_info(k: TokKind) -> (bool, bool) {
    match k {
        TokKind::Identifier(_)
        | TokKind::PpNumber
        | TokKind::CharLit(_)
        | TokKind::StringLit(_)
        | TokKind::RawStringLit(_) => (true, false),
        TokKind::Punct(p) => match p {
            // `^` for kiln's `^^` (C++26 reflection).
            Punct::Amp
            | Punct::Plus
            | Punct::Minus
            | Punct::Slash
            | Punct::Less
            | Punct::Greater
            | Punct::Pipe
            | Punct::Percent
            | Punct::Caret => (true, true),
            Punct::Period
            | Punct::Colon
            | Punct::Hash
            | Punct::Arrow
            | Punct::LessEqual
            | Punct::LSquare => (true, false),
            Punct::Star
            | Punct::Exclaim
            | Punct::LessLess
            | Punct::GreaterGreater
            | Punct::Equal => (false, true),
            _ => (false, false),
        },
        _ => (false, false),
    }
}

/// The identifier rule of `AvoidConcat`: `id id`, `id 1`, `id L"x"`, and
/// `L "x"` (an identifier that is a string prefix before a string).
fn ident_concat(prev: PrevTok, tok: TokKind, first: u8) -> bool {
    match tok {
        TokKind::PpNumber => first != b'.',
        TokKind::Identifier(_) => true,
        TokKind::CharLit(e) | TokKind::StringLit(e) | TokKind::RawStringLit(e) => {
            e != Encoding::Ordinary || prev.str_prefix
        }
        _ => false,
    }
}

impl Printer<'_, '_, '_, '_> {
    fn run(&mut self) {
        // Tokens from the predefines buffer are not printed (there are
        // normally none: it holds only directives).
        let mut item = self.pp.next_item();
        loop {
            match &item {
                PpItem::Marker(m) => {
                    let m = m.clone();
                    self.marker(m);
                }
                PpItem::Token(t)
                    if t.kind != TokKind::Eof
                        && t.span.lo.is_file()
                        && self
                            .sm
                            .presumed_line_col(t.span.lo)
                            .is_some_and(|p| p.file_name == "<built-in>") => {}
                PpItem::Token(_) => break,
            }
            item = self.pp.next_item();
        }
        let mut sol = false;
        let mut in_bytes = false;
        loop {
            match item {
                PpItem::Marker(m) => {
                    if matches!(m, Marker::Embed { .. }) {
                        in_bytes = false;
                    }
                    self.marker(m);
                }
                PpItem::Token(t) => {
                    if t.kind == TokKind::Eof {
                        break;
                    }
                    self.token(&t, &mut sol, &mut in_bytes);
                }
            }
            item = self.pp.next_item();
        }
        self.out.push('\n');
    }

    fn token(&mut self, t: &Token, sol: &mut bool, in_bytes: &mut bool) {
        let sp = self.pp.spelling(t);
        let mut same_line = false;
        if let Some(e) = self.embed {
            if self.pp.is_embed_byte(t) {
                self.embed_byte(t, &sp, *in_bytes);
                *in_bytes = true;
                return;
            }
            let at = self.sm.presumed_line_col(t.span.lo).map(|p| (p.file, p.line));
            if at == Some((e.file, e.line)) {
                // `prefix` / `suffix` / `if_empty` tokens.
                same_line = true;
            } else {
                // clang does not start a new line for the token right after
                // an `#embed` directive.
                self.embed = None;
                same_line = at == Some((e.file, e.line + 1));
            }
        }
        *in_bytes = false;
        if same_line {
            *sol = false;
        } else {
            *sol |= t.flags.contains(TokFlags::START_OF_LINE);
        }
        self.whitespace_before(t, &sp, false, !*sol);
        self.out.push_str(&sp);
        if matches!(t.kind, TokKind::Comment | TokKind::Unknown) {
            self.newlines_in_token(&sp);
        }
        if t.kind == TokKind::Comment && sp.starts_with("//") {
            // The line comment must end the line.
            self.emitted_directive = true;
        }
        self.emitted_tokens = true;
        *sol = false;
    }

    /// One token of an `#embed`'s bytes, printed like clang's
    /// `annot_embed` (`b0, b1, ...` as one token).
    fn embed_byte(&mut self, t: &Token, sp: &str, in_bytes: bool) {
        if in_bytes {
            if t.kind != TokKind::Punct(Punct::Comma) {
                self.out.push(' ');
            }
            self.out.push_str(sp);
            return;
        }
        if self.emitted_directive {
            self.out.push('\n');
            self.cur_line += 1;
            self.emitted_tokens = false;
            self.emitted_directive = false;
        } else if self.emitted_tokens {
            let space = match self.prev {
                Prev::None => false,
                Prev::Annot => true,
                Prev::Tok(p) => {
                    let (a, b) = concat_info(p.kind);
                    a || b
                }
            };
            if space {
                self.out.push(' ');
            }
        }
        self.out.push_str(sp);
        self.prev_prev = self.prev;
        self.prev = Prev::Annot;
        self.emitted_tokens = true;
    }

    /// clang `HandleWhitespaceBeforeTok`.
    fn whitespace_before(&mut self, t: &Token, sp: &str, require_space: bool, same_line: bool) {
        let directive = self.emitted_directive;
        if (!same_line || directive) && self.move_to_tok(t, directive) {
            let mut col = self.sm.presumed_line_col(t.span.lo).map_or(1, |p| p.col);
            if col == 1 && t.flags.contains(TokFlags::LEADING_SPACE) {
                col = 2;
            }
            if col <= 1 && t.kind == TokKind::Punct(Punct::Hash) {
                self.out.push(' ');
            }
            for _ in 1..col {
                self.out.push(' ');
            }
        } else if require_space
            || t.flags.contains(TokFlags::LEADING_SPACE)
            || ((self.emitted_tokens || self.emitted_directive) && self.avoid_concat(t, sp))
        {
            self.out.push(' ');
        }
        let end = self
            .pp
            .loc_diff(t.span.lo, t.span.hi)
            .map(|n| self.sm.spelling_loc(t.span.lo).with_offset(n));
        self.prev_prev = self.prev;
        self.prev = Prev::Tok(PrevTok {
            kind: t.kind,
            ud: t.flags.contains(TokFlags::UD_SUFFIX),
            str_prefix: matches!(t.kind, TokKind::Identifier(_)) && is_string_prefix(sp),
            end,
        });
    }

    /// clang `TokenConcatenation::AvoidConcat`: true if printing `t`
    /// (spelled `sp`) right after the previous token would lex
    /// differently.
    fn avoid_concat(&self, t: &Token, sp: &str) -> bool {
        let p = match self.prev {
            Prev::None => return false,
            Prev::Annot => return true,
            Prev::Tok(p) => p,
        };
        // Adjacent in the source: they were one token sequence already.
        if p.end.is_some() && p.end == Some(self.sm.spelling_loc(t.span.lo)) {
            return false;
        }
        let (info, avoid_equal) = concat_info(p.kind);
        if avoid_equal && matches!(t.kind, TokKind::Punct(Punct::Equal | Punct::EqualEqual)) {
            return true;
        }
        if !info {
            return false;
        }
        let first = sp.as_bytes().first().copied().unwrap_or(0);
        match p.kind {
            TokKind::CharLit(_) | TokKind::StringLit(_) | TokKind::RawStringLit(_) => {
                if matches!(t.kind, TokKind::Identifier(_)) {
                    return true;
                }
                p.ud && ident_concat(p, t.kind, first)
            }
            TokKind::Identifier(_) => ident_concat(p, t.kind, first),
            TokKind::PpNumber => is_pp_number_body(first) || first == b'+' || first == b'-',
            TokKind::Punct(q) => match q {
                Punct::Period => {
                    (first == b'.'
                        && matches!(self.prev_prev, Prev::Tok(pp)
                            if pp.kind == TokKind::Punct(Punct::Period)))
                        || first.is_ascii_digit()
                        || first == b'*'
                }
                Punct::Amp => first == b'&',
                Punct::Plus => first == b'+',
                Punct::Minus => first == b'-' || first == b'>',
                Punct::Slash => first == b'*' || first == b'/',
                Punct::Less => matches!(first, b'<' | b':' | b'%'),
                Punct::Greater | Punct::LessEqual => first == b'>',
                Punct::Pipe => first == b'|',
                Punct::Percent => first == b'>' || first == b':',
                // `::`, `:>`, and kiln's `:]` (C++26 splice).
                Punct::Colon => matches!(first, b'>' | b':' | b']'),
                Punct::Hash => matches!(first, b'#' | b'@' | b'%'),
                Punct::Arrow => first == b'*',
                // kiln's `[:` (C++26 splice; `[::` stays `[` `::`).
                Punct::LSquare => first == b':' && t.kind != TokKind::Punct(Punct::ColonColon),
                // kiln's `^^` (C++26 reflection).
                Punct::Caret => first == b'^',
                _ => false,
            },
            _ => false,
        }
    }

    fn move_to_tok(&mut self, t: &Token, require_sol: bool) -> bool {
        let p = self.sm.presumed_line_col(t.span.lo);
        let target = p.as_ref().map_or(self.cur_line, |p| p.line);
        let first_in_file =
            t.flags.contains(TokFlags::START_OF_LINE) && p.as_ref().is_some_and(|p| p.line == 1);
        self.move_to_line(target, require_sol) || first_in_file
    }

    fn move_to_loc(&mut self, at: Loc, require_sol: bool) -> bool {
        let target = self.sm.presumed_line_col(at).map_or(self.cur_line, |p| p.line);
        self.move_to_line(target, require_sol)
    }

    /// clang `MoveToLine`: returns true if a new line was started.
    fn move_to_line(&mut self, line: u32, require_sol: bool) -> bool {
        let mut started = false;
        if (require_sol && self.emitted_tokens) || self.emitted_directive {
            self.out.push('\n');
            started = true;
            self.cur_line += 1;
            self.emitted_tokens = false;
            self.emitted_directive = false;
        }
        let delta = line.wrapping_sub(self.cur_line);
        if self.cur_line == line {
        } else if !started && delta == 1 {
            self.out.push('\n');
            started = true;
        } else if self.opts.line_markers {
            if delta <= 8 {
                for _ in 0..delta {
                    self.out.push('\n');
                }
            } else {
                self.write_line_info(line, "");
            }
            started = true;
        } else if self.emitted_tokens {
            self.out.push('\n');
            started = true;
        }
        if started {
            self.emitted_tokens = false;
            self.emitted_directive = false;
        }
        self.cur_line = line;
        started
    }

    fn start_new_line_if_needed(&mut self) {
        if self.emitted_tokens || self.emitted_directive {
            self.out.push('\n');
            self.emitted_tokens = false;
            self.emitted_directive = false;
        }
    }

    /// A line marker (`# 12 "file.h" 2 3`) or `#line` directive.
    fn write_line_info(&mut self, line: u32, extra: &str) {
        use std::fmt::Write as _;
        self.start_new_line_if_needed();
        if self.opts.line_directives {
            let _ = write!(self.out, "#line {line} \"");
            push_escaped(self.out, &self.cur_filename);
            self.out.push('"');
        } else {
            let _ = write!(self.out, "# {line} \"");
            push_escaped(self.out, &self.cur_filename);
            self.out.push('"');
            self.out.push_str(extra);
            if self.system {
                self.out.push_str(" 3");
            }
        }
        self.out.push('\n');
    }

    /// clang `HandleNewlinesInToken`.
    fn newlines_in_token(&mut self, sp: &str) {
        let b = sp.as_bytes();
        let mut n = 0u32;
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\n' || b[i] == b'\r' {
                n += 1;
                if i + 1 < b.len() && (b[i + 1] == b'\n' || b[i + 1] == b'\r') && b[i] != b[i + 1] {
                    i += 1;
                }
            }
            i += 1;
        }
        self.cur_line += n;
    }

    fn marker(&mut self, m: Marker) {
        match m {
            Marker::FileChange(c) => self.file_changed(c),
            Marker::Pragma { span, tokens, .. } => {
                // clang's `UnknownPragmaHandler`.
                self.move_to_loc(span.lo, true);
                self.out.push_str("#pragma");
                self.emitted_tokens = true;
                let mut first = true;
                for t in &tokens {
                    let sp = self.pp.spelling(t);
                    self.whitespace_before(t, &sp, first, true);
                    first = false;
                    self.out.push_str(&sp);
                    self.emitted_tokens = true;
                }
                self.emitted_directive = true;
            }
            Marker::PragmaMessage { span, namespace, kind, text } => {
                self.move_to_loc(span.lo, true);
                self.out.push_str("#pragma ");
                if let Some(ns) = namespace {
                    self.out.push_str(&ns);
                    self.out.push(' ');
                }
                self.out.push_str(match kind {
                    PragmaMessageKind::Message => "message(\"",
                    PragmaMessageKind::Warning => "warning \"",
                    PragmaMessageKind::Error => "error \"",
                });
                push_printable(self.out, &text);
                self.out.push('"');
                if kind == PragmaMessageKind::Message {
                    self.out.push(')');
                }
                self.emitted_directive = true;
            }
            Marker::Ident { span, text } => {
                self.move_to_loc(span.lo, true);
                self.out.push_str("#ident ");
                self.out.push_str(&text);
                self.emitted_tokens = true;
            }
            Marker::Module { .. } => {}
            Marker::Embed { span } => {
                self.embed = self
                    .sm
                    .presumed_line_col(span.lo)
                    .map(|p| EmbedState { file: p.file, line: p.line });
            }
        }
    }

    /// clang `FileChanged`.
    fn file_changed(&mut self, c: FileChange) {
        let (at, system) = match c {
            FileChange::Enter { file, system, .. } => (self.sm.loc(file, 0), system),
            FileChange::Exit { resume, system, .. } => (resume, system),
            FileChange::Rename { at, system, .. } => (at, system),
            FileChange::SystemHeader { at, .. } => (at, true),
        };
        let Some(p) = self.sm.presumed_line_col(at) else { return };
        let mut line = p.line;
        let name = p.file_name.to_owned();
        match c {
            FileChange::Enter { from: Some(from), .. } => {
                self.move_to_loc(from, false);
            }
            FileChange::SystemHeader { .. } => line += 1,
            _ => {}
        }
        self.cur_line = line;
        self.cur_filename = name;
        self.system = system;
        if !self.opts.line_markers {
            self.start_new_line_if_needed();
            return;
        }
        if !self.initialized {
            self.write_line_info(self.cur_line, "");
            self.initialized = true;
        }
        let extra = match c {
            FileChange::Enter { .. } if !self.first_file_entered => {
                self.first_file_entered = true;
                return;
            }
            FileChange::Enter { .. } => " 1",
            FileChange::Exit { .. } => " 2",
            FileChange::Rename { .. } | FileChange::SystemHeader { .. } => "",
        };
        self.write_line_info(self.cur_line, extra);
    }
}
