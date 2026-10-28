//! The clang-compatible text renderer (`spec/diag.md` section "Text
//! output"). A transcription of clang 19's `TextDiagnosticPrinter` and
//! `TextDiagnostic`.

use std::io::Write;

use kiln_source::{Loc, SourceManager, Span};

use crate::columns::{ColumnMap, expand_tabs, printable_next, raw_line, raw_line_col};
use crate::diag::{Diagnostic, FixIt, Level};
use crate::engine::DiagSink;
use crate::layout::{self, Entry, EntryKind, Range};
use crate::options::DiagOptions;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const REVERSE: &str = "\x1b[7m";
const CARET_COLOR: &str = "\x1b[0;1;32m";
const FIXIT_COLOR: &str = "\x1b[0;32m";

/// Lines longer than this (in bytes) are not shown (clang's
/// `MaxLineLengthToPrint`).
const MAX_LINE_LENGTH: usize = 4096;

fn level_color(level: Level) -> &'static str {
    match level {
        Level::Note => "\x1b[0;1;36m",
        Level::Remark => "\x1b[0;1;34m",
        Level::Warning => "\x1b[0;1;35m",
        Level::Error | Level::Fatal => "\x1b[0;1;31m",
    }
}

/// The settings of [`DiagOptions`] the text renderer uses.
#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_excessive_bools)]
struct TextOptions {
    colors: bool,
    carets: bool,
    line_numbers: bool,
    option_names: bool,
    column: bool,
    location: bool,
    fixits: bool,
    tabstop: u32,
    snippet_lines: u32,
    backtrace_limit: u32,
}

/// Renders diagnostics as clang does, to any [`Write`]r. Output errors are
/// ignored.
#[derive(Debug)]
pub struct TextSink<W: Write> {
    out: W,
    opts: TextOptions,
    prefix: Option<String>,
    last_loc: Loc,
    last_level: Option<Level>,
    last_include_loc: Loc,
}

impl<W: Write> TextSink<W> {
    /// A text sink writing to `out` with the display settings of `opts`.
    pub fn new(out: W, opts: &DiagOptions) -> TextSink<W> {
        TextSink {
            out,
            opts: TextOptions {
                colors: opts.show_colors,
                carets: opts.show_carets,
                line_numbers: opts.show_line_numbers,
                option_names: opts.show_option,
                column: opts.show_column,
                location: opts.show_location,
                fixits: opts.show_fixits,
                tabstop: opts.tabstop,
                snippet_lines: opts.snippet_line_limit,
                backtrace_limit: opts.macro_backtrace_limit,
            },
            prefix: None,
            last_loc: Loc::INVALID,
            last_level: None,
            last_include_loc: Loc::INVALID,
        }
    }

    /// Prints `prefix: ` before each diagnostic (clang's
    /// `TextDiagnosticPrinter::setPrefix`; the driver passes the program
    /// name).
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> TextSink<W> {
        self.prefix = Some(prefix.into());
        self
    }

    /// The underlying writer.
    pub fn get_ref(&self) -> &W {
        &self.out
    }

    /// Renders one diagnostic to a byte vector (used by the sink).
    fn render(&mut self, sm: &SourceManager, d: &Diagnostic) -> Vec<u8> {
        let mut buf = Vec::new();
        for e in layout::layout(sm, d, self.opts.backtrace_limit) {
            self.entry(sm, &e, &mut buf);
        }
        buf
    }

    fn entry(&mut self, sm: &SourceManager, e: &Entry, buf: &mut Vec<u8>) {
        let o = self.opts;
        let mut message = e.message.clone();
        if o.option_names
            && let Some(opt) = &e.option
        {
            message.push_str(" [");
            message.push_str(opt);
            message.push(']');
        }
        match e.kind {
            EntryKind::BasicNote => {
                // clang's emitBasicNote: no colors, no renderer state.
                push(buf, "note: ");
                push(buf, &e.message);
                buf.push(b'\n');
            }
            EntryKind::Locationless => {
                self.print_prefix(buf);
                self.print_level(buf, e.level);
                self.print_message(buf, e.level, &message);
            }
            EntryKind::Located | EntryKind::MacroNote => {
                if e.kind == EntryKind::Located {
                    self.print_prefix(buf);
                }
                self.include_stack(sm, e.loc, e.level, buf);
                self.print_loc(sm, e.loc, buf);
                if o.colors {
                    push(buf, RESET);
                }
                self.print_level(buf, e.level);
                self.print_message(buf, e.level, &message);
                self.snippet(sm, e.loc, e.level, &e.ranges, &e.fixits, buf);
                self.last_loc = e.loc;
                self.last_level = Some(e.level);
            }
        }
    }

