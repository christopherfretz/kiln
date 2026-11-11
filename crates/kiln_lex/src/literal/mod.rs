//! Literal evaluation: the values of character, string and numeric
//! literals ([lex.ccon], [lex.string], [lex.icon], [lex.fcon]).
//!
//! The [`crate::Lexer`] only finds where literals end; everything that
//! depends on their contents happens here, on the token's *spelling* (see
//! [`crate::spelling`]): escape sequences are validated and decoded,
//! characters are encoded into the code units of the literal's encoding,
//! character-literal values are computed as clang computes them, and
//! pp-numbers are classified. Like clang's `LiteralSupport`, problems are
//! reported when a literal is *evaluated*, so a literal in a skipped group
//! or an unused macro is never diagnosed.
//!
//! Diagnostics are [`LitDiag`]s with byte ranges into the spelling;
//! [`LitDiag::locate`] maps them to source spans. Nothing here panics on
//! any input string, well-formed or not.
//!
//! Floating values beyond [`FloatLiteral::parse_f64`] /
//! [`FloatLiteral::parse_f32`] (`long double`, `__float128`, the extended
//! floating types, and the exactness the constant evaluator needs) are
//! sema's and the oven's: this module hands them the digits.

mod number;

pub use number::{
    FloatLiteral, FloatSuffix, IntSize, IntSuffix, IntegerLiteral, Number, NumberClass, Radix,
    classify_number, is_valid_ud_suffix,
};

use std::fmt::Write as _;
use std::ops::Range;

use kiln_source::{SourceManager, Span};

use crate::diag::{LexDiag, LexDiagKind, LexSeverity};
use crate::options::{CStd, LexOptions, Std};
use crate::token::{Encoding, TokFlags, Token};
use crate::unicode::{NameMatch, lookup_char_name};

/// Target properties that literal values depend on.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EncodingParams {
    /// Whether plain `char` is signed.
    pub char_signed: bool,
    /// Width of `wchar_t` in bits: 16 or 32 (anything else is treated
    /// as 32). Wide literals are UTF-16 or UTF-32 accordingly.
    pub wchar_bits: u8,
    /// Whether `wchar_t` is signed.
    pub wchar_signed: bool,
}

impl EncodingParams {
    /// x86-64 Linux: signed `char`, 32-bit signed `wchar_t`.
    pub const LINUX_X86_64: EncodingParams =
        EncodingParams { char_signed: true, wchar_bits: 32, wchar_signed: true };
    /// `AArch64` Linux: unsigned `char`, 32-bit unsigned `wchar_t`.
    pub const LINUX_AARCH64: EncodingParams =
        EncodingParams { char_signed: false, wchar_bits: 32, wchar_signed: false };
    /// Windows (x86-64): signed `char`, 16-bit unsigned `wchar_t`.
    pub const WINDOWS: EncodingParams =
        EncodingParams { char_signed: true, wchar_bits: 16, wchar_signed: false };

    fn wchar_width(self) -> u32 {
        if self.wchar_bits == 16 { 16 } else { 32 }
    }
}

impl Default for EncodingParams {
    /// [`EncodingParams::LINUX_X86_64`].
    fn default() -> Self {
        EncodingParams::LINUX_X86_64
    }
}

/// What literal evaluation depends on. The ordinary and `u8` literal
/// encoding is always UTF-8 (kiln's execution character set).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LiteralOptions {
    /// The language standard.
    pub std: Std,
    /// The target's character types.
    pub params: EncodingParams,
    /// `-f[no-]char8_t` (clang `LangOptions::Char8`): a UTF-8 literal has
    /// `char8_t` type ([lex.ccon]/3, P0482R6); `None` is the default, C++20
    /// and later.
    pub char8: Option<bool>,
    /// `-fclang-compat=19`: clang 19's escape-sequence recovery. An empty
    /// `\x{}` / `\o{}` leaves its `}` unconsumed (so `'\x{}'` is a
    /// multicharacter literal) and is not then an invalid escape in an
    /// unevaluated string; clang 22 consumes the `}` and reports both.
    pub clang19_escapes: bool,
    /// C mode ([`LexOptions::c`]): the C rules for universal character
    /// names in literals (C11 6.4.3p2 until C23; none in C89).
    pub c: Option<CStd>,
}

impl Default for LiteralOptions {
    /// C++26 with the default [`EncodingParams`].
    fn default() -> Self {
        LiteralOptions::new(Std::Cxx26, EncodingParams::default())
    }
}

impl LiteralOptions {
    /// Options for `std` and `params`.
    pub const fn new(std: Std, params: EncodingParams) -> Self {
        LiteralOptions { std, params, char8: None, clang19_escapes: false, c: None }
    }

    /// Whether a UTF-8 literal has `char8_t` type: the `char8` field, else
    /// from C++20 on.
    #[must_use]
    pub fn char8(&self) -> bool {
        self.char8.unwrap_or(self.std >= Std::Cxx20)
    }
}

/// A problem found while evaluating a literal, located by byte offsets
/// into the literal's spelling.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct LitDiag {
    /// What went wrong.
    pub kind: LexDiagKind,
    /// The offending bytes of the spelling.
    pub range: Range<u32>,
}

impl LitDiag {
    /// This diagnostic as a [`LexDiag`] on the token `tok`, whose source
    /// text (`SourceManager::span_text(tok.span)`) is `text`; `opts` are
    /// the options it was lexed with (for trigraphs).
    pub fn locate(&self, tok: &Token, text: &str, opts: &LexOptions) -> LexDiag {
        let map = |o: u32| {
            let off = crate::spelling::source_offset(text, tok.kind, opts.trigraphs, o as usize);
            tok.span.lo.with_offset(u32::try_from(off).unwrap_or(u32::MAX))
        };
        let lo = map(self.range.start);
        let hi = if self.range.end == self.range.start { lo } else { map(self.range.end) };
        LexDiag { kind: self.kind, span: Span::new(lo, hi) }
    }
}

