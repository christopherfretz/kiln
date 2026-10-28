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

/// Approximation of `llvm::sys::unicode::isPrintable` (`spec/diag.md`
/// deviation D5): control, format, line/paragraph separator,
/// private-use and noncharacter code points are not printable.
pub(crate) fn is_printable(c: char) -> bool {
    let c = u32::from(c);
    !matches!(c,
        0x00..=0x1F | 0x7F..=0x9F | 0xAD
        | 0x600..=0x605 | 0x61C | 0x6DD | 0x70F | 0x180E
        | 0x200B..=0x200F | 0x2028..=0x202E | 0x2060..=0x206F
        | 0xD800..=0xF8FF | 0xFDD0..=0xFDEF | 0xFEFF | 0xFFF9..=0xFFFB | 0xFFFE | 0xFFFF
        | 0x110BD | 0x1D173..=0x1D17A | 0xE0000..=0xE0FFF | 0xF0000..)
        && c & 0xFFFE != 0xFFFE
}

/// Approximation of `llvm::sys::locale::columnWidth` for a printable
/// string: combining marks are 0 columns, East Asian wide characters 2.
pub(crate) fn column_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

fn char_width(c: char) -> usize {
    let c = u32::from(c);
    match c {
        0x300..=0x36F
        | 0x483..=0x489
        | 0x591..=0x5BD
        | 0x610..=0x61A
        | 0x64B..=0x65F
        | 0x200B
        | 0x1AB0..=0x1AFF
        | 0x1DC0..=0x1DFF
        | 0x20D0..=0x20FF
        | 0xFE00..=0xFE0F
        | 0xFE20..=0xFE2F
        | 0xE0100..=0xE01EF => 0,
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1F64F
        | 0x1F900..=0x1F9FF
        | 0x20000..=0x3FFFD => 2,
        _ => 1,
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