    fn print_prefix(&self, buf: &mut Vec<u8>) {
        if let Some(p) = &self.prefix {
            push(buf, p);
            push(buf, ": ");
        }
    }

    fn print_level(&self, buf: &mut Vec<u8>, level: Level) {
        if self.opts.colors {
            push(buf, level_color(level));
        }
        push(buf, level.text_name());
        push(buf, ": ");
        if self.opts.colors {
            push(buf, RESET);
        }
    }

    fn print_message(&self, buf: &mut Vec<u8>, level: Level, message: &str) {
        if self.opts.colors && level != Level::Note {
            push(buf, BOLD);
        }
        push(buf, message);
        if self.opts.colors {
            push(buf, RESET);
        }
        buf.push(b'\n');
    }

    /// clang's `emitDiagnosticLoc`: `file:line:col: `.
    fn print_loc(&self, sm: &SourceManager, loc: Loc, buf: &mut Vec<u8>) {
        if !self.opts.location {
            return;
        }
        let Some(p) = sm.presumed_line_col(loc) else { return };
        if self.opts.colors {
            push(buf, BOLD);
        }
        push(buf, p.file_name);
        push(buf, &format!(":{}", p.line));
        if self.opts.column
            && let Some((_, _, col)) = raw_line_col(sm, loc)
        {
            push(buf, &format!(":{col}"));
        }
        push(buf, ": ");
    }

    /// clang's `emitIncludeStack`. Kiln does not record inclusions yet
    /// (WP-014), so the include location is always invalid and nothing
    /// is printed; the state machine is in place for when it does.
    fn include_stack(&mut self, sm: &SourceManager, loc: Loc, level: Level, buf: &mut Vec<u8>) {
        let inc = if loc.is_valid() { include_loc(sm, loc) } else { Loc::INVALID };
        if inc == self.last_include_loc {
            return;
        }
        self.last_include_loc = inc;
        if level == Level::Note {
            return;
        }
        self.include_stack_recursively(sm, inc, buf);
    }

    fn include_stack_recursively(&self, sm: &SourceManager, loc: Loc, buf: &mut Vec<u8>) {
        if !loc.is_valid() {
            return;
        }
        let Some(p) = sm.presumed_line_col(loc) else { return };
        self.include_stack_recursively(sm, include_loc(sm, loc), buf);
        if self.opts.location {
            push(buf, &format!("In file included from {}:{}:\n", p.file_name, p.line));
        } else {
            push(buf, "In included file:\n");
        }
    }