/// Source bytes of a literal that are not valid UTF-8: the file was
/// decoded with each maximal invalid sequence replaced by U+FFFD
/// (`kiln_source::LoadReport`), and this records which U+FFFDs of the
/// spelling stand for such bytes, and which bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InvalidUtf8<'a> {
    /// Byte offset of the U+FFFD (3 bytes) in the literal's spelling.
    pub offset: u32,
    /// The source bytes it replaced.
    pub bytes: &'a [u8],
}

/// The U+FFFDs of literal token `tok`'s `spelling` ([`crate::spelling`])
/// that replaced invalid UTF-8 when its file was loaded. Empty (and
/// cheap) for a spelling without U+FFFD. A token spelled in a scratch
/// buffer (`#`, `##`) has none: its text was re-spelled.
pub fn invalid_utf8<'s>(
    sm: &'s SourceManager,
    tok: &Token,
    spelling: &str,
    opts: &LexOptions,
) -> Vec<InvalidUtf8<'s>> {
    // U+FFFD is EF BF BD.
    if !spelling.as_bytes().contains(&0xEF) {
        return Vec::new();
    }
    invalid_utf8_slow(sm, tok, spelling, opts.trigraphs)
}

#[cold]
fn invalid_utf8_slow<'s>(
    sm: &'s SourceManager,
    tok: &Token,
    spelling: &str,
    trigraphs: bool,
) -> Vec<InvalidUtf8<'s>> {
    let Some((file, start)) = sm.offset_of(sm.spelling_loc(tok.span.lo)) else {
        return Vec::new();
    };
    let reps = &sm.load_report(file).replacements;
    if reps.is_empty() {
        return Vec::new();
    }
    let text = sm.span_text(tok.span).unwrap_or("");
    let cleaned = tok.flags.contains(TokFlags::NEEDS_CLEANING);
    let mut out = Vec::new();
    for (k, _) in spelling.match_indices('\u{FFFD}') {
        let off =
            if cleaned { crate::spelling::source_offset(text, tok.kind, trigraphs, k) } else { k };
        let Ok(off) = u32::try_from(off) else { break };
        let at = start.saturating_add(off);
        if let Ok(i) = reps.binary_search_by_key(&at, |r| r.offset) {
            let offset = u32::try_from(k).unwrap_or(u32::MAX);
            out.push(InvalidUtf8 { offset, bytes: &reps[i].bytes });
        }
    }
    out
}

/// Where a literal's spelling came from, for the decoders that report
/// invalid UTF-8 ([`decode_string_from_source`],
/// [`decode_char_from_source`]).
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
pub struct SourceBytes<'a> {
    /// The spelling's U+FFFDs that replaced invalid UTF-8, ascending by
    /// offset (empty for valid source).
    pub invalid: &'a [InvalidUtf8<'a>],
    /// The literal is part of a concatenation whose encoding is not
    /// ordinary ([lex.string.concat]): invalid UTF-8 in an unprefixed
    /// piece is then an error too, as in clang.
    pub concat_prefixed: bool,
}

/// How a literal treats invalid UTF-8 in its source.
#[derive(Clone, Copy)]
struct Bad<'a> {
    seqs: &'a [InvalidUtf8<'a>],
    /// Not an ordinary literal: an error, and the value is not kept.
    error: bool,
    /// A string literal (else a character literal).
    string: bool,
}

impl Bad<'_> {
    /// The invalid sequences in spelling bytes `range`.
    fn within(&self, range: Range<usize>) -> &[InvalidUtf8<'_>] {
        let first = self.seqs.partition_point(|s| (s.offset as usize) < range.start);
        let n = self.seqs[first..].iter().take_while(|s| (s.offset as usize) < range.end).count();
        &self.seqs[first..first + n]
    }

    /// Diagnoses the invalid sequences `seqs` of one fragment (clang's
    /// `CopyStringFragment` / `CharLiteralParser` loop): a string
    /// literal's diagnostic is at the first, highlighting each; a
    /// character literal's is at the token start.
    fn diagnose(&self, seqs: &[InvalidUtf8<'_>], diags: &mut Diags) {
        let kind = LexDiagKind::InvalidEncoding { string: self.string, error: self.error };
        if !self.string {
            diags.push(kind, 0, 0);
            return;
        }
        for (k, q) in seqs.iter().enumerate() {
            let lo = q.offset as usize;
            let kind = if k == 0 { kind } else { LexDiagKind::InvalidEncodingRange };
            diags.push(kind, lo, lo + '\u{FFFD}'.len_utf8());
        }
    }
}

/// Collected diagnostics plus whether any is an error.
#[derive(Default, Debug)]
struct Diags {
    list: Vec<LitDiag>,
    error: bool,
    /// clang's per-literal `HadError` flag, which gates some later
    /// diagnostics. Not every error sets it (see [`Diags::push_soft`]).
    sticky: bool,
}

impl Diags {
    fn push(&mut self, kind: LexDiagKind, lo: usize, hi: usize) {
        self.sticky |= kind.severity() == LexSeverity::Error;
        self.push_soft(kind, lo, hi);
    }

    /// Records an error that clang reports without setting `HadError`
    /// (`\x` without digits, empty or unterminated `\x{}`/`\o{}`, a
    /// too-large plain octal escape).
    fn push_soft(&mut self, kind: LexDiagKind, lo: usize, hi: usize) {
        self.error |= kind.severity() == LexSeverity::Error;
        let lo = u32::try_from(lo).unwrap_or(u32::MAX);
        let hi = u32::try_from(hi).unwrap_or(u32::MAX);
        self.list.push(LitDiag { kind, range: lo..hi });
    }
}

