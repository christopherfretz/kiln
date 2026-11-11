//! Byte/column arithmetic for source snippets (`spec/diag.md` section
//! "Text rendering: printable text"). Transcribes clang's
//! `printableTextForNextCharacter`, `genColumnByteMapping` and
//! `SourceColumnMap`.
//!
//! Lines are handled as *raw* bytes: kiln decodes buffers to UTF-8 and
//! replaces invalid sequences by U+FFFD, recording the original bytes in
//! the buffer's `LoadReport`. Snippets put the original bytes back so that
//! they render as clang renders them (`<FF>`), and columns are counted in
//! original bytes, as clang counts them.

use std::borrow::Cow;

use kiln_source::{FileId, Loc, SourceManager};

/// One source line in original bytes.
#[derive(Debug)]
pub(crate) struct RawLine<'a> {
    /// The bytes, without the terminator.
    pub bytes: Cow<'a, [u8]>,
}

/// The start offset of 1-based `line` of `file`, with its text, or `None`
/// if the line does not exist.
pub(crate) fn line_start(sm: &SourceManager, file: FileId, line: u32) -> Option<(u32, &str)> {
    let text = sm.text(file);
    let lt = sm.line_text(file, line)?;
    Some(((lt.as_ptr().addr() - text.as_ptr().addr()) as u32, lt))
}

/// Line `line` of `file` in original bytes; `None` if the line does not
/// exist or starts at the end of the buffer (clang shows nothing there).
pub(crate) fn raw_line(sm: &SourceManager, file: FileId, line: u32) -> Option<RawLine<'_>> {
    let (start, text) = line_start(sm, file, line)?;
    if start as usize == sm.text(file).len() {
        return None;
    }
    let end = start + text.len() as u32;
    let reps = &sm.load_report(file).replacements;
    let first = reps.partition_point(|r| r.offset < start);
    let in_line = reps[first..].iter().take_while(|r| r.offset < end);
    let mut out: Option<Vec<u8>> = None;
    let mut copied = start;
    for r in in_line {
        let buf = out.get_or_insert_with(Vec::new);
        buf.extend_from_slice(
            &text.as_bytes()[(copied - start) as usize..(r.offset - start) as usize],
        );
        buf.extend_from_slice(&r.bytes);
        copied = r.offset + 3;
    }
    let bytes = match out {
        Some(mut buf) => {
            buf.extend_from_slice(&text.as_bytes()[(copied - start) as usize..]);
            Cow::Owned(buf)
        }
        None => Cow::Borrowed(text.as_bytes()),
    };
    Some(RawLine { bytes })
}

/// The physical line and 1-based column of file location `loc`, the
/// column counted in original bytes. `None` for non-file locations.
pub(crate) fn raw_line_col(sm: &SourceManager, loc: Loc) -> Option<(FileId, u32, u32)> {
    let (file, off) = sm.offset_of(loc)?;
    let lc = sm.line_col(loc)?;
    let start = off - (lc.col - 1);
    Some((file, lc.line, raw_col_between(sm, file, start, off) + 1))
}

/// Number of original bytes between decoded offsets `from <= to`.
fn raw_col_between(sm: &SourceManager, file: FileId, from: u32, to: u32) -> u32 {
    let reps = &sm.load_report(file).replacements;
    let first = reps.partition_point(|r| r.offset < from);
    let mut n = to - from;
    for r in reps[first..].iter().take_while(|r| r.offset < to) {
        n = n - 3 + r.bytes.len() as u32;
    }
    n
}

/// clang's `printableTextForNextCharacter`: the printable form of the
/// character at `*i` (advanced past it) and whether it was printable.
pub(crate) fn printable_next(
    line: &[u8],
    i: &mut usize,
    tabstop: u32,
) -> (Cow<'static, str>, bool) {
    let b = line[*i];
    if b == b'\t' {
        let since = line[..*i].iter().rev().take_while(|&&c| c != b'\t').count() as u32;
        let n = tabstop - since % tabstop;
        *i += 1;
        return (Cow::Owned(" ".repeat(n as usize)), true);
    }
    if b < 0x80 && (0x20..0x7f).contains(&b) {
        *i += 1;
        return (Cow::Owned(char::from(b).to_string()), true);
    }
    let len = utf8_len(b);
    if let Some(ch) = line
        .get(*i..*i + len)
        .and_then(|s| std::str::from_utf8(s).ok())
        .and_then(|s| s.chars().next())
    {
        *i += len;
        if is_printable(ch) {
            return (Cow::Owned(ch.to_string()), true);
        }
        return (Cow::Owned(format!("<U+{:04X}>", u32::from(ch))), false);
    }
    *i += 1;
    (Cow::Owned(format!("<{b:02X}>")), false)
}

