//! The pp-token lexer ([lex.pptoken]).
#![allow(clippy::many_single_char_names, reason = "p/q positions, c/s decoded char and size")]

use std::borrow::Cow;
use std::ops::Range;

use kiln_source::{FileId, Loc, Replacement, SourceManager, Span};

use crate::diag::{LexDiag, LexDiagKind};
use crate::options::{CStd, LexMode, LexOptions, Std};
use crate::phase::{Src, is_hws};
use crate::token::{Encoding, Punct, TokFlags, TokKind, Token};
use crate::unicode::{
    IdTables, NameMatch, ascii_ident_end, c99_incompat, is_ascii_ident_continue, is_nfc,
    is_unicode_whitespace, is_xid_continue, is_xid_start, lookup_char_name,
};
use kiln_source::symbol::Interner;

/// The position of the first `\` or `?` at or after `from` (the bytes
/// phase 2 can act on), or the end of `bytes`.
fn next_special(bytes: &[u8], from: usize) -> usize {
    const ONES: u64 = 0x0101_0101_0101_0101;
    const HIGHS: u64 = 0x8080_8080_8080_8080;
    // SWAR zero-byte test: nonzero iff some byte of `x` is zero.
    let zero = |x: u64| x.wrapping_sub(ONES) & !x & HIGHS;
    let mut i = from;
    while let Some(w) = bytes.get(i..i + 8) {
        let mut word = [0u8; 8];
        word.copy_from_slice(w);
        let w = u64::from_le_bytes(word);
        if zero(w ^ (ONES * u64::from(b'\\'))) | zero(w ^ (ONES * u64::from(b'?'))) != 0 {
            break;
        }
        i += 8;
    }
    bytes
        .get(i..)
        .and_then(|rest| rest.iter().position(|&c| c == b'\\' || c == b'?'))
        .map_or(bytes.len(), |k| i + k)
}

/// Result of reading a universal-character-name at a decoded `\`.
#[derive(Clone, Copy, Debug)]
enum Ucn {
    /// Not `\u`, `\U` or `\N`.
    NotOne,
    /// Starts like a UCN but is not one (too few digits, missing brace):
    /// the `\` is a token by itself. Carries the warning to give.
    Malformed(LexDiagKind),
    /// A UCN designating `ch`, ending at `end`; `ext` is an extension
    /// warning to give if it is used (delimited forms before C++23).
    Valid { ch: char, end: usize, ext: Option<LexDiagKind> },
    /// A syntactically complete UCN with an invalid value.
    Invalid { end: usize, kind: LexDiagKind },
}

/// Per-identifier bookkeeping while lexing an identifier tail.
#[derive(Clone, Copy, Default, Debug)]
struct IdentInfo {
    has_ucn: bool,
    dollar: bool,
    /// A non-ASCII character was seen: the NFC check must run.
    non_ascii: bool,
}

/// A pull lexer over one buffer (or a byte range of one).
///
/// Call [`next`](Self::next) until it returns [`TokKind::Eof`] (which it
/// keeps returning). The preprocessor steers it with
/// [`set_in_directive`](Self::set_in_directive) and
/// [`set_mode`](Self::set_mode).
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools, reason = "independent lexer state bits")]
pub struct Lexer<'a> {
    sm: &'a SourceManager,
    interner: &'a Interner,
    file: FileId,
    src: Src<'a>,
    /// The U+FFFDs that replaced invalid UTF-8 when the file was loaded
    /// (ascending; empty for a valid file).
    bad: &'a [Replacement],
    pos: usize,
    base: Loc,
    opts: LexOptions,
    mode: LexMode,
    in_directive: bool,
    at_line_start: bool,
    leading_space: bool,
    whole_file: bool,
    eof_seen: bool,
    /// Raw-string body of the token being lexed: excluded from phase-2
    /// scanning (splices inside raw strings are reverted, [lex.pptoken]/3.1).
    raw_body: Option<(usize, usize)>,
    /// No `\` or `?` occurs in `[pos, clean_until)`: tokens ending by
    /// then need no phase-2 scan (the scan is the per-token hot spot on
    /// splice-free text).
    clean_until: usize,
    diags: Vec<LexDiag>,
    scratch: String,
    /// Spans of the comments lexed since the last
    /// [`take_comments`](Lexer::take_comments), when recording is on.
    comments: Option<Vec<Span>>,
    /// Strict C89 only: a `//` comment has been lexed in this buffer, so
    /// `//` is a comment from here on (clang's per-lexer `LineComment`,
    /// set by the first one, which `ext_line_comment` reports).
    c89_line_comments: bool,
}

impl<'a> Lexer<'a> {
    /// Lexes the whole of `file`. `base` is the location of byte 0 of the
    /// file's text (normally `sm.loc(file, 0)`; a per-inclusion range for
    /// `#include`d files); token locations are `base.with_offset(offset)`.
    /// A UTF-8 BOM at offset 0 is skipped.
    pub fn new(
        sm: &'a SourceManager,
        interner: &'a Interner,
        file: FileId,
        base: Loc,
        opts: LexOptions,
    ) -> Self {
        let len = sm.text(file).len() as u32;
        let mut lx = Self::new_in_range(sm, interner, file, 0..len, base, opts);
        lx.whole_file = true;
        lx
    }

    /// Lexes bytes `range` of `file` (for scratch-buffer chunks and
    /// `_Pragma` strings). `base` is still the location of byte 0 of the
    /// file. A BOM is skipped only if the range starts at 0. No
    /// [`LexDiagKind::NoNewlineAtEof`] is recorded.
    ///
    /// # Panics
    /// If `range` is out of bounds or not on character boundaries.
    pub fn new_in_range(
        sm: &'a SourceManager,
        interner: &'a Interner,
        file: FileId,
        range: Range<u32>,
        base: Loc,
        opts: LexOptions,
    ) -> Self {
        let full = sm.text(file);
        let (start, end) = (range.start as usize, range.end as usize);
        assert!(
            start <= end && full.is_char_boundary(start) && full.is_char_boundary(end),
            "Lexer range {range:?} is not a valid range of {file:?}"
        );
        let text = &full[..end];
        let pos = if start == 0 && text.starts_with('\u{FEFF}') { 3 } else { start };
        Lexer {
            sm,
            interner,
            file,
            src: Src::new(text, opts.trigraphs),
            bad: &sm.load_report(file).replacements,
            pos,
            base,
            opts,
            mode: LexMode::Normal,
            in_directive: false,
            at_line_start: true,
            leading_space: false,
            whole_file: false,
            eof_seen: false,
            raw_body: None,
            clean_until: 0,
            diags: Vec::new(),
            scratch: String::new(),
            comments: None,
            c89_line_comments: false,
        }
    }

    // ----- control ------------------------------------------------------

    /// Sets the mode for the next token (see [`LexMode`]).
    pub fn set_mode(&mut self, mode: LexMode) {
        self.mode = mode;
    }

    /// The current mode.
    pub fn mode(&self) -> LexMode {
        self.mode
    }

    /// While set, the next newline (or end of input) produces
    /// [`TokKind::Eod`] instead of being skipped. Cleared automatically
    /// when `Eod` is returned.
    pub fn set_in_directive(&mut self, on: bool) {
        self.in_directive = on;
    }

    /// Whether a directive line is being lexed.
    pub fn in_directive(&self) -> bool {
        self.in_directive
    }

    /// Return comments as [`TokKind::Comment`] tokens (`-C`/`-CC`).
    pub fn set_keep_comments(&mut self, on: bool) {
        self.opts.keep_comments = on;
    }

    /// Records the span of every comment lexed from now on, whether or
    /// not it is returned as a token (clang's `CommentHandler` feed, used
    /// by `-verify`); `false` stops recording and drops what was recorded.
    pub fn set_record_comments(&mut self, on: bool) {
        self.comments = on.then(Vec::new);
    }

    /// Whether comments were recorded since the last
    /// [`take_comments`](Self::take_comments).
    pub fn has_comments(&self) -> bool {
        self.comments.as_ref().is_some_and(|c| !c.is_empty())
    }