/// The code units of a string literal's value, without the terminating
/// null character.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum CodeUnits {
    /// Ordinary and `u8` literals (UTF-8).
    U8(Vec<u8>),
    /// `u` literals, and `L` literals with 16-bit `wchar_t` (UTF-16).
    U16(Vec<u16>),
    /// `U` literals, and `L` literals with 32-bit `wchar_t` (UTF-32).
    U32(Vec<u32>),
}

impl CodeUnits {
    fn new(bits: u32) -> CodeUnits {
        match bits {
            8 => CodeUnits::U8(Vec::new()),
            16 => CodeUnits::U16(Vec::new()),
            _ => CodeUnits::U32(Vec::new()),
        }
    }

    /// Number of code units.
    pub fn len(&self) -> usize {
        match self {
            CodeUnits::U8(v) => v.len(),
            CodeUnits::U16(v) => v.len(),
            CodeUnits::U32(v) => v.len(),
        }
    }

    /// Whether there are no code units.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Code unit `i`, widened.
    pub fn get(&self, i: usize) -> Option<u32> {
        match self {
            CodeUnits::U8(v) => v.get(i).map(|&u| u32::from(u)),
            CodeUnits::U16(v) => v.get(i).map(|&u| u32::from(u)),
            CodeUnits::U32(v) => v.get(i).copied(),
        }
    }

    /// Width of one code unit in bits.
    pub fn unit_bits(&self) -> u32 {
        match self {
            CodeUnits::U8(_) => 8,
            CodeUnits::U16(_) => 16,
            CodeUnits::U32(_) => 32,
        }
    }

    /// Iterates over the code units, widened.
    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.len()).filter_map(|i| self.get(i))
    }

    /// Appends one code unit (truncated to the unit width).
    fn push_unit(&mut self, u: u32) {
        match self {
            CodeUnits::U8(v) => v.push(u as u8),
            CodeUnits::U16(v) => v.push(u as u16),
            CodeUnits::U32(v) => v.push(u),
        }
    }

    /// Appends `c` encoded in UTF-8/16/32 per the unit width.
    fn push_char(&mut self, c: char) {
        match self {
            CodeUnits::U8(v) => v.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
            CodeUnits::U16(v) => v.extend_from_slice(c.encode_utf16(&mut [0; 2])),
            CodeUnits::U32(v) => v.push(u32::from(c)),
        }
    }
}

/// The value of a string literal ([lex.string]).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct StringLiteral<'s> {
    /// The encoding prefix.
    pub encoding: Encoding,
    /// Whether it is a raw string literal.
    pub raw: bool,
    /// The value, without the terminating null character. After an error
    /// the content is clang's recovery value (for the erroneous escape,
    /// nothing, or the truncated value) and must not be relied on.
    pub units: CodeUnits,
    /// The ud-suffix as spelled, if any.
    pub ud_suffix: Option<&'s str>,
    /// Problems found, with ranges into the spelling.
    pub diags: Vec<LitDiag>,
    /// Whether any diagnostic is an error.
    pub has_error: bool,
    /// clang's `StringLiteralParser::hadError`: an error other than the
    /// "soft" ones clang reports without invalidating the literal (`\x`
    /// with no digits, an unterminated `\x{`/`\o{`, a too-large plain
    /// octal escape), whose recovery value is then used.
    pub had_error: bool,
}

/// The type of a character literal ([lex.ccon]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CharType {
    /// `char`: an ordinary literal with one c-char (and `u8` before C++20).
    Char,
    /// `int`: an ordinary multicharacter literal.
    Int,
    /// `wchar_t`.
    WChar,
    /// `char8_t` (`u8`, C++20 and later).
    Char8,
    /// `char16_t`.
    Char16,
    /// `char32_t`.
    Char32,
}

/// The value of a character literal ([lex.ccon]), as clang computes it.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct CharLiteral<'s> {
    /// The encoding prefix.
    pub encoding: Encoding,
    /// The literal's type.
    pub ty: CharType,
    /// Number of c-chars.
    pub chars: u32,
    /// The value as an object of type [`ty`](Self::ty), widened to `i64`
    /// (sign-extended if the type is signed). A multicharacter literal
    /// packs 8 bits per c-char, the last in the lowest byte, keeping the
    /// low 32 bits (clang).
    pub value: i64,
    /// The value in a `#if` expression ([cpp.cond]/12): clang's
    /// `intmax_t`/`uintmax_t` value, interpreted per
    /// [`pp_unsigned`](Self::pp_unsigned).
    pub pp_value: i64,
    /// Whether the literal is unsigned in `#if`: `char` (and `u8` before
    /// C++20) per the signedness of `char`, `wchar_t` per its signedness,
    /// `char8_t`/`char16_t`/`char32_t` always.
    pub pp_unsigned: bool,
    /// The ud-suffix as spelled, if any.
    pub ud_suffix: Option<&'s str>,
    /// Problems found, with ranges into the spelling.
    pub diags: Vec<LitDiag>,
    /// Whether any diagnostic is an error.
    pub has_error: bool,
}

/// One c-char / s-char, after escape processing.
#[derive(Clone, Copy, Debug)]
enum Item {
    /// A character, to be encoded (source character, simple or unknown
    /// escape, universal-character-name).
    Char(char),
    /// A code unit from a numeric escape, already truncated to the width
    /// (or a source byte of an ordinary literal holding invalid UTF-8).
    Unit(u32),
}

/// The shape of a literal's spelling.
struct Parts<'s> {
    encoding: Encoding,
    raw: bool,
    /// Byte range of the body (between the quotes / the raw delimiters).
    body: Range<usize>,
    ud_suffix: Option<&'s str>,
}