    /// clang's `emitSnippetAndCaret`.
    #[allow(clippy::too_many_lines)]
    fn snippet(
        &self,
        sm: &SourceManager,
        loc: Loc,
        level: Level,
        ranges: &[Range],
        fixits: &[FixIt],
        buf: &mut Vec<u8>,
    ) {
        let o = self.opts;
        if !o.carets {
            return;
        }
        if loc == self.last_loc
            && ranges.is_empty()
            && fixits.is_empty()
            && (self.last_level != Some(Level::Note) || Some(level) == self.last_level)
        {
            return;
        }
        let Some((file, caret_line, caret_col)) = raw_line_col(sm, loc) else { return };
        if caret_col as usize > MAX_LINE_LENGTH {
            return;
        }
        let max_lines = o.snippet_lines;
        let mut lines = (caret_line, caret_line);
        let mut display = sm.presumed_line_col(loc).map_or(caret_line, |p| p.line);
        for r in ranges {
            if let (Some((f1, l1, _)), Some((f2, l2, _))) =
                (raw_line_col(sm, r.span.lo), raw_line_col(sm, r.span.hi))
                && f1 == file
                && f2 == file
            {
                lines = maybe_add_range(lines, (l1, l2), max_lines);
            }
            if let Some(p) = sm.presumed_line_col(r.span.lo) {
                display = display.min(p.line);
            }
        }
        let width =
            if o.line_numbers { digits(display.saturating_add(max_lines)).max(4) } else { 0 };
        let line_ranges = prepare_ranges(sm, ranges, lines, file);

        let mut line_no = lines.0;
        while line_no <= lines.1 {
            let Some(raw) = raw_line(sm, file, line_no) else { break };
            let mut src: Vec<u8> = raw.bytes.into_owned();
            if src.len() > MAX_LINE_LENGTH {
                return;
            }
            while src.last() == Some(&0)
                && (line_no != caret_line || src.len() > caret_col as usize)
            {
                src.pop();
            }
            let map = ColumnMap::new(&src, o.tabstop);
            let mut caret: Vec<u8> = Vec::new();
            for lr in line_ranges.iter().filter(|lr| lr.line == line_no) {
                highlight_range(lr, &map, &src, &mut caret);
            }
            if line_no == caret_line {
                let col = map.byte_to_containing_column(caret_col as usize - 1);
                if caret.len() < col + 1 {
                    caret.resize(col + 1, b' ');
                }
                caret[col] = b'^';
            }
            let fixit_line = self.fixit_line(sm, file, line_no, &map, fixits);

            // emitSnippet
            if width > 0 {
                indent(buf, width - digits(display) + 1);
                push(buf, &format!("{display} | "));
            }
            let mut i = 0;
            let mut reversed = false;
            while i < src.len() {
                let (s, printable) = printable_next(&src, &mut i, o.tabstop);
                if o.colors && printable == reversed {
                    reversed = !reversed;
                    push(buf, if reversed { REVERSE } else { RESET });
                }
                push(buf, &s);
            }
            if o.colors {
                push(buf, RESET);
            }
            buf.push(b'\n');

            if !caret.is_empty() {
                gutter(buf, width);
                if o.colors {
                    push(buf, CARET_COLOR);
                }
                buf.extend_from_slice(&caret);
                buf.push(b'\n');
                if o.colors {
                    push(buf, RESET);
                }
            }
            if !fixit_line.is_empty() {
                gutter(buf, width);
                if o.colors {
                    push(buf, FIXIT_COLOR);
                }
                buf.extend_from_slice(&fixit_line);
                buf.push(b'\n');
                if o.colors {
                    push(buf, RESET);
                }
            }
            line_no += 1;
            display = display.saturating_add(1);
        }
    }

    /// clang's `buildFixItInsertionLine`.
    fn fixit_line(
        &self,
        sm: &SourceManager,
        file: kiln_source::FileId,
        line: u32,
        map: &ColumnMap,
        fixits: &[FixIt],
    ) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        if fixits.is_empty() || !self.opts.fixits {
            return out;
        }
        let mut prev_end = 0usize;
        for f in fixits {
            if f.text.is_empty() || f.text.contains(['\n', '\r']) {
                continue;
            }
            let Some((hf, hl, hc)) = raw_line_col(sm, sm.expansion_loc(f.span.lo)) else {
                continue;
            };
            if hf != file || hl != line {
                continue;
            }
            let mut col = map.byte_to_containing_column(hc as usize - 1);
            if col < prev_end {
                col = prev_end + 1;
            }
            let new_len = out.len() + (col - prev_end) + f.text.len();
            if new_len > out.len() {
                out.resize(new_len, b' ');
            }
            let start = out.len() - f.text.len();
            out[start..].copy_from_slice(f.text.as_bytes());
            prev_end = col + crate::columns::column_width(&f.text);
        }
        expand_tabs(&out, self.opts.tabstop)
    }
}

/// The location of the `#include` that entered the buffer of `loc`. Hook
/// for WP-014: always invalid until `kiln_source` records inclusions.
fn include_loc(_sm: &SourceManager, _loc: Loc) -> Loc {
    Loc::INVALID
}