    /// Removes and returns the recorded comment spans, in source order.
    pub fn take_comments(&mut self) -> Vec<Span> {
        self.comments.as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// The options in effect.
    pub fn options(&self) -> &LexOptions {
        &self.opts
    }

    /// The buffer being lexed.
    pub fn file(&self) -> FileId {
        self.file
    }

    /// The interner identifiers are interned in.
    pub fn interner(&self) -> &'a Interner {
        self.interner
    }

    /// The source manager.
    pub fn source_manager(&self) -> &'a SourceManager {
        self.sm
    }

    /// Byte offset in the file of the next character to lex.
    pub fn offset(&self) -> u32 {
        self.pos as u32
    }

    /// Fast path for skipped groups ([cpp.cond]/13): moves to the end of
    /// the current logical line without forming its tokens, if that end is
    /// the next line terminator, and returns true; otherwise does nothing
    /// and returns false. Only a token can carry a logical line past a
    /// terminator, and only three can: a block comment, a raw string
    /// literal and a line splice. So the rest of the line qualifies unless
    /// it holds a `/*` with no `*/` after it on the line (or one that a
    /// `/*/` could reopen), an `R"`, a `\` as its last non-blank byte, or
    /// (with trigraphs) a `??/`, or, compiling strict C89 before the
    /// file's first line comment, a `//`. Lexing on from there gives the same
    /// following tokens as lexing the skipped ones would have; no
    /// diagnostics are recorded and nothing is interned for the skipped
    /// bytes.
    pub fn skip_plain_line_rest(&mut self) -> bool {
        // Only after a token on this line, in the ordinary mode: a `#` at
        // the start of a line is never skipped.
        if self.at_line_start || self.mode != LexMode::Normal {
            return false;
        }
        let from = self.pos;
        let Some(rest) = self.src.bytes.get(from..) else {
            return false;
        };
        // Strict C89 compiling: the file's first `//` comment changes how
        // later `//*` lex, and clang's raw lexer sees it in a skipped group
        // too, so a line that may hold it is lexed.
        if self.opts.c == Some(CStd::C89)
            && !self.opts.gnu
            && !self.c89_line_comments
            && !self.opts.preprocessed_output
        {
            let eol = rest.iter().position(|&c| c == b'\n' || c == b'\r').unwrap_or(rest.len());
            if rest[..eol].windows(2).any(|w| w == b"//") {
                return false;
            }
        }
        let at = |k: usize| rest.get(k).copied();
        let trigraphs = self.opts.trigraphs;
        // `R"` or `??/` at `k`: both can matter even inside what looks
        // like a comment here (the `/*` may be inside a literal).
        let opener = |k: usize| match at(k) {
            Some(b'"') => k > 0 && at(k - 1) == Some(b'R'),
            Some(b'?') => trigraphs && at(k + 1) == Some(b'?') && at(k + 2) == Some(b'/'),
            _ => false,
        };
        let mut k = 0;
        let mut last = b' ';
        while let Some(c) = at(k) {
            match c {
                b'\n' | b'\r' => break,
                b'/' if at(k + 1) == Some(b'*') => {
                    // A comment (or `/*` in a literal) must close on this
                    // line; the first `*/` after it is where a comment
                    // opened here or anywhere before that `*/` closes,
                    // except one opened by its own `/` (`/*/`).
                    let mut q = k + 2;
                    loop {
                        match at(q) {
                            None | Some(b'\n' | b'\r') => return false,
                            Some(b'*') if at(q + 1) == Some(b'/') => break,
                            _ if opener(q) => return false,
                            _ => q += 1,
                        }
                    }
                    if q >= k + 3 && at(q - 1) == Some(b'/') {
                        return false;
                    }
                    k = q + 2;
                    last = b'/';
                    continue;
                }
                _ if opener(k) => return false,
                b' ' | b'\t' | 0x0B | 0x0C => {}
                _ => last = c,
            }
            k += 1;
        }
        if k == 0 || last == b'\\' {
            return false;
        }
        self.pos = from + k;
        self.at_line_start = false;
        self.leading_space = true;
        true
    }

    /// Diagnostics recorded so far.
    pub fn diagnostics(&self) -> &[LexDiag] {
        &self.diags
    }

    /// Removes and returns the diagnostics recorded so far (the
    /// preprocessor drops them for skipped `#if` groups).
    pub fn take_diagnostics(&mut self) -> Vec<LexDiag> {
        std::mem::take(&mut self.diags)
    }

    /// The token's spelling: its source text with line splices removed
    /// (and trigraphs replaced, if enabled); UCNs are kept as written.
    pub fn spelling(&self, tok: &Token) -> Cow<'a, str> {
        crate::spelling(self.sm, tok, &self.opts)
    }

    // ----- helpers ------------------------------------------------------

    fn loc(&self, off: usize) -> Loc {
        self.base.with_offset(off as u32)
    }

    fn span(&self, lo: usize, hi: usize) -> Span {
        Span::new(self.loc(lo), self.loc(hi))
    }

    fn diag(&mut self, kind: LexDiagKind, lo: usize, hi: usize) {
        let span = self.span(lo, hi);
        self.diags.push(LexDiag { kind, span });
    }

    /// Whether the U+FFFD at `p` replaced invalid UTF-8 at load time
    /// (rather than being spelled in the source).
    #[cold]
    fn is_replacement(&self, p: usize) -> bool {
        u32::try_from(p).is_ok_and(|p| self.bad.binary_search_by_key(&p, |r| r.offset).is_ok())
    }

    fn std_at_least(&self, std: Std) -> bool {
        self.opts.std >= std
    }

    /// C++ (not C): the C++-only tokens and literal forms.
    fn cxx(&self) -> bool {
        self.opts.c.is_none()
    }

    /// C at `std` or later.
    fn c_at_least(&self, std: CStd) -> bool {
        self.opts.c.is_some_and(|c| c >= std)
    }

    /// The identifier-character tables of the language (C++ and C23:
    /// XID; C11/C17: C11 Annex D; C99 and C89: C99 Annex D).
    fn id_tables(&self) -> IdTables {
        match self.opts.c {
            None | Some(CStd::C23) => IdTables::Xid,
            Some(CStd::C11 | CStd::C17) => IdTables::C11,
            Some(CStd::C89 | CStd::C99) => IdTables::C99,
        }
    }

    /// C89 (and gnu89): `\u`/`\U` are not universal-character-names.
    fn no_ucns(&self) -> bool {
        self.opts.c == Some(CStd::C89)
    }

    /// Outside a directive and `-E`, where clang diagnoses (or drops) a
    /// non-ASCII character that cannot appear where it is.
    fn loud_ids(&self) -> bool {
        !self.in_directive && !self.opts.preprocessed_output
    }

    /// The mathematical-notation extension warning, then `-Wc99-compat`,
    /// for the accepted identifier character `c` at `[lo, hi)` (clang
    /// `diagnoseExtensionInIdentifier`, `maybeDiagnoseIDCharCompat`).
    fn c99_compat(&mut self, c: char, first: bool, lo: usize, hi: usize) {
        if c.is_ascii() {
            return;
        }
        if self.id_tables().math_extension(c, first) {
            self.diag(LexDiagKind::MathematicalNotation { ch: c }, lo, hi);
        }
        if let Some(start) = c99_incompat(c, first) {
            self.diag(LexDiagKind::C99CompatUnicodeId { start }, lo, hi);
        }
    }

    fn take_line_flags(&mut self) -> TokFlags {
        let mut f = TokFlags::empty();
        if self.at_line_start {
            f |= TokFlags::START_OF_LINE;
        }
        if self.leading_space {
            f |= TokFlags::LEADING_SPACE;
        }
        self.at_line_start = false;
        self.leading_space = false;
        f
    }

    /// Scans `[a, b)` for line splices and trigraphs: returns whether any
    /// splice (or translated trigraph) occurs, and, if `diagnose`, records
    /// `BackslashNewlineSpace`, `TrigraphConverted` and (for literals,
    /// with trigraphs off) `TrigraphIgnored`.
    fn phase2(&mut self, a: usize, b: usize, literal: bool, diagnose: bool) -> bool {
        let Some(slice) = self.src.bytes.get(a..b) else {
            return false;
        };
        if !slice.iter().any(|&c| c == b'\\' || c == b'?') {
            return false;
        }
        let mut dirty = false;
        let mut i = a;
        while i < b {
            let c = self.src.bytes[i];
            if c == b'\\' || c == b'?' {
                if let Some((q, spaced)) = self.src.splice_at(i) {
                    dirty = true;
                    if diagnose {
                        if c == b'?' {
                            self.diag(LexDiagKind::TrigraphConverted { ch: '\\' }, i, i + 3);
                        }
                        if spaced {
                            self.diag(LexDiagKind::BackslashNewlineSpace, i, i + 1);
                        }
                    }
                    i = q;
                    continue;
                }
                if c == b'?'
                    && let Some(ch) = self.src.trigraph_at(i)
                {
                    if self.opts.trigraphs {
                        dirty = true;
                        if diagnose {
                            self.diag(LexDiagKind::TrigraphConverted { ch }, i, i + 3);
                        }
                        i += 3;
                        continue;
                    } else if diagnose && literal {
                        self.diag(LexDiagKind::TrigraphIgnored { ch }, i, i + 3);
                    }
                }
            }
            i += 1;
        }
        dirty
    }

    // ----- the main loop -----------------------------------------------

    /// Lexes and returns the next token. After the end of input, returns
    /// [`TokKind::Eof`] on every call.
    #[allow(clippy::should_implement_trait, reason = "never ends, so not an Iterator")]
    pub fn next(&mut self) -> Token {
        if let Some(tok) = self.skip_whitespace() {
            return tok;
        }
        if self.pos >= self.src.len() {
            return self.lex_eof();
        }
        let start = self.pos;
        let flags = self.take_line_flags();
        let mode = std::mem::take(&mut self.mode);
        self.raw_body = None;
        let (kind, end, extra) = self.lex_token(start, mode);
        // Every path consumes at least one character; guard anyway so that
        // a bug can never turn into an infinite loop.
        let end = if end > start {
            end
        } else {
            let mut e = start + 1;
            while e < self.src.len() && !self.src.text.is_char_boundary(e) {
                e += 1;
            }
            e
        };
        self.pos = end;
        let literal = matches!(
            kind,
            TokKind::CharLit(_)
                | TokKind::StringLit(_)
                | TokKind::RawStringLit(_)
                | TokKind::Unknown
        );
        if end > self.clean_until {
            self.clean_until = next_special(self.src.bytes, start);
        }
        let dirty = match self.raw_body.take() {
            _ if end <= self.clean_until => false,
            Some((a, b)) => {
                let x = self.phase2(start, a, literal, true);
                self.phase2(b, end, literal, true) || x
            }
            None => self.phase2(start, end, literal, true),
        };
        let mut flags = flags | extra;
        if dirty {
            flags |= TokFlags::NEEDS_CLEANING;
        }
        Token { kind, span: self.span(start, end), flags }
    }

    fn lex_eof(&mut self) -> Token {
        let end = self.src.len();
        if self.in_directive {
            self.in_directive = false;
            return Token {
                kind: TokKind::Eod,
                span: Span::empty(self.loc(end)),
                flags: TokFlags::empty(),
            };
        }
        if !self.eof_seen {
            self.eof_seen = true;
            if self.whole_file
                && !self.src.bytes.is_empty()
                && !matches!(self.src.bytes[end - 1], b'\n' | b'\r')
            {
                self.diag(LexDiagKind::NoNewlineAtEof, end, end);
            }
        }
        let flags = self.take_line_flags();
        Token { kind: TokKind::Eof, span: Span::empty(self.loc(end)), flags }
    }

    /// The non-ASCII arm of [`skip_whitespace`](Self::skip_whitespace):
    /// skips one character at `pos` if it is whitespace or dropped with an
    /// error, else returns `false` (a token starts there).
    fn skip_non_ascii_space(&mut self) -> bool {
        let (c, s) = self.src.decode(self.pos);
        match c {
            Some(c) if is_unicode_whitespace(c) && !self.opts.preprocessed_output => {
                self.diag(LexDiagKind::UnicodeWhitespace { ch: c }, self.pos, self.pos + s);
                self.leading_space = true;
                self.pos += s;
            }
            // Invalid UTF-8 outside a literal or comment is
            // dropped with an error (clang); a directive keeps
            // it as an unknown token.
            Some('\u{FFFD}') if !self.in_directive && self.is_replacement(self.pos) => {
                self.diag(LexDiagKind::InvalidUtf8, self.pos, self.pos + s);
                self.pos += s;
            }
            // A character that cannot start an identifier is
            // dropped with an error outside a directive and
            // `-E` (clang `LexUnicodeIdentifierStart`); the
            // next token loses its line-start and leading-space
            // flags, as in clang.
            Some(c)
                if self.loud_ids()
                    && !is_unicode_whitespace(c)
                    && !self.id_tables().allowed_initially(c) =>
            {
                let kind = if self.id_tables().allowed(c) {
                    LexDiagKind::CharNotAllowedInIdentifier { ch: c, at_start: true }
                } else {
                    LexDiagKind::InvalidChar { ch: c }
                };
                self.diag(kind, self.pos, self.pos + s);
                self.at_line_start = false;
                self.leading_space = false;
                self.pos += s;
            }
            _ => return false,
        }
        true
    }

    /// Skips whitespace, newlines and comments. Returns a token if one is
    /// produced on the way (`Eod`, or a kept `Comment`).
    fn skip_whitespace(&mut self) -> Option<Token> {
        loop {
            let b = self.src.byte(self.pos)?;
            match b {
                b' ' | b'\t' | 0x0B | 0x0C => {
                    self.leading_space = true;
                    self.pos += 1;
                    while self.src.byte(self.pos).is_some_and(|c| c == b' ' || c == b'\t') {
                        self.pos += 1;
                    }
                }
                b'\n' | b'\r' => {
                    if self.in_directive {
                        self.in_directive = false;
                        return Some(Token {
                            kind: TokKind::Eod,
                            span: Span::empty(self.loc(self.pos)),
                            flags: TokFlags::empty(),
                        });
                    }
                    self.pos += if b == b'\r' && self.src.byte(self.pos + 1) == Some(b'\n') {
                        2
                    } else {
                        1
                    };
                    self.at_line_start = true;
                    self.leading_space = false;
                }
                0 => {
                    self.diag(LexDiagKind::NullCharacter, self.pos, self.pos + 1);
                    self.leading_space = true;
                    self.pos += 1;
                }
                b'/' => {
                    let (c2, _) = self.src.decode(self.pos + 1);
                    match c2 {
                        Some('/') if !self.c89_line_comment() => return None,
                        Some('/') => {
                            if let Some(t) = self.line_comment() {
                                return Some(t);
                            }
                        }
                        Some('*') => {
                            if let Some(t) = self.block_comment() {
                                return Some(t);
                            }
                        }
                        _ => return None,
                    }
                }
                b'\\' | b'?' => {
                    if b == b'?' && !self.opts.trigraphs {
                        return None;
                    }
                    let q = self.src.skip_splices(self.pos);
                    if q == self.pos {
                        // A UCN naming whitespace is whitespace, except
                        // in `-E` (clang `CheckUnicodeWhitespace`).
                        if b == b'\\'
                            && !self.opts.preprocessed_output
                            && let Ucn::Valid { ch, end, .. } = self.scan_ucn(self.pos)
                            && is_unicode_whitespace(ch)
                        {
                            self.diag(LexDiagKind::UnicodeWhitespace { ch }, self.pos, end);
                            self.leading_space = true;
                            self.pos = end;
                            continue;
                        }
                        return None;
                    }
                    let (c, s) = self.src.decode(q);
                    let skip = match c {
                        None => {
                            self.diag(LexDiagKind::BackslashNewlineEof, self.pos, self.pos + 1);
                            true
                        }
                        Some(' ' | '\t' | '\u{0B}' | '\u{0C}' | '\n' | '\r' | '\0') => true,
                        Some('/') => matches!(self.src.decode(q + s).0, Some('/' | '*')),
                        Some(c) => is_unicode_whitespace(c),
                    };
                    if !skip {
                        return None;
                    }
                    self.phase2(self.pos, q, false, true);
                    self.pos = q;
                }
                0x80.. => {
                    if !self.skip_non_ascii_space() {
                        return None;
                    }
                }
                _ => return None,
            }
        }
    }

    /// Whether the `//` at `self.pos` starts a comment. Always, except in
    /// strict C89, which has no line comments (C89 3.1.9). There clang 19
    /// and 22 (`Lexer::LexTokenInternal`) lex `//` as two `/` in
    /// preprocessed output, skipped groups included. When compiling,
    /// `//` is a comment unless `*` follows it (`a //**/ b` is C89's
    /// `a / b`), and every `//` is one after the buffer's first line
    /// comment (spec/lex.md "C mode").
    fn c89_line_comment(&self) -> bool {
        if self.opts.c != Some(CStd::C89) || self.opts.gnu || self.c89_line_comments {
            return true;
        }
        let (_, s1) = self.src.decode(self.pos + 1);
        !self.opts.preprocessed_output && self.src.decode(self.pos + 1 + s1).0 != Some('*')
    }

    /// Lexes a `//` comment starting at `self.pos` ([lex.comment]). The
    /// newline ending it is not consumed.
    fn line_comment(&mut self) -> Option<Token> {
        let start = self.pos;
        if self.opts.c == Some(CStd::C89) && !self.opts.gnu && !self.c89_line_comments {
            // clang `ext_line_comment`, once per buffer (the preprocessor
            // drops it in a skipped group, as clang's raw lexer does).
            self.c89_line_comments = true;
            self.diag(LexDiagKind::C89LineComment, start, start + 2);
        }
        let (_, s1) = self.src.decode(start + 1);
        let body = start + 1 + s1;
        let bytes = self.src.bytes;
        let mut i = body;
        let mut warned = false;
        let end = loop {
            let Some(k) = bytes[i..].iter().position(|&c| c == b'\n' || c == b'\r') else {
                break bytes.len();
            };
            let nl = i + k;
            // Escaped newline? Walk back over horizontal whitespace.
            let mut j = nl;
            while j > body && is_hws(bytes[j - 1]) {
                j -= 1;
            }
            let bs = if j > body && bytes[j - 1] == b'\\' {
                Some(j - 1)
            } else if self.opts.trigraphs && j >= body + 3 && &bytes[j - 3..j] == b"??/" {
                Some(j - 3)
            } else {
                None
            };
            let Some(bs) = bs else {
                break nl;
            };
            if j != nl {
                self.diag(LexDiagKind::BackslashNewlineSpace, bs, bs + 1);
            }
            i = if bytes[nl] == b'\r' && bytes.get(nl + 1) == Some(&b'\n') {
                nl + 2
            } else {
                nl + 1
            };
            // clang `SkipLineComment`: no warning when the spliced-in line
            // carries on a comment block, i.e. a `/` is its first or second
            // character, or it is `//` after whitespace.
            let next = &bytes[i..];
            let ws = |c: &u8| is_hws(*c) || *c == b'\n' || *c == b'\r';
            let continues = next.first() == Some(&b'/')
                || next.get(1) == Some(&b'/')
                || next.first().is_some_and(ws)
                    && next
                        .iter()
                        .position(|c| !ws(c))
                        .is_some_and(|p| next[p..].starts_with(b"//"));
            if !warned && !continues {
                warned = true;
                self.diag(LexDiagKind::MultiLineLineComment, start, start + 2);
            }
        };
        self.pos = end;
        self.comment_token(start, end)
    }

    /// Lexes a `/* */` comment starting at `self.pos`.
    fn block_comment(&mut self) -> Option<Token> {
        let start = self.pos;
        let (_, s1) = self.src.decode(start + 1);
        let body = start + 1 + s1;
        let bytes = self.src.bytes;
        let mut i = body;
        let end = loop {
            let Some(k) = bytes[i..].iter().position(|&c| c == b'/') else {
                self.diag(LexDiagKind::UnterminatedBlockComment, start, start + 2);
                break bytes.len();
            };
            let slash = i + k;
            // The decoded character before `slash`, walking back over splices.
            let mut j = slash;
            let mut escaped = false;
            let mut spaced = false;
            while j > body && matches!(bytes[j - 1], b'\n' | b'\r') {
                let mut m = j - 1;
                if bytes[m] == b'\n' && m > body && bytes[m - 1] == b'\r' {
                    m -= 1;
                }
                let nl = m;
                while m > body && is_hws(bytes[m - 1]) {
                    m -= 1;
                }
                if m > body && bytes[m - 1] == b'\\' {
                    j = m - 1;
                } else if self.opts.trigraphs && m >= body + 3 && &bytes[m - 3..m] == b"??/" {
                    j = m - 3;
                } else {
                    break;
                }
                escaped = true;
                spaced |= m != nl;
            }
            if j > body && bytes[j - 1] == b'*' {
                if escaped {
                    self.diag(LexDiagKind::EscapedNewlineBlockCommentEnd, j - 1, slash + 1);
                    if spaced {
                        self.diag(LexDiagKind::BackslashNewlineSpace, j, j + 1);
                    }
                }
                break slash + 1;
            }
            if bytes.get(slash + 1) == Some(&b'*') && bytes.get(slash + 2) != Some(&b'/') {
                self.diag(LexDiagKind::NestedBlockComment, slash, slash + 2);
            }
            i = slash + 1;
        };
        self.pos = end;
        self.comment_token(start, end)
    }

    fn comment_token(&mut self, start: usize, end: usize) -> Option<Token> {
        if self.comments.is_some() {
            let span = self.span(start, end);
            self.comments.get_or_insert_default().push(span);
        }
        if self.opts.keep_comments {
            let mut flags = self.take_line_flags();
            if self.phase2(start, end, false, false) {
                flags |= TokFlags::NEEDS_CLEANING;
            }
            Some(Token { kind: TokKind::Comment, span: self.span(start, end), flags })
        } else {
            self.leading_space = true;
            None
        }
    }

    /// Lexes one token starting at `start` (not whitespace, not EOF).
    /// Returns its kind, end, and flags other than line/cleaning flags.
    fn lex_token(&mut self, start: usize, mode: LexMode) -> (TokKind, usize, TokFlags) {
        let (c, s) = self.src.decode(start);
        let Some(c) = c else {
            return (TokKind::Unknown, self.src.len(), TokFlags::empty());
        };
        let p1 = start + s;
        match c {
            'a'..='z' | 'A'..='Z' | '_' => self.lex_ident_or_literal(start, c, p1),
            '$' if self.opts.dollar_in_identifiers => self.lex_identifier(start, c, p1, false),
            '0'..='9' => self.lex_number(start, c, p1),
            '"' => {
                if mode == LexMode::HeaderName
                    && let Some(end) = self.scan_header_name(p1, '"')
                {
                    return (TokKind::HeaderName, end, TokFlags::empty());
                }
                self.lex_quoted(start, p1, '"', Encoding::Ordinary)
            }
            '\'' => self.lex_quoted(start, p1, '\'', Encoding::Ordinary),
            '<' if mode == LexMode::HeaderName => match self.scan_header_name(p1, '>') {
                Some(end) => (TokKind::HeaderName, end, TokFlags::empty()),
                // clang: an unterminated `<...` is a lone `<`.
                None => (TokKind::Punct(Punct::Less), p1, TokFlags::empty()),
            },
            '\\' => self.lex_backslash(start, p1),
            c if !c.is_ascii() => self.lex_non_ascii(start, c, p1),
            _ => self.lex_punct(start, c, p1),
        }
    }

    // ----- identifiers --------------------------------------------------

    fn lex_non_ascii(&mut self, start: usize, c: char, p1: usize) -> (TokKind, usize, TokFlags) {
        if c == '\u{FFFD}' && self.is_replacement(start) {
            // Invalid UTF-8 in a directive: an unknown token, no
            // diagnostic (clang).
            (TokKind::Other(c), p1, TokFlags::empty())
        } else if self.id_tables().allowed_initially(c) {
            if self.loud_ids() {
                self.c99_compat(c, true, start, p1);
            }
            self.lex_identifier(start, c, p1, false)
        } else {
            // [lex.pptoken]/2: a lone (ill-formed) "other" pp-token. Only
            // a directive or `-E` gets here: elsewhere `skip_whitespace`
            // dropped the character with its error.
            (TokKind::Other(c), p1, TokFlags::empty())
        }
    }

    fn lex_backslash(&mut self, start: usize, p1: usize) -> (TokKind, usize, TokFlags) {
        match self.scan_ucn(start) {
            Ucn::NotOne => {
                if self.no_ucns() && matches!(self.src.decode(p1).0, Some('u' | 'U')) {
                    self.diag(LexDiagKind::UcnNotValidInC89, start, p1);
                }
                (TokKind::Other('\\'), p1, TokFlags::empty())
            }
            Ucn::Malformed(kind) => {
                self.diag(kind, start, p1);
                (TokKind::Other('\\'), p1, TokFlags::empty())
            }
            Ucn::Invalid { end, kind } => {
                self.diag(kind, start, end);
                (TokKind::Unknown, end, TokFlags::empty())
            }
            Ucn::Valid { ch, end, ext } => {
                if let Some(ext) = ext {
                    self.diag(ext, start, end);
                }
                let dollar = ch == '$' && self.opts.dollar_in_identifiers;
                if !dollar && (ch.is_ascii() || !self.id_tables().allowed_initially(ch)) {
                    // [lex.pptoken]/2: a lone ill-formed pp-token, which
                    // the parser diagnoses (clang: `tok::unknown`, no
                    // lexer diagnostic, for a UCN that cannot start an
                    // identifier).
                    return (TokKind::Unknown, end, TokFlags::empty());
                }
                if self.loud_ids() {
                    self.c99_compat(ch, true, start, end);
                }
                self.lex_identifier(start, ch, end, true)
            }
        }
    }

    /// Lexes an identifier whose first (decoded) character `first` ends at
    /// `p1`; `first_ucn` if it was spelled as a UCN.
    fn lex_identifier(
        &mut self,
        start: usize,
        first: char,
        p1: usize,
        first_ucn: bool,
    ) -> (TokKind, usize, TokFlags) {
        let bytes = self.src.bytes;
        let mut p = p1;
        let simple_first = first.is_ascii() && first != '$' && !first_ucn && p1 == start + 1;
        if simple_first {
            p = ascii_ident_end(bytes, p);
            let slow = match bytes.get(p) {
                Some(b'\\' | b'$' | 0x80..) => true,
                Some(b'?') => self.opts.trigraphs,
                _ => false,
            };
            if !slow {
                let sym = self.interner.intern(&self.src.text[start..p]);
                return (TokKind::Identifier(sym), p, TokFlags::empty());
            }
        }
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.clear();
        if simple_first {
            scratch.push_str(&self.src.text[start..p]);
        } else {
            scratch.push(first);
        }
        let mut info =
            IdentInfo { has_ucn: first_ucn, dollar: first == '$', non_ascii: !first.is_ascii() };
        let end = self.ident_tail(p, Some(&mut scratch), &mut info);
        if info.dollar {
            self.diag(LexDiagKind::DollarInIdentifier, start, end);
        }
        if info.non_ascii && !is_nfc(&scratch) {
            self.diag(LexDiagKind::IdentifierNotNfc, start, end);
        }
        let sym = self.interner.intern(&scratch);
        self.scratch = scratch;
        let flags = if info.has_ucn { TokFlags::HAS_UCN } else { TokFlags::empty() };
        (TokKind::Identifier(sym), end, flags)
    }

    /// Consumes identifier-continue characters from `p` ([lex.name]),
    /// pushing decoded characters to `out`. Returns the end position.
    fn ident_tail(
        &mut self,
        mut p: usize,
        mut out: Option<&mut String>,
        info: &mut IdentInfo,
    ) -> usize {
        loop {
            let (c, s) = self.src.decode(p);
            let Some(c) = c else {
                return p;
            };
            if c.is_ascii() {
                if is_ascii_ident_continue(c as u8) || (c == '$' && self.opts.dollar_in_identifiers)
                {
                    info.dollar |= c == '$';
                } else if c == '\\' {
                    match self.scan_ucn(p) {
                        Ucn::Valid { ch, end, ext } => {
                            let dollar = ch == '$' && self.opts.dollar_in_identifiers;
                            if dollar || (!ch.is_ascii() && self.id_tables().allowed(ch)) {
                                info.dollar |= dollar;
                                info.non_ascii |= !ch.is_ascii();
                                if let Some(ext) = ext {
                                    self.diag(ext, p, end);
                                }
                                self.c99_compat(ch, false, p, end);
                                if let Some(o) = out.as_deref_mut() {
                                    o.push(ch);
                                }
                            } else if ch.is_ascii() || is_unicode_whitespace(ch) {
                                return p;
                            } else {
                                // Kept in the identifier for recovery
                                // (clang `tryConsumeIdentifierUCN`).
                                if let Some(ext) = ext {
                                    self.diag(ext, p, end);
                                }
                                if self.loud_ids() {
                                    self.diag(
                                        LexDiagKind::CharNotAllowedInIdentifier {
                                            ch,
                                            at_start: false,
                                        },
                                        p,
                                        end,
                                    );
                                }
                                info.non_ascii = true;
                                if let Some(o) = out.as_deref_mut() {
                                    o.push(ch);
                                }
                            }
                            info.has_ucn = true;
                            p = end;
                            continue;
                        }
                        Ucn::Invalid { end, kind } => {
                            self.diag(kind, p, end);
                            if let Some(o) = out.as_deref_mut() {
                                push_decoded(self.src, o, p, end);
                            }
                            info.has_ucn = true;
                            p = end;
                            continue;
                        }
                        Ucn::NotOne | Ucn::Malformed(_) => return p,
                    }
                } else {
                    return p;
                }
            } else if self.id_tables().allowed(c) {
                info.non_ascii = true;
                self.c99_compat(c, false, p, p + s);
            } else if is_unicode_whitespace(c) || (c == '\u{FFFD}' && self.is_replacement(p)) {
                // Invalid UTF-8 ends the identifier (clang).
                return p;
            } else {
                // Kept for recovery (clang `tryConsumeIdentifierUTF8Char`).
                info.non_ascii = true;
                if self.loud_ids() {
                    self.diag(
                        LexDiagKind::CharNotAllowedInIdentifier { ch: c, at_start: false },
                        p,
                        p + s,
                    );
                }
            }
            if let Some(o) = out.as_deref_mut() {
                o.push(c);
            }
            p += s;
        }
    }

    /// Reads a universal-character-name at `p`, where a `\` decodes
    /// ([lex.universal.char]). Records no diagnostics.
    fn scan_ucn(&self, p: usize) -> Ucn {
        let (_, s0) = self.src.decode(p);
        let mut q = p + s0;
        let (c1, s1) = self.src.decode(q);
        let Some(kind @ ('u' | 'U' | 'N')) = c1 else {
            return Ucn::NotOne;
        };
        if kind != 'N' && self.no_ucns() {
            return Ucn::NotOne;
        }
        q += s1;
        let (c2, s2) = self.src.decode(q);
        let modern = self.std_at_least(Std::Cxx23);
        if kind == 'N' {
            if c2 != Some('{') {
                return Ucn::Malformed(LexDiagKind::UcnIncomplete);
            }
            return self.scan_ucn_name(q + s2, modern);
        }
        if kind == 'u' && c2 == Some('{') {
            q += s2;
            let mut value: u32 = 0;
            let mut digits = 0u32;
            let mut overflow = false;
            loop {
                let (c, s) = self.src.decode(q);
                match c {
                    Some('}') => {
                        q += s;
                        break;
                    }
                    Some(h) if h.is_ascii_hexdigit() => {
                        let d = h.to_digit(16).unwrap_or(0);
                        match value.checked_mul(16).and_then(|v| v.checked_add(d)) {
                            Some(v) if v <= 0x10_FFFF => value = v,
                            _ => overflow = true,
                        }
                        digits += 1;
                        q += s;
                    }
                    _ => return Ucn::Malformed(LexDiagKind::UcnDelimitedIncomplete),
                }
            }
            if digits == 0 {
                return Ucn::Malformed(LexDiagKind::UcnDelimitedEmpty);
            }
            if overflow {
                return Ucn::Invalid { end: q, kind: LexDiagKind::UcnTooLarge };
            }
            return validate_ucn(value, q, (!modern).then_some(LexDiagKind::UcnDelimitedExtension));
        }
        let want = if kind == 'u' { 4 } else { 8 };
        let mut value: u32 = 0;
        let mut digits = 0;
        while digits < want {
            let (c, s) = self.src.decode(q);
            match c.and_then(|c| c.to_digit(16)) {
                Some(d) => {
                    value = value.wrapping_mul(16).wrapping_add(d);
                    digits += 1;
                    q += s;
                }
                None => break,
            }
        }
        match digits {
            0 => Ucn::Malformed(LexDiagKind::UcnNoDigits),
            d if d < want => Ucn::Malformed(LexDiagKind::UcnIncomplete),
            _ => validate_ucn(value, q, None),
        }
    }

    /// The rest of a `\N{…}` named universal-character-name, from `q`
    /// after the `{` ([lex.universal.char]/3).
    fn scan_ucn_name(&self, mut q: usize, modern: bool) -> Ucn {
        // Names are at most `MAX_NAME_LEN` bytes; a longer or non-ASCII
        // one cannot match but is still scanned to its `}`.
        let mut buf = [0u8; 128];
        let mut len = 0usize;
        let mut fits = true;
        loop {
            let (c, s) = self.src.decode(q);
            match c {
                Some('}') => {
                    q += s;
                    break;
                }
                None | Some('\n' | '\r') => {
                    return Ucn::Malformed(LexDiagKind::UcnDelimitedIncomplete);
                }
                Some(c) => {
                    if c.is_ascii() && len < buf.len() {
                        buf[len] = c as u8;
                        len += 1;
                    } else {
                        fits = false;
                    }
                    q += s;
                }
            }
        }
        if len == 0 && fits {
            return Ucn::Malformed(LexDiagKind::UcnDelimitedEmpty);
        }
        let ext = (!modern).then_some(LexDiagKind::UcnNamedExtension);
        let found = if fits {
            std::str::from_utf8(&buf[..len]).ok().and_then(lookup_char_name)
        } else {
            None
        };
        match found {
            Some(NameMatch::Exact(ch)) => validate_ucn(u32::from(ch), q, ext),
            Some(NameMatch::Loose(ch)) => match validate_ucn(u32::from(ch), q, ext) {
                Ucn::Valid { ch, end, .. } => {
                    Ucn::Valid { ch, end, ext: Some(LexDiagKind::UcnNameLooseMatch { ch }) }
                }
                other => other,
            },
            None => Ucn::Invalid { end: q, kind: LexDiagKind::UcnUnknownName },
        }
    }

    /// An identifier that may be an encoding prefix of a literal
    /// ([lex.ccon], [lex.string]).
    fn lex_ident_or_literal(
        &mut self,
        start: usize,
        c: char,
        p1: usize,
    ) -> (TokKind, usize, TokFlags) {
        let (kind, p, flags) = self.lex_identifier(start, c, p1, false);
        if !matches!(c, 'u' | 'U' | 'L' | 'R') || flags.contains(TokFlags::HAS_UCN) {
            return (kind, p, flags);
        }
        let TokKind::Identifier(sym) = kind else {
            return (kind, p, flags);
        };
        let (enc, raw) = match self.interner.as_str(sym) {
            "L" => (Encoding::Wide, false),
            "u8" => (Encoding::Utf8, false),
            "u" => (Encoding::Utf16, false),
            "U" => (Encoding::Utf32, false),
            "R" => (Encoding::Ordinary, true),
            "LR" => (Encoding::Wide, true),
            "u8R" => (Encoding::Utf8, true),
            "uR" => (Encoding::Utf16, true),
            "UR" => (Encoding::Utf32, true),
            _ => return (kind, p, flags),
        };
        let cxx11 = self.std_at_least(Std::Cxx11);
        // Raw strings: C++11, or a GNU C mode from gnu99 (clang 19).
        let raw_ok = if self.cxx() { cxx11 } else { self.opts.gnu && self.c_at_least(CStd::C99) };
        match self.src.decode(p) {
            (Some('"'), s) if raw && raw_ok => self.lex_raw_string(start, p, s, enc),
            (Some('"'), s) if !raw && (cxx11 || enc == Encoding::Wide) => {
                self.lex_quoted(start, p + s, '"', enc)
            }
            (Some('\''), s)
                if !raw
                    && match enc {
                        Encoding::Wide => true,
                        Encoding::Utf8 => self.std_at_least(Std::Cxx17),
                        _ => cxx11,
                    } =>
            {
                self.lex_quoted(start, p + s, '\'', enc)
            }
            _ => (kind, p, flags),
        }
    }

    // ----- pp-numbers ---------------------------------------------------

    /// [lex.ppnumber]. `first` (a digit, or `.` followed by a digit) ends
    /// at `p1`.
    fn lex_number(&mut self, start: usize, first: char, p1: usize) -> (TokKind, usize, TokFlags) {
        let mut p = p1;
        let mut prev = first;
        let mut flags = TokFlags::empty();
        loop {
            let (c, s) = self.src.decode(p);
            let Some(c) = c else { break };
            let ok = match c {
                '+' | '-' => match prev {
                    'e' | 'E' => true,
                    'p' | 'P' => {
                        self.std_at_least(Std::Cxx17)
                            || self.c_at_least(CStd::C99)
                            || self.looks_like_hex_float(start, p)
                    }
                    _ => false,
                },
                c if c.is_ascii_alphanumeric() || c == '_' || c == '.' => true,
                '\'' if self.std_at_least(Std::Cxx14) => {
                    let (c2, s2) = self.src.decode(p + s);
                    match c2 {
                        Some(d) if d.is_ascii_alphanumeric() || d == '_' => {
                            p += s + s2;
                            prev = d;
                            continue;
                        }
                        _ => false,
                    }
                }
                '\\' => {
                    if let Ucn::Valid { ch, end, ext } = self.scan_ucn(p)
                        && !ch.is_ascii()
                        && self.id_tables().allowed(ch)
                    {
                        if let Some(ext) = ext {
                            self.diag(ext, p, end);
                        }
                        flags |= TokFlags::HAS_UCN;
                        p = end;
                        prev = ch;
                        continue;
                    }
                    false
                }
                c => !c.is_ascii() && self.id_tables().allowed(c),
            };
            if !ok {
                break;
            }
            p += s;
            prev = c;
        }
        (TokKind::PpNumber, p, flags)
    }

    /// Before C++17, `p+`/`p-` continue a pp-number only for hexadecimal
    /// floating literals without a ud-suffix (clang's extension).
    fn looks_like_hex_float(&self, start: usize, p: usize) -> bool {
        let t = &self.src.bytes[start..p];
        (t.starts_with(b"0x") || t.starts_with(b"0X")) && !t.contains(&b'_')
    }

    // ----- literals ------------------------------------------------------

    /// A character or string literal whose opening `quote` ends at `p`.
    ///
    /// Escape sequences are only skipped here (`\` consumes the next
    /// character); they are validated and evaluated by
    /// [`crate::literal::decode_string`] / [`crate::literal::decode_char`],
    /// as in clang's `LiteralSupport`.
    fn lex_quoted(
        &mut self,
        start: usize,
        mut p: usize,
        quote: char,
        enc: Encoding,
    ) -> (TokKind, usize, TokFlags) {
        let is_string = quote == '"';
        if !is_string && self.src.decode(p).0 == Some('\'') {
            let (_, s) = self.src.decode(p);
            if !self.opts.asm {
                self.diag(LexDiagKind::EmptyCharLiteral, start, p + s);
            }
            return (TokKind::Unknown, p + s, TokFlags::empty());
        }
        loop {
            let (c, s) = self.src.decode(p);
            match c {
                None | Some('\n' | '\r') => {
                    let kind = if is_string {
                        LexDiagKind::UnterminatedString
                    } else {
                        LexDiagKind::UnterminatedChar
                    };
                    if !self.opts.asm {
                        self.diag(kind, start, p);
                    }
                    return (TokKind::Unknown, p, TokFlags::empty());
                }
                Some(c) if c == quote => {
                    p += s;
                    break;
                }
                Some('\\') => {
                    p += s;
                    let (c2, s2) = self.src.decode(p);
                    if !matches!(c2, None | Some('\n' | '\r')) {
                        p += s2;
                    }
                }
                Some('\0') => {
                    self.diag(LexDiagKind::NullCharacter, p, p + s);
                    p += s;
                }
                Some(_) => p += s,
            }
        }
        let (end, flags) = self.lex_ud_suffix(p, is_string);
        let kind = if is_string { TokKind::StringLit(enc) } else { TokKind::CharLit(enc) };
        (kind, end, flags)
    }

    /// A raw string literal ([lex.string]) whose `"` (after the `R`) is at
    /// `q` with decoded size `qs`. The delimiter and body are read from the
    /// raw source bytes: phase 1-2 transformations are reverted inside.
    ///
    /// The body is not inspected beyond finding its end; see
    /// [`crate::literal::decode_string`] for its value.
    fn lex_raw_string(
        &mut self,
        start: usize,
        q: usize,
        qs: usize,
        enc: Encoding,
    ) -> (TokKind, usize, TokFlags) {
        let bytes = self.src.bytes;
        let len = bytes.len();
        let d0 = q + qs;
        let mut i = d0;
        while i < len && i - d0 < 16 && self.is_d_char(bytes[i]) {
            i += 1;
        }
        if bytes.get(i) != Some(&b'(') {
            match bytes.get(i) {
                None => self.diag(LexDiagKind::UnterminatedRawString, start, start),
                Some(&b) if self.is_d_char(b) => {
                    self.diag(LexDiagKind::RawDelimiterTooLong, d0, i + 1);
                }
                Some(_) => {
                    let ch = self.src.text.get(i..).and_then(|t| t.chars().next()).unwrap_or('\0');
                    self.diag(LexDiagKind::RawDelimiterInvalidChar { ch }, i, i + ch.len_utf8());
                }
            }
            // clang's recovery: skip to the next `"`.
            let end = bytes[d0..].iter().position(|&b| b == b'"').map_or(len, |k| d0 + k + 1);
            // An error token: its spelling is cleaned like any other
            // `Unknown` (no raw body), keeping `spelling` == `clean`.
            return (TokKind::Unknown, end, TokFlags::empty());
        }
        let delim = &bytes[d0..i];
        let mut k = i + 1;
        let body_end = loop {
            let Some(j) = bytes[k..].iter().position(|&b| b == b')') else {
                self.diag(LexDiagKind::UnterminatedRawString, start, start);
                return (TokKind::Unknown, len, TokFlags::empty());
            };
            let close = k + j + 1;
            if bytes[close..].starts_with(delim) && bytes.get(close + delim.len()) == Some(&b'"') {
                break close + delim.len() + 1;
            }
            k = close;
        };
        self.raw_body = Some((d0, body_end));
        let (end, flags) = self.lex_ud_suffix(body_end, true);
        (TokKind::RawStringLit(enc), end, flags)
    }

    /// d-char ([lex.string]): a basic character other than space, `(`,
    /// `)`, `\`, tab, vertical tab, form feed and newline. `$`, `@` and
    /// `` ` `` joined the basic character set in C++26.
    fn is_d_char(&self, b: u8) -> bool {
        matches!(b, 0x21..=0x7E)
            && !matches!(b, b'(' | b')' | b'\\')
            && (self.std_at_least(Std::Cxx26) || !matches!(b, b'$' | b'@' | b'`'))
    }

    /// A ud-suffix after a literal ending at `p` ([lex.ext]), following
    /// clang: a suffix not starting with `_` is part of the token only if
    /// (C++14) it is a standard library suffix, see
    /// [`Self::is_std_string_suffix`]; otherwise
    /// the literal ends and `ReservedUdSuffix` is recorded.
    fn lex_ud_suffix(&mut self, p: usize, is_string: bool) -> (usize, TokFlags) {
        // C has no ud-suffixes.
        if !self.std_at_least(Std::Cxx11) || !self.cxx() {
            return (p, TokFlags::empty());
        }
        let (c, s) = self.src.decode(p);
        let Some(c) = c else {
            return (p, TokFlags::empty());
        };
        let (first_end, first_ch) = if c == '_' {
            (p + s, c)
        } else if c.is_ascii_alphabetic() || (c == '$' && self.opts.dollar_in_identifiers) {
            if !(is_string && self.std_at_least(Std::Cxx14) && self.is_std_string_suffix(p)) {
                self.diag(LexDiagKind::ReservedUdSuffix, p, p);
                return (p, TokFlags::empty());
            }
            (p + s, c)
        } else if c == '\\' {
            if let Ucn::Valid { ch, .. } = self.scan_ucn(p)
                && (is_xid_start(ch) || is_xid_continue(ch))
            {
                self.diag(LexDiagKind::ReservedUdSuffix, p, p);
            }
            return (p, TokFlags::empty());
        } else {
            if !c.is_ascii() && is_xid_start(c) {
                self.diag(LexDiagKind::ReservedUdSuffix, p, p);
            }
            return (p, TokFlags::empty());
        };
        let mut info = IdentInfo { dollar: first_ch == '$', ..IdentInfo::default() };
        let end = self.ident_tail(first_end, None, &mut info);
        if info.dollar {
            self.diag(LexDiagKind::DollarInIdentifier, p, end);
        }
        (end, TokFlags::UD_SUFFIX)
    }

    /// Whether the identifier at `p` is a standard library ud-suffix that
    /// clang lexes after a string literal (measured against clang 19).
    fn is_std_string_suffix(&self, p: usize) -> bool {
        let mut buf = [0u8; 3];
        let mut n = 0;
        let mut q = p;
        loop {
            let (c, s) = self.src.decode(q);
            match c {
                Some(c) if c.is_ascii() && is_ascii_ident_continue(c as u8) => {
                    if n == buf.len() {
                        return false;
                    }
                    buf[n] = c as u8;
                    n += 1;
                    q += s;
                }
                _ => break,
            }
        }
        // clang's lookahead accepts every standard library suffix here,
        // string or numeric (`operator""h` declares a numeric operator),
        // and `sv` from C++14; `d` and `y` from C++20.
        match &buf[..n] {
            b"s" | b"sv" | b"h" | b"min" | b"ms" | b"us" | b"ns" | b"il" | b"i" | b"if" => true,
            b"d" | b"y" => self.std_at_least(Std::Cxx20),
            _ => false,
        }
    }

    /// In header-name mode: the end of `<h-chars>` or `"q-chars"` whose
    /// opener ends at `p`, or `None` if no `close` precedes the newline.
    ///
    /// [lex.header]/2: `'`, `\\`, `//`, `/*` (and `"` in `<…>`) in a
    /// header name are conditionally-supported; each occurrence is
    /// accepted with [`LexDiagKind::HeaderNameSpecialChars`].
    fn scan_header_name(&mut self, start: usize, close: char) -> Option<usize> {
        let mut p = start;
        let end = loop {
            let (c, s) = self.src.decode(p);
            match c {
                None | Some('\n' | '\r') => return None,
                Some(c) if c == close => break p + s,
                Some(_) => p += s,
            }
        };
        let mut p = start;
        while p < end {
            let (c, s) = self.src.decode(p);
            let (next, s2) = self.src.decode(p + s);
            let (seq, n) = match (c, next) {
                (Some('\''), _) => ("'", s),
                (Some('\\'), _) => ("\\", s),
                (Some('"'), _) if close == '>' => ("\"", s),
                (Some('/'), Some('/')) => ("//", s + s2),
                (Some('/'), Some('*')) => ("/*", s + s2),
                _ => {
                    p += s.max(1);
                    continue;
                }
            };
            self.diag(LexDiagKind::HeaderNameSpecialChars { seq }, p, p + n);
            p += n;
        }
        Some(end)
    }

    // ----- punctuators --------------------------------------------------

    /// [lex.operators], [lex.digraph], and the `<::` / `[::` rules of
    /// [lex.pptoken]/3.
    #[allow(clippy::too_many_lines, reason = "one flat maximal-munch table")]
    fn lex_punct(&mut self, start: usize, c: char, p1: usize) -> (TokKind, usize, TokFlags) {
        use Punct as P;
        let (c2, s2) = self.src.decode(p1);
        let p2 = p1 + s2;
        let none = TokFlags::empty();
        let one = |p: Punct| (TokKind::Punct(p), p1, none);
        let two = |p: Punct| (TokKind::Punct(p), p2, none);
        let digraph = |p: Punct, end: usize| (TokKind::Punct(p), end, TokFlags::DIGRAPH);
        let cxx26 = self.opts.reflection && self.std_at_least(Std::Cxx26);
        let dg = self.opts.digraphs;
        match (c, c2) {
            ('{', _) => one(P::LBrace),
            ('}', _) => one(P::RBrace),
            ('[', Some(':')) if cxx26 => {
                // [lex.pptoken]/3: `[::` not followed by `:`, and `[:>`,
                // leave `[` alone.
                let (c3, s3) = self.src.decode(p2);
                match c3 {
                    Some(':') if self.src.decode(p2 + s3).0 != Some(':') => one(P::LSquare),
                    Some('>') => one(P::LSquare),
                    _ => two(P::SpliceOpen),
                }
            }
            ('[', _) => one(P::LSquare),
            (']', _) => one(P::RSquare),
            ('(', _) => one(P::LParen),
            (')', _) => one(P::RParen),
            (';', _) => one(P::Semi),
            (',', _) => one(P::Comma),
            ('~', _) => one(P::Tilde),
            ('?', _) => {
                if !self.opts.trigraphs
                    && let Some(ch) = self.src.trigraph_at(start)
                {
                    self.diag(LexDiagKind::TrigraphIgnored { ch }, start, start + 3);
                }
                one(P::Question)
            }
            ('.', Some('0'..='9')) => self.lex_number(start, '.', p1),
            ('.', Some('*')) if self.cxx() => two(P::PeriodStar),
            ('.', Some('.')) => match self.src.decode(p2) {
                (Some('.'), s3) => (TokKind::Punct(P::Ellipsis), p2 + s3, none),
                _ => one(P::Period),
            },
            ('.', _) => one(P::Period),
            // `::` is a token in every C mode too (C23 6.4.6; clang lexes it
            // before C23 for `[[gnu::x]]`).
            (':', Some(':')) => two(P::ColonColon),
            (':', Some('>')) if dg => digraph(P::RSquare, p2),
            (':', Some(']')) if cxx26 => two(P::SpliceClose),
            (':', _) => one(P::Colon),
            ('+', Some('+')) => two(P::PlusPlus),
            ('+', Some('=')) => two(P::PlusEqual),
            ('+', _) => one(P::Plus),
            ('-', Some('-')) => two(P::MinusMinus),
            ('-', Some('=')) => two(P::MinusEqual),
            ('-', Some('>')) => match self.src.decode(p2) {
                (Some('*'), s3) if self.cxx() => (TokKind::Punct(P::ArrowStar), p2 + s3, none),
                _ => two(P::Arrow),
            },
            ('-', _) => one(P::Minus),
            ('*', Some('=')) => two(P::StarEqual),
            ('*', _) => one(P::Star),
            ('/', Some('=')) => two(P::SlashEqual),
            ('/', _) => one(P::Slash),
            ('%', Some('=')) => two(P::PercentEqual),
            ('%', Some('>')) if dg => digraph(P::RBrace, p2),
            ('%', Some(':')) if dg => {
                let (c3, s3) = self.src.decode(p2);
                if c3 == Some('%') {
                    let (c4, s4) = self.src.decode(p2 + s3);
                    if c4 == Some(':') {
                        return digraph(P::HashHash, p2 + s3 + s4);
                    }
                }
                digraph(P::Hash, p2)
            }
            ('%', _) => one(P::Percent),
            ('^', Some('=')) => two(P::CaretEqual),
            ('^', Some('^')) if cxx26 => two(P::CaretCaret),
            ('^', _) => one(P::Caret),
            ('&', Some('&')) => two(P::AmpAmp),
            ('&', Some('=')) => two(P::AmpEqual),
            ('&', _) => one(P::Amp),
            ('|', Some('|')) => two(P::PipePipe),
            ('|', Some('=')) => two(P::PipeEqual),
            ('|', _) => one(P::Pipe),
            ('=', Some('=')) => two(P::EqualEqual),
            ('=', _) => one(P::Equal),
            ('!', Some('=')) => two(P::ExclaimEqual),
            ('!', _) => one(P::Exclaim),
            ('<', Some('<')) => match self.src.decode(p2) {
                (Some('='), s3) => (TokKind::Punct(P::LessLessEqual), p2 + s3, none),
                _ => two(P::LessLess),
            },
            ('<', Some('=')) => match self.src.decode(p2) {
                (Some('>'), s3) if self.std_at_least(Std::Cxx20) => {
                    (TokKind::Punct(P::Spaceship), p2 + s3, none)
                }
                (Some('>'), s3) if self.cxx() => {
                    self.diag(LexDiagKind::SpaceshipCompat, start, p2 + s3);
                    two(P::LessEqual)
                }
                _ => two(P::LessEqual),
            },
            ('<', Some(':')) if dg => {
                // [lex.pptoken]/3.2: `<::` not followed by `:` or `>` is `<` `::`.
                if self.std_at_least(Std::Cxx11) && self.cxx() {
                    let (c3, s3) = self.src.decode(p2);
                    if c3 == Some(':') && !matches!(self.src.decode(p2 + s3).0, Some(':' | '>')) {
                        return one(P::Less);
                    }
                }
                digraph(P::LSquare, p2)
            }
            ('<', Some('%')) if dg => digraph(P::LBrace, p2),
            ('<', _) => one(P::Less),
            ('>', Some('=')) => two(P::GreaterEqual),
            ('>', Some('>')) => match self.src.decode(p2) {
                (Some('='), s3) => (TokKind::Punct(P::GreaterGreaterEqual), p2 + s3, none),
                _ => two(P::GreaterGreater),
            },
            ('>', _) => one(P::Greater),
            ('#', Some('#')) => two(P::HashHash),
            ('#', _) => one(P::Hash),
            // [lex.pptoken]/2: any other single non-whitespace character.
            (c, _) => (TokKind::Other(c), p1, none),
        }
    }
}

fn validate_ucn(cp: u32, end: usize, ext: Option<LexDiagKind>) -> Ucn {
    if (0xD800..=0xDFFF).contains(&cp) || cp > 0x10_FFFF {
        return Ucn::Invalid { end, kind: LexDiagKind::UcnInvalidCodePoint { cp } };
    }
    if cp < 0xA0 && !matches!(cp, 0x24 | 0x40 | 0x60) {
        return Ucn::Invalid { end, kind: LexDiagKind::UcnBasicOrControl { cp } };
    }
    match char::from_u32(cp) {
        Some(ch) => Ucn::Valid { ch, end, ext },
        None => Ucn::Invalid { end, kind: LexDiagKind::UcnInvalidCodePoint { cp } },
    }
}

/// Pushes the decoded characters of `[a, b)` (an invalid UCN kept as
/// written in an identifier's symbol).
fn push_decoded(src: Src<'_>, out: &mut String, mut a: usize, b: usize) {
    while a < b {
        let (c, s) = src.decode(a);
        let Some(c) = c else { break };
        out.push(c);
        a += s.max(1);
    }
}