/// Splits a character or string literal spelling into prefix, body and
/// ud-suffix. `None` if it is not shaped like one.
fn split(spelling: &str, quote: u8) -> Option<Parts<'_>> {
    let b = spelling.as_bytes();
    let (encoding, mut i) = match b {
        [b'u', b'8', ..] => (Encoding::Utf8, 2),
        [b'u', ..] => (Encoding::Utf16, 1),
        [b'U', ..] => (Encoding::Utf32, 1),
        [b'L', ..] => (Encoding::Wide, 1),
        _ => (Encoding::Ordinary, 0),
    };
    let raw = quote == b'"' && b.get(i) == Some(&b'R');
    if raw {
        i += 1;
    }
    if b.get(i) != Some(&quote) {
        return None;
    }
    i += 1;
    let (body, end) = if raw {
        let open = i + spelling[i..].find('(')?;
        let delim = &spelling[i..open];
        // The body ends at the last `)delim"`; the ud-suffix cannot
        // contain `"`.
        let close_quote = spelling.rfind('"')?;
        let close = close_quote.checked_sub(delim.len() + 1)?;
        if close < open + 1
            || spelling.get(close + 1..close_quote) != Some(delim)
            || b[close] != b')'
        {
            return None;
        }
        (open + 1..close, close_quote + 1)
    } else {
        let start = i;
        loop {
            match b.get(i)? {
                b'\\' => i += 2,
                &c if c == quote => break,
                _ => i += 1,
            }
        }
        (start..i, i + 1)
    };
    let ud_suffix = spelling.get(end..).filter(|s| !s.is_empty());
    Some(Parts { encoding, raw, body, ud_suffix })
}

fn unit_bits(enc: Encoding, params: EncodingParams) -> u32 {
    match enc {
        Encoding::Ordinary | Encoding::Utf8 => 8,
        Encoding::Utf16 => 16,
        Encoding::Utf32 => 32,
        Encoding::Wide => params.wchar_width(),
    }
}

/// Escape-sequence decoder over a literal body.
struct Escapes<'a> {
    s: &'a str,
    bits: u32,
    std: Std,
    /// [`LiteralOptions::clang19_escapes`].
    clang19: bool,
    /// [`LiteralOptions::c`].
    c: Option<CStd>,
    /// Unevaluated string ([lex.string.uneval]): numeric escapes are
    /// errors.
    unevaluated: bool,
    /// Set by an escape that clang 19 returns from before its
    /// unevaluated-string check (an empty `\x{}` / `\o{}`).
    skip_unevaluated: bool,
    diags: &'a mut Diags,
}