fn gutter(buf: &mut Vec<u8>, width: u32) {
    if width > 0 {
        indent(buf, width + 2);
        push(buf, "| ");
    }
}

fn push(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(s.as_bytes());
}

fn indent(buf: &mut Vec<u8>, n: u32) {
    buf.extend(std::iter::repeat_n(b' ', n as usize));
}

fn digits(n: u32) -> u32 {
    n.checked_ilog10().map_or(1, |l| l + 1)
}

/// clang's `maybeAddRange`.
fn maybe_add_range(mut a: (u32, u32), b: (u32, u32), max_range: u32) -> (u32, u32) {
    let mut slack = max_range.saturating_sub(a.1 - a.0 + 1);
    if slack == 0 {
        return a;
    }
    let min = a.0.min(b.0);
    let max = a.1.max(b.1);
    if max - min < max_range {
        return (min, max);
    }
    if (b.0 > a.0 && b.0 - a.0 + 1 > max_range) || (b.1 < a.1 && a.1 - b.1 + 1 > max_range) {
        return a;
    }
    a.1 = (a.1 + slack.div_ceil(2)).min(max);
    slack = max_range.saturating_sub(a.1 - a.0 + 1);
    a.0 = (min + slack).max(a.0) - slack;
    a.1 = (a.0 + max_range - 1).min(max);
    a
}

/// One line's share of a range, in raw byte columns (`end` exclusive).
#[derive(Debug)]
struct LineRange {
    line: u32,
    start: usize,
    end: usize,
}

/// clang's `prepareAndFilterRanges`.
fn prepare_ranges(
    sm: &SourceManager,
    ranges: &[Range],
    lines: (u32, u32),
    file: kiln_source::FileId,
) -> Vec<LineRange> {
    let mut out = Vec::new();
    for r in ranges {
        let Span { lo, hi } = r.span;
        let Some((f1, sl, sc)) = raw_line_col(sm, lo) else { continue };
        if sl > lines.1 || f1 != file {
            continue;
        }
        let Some((f2, el, ec)) = raw_line_col(sm, hi) else { continue };
        if el < lines.0 || f2 != file {
            continue;
        }
        let (sc, ec) = (sc as usize - 1, ec as usize - 1);
        if sl == el {
            out.push(LineRange { line: sl, start: sc, end: ec });
            continue;
        }
        out.push(LineRange { line: sl, start: sc, end: usize::MAX });
        for l in sl + 1..el {
            out.push(LineRange { line: l, start: 0, end: usize::MAX });
        }
        out.push(LineRange { line: el, start: 0, end: ec });
    }
    out
}

/// clang's `highlightRange`.
fn highlight_range(r: &LineRange, map: &ColumnMap, src: &[u8], caret: &mut Vec<u8>) {
    let is_space = |b: u8| b == b' ' || b == b'\t';
    let mut start = r.start;
    while start < src.len() && is_space(src[start]) {
        start = map.start_of_next_column(start);
    }
    let mut end = r.end.min(src.len());
    while end > 0 && is_space(src[end - 1]) {
        end = map.start_of_previous_column(end);
    }
    if start > end {
        return;
    }
    let start = map.byte_to_containing_column(start);
    let end = map.byte_to_containing_column(end);
    if caret.len() < end {
        caret.resize(end, b' ');
    }
    caret[start..end].fill(b'~');
}

impl<W: Write> DiagSink for TextSink<W> {
    fn emit(&mut self, sm: &SourceManager, d: &Diagnostic) {
        let buf = self.render(sm, d);
        let _ = self.out.write_all(&buf);
    }

    fn flush(&mut self) {
        let _ = self.out.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_range() {
        assert_eq!(maybe_add_range((4, 4), (3, 6), 16), (3, 6));
        assert_eq!(maybe_add_range((10, 10), (1, 40), 16), (3, 18));
        assert_eq!(maybe_add_range((10, 10), (40, 50), 16), (10, 10));
        assert_eq!(digits(9), 1);
        assert_eq!(digits(10015), 5);
    }
}
