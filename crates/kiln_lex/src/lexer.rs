//! The pp-token lexer ([lex.pptoken]).
#![allow(clippy::many_single_char_names, reason = "p/q positions, c/s decoded char and size")]

use std::borrow::Cow;
use std::ops::Range;

use kiln_source::{FileId, Loc, SourceManager, Span};

use crate::diag::{LexDiag, LexDiagKind};
use crate::interner::Interner;
use crate::options::{LexMode, LexOptions, Std};
use crate::phase::{Src, is_hws};
use crate::token::{Encoding, Punct, TokFlags, TokKind, Token};
use crate::unicode::{
    is_ascii_ident_continue, is_nfc_qc_no, is_unicode_whitespace, is_xid_continue, is_xid_start,
    lookup_char_name,
};

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
    not_nfc: bool,
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
    diags: Vec<LexDiag>,
    scratch: String,
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
            diags: Vec::new(),
            scratch: String::new(),
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

    fn std_at_least(&self, std: Std) -> bool {
        self.opts.std >= std
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
            TokKind::CharLit(_) | TokKind::StringLit(_) | TokKind::RawStringLit(_) | TokKind::Unknown
        );
        let dirty = match self.raw_body.take() {
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
            return Token { kind: TokKind::Eod, span: Span::empty(self.loc(end)), flags: TokFlags::empty() };
        }
        if !self.eof_seen {
            self.eof_seen = true;
            if self.whole_file && !self.src.bytes.is_empty() && !matches!(self.src.bytes[end - 1], b'\n' | b'\r') {
                self.diag(LexDiagKind::NoNewlineAtEof, end, end);
            }
        }
        let flags = self.take_line_flags();
        Token { kind: TokKind::Eof, span: Span::empty(self.loc(end)), flags }
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
                    self.pos += if b == b'\r' && self.src.byte(self.pos + 1) == Some(b'\n') { 2 } else { 1 };
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
                    let (c, s) = self.src.decode(self.pos);
                    match c {
                        Some(c) if is_unicode_whitespace(c) => {
                            self.diag(LexDiagKind::UnicodeWhitespace { ch: c }, self.pos, self.pos + s);
                            self.leading_space = true;
                            self.pos += s;
                        }
                        _ => return None,
                    }
                }
                _ => return None,
            }
        }
    }

    /// Lexes a `//` comment starting at `self.pos` ([lex.comment]). The
    /// newline ending it is not consumed.
    fn line_comment(&mut self) -> Option<Token> {
        let start = self.pos;
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
            i = if bytes[nl] == b'\r' && bytes.get(nl + 1) == Some(&b'\n') { nl + 2 } else { nl + 1 };
            if !warned && !bytes[i..].starts_with(b"//") {
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
        if is_xid_start(c) {
            self.lex_identifier(start, c, p1, false)
        } else if is_xid_continue(c) {
            // [lex.pptoken]/2: not XID_Start, so it cannot begin an
            // identifier; it is a lone (ill-formed) "other" pp-token.
            self.diag(LexDiagKind::CharNotAllowedInIdentifier { ch: c }, start, p1);
            (TokKind::Other(c), p1, TokFlags::empty())
        } else {
            self.diag(LexDiagKind::InvalidChar { ch: c }, start, p1);
            (TokKind::Other(c), p1, TokFlags::empty())
        }
    }

    fn lex_backslash(&mut self, start: usize, p1: usize) -> (TokKind, usize, TokFlags) {
        match self.scan_ucn(start) {
            Ucn::NotOne => (TokKind::Other('\\'), p1, TokFlags::empty()),
            Ucn::Malformed(kind) => {
                self.diag(kind, start, p1);
                (TokKind::Other('\\'), p1, TokFlags::empty())
            }
            Ucn::Invalid { end, kind } => {
                self.diag(kind, start, end);
                (TokKind::Unknown, end, TokFlags::empty())
            }
            Ucn::Valid { ch, end, ext } => {
                let ident_start = is_xid_start(ch) || (ch == '$' && self.opts.dollar_in_identifiers);
                if !ident_start && !is_xid_continue(ch) {
                    self.diag(LexDiagKind::UcnInvalidCodePoint { cp: u32::from(ch) }, start, end);
                    return (TokKind::Unknown, end, TokFlags::empty());
                }
                if let Some(ext) = ext {
                    self.diag(ext, start, end);
                }
                if !ident_start {
                    // [lex.pptoken]/2: a lone ill-formed pp-token.
                    self.diag(LexDiagKind::CharNotAllowedInIdentifier { ch }, start, end);
                    return (TokKind::Unknown, end, TokFlags::HAS_UCN);
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
            while p < bytes.len() && is_ascii_ident_continue(bytes[p]) {
                p += 1;
            }
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
        let mut info = IdentInfo {
            has_ucn: first_ucn,
            dollar: first == '$',
            not_nfc: is_nfc_qc_no(first),
        };
        let end = self.ident_tail(p, Some(&mut scratch), &mut info);
        if info.dollar {
            self.diag(LexDiagKind::DollarInIdentifier, start, end);
        }
        if info.not_nfc {
            self.diag(LexDiagKind::IdentifierNotNfc, start, end);
        }
        let sym = self.interner.intern(&scratch);
        self.scratch = scratch;
        let flags = if info.has_ucn { TokFlags::HAS_UCN } else { TokFlags::empty() };
        (TokKind::Identifier(sym), end, flags)
    }

    /// Consumes identifier-continue characters from `p` ([lex.name]),
    /// pushing decoded characters to `out`. Returns the end position.
    fn ident_tail(&mut self, mut p: usize, mut out: Option<&mut String>, info: &mut IdentInfo) -> usize {
        loop {
            let (c, s) = self.src.decode(p);
            let Some(c) = c else {
                return p;
            };
            if c.is_ascii() {
                if is_ascii_ident_continue(c as u8) || (c == '$' && self.opts.dollar_in_identifiers) {
                    info.dollar |= c == '$';
                } else if c == '\\' {
                    match self.scan_ucn(p) {
                        Ucn::Valid { ch, end, ext } => {
                            if is_xid_continue(ch) || (ch == '$' && self.opts.dollar_in_identifiers) {
                                info.dollar |= ch == '$';
                                info.not_nfc |= is_nfc_qc_no(ch);
                                if let Some(ext) = ext {
                                    self.diag(ext, p, end);
                                }
                                if let Some(o) = out.as_deref_mut() {
                                    o.push(ch);
                                }
                            } else if is_unicode_whitespace(ch) {
                                return p;
                            } else {
                                self.diag(LexDiagKind::UcnInvalidCodePoint { cp: u32::from(ch) }, p, end);
                                if let Some(o) = out.as_deref_mut() {
                                    push_decoded(self.src, o, p, end);
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
            } else if is_xid_continue(c) {
                info.not_nfc |= is_nfc_qc_no(c);
            } else if is_unicode_whitespace(c) {
                return p;
            } else {
                self.diag(LexDiagKind::CharNotAllowedInIdentifier { ch: c }, p, p + s);
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
        q += s1;
        let (c2, s2) = self.src.decode(q);
        let modern = self.std_at_least(Std::Cxx23);
        if kind == 'N' {
            if c2 != Some('{') {
                return Ucn::Malformed(LexDiagKind::UcnIncomplete);
            }
            q += s2;
            // Rare: a small allocation is fine here.
            let mut name = String::new();
            loop {
                let (c, s) = self.src.decode(q);
                match c {
                    Some('}') => {
                        q += s;
                        break;
                    }
                    Some(c) if c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_') => {
                        name.push(c);
                        q += s;
                    }
                    _ => return Ucn::Malformed(LexDiagKind::UcnDelimitedIncomplete),
                }
            }
            if name.is_empty() {
                return Ucn::Malformed(LexDiagKind::UcnDelimitedEmpty);
            }
            let ext = (!modern).then_some(LexDiagKind::UcnNamedExtension);
            return match lookup_char_name(&name) {
                Some(ch) => validate_ucn(u32::from(ch), q, ext),
                None => Ucn::Invalid { end: q, kind: LexDiagKind::UcnUnknownName },
            };
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

    /// An identifier that may be an encoding prefix of a literal
    /// ([lex.ccon], [lex.string]).
    fn lex_ident_or_literal(&mut self, start: usize, c: char, p1: usize) -> (TokKind, usize, TokFlags) {
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
        match self.src.decode(p) {
            (Some('"'), s) if raw && cxx11 => self.lex_raw_string(start, p, s, enc),
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
                    'p' | 'P' => self.std_at_least(Std::Cxx17) || self.looks_like_hex_float(start, p),
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
                        && is_xid_continue(ch)
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
                c => !c.is_ascii() && is_xid_continue(c),
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

    // ----- literals (TODO(WP-011): contents are only skipped) ------------

    /// A character or string literal whose opening `quote` ends at `p`.
    ///
    /// TODO(WP-011): escape sequences are only skipped (`\` consumes the
    /// next character), not validated; values are computed later.
    fn lex_quoted(&mut self, start: usize, mut p: usize, quote: char, enc: Encoding) -> (TokKind, usize, TokFlags) {
        let is_string = quote == '"';
        if !is_string && self.src.decode(p).0 == Some('\'') {
            let (_, s) = self.src.decode(p);
            self.diag(LexDiagKind::EmptyCharLiteral, start, p + s);
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
                    self.diag(kind, start, p);
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
    /// TODO(WP-011): the body is not inspected beyond finding its end.
    fn lex_raw_string(&mut self, start: usize, q: usize, qs: usize, enc: Encoding) -> (TokKind, usize, TokFlags) {
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
                Some(&b) if self.is_d_char(b) => self.diag(LexDiagKind::RawDelimiterTooLong, d0, i + 1),
                Some(_) => {
                    let ch = self.src.text.get(i..).and_then(|t| t.chars().next()).unwrap_or('\0');
                    self.diag(LexDiagKind::RawDelimiterInvalidChar { ch }, i, i + ch.len_utf8());
                }
            }
            // clang's recovery: skip to the next `"`.
            let end = bytes[d0..].iter().position(|&b| b == b'"').map_or(len, |k| d0 + k + 1);
            self.raw_body = Some((d0, end));
            return (TokKind::Unknown, end, TokFlags::empty());
        }
        let delim = &bytes[d0..i];
        let mut k = i + 1;
        let body_end = loop {
            let Some(j) = bytes[k..].iter().position(|&b| b == b')') else {
                self.diag(LexDiagKind::UnterminatedRawString, start, start);
                self.raw_body = Some((d0, len));
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
    /// it is a standard string suffix (`s`, C++14; `sv`, C++17); otherwise
    /// the literal ends and `ReservedUdSuffix` is recorded.
    fn lex_ud_suffix(&mut self, p: usize, is_string: bool) -> (usize, TokFlags) {
        if !self.std_at_least(Std::Cxx11) {
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

    /// Whether the identifier at `p` is exactly `s` or (C++17) `sv`.
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
        match &buf[..n] {
            b"s" => true,
            b"sv" => self.std_at_least(Std::Cxx17),
            _ => false,
        }
    }

    /// In header-name mode: the end of `<h-chars>` or `"q-chars"` whose
    /// opener ends at `p`, or `None` if no `close` precedes the newline.
    ///
    /// TODO(WP-011): conditionally-supported characters in header names
    /// (`'`, `\`, `//`, `/*`) are accepted without diagnosis.
    fn scan_header_name(&self, mut p: usize, close: char) -> Option<usize> {
        loop {
            let (c, s) = self.src.decode(p);
            match c {
                None | Some('\n' | '\r') => return None,
                Some(c) if c == close => return Some(p + s),
                Some(_) => p += s,
            }
        }
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
        let cxx26 = self.std_at_least(Std::Cxx26);
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
            ('.', Some('*')) => two(P::PeriodStar),
            ('.', Some('.')) => match self.src.decode(p2) {
                (Some('.'), s3) => (TokKind::Punct(P::Ellipsis), p2 + s3, none),
                _ => one(P::Period),
            },
            ('.', _) => one(P::Period),
            (':', Some(':')) => two(P::ColonColon),
            (':', Some('>')) => digraph(P::RSquare, p2),
            (':', Some(']')) if cxx26 => two(P::SpliceClose),
            (':', _) => one(P::Colon),
            ('+', Some('+')) => two(P::PlusPlus),
            ('+', Some('=')) => two(P::PlusEqual),
            ('+', _) => one(P::Plus),
            ('-', Some('-')) => two(P::MinusMinus),
            ('-', Some('=')) => two(P::MinusEqual),
            ('-', Some('>')) => match self.src.decode(p2) {
                (Some('*'), s3) => (TokKind::Punct(P::ArrowStar), p2 + s3, none),
                _ => two(P::Arrow),
            },
            ('-', _) => one(P::Minus),
            ('*', Some('=')) => two(P::StarEqual),
            ('*', _) => one(P::Star),
            ('/', Some('=')) => two(P::SlashEqual),
            ('/', _) => one(P::Slash),
            ('%', Some('=')) => two(P::PercentEqual),
            ('%', Some('>')) => digraph(P::RBrace, p2),
            ('%', Some(':')) => {
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
                (Some('>'), s3) if self.std_at_least(Std::Cxx20) => (TokKind::Punct(P::Spaceship), p2 + s3, none),
                (Some('>'), s3) => {
                    self.diag(LexDiagKind::SpaceshipCompat, start, p2 + s3);
                    two(P::LessEqual)
                }
                _ => two(P::LessEqual),
            },
            ('<', Some(':')) => {
                // [lex.pptoken]/3.2: `<::` not followed by `:` or `>` is `<` `::`.
                if self.std_at_least(Std::Cxx11) {
                    let (c3, s3) = self.src.decode(p2);
                    if c3 == Some(':') && !matches!(self.src.decode(p2 + s3).0, Some(':' | '>')) {
                        return one(P::Less);
                    }
                }
                digraph(P::LSquare, p2)
            }
            ('<', Some('%')) => digraph(P::LBrace, p2),
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