impl Escapes<'_> {
    fn char_at(&self, i: usize) -> Option<char> {
        self.s.get(i..).and_then(|t| t.chars().next())
    }

    /// Decodes the escape sequence whose `\` is at `at`; returns the item
    /// (`None` after an error that drops it) and the end offset.
    fn escape(&mut self, at: usize) -> (Option<Item>, usize) {
        let Some(c) = self.char_at(at + 1) else {
            // A lone trailing backslash cannot end a well-formed literal
            // body; keep it.
            return (Some(Item::Char('\\')), at + 1);
        };
        let next = at + 1 + c.len_utf8();
        let simple = match c {
            '\'' | '"' | '?' | '\\' => Some(c),
            'a' => Some('\x07'),
            'b' => Some('\x08'),
            'f' => Some('\x0C'),
            'n' => Some('\n'),
            'r' => Some('\r'),
            't' => Some('\t'),
            'v' => Some('\x0B'),
            _ => None,
        };
        if let Some(v) = simple {
            return (Some(Item::Char(v)), next);
        }
        match c {
            'e' | 'E' | '(' | '{' | '[' | '%' => {
                self.diags.push(LexDiagKind::NonstandardEscape { ch: c }, at, next);
                self.unevaluated_check(at, c, next);
                let v = if matches!(c, 'e' | 'E') { '\x1B' } else { c };
                (Some(Item::Char(v)), next)
            }
            '0'..='7' => {
                let mut v = 0u32;
                let mut i = at + 1;
                while i < at + 4
                    && self.s.as_bytes().get(i).is_some_and(|b| (b'0'..=b'7').contains(b))
                {
                    v = v * 8 + u32::from(self.s.as_bytes()[i] - b'0');
                    i += 1;
                }
                let item = self.fit(v, false, false, at, i);
                self.unevaluated_check(at, c, i);
                (Some(item), i)
            }
            'o' => {
                if self.char_at(next) != Some('{') {
                    self.diags.push(LexDiagKind::EscapeMissingBrace { esc: 'o' }, at, next);
                    self.unevaluated_check(at, c, next);
                    return (None, next);
                }
                let (v, overflow, end) = self.delimited(at, next + 1, 8);
                let item = v.map(|v| self.fit(v, overflow, false, at, end));
                self.unevaluated_check(at, c, end);
                (item, end)
            }
            'x' => {
                if self.char_at(next) == Some('{') {
                    let (v, overflow, end) = self.delimited(at, next + 1, 16);
                    let item = v.map(|v| self.fit(v, overflow, true, at, end));
                    self.unevaluated_check(at, c, end);
                    return (item, end);
                }
                let mut v = 0u32;
                let mut overflow = false;
                let mut i = next;
                while let Some(d) =
                    self.s.as_bytes().get(i).and_then(|&b| char::from(b).to_digit(16))
                {
                    overflow |= v & 0xF000_0000 != 0;
                    v = (v << 4) | d;
                    i += 1;
                }
                if i == next {
                    // clang returns 0 before its unevaluated-string check.
                    self.diags.push_soft(LexDiagKind::EscapeNoDigits { esc: 'x' }, at, next);
                    return (Some(Item::Unit(0)), next);
                }
                let item = self.fit(v, overflow, true, at, i);
                self.unevaluated_check(at, c, i);
                (Some(item), i)
            }
            'u' | 'U' | 'N' => self.ucn(at, c, next),
            _ => {
                self.diags.push(LexDiagKind::UnknownEscape { ch: c }, at, next);
                self.unevaluated_check(at, c, next);
                (Some(Item::Char(c)), next)
            }
        }
    }

    /// [lex.string.uneval]/1: in an unevaluated string only the simple
    /// escape sequences and universal-character-names are valid. As
    /// clang's `ProcessCharEscape`, the error follows the escape's own
    /// diagnostics and covers the whole sequence `at..end` (`\x01b`,
    /// `\101`, `\o{101}`, `\q`).
    fn unevaluated_check(&mut self, at: usize, c: char, end: usize) {
        if self.unevaluated && !self.skip_unevaluated {
            self.diags.push(LexDiagKind::UnevaluatedStringEscape { ch: c }, at, end);
        }
    }

    /// Checks a numeric escape's value against the code unit width
    /// (clang truncates after diagnosing). As clang, a too-large hex or
    /// `\o{}` escape is only diagnosed (and then sets `HadError`) if
    /// `HadError` is not yet set; a plain octal one always is, softly.
    fn fit(&mut self, v: u32, overflow: bool, hex: bool, at: usize, end: usize) -> Item {
        let too_large = overflow || (self.bits < 32 && v >> self.bits != 0);
        let plain_octal = !hex && self.char_at(at + 1) != Some('o');
        if too_large && plain_octal {
            self.diags.push_soft(LexDiagKind::OctalEscapeTooLarge, at, end);
        } else if too_large && !self.diags.sticky {
            let kind =
                if hex { LexDiagKind::HexEscapeTooLarge } else { LexDiagKind::OctalEscapeTooLarge };
            self.diags.push(kind, at, end);
        }
        let mask = if self.bits >= 32 { u32::MAX } else { (1 << self.bits) - 1 };
        Item::Unit(v & mask)
    }

    /// Parses the digits of `\x{…}`, `\o{…}` or `\u{…}` starting at `p`
    /// (after the `{`): the value (`None` after an error), whether it
    /// overflowed 32 bits, and the end offset (after the `}`).
    #[allow(clippy::many_single_char_names, reason = "short-lived scanner state")]
    fn delimited(&mut self, at: usize, p: usize, radix: u32) -> (Option<u32>, bool, usize) {
        let mut v = 0u32;
        let mut overflow = false;
        let mut digits = 0;
        let mut bad = None;
        let mut i = p;
        // `\x{}`/`\o{}` (clang's ProcessCharEscape) vs `\u{}` (a UCN).
        let char_escape = radix == 8 || self.char_at(at + 1) == Some('x');
        loop {
            let Some(c) = self.char_at(i) else {
                if char_escape {
                    self.diags.push_soft(LexDiagKind::DelimitedEscapeUnterminated, at, i);
                } else {
                    self.diags.push(LexDiagKind::DelimitedEscapeUnterminated, at, i);
                }
                return (None, false, i);
            };
            if c == '}' {
                i += 1;
                break;
            }
            match c.to_digit(radix) {
                Some(d) if bad.is_none() => {
                    match v.checked_mul(radix).and_then(|v| v.checked_add(d)) {
                        Some(n) => v = n,
                        None => overflow = true,
                    }
                    digits += 1;
                }
                Some(_) => {}
                None => {
                    if bad.is_none() {
                        bad = Some((c, i));
                    }
                }
            }
            i += c.len_utf8();
        }
        if let Some((c, ci)) = bad {
            self.diags.push(
                LexDiagKind::DelimitedEscapeInvalidDigit { ch: c },
                ci,
                ci + c.len_utf8(),
            );
            return (None, false, i);
        }
        if digits == 0 {
            // clang's range for an empty `\x{}`/`\o{}` stops before the
            // `}`; for `\u{}` it includes it.
            let hi = if char_escape { i - 1 } else { i };
            if char_escape && self.clang19 {
                self.diags.push_soft(LexDiagKind::DelimitedEscapeEmpty, at, hi);
                // clang 19 leaves the `}` of an empty `\x{}`/`\o{}`
                // unconsumed, so it lexes as a further character (making
                // `'\x{}'` a multicharacter literal), and returns before
                // its unevaluated-string check.
                self.skip_unevaluated = true;
                return (Some(0), false, i - 1);
            }
            self.diags.push(LexDiagKind::DelimitedEscapeEmpty, at, hi);
            return (None, false, i);
        }
        if char_escape {
            // `\x{}` / `\o{}`; `\u{}` reports its own extension.
            if self.std < Std::Cxx23 {
                self.diags.push(LexDiagKind::DelimitedEscapeExtension, at, i);
            }
        }
        (Some(v), overflow, i)
    }

    /// `\u`, `\U`, `\u{…}` or `\N{…}` at `at`; `next` is after the letter.
    fn ucn(&mut self, at: usize, kind: char, next: usize) -> (Option<Item>, usize) {
        let (cp, end) = match kind {
            'N' => {
                if self.char_at(next) != Some('{') {
                    self.diags.push(LexDiagKind::EscapeMissingBrace { esc: 'N' }, at, next);
                    return (None, next);
                }
                let start = next + 1;
                let Some(close) = self.s[start..].find('}').map(|k| start + k) else {
                    // clang's range is `\N{`.
                    self.diags.push(LexDiagKind::UcnEscapeIncomplete, at, start);
                    return (None, self.s.len());
                };
                let end = close + 1;
                if close == start {
                    self.diags.push(LexDiagKind::DelimitedEscapeEmpty, at, end);
                    return (None, end);
                }
                if self.std < Std::Cxx23 {
                    self.diags.push(LexDiagKind::UcnNamedExtension, at, end);
                }
                match lookup_char_name(&self.s[start..close]) {
                    Some(NameMatch::Exact(c)) => (u32::from(c), end),
                    Some(NameMatch::Loose(c)) => {
                        self.diags.push(LexDiagKind::UcnNameLooseMatch { ch: c }, start, close);
                        (u32::from(c), end)
                    }
                    None => {
                        self.diags.push(LexDiagKind::UcnUnknownName, start, close);
                        return (None, end);
                    }
                }
            }
            'u' if self.char_at(next) == Some('{') => {
                let (v, overflow, end) = self.delimited(at, next + 1, 16);
                let Some(v) = v else {
                    return (None, end);
                };
                if self.std < Std::Cxx23 {
                    self.diags.push(LexDiagKind::UcnDelimitedExtension, at, end);
                }
                (if overflow { u32::MAX } else { v }, end)
            }
            _ => {
                let want = if kind == 'u' { 4 } else { 8 };
                let mut v = 0u32;
                let mut i = next;
                while i < next + want
                    && let Some(d) =
                        self.s.as_bytes().get(i).and_then(|&b| char::from(b).to_digit(16))
                {
                    v = (v << 4) | d;
                    i += 1;
                }
                if i == next {
                    self.diags.push(LexDiagKind::EscapeNoDigits { esc: kind }, at, next);
                    return (None, next);
                }
                if i < next + want {
                    self.diags.push(LexDiagKind::UcnEscapeIncomplete, at, i);
                    return (None, i);
                }
                (v, i)
            }
        };
        match char::from_u32(cp) {
            None => {
                self.diags.push(LexDiagKind::UcnInvalidCodePoint { cp }, at, end);
                (None, end)
            }
            Some(c) => {
                // C++98/03 [lex.charset]/2 and C before C23 (C11 6.4.3p2):
                // no UCN for a control or basic source character, even in
                // a literal ($, @, ` excepted).
                let strict = self.c.map_or(self.std < Std::Cxx11, |c| c < CStd::C23);
                if strict && cp < 0xA0 && !matches!(cp, 0x24 | 0x40 | 0x60) {
                    self.diags.push(LexDiagKind::UcnBasicOrControl { cp }, at, end);
                    return (None, end);
                }
                // C89 has no UCNs; clang still decodes this one, with
                // `warn_ucn_not_valid_in_c89_literal`.
                if self.c == Some(CStd::C89) {
                    self.diags.push(LexDiagKind::UcnNotValidInC89Literal, at, end);
                }
                (Some(Item::Char(c)), end)
            }
        }
    }
}