/// Length of the UTF-8 sequence a lead byte announces (1 for invalid
/// leads, as LLVM's `getNumBytesForUTF8` table gives for continuation
/// bytes... approximated: they then fail validation and print as `<XX>`).
fn utf8_len(b: u8) -> usize {
    match b {
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

/// Whether a sorted, disjoint `(lo, hi)` range table holds `c`.
fn in_table(table: &[(u32, u32)], c: char) -> bool {
    let c = u32::from(c);
    let i = table.partition_point(|&(_, hi)| hi < c);
    table.get(i).is_some_and(|&(lo, _)| lo <= c)
}

/// `llvm::sys::unicode::isPrintable` (LLVM 22, UCD 15.1.0): general
/// categories L, M, N, P, S and Zs, plus U+00AD SOFT HYPHEN, which most
/// terminals render. Unassigned code points are not printable.
pub(crate) fn is_printable(c: char) -> bool {
    c == '\u{AD}' || in_table(crate::unicode_tables::PRINTABLE, c)
}

/// `llvm::sys::unicode::isFormatting` (LLVM 22, UCD 15.1.0): the format
/// characters, general category Cf.
fn is_formatting(c: char) -> bool {
    in_table(crate::unicode_tables::FORMATTING, c)
}

/// Program-provided text (a `static_assert` message, an uncaught
/// exception's `what()`, an attribute's message) as clang's diagnostic
/// engine prints a string argument (`pushEscapedString`; `spec/diag.md`
/// §6.2): ASCII whitespace (tab, newline, vertical tab, form feed,
/// carriage return) and printable or format characters as they are; any
/// other code point as `<U+XXXX>` (a control character, a line or
/// paragraph separator, a private-use or noncharacter code point); each
/// byte that is not part of valid UTF-8 as `<XX>` (hex, uppercase).
///
/// ```
/// assert_eq!(kiln_diag::escape_diag_text(b"a\tb\nc\x01\xff"), "a\tb\nc<U+0001><FF>");
/// ```
#[must_use]
pub fn escape_diag_text(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len());
    for chunk in bytes.utf8_chunks() {
        for c in chunk.valid().chars() {
            if matches!(c, '\t' | '\n' | '\x0B' | '\x0C' | '\r')
                || is_printable(c)
                || is_formatting(c)
            {
                out.push(c);
            } else {
                let _ = write!(out, "<U+{:04X}>", u32::from(c));
            }
        }
        for b in chunk.invalid() {
            let _ = write!(out, "<{b:02X}>");
        }
    }
    out
}

/// `llvm::sys::locale::columnWidth` for a printable string (LLVM 22's
/// `charWidth`, UCD 15.1.0): non-spacing and enclosing marks are 0
/// columns, East Asian full-width and wide characters (and the pictograph
/// blocks LLVM adds) 2, everything else 1. Callers pass printable text
/// (non-printable characters are already `<U+XXXX>`), so LLVM's
/// non-printable error result has no counterpart.
pub(crate) fn column_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

fn char_width(c: char) -> usize {
    if c.is_ascii() {
        1
    } else if in_table(crate::unicode_tables::COMBINING, c) {
        0
    } else if in_table(crate::unicode_tables::DOUBLE_WIDTH, c) {
        2
    } else {
        1
    }
}

/// clang's `SourceColumnMap`: byte offsets to display columns and back
/// for one line.
#[derive(Debug)]
pub(crate) struct ColumnMap {
    byte_to_col: Vec<i32>,
    #[cfg(test)]
    columns: usize,
}

impl ColumnMap {
    pub(crate) fn new(line: &[u8], tabstop: u32) -> ColumnMap {
        if line.is_empty() {
            return ColumnMap {
                byte_to_col: vec![0],
                #[cfg(test)]
                columns: 0,
            };
        }
        let mut byte_to_col = vec![-1; line.len() + 1];
        let mut cols = 0usize;
        let mut i = 0usize;
        while i < line.len() {
            byte_to_col[i] = cols as i32;
            let (s, _) = printable_next(line, &mut i, tabstop);
            cols += column_width(&s);
        }
        byte_to_col[line.len()] = cols as i32;
        ColumnMap {
            byte_to_col,
            #[cfg(test)]
            columns: cols,
        }
    }

    /// Number of bytes in the line.
    pub(crate) fn bytes(&self) -> usize {
        self.byte_to_col.len() - 1
    }

    /// Number of display columns in the line.
    #[cfg(test)]
    pub(crate) fn columns(&self) -> usize {
        self.columns
    }

    /// The column of the character containing byte `n` (clamped to the
    /// line end).
    pub(crate) fn byte_to_containing_column(&self, n: usize) -> usize {
        let mut n = n.min(self.bytes());
        while self.byte_to_col[n] == -1 {
            n -= 1;
        }
        self.byte_to_col[n] as usize
    }

    pub(crate) fn start_of_next_column(&self, mut n: usize) -> usize {
        loop {
            n += 1;
            if n >= self.byte_to_col.len() || self.byte_to_col[n] != -1 {
                return n;
            }
        }
    }

    pub(crate) fn start_of_previous_column(&self, mut n: usize) -> usize {
        loop {
            n -= 1;
            if n == 0 || self.byte_to_col[n] != -1 {
                return n;
            }
        }
    }
}

/// clang's `expandTabs` for a fix-it insertion line.
pub(crate) fn expand_tabs(line: &[u8], tabstop: u32) -> Vec<u8> {
    let mut out = line.to_vec();
    let mut i = out.len();
    while i > 0 {
        i -= 1;
        if out[i] != b'\t' {
            continue;
        }
        let mut j = i;
        let (s, _) = printable_next(&out, &mut j, tabstop);
        out.splice(i..=i, s.bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn printable(line: &str) -> String {
        let b = line.as_bytes();
        let mut i = 0;
        let mut out = String::new();
        while i < b.len() {
            out.push_str(&printable_next(b, &mut i, 8).0);
        }
        out
    }

    #[test]
    fn tabs_expand_by_bytes_since_previous_tab() {
        assert_eq!(printable("\tx"), "        x");
        assert_eq!(printable("ab\tx"), "ab      x");
        assert_eq!(printable("a\t\tx"), "a               x");
    }

    #[test]
    fn escape_diag_text_as_clang() {
        // clang 22 `pushEscapedString`: whitespace and format characters
        // stay, controls, separators, private use and bad bytes escape.
        assert_eq!(escape_diag_text(b"a\tb\nc\x0B\x0C\r"), "a\tb\nc\x0B\x0C\r");
        assert_eq!(escape_diag_text(b"\x00\x01\x7F"), "<U+0000><U+0001><U+007F>");
        assert_eq!(
            escape_diag_text("\u{85}\u{2028}\u{2029}".as_bytes()),
            "<U+0085><U+2028><U+2029>"
        );
        assert_eq!(
            escape_diag_text("\u{AD}\u{200B}\u{FEFF}é".as_bytes()),
            "\u{AD}\u{200B}\u{FEFF}é"
        );
        assert_eq!(escape_diag_text("\u{E000}\u{FFFE}".as_bytes()), "<U+E000><U+FFFE>");
        assert_eq!(escape_diag_text(b"\xFF\xED\xBF\xBF\xC3"), "<FF><ED><BF><BF><C3>");
        assert_eq!(escape_diag_text(b"\xF4\x90\x80\x80"), "<F4><90><80><80>");
    }

    #[test]
    fn unicode_tables_are_sorted_and_disjoint() {
        use crate::unicode_tables::{COMBINING, DOUBLE_WIDTH, FORMATTING, PRINTABLE};
        for t in [PRINTABLE, FORMATTING, COMBINING, DOUBLE_WIDTH] {
            assert!(t.iter().all(|&(lo, hi)| lo <= hi));
            assert!(t.windows(2).all(|w| w[0].1 + 1 < w[1].0));
        }
    }

    #[test]
    fn printable_as_llvm_22() {
        // Unassigned (U+0378, U+1C89 is new in Unicode 16), control,
        // separators, private use, surrogate-adjacent noncharacters.
        for c in ['\u{378}', '\u{1C89}', '\u{85}', '\u{2028}', '\u{E000}', '\u{FFFE}', '\u{10FFFF}']
        {
            assert!(!is_printable(c), "{c:?}");
        }
        // Letters, marks, Zs, U+00AD, a Unicode 15.1 addition (U+2FFC).
        for c in ['é', '\u{301}', '\u{3000}', '\u{AD}', '\u{2FFC}', '\u{1F600}', '\u{31350}'] {
            assert!(is_printable(c), "{c:?}");
        }
        assert!(is_formatting('\u{200B}') && !is_printable('\u{200B}'));
        assert!(is_formatting('\u{AD}') && is_printable('\u{AD}'));
        assert_eq!(escape_diag_text("\u{378}é".as_bytes()), "<U+0378>é");
    }

    #[test]
    fn widths_as_llvm_22() {
        assert_eq!(column_width("a\u{301}"), 1);
        assert_eq!(column_width("\u{4E00}\u{4DC0}\u{1F300}"), 6);
        assert_eq!(column_width("\u{FF61}"), 1); // halfwidth
        assert_eq!(column_width("\u{20000}\u{3FFFD}"), 4);
    }

    #[test]
    fn non_printable() {
        assert_eq!(printable("a\u{1}b"), "a<U+0001>b");
        assert_eq!(printable("\u{200B}"), "<U+200B>");
        assert_eq!(printable("é"), "é");
        let raw = [b'a', 0xFF, b'b'];
        let mut i = 0;
        let mut out = String::new();
        while i < raw.len() {
            out.push_str(&printable_next(&raw, &mut i, 8).0);
        }
        assert_eq!(out, "a<FF>b");
    }

    #[test]
    fn column_map() {
        let m = ColumnMap::new("\tx".as_bytes(), 8);
        assert_eq!(m.columns(), 9);
        assert_eq!(m.byte_to_containing_column(1), 8);
        let m = ColumnMap::new("é=".as_bytes(), 8);
        assert_eq!(m.byte_to_containing_column(1), 0);
        assert_eq!(m.byte_to_containing_column(2), 1);
        let m = ColumnMap::new("漢x".as_bytes(), 8);
        assert_eq!(m.byte_to_containing_column(3), 2);
        assert_eq!(m.start_of_next_column(0), 3);
        assert_eq!(m.start_of_previous_column(3), 0);
    }
}