/// Runs `f` on each item of a non-raw body; an escape sequence dropped
/// after an error is passed as `None` (clang still counts it as a
/// character of a character literal).
#[allow(clippy::too_many_arguments, reason = "internal walker shared by both decoders")]
fn for_each_item(
    body: &str,
    base: usize,
    bits: u32,
    opts: LiteralOptions,
    unevaluated: bool,
    bad: Bad<'_>,
    diags: &mut Diags,
    mut f: impl FnMut(Option<Item>, Range<usize>, &mut Diags),
) {
    let mut i = 0;
    while i < body.len() {
        if !bad.seqs.is_empty() && body.as_bytes()[i] != b'\\' {
            let end = body[i..].find('\\').map_or(body.len(), |k| i + k);
            let seqs = bad.within(base + i..base + end);
            if !seqs.is_empty() {
                bad_fragment(&body[i..end], base + i, bad, seqs, diags, &mut f);
                i = end;
                continue;
            }
        }
        if body.as_bytes()[i] == b'\\' {
            // Escape offsets are relative to `body`; shift diagnostics.
            let first = diags.list.len();
            let mut esc = Escapes {
                s: body,
                bits,
                std: opts.std,
                clang19: opts.clang19_escapes,
                c: opts.c,
                unevaluated,
                skip_unevaluated: false,
                diags,
            };
            let (item, end) = esc.escape(i);
            shift(&mut diags.list[first..], base);
            let end = end.max(i + 1);
            f(item, base + i..base + end, diags);
            i = end;
        } else {
            let c = body[i..].chars().next().unwrap_or('\u{FFFD}');
            f(Some(Item::Char(c)), base + i..base + i + c.len_utf8(), diags);
            i += c.len_utf8();
        }
    }
}

/// One fragment (the text between escapes) of a literal holding invalid
/// UTF-8 at `seqs`, starting at spelling offset `base`. An ordinary
/// literal keeps the source bytes, each a c-char (clang copies the
/// fragment bytewise); any other literal is in error and keeps the
/// characters before the first invalid sequence.
fn bad_fragment(
    frag: &str,
    base: usize,
    bad: Bad<'_>,
    seqs: &[InvalidUtf8<'_>],
    diags: &mut Diags,
    f: &mut impl FnMut(Option<Item>, Range<usize>, &mut Diags),
) {
    bad.diagnose(seqs, diags);
    let mut next = seqs.iter().peekable();
    for (k, c) in frag.char_indices() {
        let at = base + k;
        let range = at..at + c.len_utf8();
        let seq = next.next_if(|q| q.offset as usize == at);
        if bad.error {
            if seq.is_some() {
                return;
            }
            // Units, not characters: clang does not check their size.
            f(Some(Item::Unit(u32::from(c))), range, diags);
        } else if let Some(q) = seq {
            for &b in q.bytes {
                f(Some(Item::Unit(u32::from(b))), range.clone(), diags);
            }
        } else {
            for &b in c.encode_utf8(&mut [0; 4]).as_bytes() {
                f(Some(Item::Unit(u32::from(b))), range.clone(), diags);
            }
        }
    }
}

fn shift(diags: &mut [LitDiag], base: usize) {
    let base = u32::try_from(base).unwrap_or(u32::MAX);
    for d in diags {
        d.range = d.range.start.saturating_add(base)..d.range.end.saturating_add(base);
    }
}

fn unterminated(spelling: &str, quote: u8) -> Diags {
    let mut d = Diags::default();
    let kind =
        if quote == b'"' { LexDiagKind::UnterminatedString } else { LexDiagKind::UnterminatedChar };
    d.push(kind, 0, spelling.len());
    d
}

/// Evaluates a string literal ([lex.string]) from its spelling: prefix,
/// escapes, encoding into code units, ud-suffix. Raw string bodies are
/// taken verbatim except that CR LF becomes LF (as clang does; phase 1
/// already does this for a file read with LF line ends).
pub fn decode_string(spelling: &str, opts: LiteralOptions) -> StringLiteral<'_> {
    decode_string_impl(spelling, opts, false, SourceBytes::default())
}

/// [`decode_string`] or (if `unevaluated`) [`decode_unevaluated_string`]
/// of a spelling that holds the invalid UTF-8 described by `src`
/// ([lex.charset]/1, matching clang): in an ordinary literal it is
/// accepted with a warning ([`LexDiagKind::InvalidEncoding`]) and the
/// value holds the source bytes; in any other literal it is an error.
pub fn decode_string_from_source<'s>(
    spelling: &'s str,
    opts: LiteralOptions,
    unevaluated: bool,
    src: SourceBytes<'_>,
) -> StringLiteral<'s> {
    decode_string_impl(spelling, opts, unevaluated, src)
}

/// Evaluates an unevaluated string ([lex.string.uneval]: `static_assert`,
/// `[[deprecated]]`, `= delete("…")`, …): like an ordinary string literal,
/// but an encoding prefix, a ud-suffix, and numeric or unknown escape
/// sequences are errors (clang's `err_unevaluated_string_*`). The value
/// is UTF-8.
pub fn decode_unevaluated_string(spelling: &str, opts: LiteralOptions) -> StringLiteral<'_> {
    decode_string_impl(spelling, opts, true, SourceBytes::default())
}

fn decode_string_impl<'s>(
    spelling: &'s str,
    opts: LiteralOptions,
    unevaluated: bool,
    src: SourceBytes<'_>,
) -> StringLiteral<'s> {
    let Some(parts) = split(spelling, b'"') else {
        let d = unterminated(spelling, b'"');
        return StringLiteral {
            encoding: Encoding::Ordinary,
            raw: false,
            units: CodeUnits::U8(Vec::new()),
            ud_suffix: None,
            diags: d.list,
            has_error: true,
            had_error: true,
        };
    };
    let mut diags = Diags::default();
    let mut encoding = parts.encoding;
    if unevaluated {
        if encoding != Encoding::Ordinary {
            let len = encoding.prefix().len();
            diags.push(LexDiagKind::UnevaluatedStringPrefix, 0, len);
            encoding = Encoding::Ordinary;
        }
        if parts.ud_suffix.is_some() {
            diags.push(LexDiagKind::UnevaluatedStringUdSuffix, parts.body.end + 1, spelling.len());
        }
    }
    let bits = unit_bits(encoding, opts.params);
    let mut units = CodeUnits::new(bits);
    let body = &spelling[parts.body.clone()];
    let bad = Bad {
        seqs: src.invalid,
        error: encoding != Encoding::Ordinary || src.concat_prefixed,
        string: true,
    };
    let raw_bad = if parts.raw { bad.within(parts.body.clone()) } else { &[] };
    if !raw_bad.is_empty() {
        // The body is one fragment; CR LF still becomes LF.
        bad_fragment(body, parts.body.start, bad, raw_bad, &mut diags, &mut |item, r, _| {
            if let Some(Item::Unit(u)) = item
                && !(u == u32::from(b'\r')
                    && body.as_bytes().get(r.end - parts.body.start) == Some(&b'\n'))
            {
                units.push_unit(u);
            }
        });
    } else if parts.raw {
        let mut chars = body.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\r' && chars.peek() == Some(&'\n') {
                continue;
            }
            units.push_char(c);
        }
    } else {
        for_each_item(
            body,
            parts.body.start,
            bits,
            opts,
            unevaluated,
            bad,
            &mut diags,
            |item, _, _| match item {
                Some(Item::Char(c)) => units.push_char(c),
                Some(Item::Unit(u)) => units.push_unit(u),
                None => {}
            },
        );
    }
    StringLiteral {
        encoding: parts.encoding,
        raw: parts.raw,
        units,
        ud_suffix: parts.ud_suffix,
        has_error: diags.error,
        had_error: diags.sticky,
        diags: diags.list,
    }
}

/// Evaluates a character literal ([lex.ccon]) from its spelling, matching
/// clang: a c-char that needs more than one code unit is an error; an
/// ordinary literal with several c-chars has type `int` and packs them 8
/// bits each (warning); other encodings allow only one c-char.
pub fn decode_char(spelling: &str, opts: LiteralOptions) -> CharLiteral<'_> {
    decode_char_from_source(spelling, opts, &[])
}

/// [`decode_char`] of a spelling holding the invalid UTF-8 `invalid`
/// (see [`SourceBytes::invalid`]): in an ordinary literal it is accepted
/// with a warning and each source byte of the text around it (up to the
/// next escape) is a c-char, as in clang; in any other literal it is an
/// error.
#[allow(clippy::too_many_lines, reason = "mirrors clang's CharLiteralParser")]
pub fn decode_char_from_source<'s>(
    spelling: &'s str,
    opts: LiteralOptions,
    invalid: &[InvalidUtf8<'_>],
) -> CharLiteral<'s> {
    let params = opts.params;
    let Some(parts) = split(spelling, b'\'') else {
        let d = unterminated(spelling, b'\'');
        return CharLiteral {
            encoding: Encoding::Ordinary,
            ty: CharType::Char,
            chars: 0,
            value: 0,
            pp_value: 0,
            pp_unsigned: !params.char_signed,
            ud_suffix: None,
            diags: d.list,
            has_error: true,
        };
    };
    let enc = parts.encoding;
    let bits = unit_bits(enc, params);
    let mut diags = Diags::default();
    let mut chars = 0u32;
    let mut packed = 0u32; // clang's multicharacter accumulator
    let mut last = 0u32;
    let mut too_long = false;
    let body = &spelling[parts.body.clone()];
    let bad = Bad { seqs: invalid, error: enc != Encoding::Ordinary, string: false };
    for_each_item(body, parts.body.start, bits, opts, false, bad, &mut diags, |item, range, d| {
        chars += 1;
        let unit = match item {
            None => 0,
            Some(Item::Unit(u)) => u,
            Some(Item::Char(c)) => {
                let cp = u32::from(c);
                let fits = match bits {
                    8 => cp < 0x80,
                    16 => cp < 0x1_0000,
                    _ => true,
                };
                if !fits {
                    d.push(LexDiagKind::CharTooLarge, range.start, range.end);
                }
                cp
            }
        };
        last = unit;
        too_long |= packed >> 24 != 0;
        packed = (packed << 8) | (unit & 0xFF);
    });
    let full = parts.body.start.saturating_sub(1)..parts.body.end + 1;
    if chars == 0 && body.is_empty() {
        diags.push(LexDiagKind::EmptyCharLiteral, full.start, full.end);
    }
    let multi = chars > 1;
    if multi {
        if enc == Encoding::Ordinary {
            let kind = if chars == 4 {
                LexDiagKind::FourCharLiteral
            } else {
                LexDiagKind::MultiCharLiteral
            };
            diags.push(kind, full.start, full.end);
            if too_long && !diags.sticky {
                diags.push(LexDiagKind::CharConstantTooLong, full.start, full.end);
            }
        } else {
            diags.push(LexDiagKind::MultiCharNotAllowed, full.start, full.end);
        }
    }
    let utf8_is_char8 = opts.char8();
    let ty = match enc {
        Encoding::Ordinary if multi => CharType::Int,
        Encoding::Utf8 if utf8_is_char8 => CharType::Char8,
        Encoding::Ordinary | Encoding::Utf8 => CharType::Char,
        Encoding::Wide => CharType::WChar,
        Encoding::Utf16 => CharType::Char16,
        Encoding::Utf32 => CharType::Char32,
    };
    // clang's `CharLiteralParser::getValue()`.
    let raw = if enc == Encoding::Ordinary && multi { packed } else { last };
    let wbits = params.wchar_width();
    let (width, signed) = match ty {
        CharType::Char => (8, params.char_signed),
        CharType::Int => (32, true),
        CharType::WChar => (wbits, params.wchar_signed),
        CharType::Char8 => (8, false),
        CharType::Char16 => (16, false),
        CharType::Char32 => (32, false),
    };
    let value = extend(raw, width, signed);
    // PPExpressions.cpp: the width follows the literal kind, the
    // signedness the character type.
    let pp_width = if multi {
        32
    } else {
        match enc {
            Encoding::Ordinary | Encoding::Utf8 => 8,
            Encoding::Wide => wbits,
            Encoding::Utf16 => 16,
            Encoding::Utf32 => 32,
        }
    };
    let pp_unsigned = match enc {
        Encoding::Wide => !params.wchar_signed,
        Encoding::Utf16 | Encoding::Utf32 => true,
        Encoding::Utf8 => utf8_is_char8 || !params.char_signed,
        Encoding::Ordinary => !params.char_signed,
    };
    let pp_value = extend(raw, pp_width, !pp_unsigned);
    CharLiteral {
        encoding: enc,
        ty,
        chars,
        value,
        pp_value,
        pp_unsigned,
        ud_suffix: parts.ud_suffix,
        has_error: diags.error,
        diags: diags.list,
    }
}

/// The low `width` bits of `v`, sign- or zero-extended.
fn extend(v: u32, width: u32, signed: bool) -> i64 {
    let v = i64::from(v);
    if width >= 64 {
        return v;
    }
    let mask = (1i64 << width) - 1;
    let low = v & mask;
    if signed && low >> (width - 1) & 1 == 1 { low - (1i64 << width) } else { low }
}

/// Spells a string literal with encoding prefix `enc` whose value is
/// exactly `units` (truncated to the encoding's unit width): printable
/// ASCII as itself, everything else as a numeric escape that cannot
/// absorb the following character. Valid in every language mode.
/// [`decode_string`] inverts it.
pub fn encode_string(units: &CodeUnits, enc: Encoding) -> String {
    let mut out = String::with_capacity(units.len() + 3);
    out.push_str(enc.prefix());
    out.push('"');
    let mut after_hex = false;
    for u in units.iter() {
        let printable = (0x20..0x7F).contains(&u);
        let c = char::from(u as u8);
        if printable && !(after_hex && c.is_ascii_hexdigit()) && !matches!(c, '"' | '\\' | '?') {
            out.push(c);
            after_hex = false;
        } else if u < 0o1000 {
            // Three digits: an octal escape never absorbs what follows.
            let _ = write!(out, "\\{u:03o}");
            after_hex = false;
        } else {
            let _ = write!(out, "\\x{u:X}");
            after_hex = true;
        }
    }
    out.push('"');
    out
}
