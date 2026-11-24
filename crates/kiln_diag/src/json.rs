//! The JSON renderer (`spec/diag.md` section "JSON output"): one object
//! per diagnostic per line.

use std::fmt::Write as _;
use std::io::Write;

use kiln_source::{Loc, SourceManager};

use crate::columns::raw_line_col;
use crate::diag::{Diagnostic, FixIt};
use crate::engine::DiagSink;
use crate::layout::{self, Entry, EntryKind};
use crate::options::DiagOptions;

/// Escapes `s` as the contents of a JSON string (RFC 8259): `"` and `\`
/// are backslash-escaped, `\n` `\r` `\t` `\u{8}` `\u{c}` use their short
/// forms, other control characters (U+0000 to U+001F and U+007F) use
/// `\u00XX`. Everything else, non-ASCII included, is copied.
pub fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn string(out: &mut String, s: &str) {
    out.push('"');
    out.push_str(&escape_json(s));
    out.push('"');
}

fn opt_string(out: &mut String, s: Option<&str>) {
    match s {
        Some(s) => string(out, s),
        None => out.push_str("null"),
    }
}

/// Writes one JSON object per diagnostic, each on its own line.
#[derive(Debug)]
pub struct JsonSink<W: Write> {
    out: W,
    backtrace_limit: u32,
    clang_compat: u32,
}

impl<W: Write> JsonSink<W> {
    /// A JSON sink writing to `out`.
    pub fn new(out: W, opts: &DiagOptions) -> JsonSink<W> {
        JsonSink {
            out,
            backtrace_limit: opts.macro_backtrace_limit,
            clang_compat: opts.clang_compat,
        }
    }

    /// The underlying writer.
    pub fn get_ref(&self) -> &W {
        &self.out
    }
}

/// `"file":..,"line":..,"col":..` for `loc` (presumed file and line, byte
/// column); nulls when `loc` is invalid.
fn position(out: &mut String, sm: &SourceManager, loc: Loc, prefix: &str) {
    let p = if loc.is_valid() { sm.presumed_line_col(loc) } else { None };
    let col = if loc.is_valid() { raw_line_col(sm, loc).map(|(_, _, c)| c) } else { None };
    if let (Some(p), Some(col)) = (p, col) {
        if prefix.is_empty() {
            out.push_str("\"file\":");
            string(out, p.file_name);
            out.push(',');
        }
        let _ = write!(out, "\"{prefix}line\":{},\"{prefix}col\":{col}", p.line);
    } else {
        if prefix.is_empty() {
            out.push_str("\"file\":null,");
        }
        let _ = write!(out, "\"{prefix}line\":null,\"{prefix}col\":null");
    }
}

fn entry_json(out: &mut String, sm: &SourceManager, e: &Entry, notes: Option<&[Entry]>) {
    out.push_str("{\"id\":");
    opt_string(out, e.id.map(crate::DiagId::name));
    out.push_str(",\"severity\":");
    string(out, e.level.json_name());
    out.push(',');
    let loc = if e.kind == EntryKind::Located || e.kind == EntryKind::MacroNote {
        e.loc
    } else {
        Loc::INVALID
    };
    position(out, sm, loc, "");
    out.push_str(",\"message\":");
    string(out, &e.message);
    out.push_str(",\"group\":");
    opt_string(out, e.id.and_then(crate::DiagId::display_group).map(crate::Group::name));
    out.push_str(",\"option\":");
    opt_string(out, e.option.as_deref());
    if let Some(notes) = notes {
        out.push_str(",\"notes\":[");
        for (i, n) in notes.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            entry_json(out, sm, n, None);
        }
        out.push(']');
    }
    out.push_str(",\"spans\":[");
    let mut first = true;
    for r in &e.ranges {
        let Some(label) = &r.label else { continue };
        if !first {
            out.push(',');
        }
        first = false;
        out.push('{');
        position(out, sm, r.span.lo, "");
        out.push(',');
        position(out, sm, r.span.hi, "end_");
        out.push_str(",\"label\":");
        string(out, label);
        out.push('}');
    }
    out.push_str("],\"fixits\":[");
    let mut first = true;
    for f in &e.fixits {
        if !fixit_json(out, sm, f, first) {
            continue;
        }
        first = false;
    }
    out.push_str("]}");
}

/// Writes a fix-it whose ends are both file locations; skips others (as
/// clang's parseable fix-its do).
fn fixit_json(out: &mut String, sm: &SourceManager, f: &FixIt, first: bool) -> bool {
    if !f.span.lo.is_file() || !f.span.hi.is_file() || raw_line_col(sm, f.span.lo).is_none() {
        return false;
    }
    if !first {
        out.push(',');
    }
    out.push('{');
    position(out, sm, f.span.lo, "");
    out.push(',');
    position(out, sm, f.span.hi, "end_");
    out.push_str(",\"text\":");
    string(out, &f.text);
    out.push('}');
    true
}

/// Where a [`DiagRecord`] is (spec/diag.md §10.4): its JSON object's
/// `file`, `line` and `col`, with the buffer and byte offset they are
/// computed from.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct RecordLocation {
    /// The buffer of the caret (a file location: clang's `getFileLoc`).
    pub file: kiln_source::FileId,
    /// The caret's byte offset in `file`.
    pub offset: u32,
    /// The presumed file name (the last `#line` name in effect, else the
    /// buffer's name), as the text header prints it.
    pub presumed_file: String,
    /// The presumed 1-based line.
    pub line: u32,
    /// The 1-based byte column (raw, unaffected by `#line`).
    pub column: u32,
}

/// One diagnostic as data (spec/diag.md §10.4, WP-2827): the fields of
/// the object `-fdiagnostics-format=json` prints for it (§10.1), computed
/// by the same layout, so they hold no location of the source manager
/// and outlive it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct DiagRecord {
    /// The id (`None` for free-text notes); macro-expansion notes are
    /// `diag_expanded_from_macro`, `diag_expanded_from_here`,
    /// `diag_backtrace_skipped`.
    pub id: Option<crate::DiagId>,
    /// The severity after policy.
    pub level: crate::Level,
    /// The message, as rendered (without the option suffix).
    pub message: String,
    /// The display group, if any.
    pub group: Option<&'static str>,
    /// The bracketed suffix (`-Wunused-variable`), if any.
    pub option: Option<String>,
    /// The caret's location; `None` for a locationless diagnostic.
    pub location: Option<RecordLocation>,
    /// The notes printed under it, in order (attached notes and the
    /// macro-expansion notes the layout derives); empty for a note.
    pub notes: Vec<DiagRecord>,
}

fn record_of(sm: &SourceManager, e: &Entry) -> DiagRecord {
    let loc = if e.kind == EntryKind::Located || e.kind == EntryKind::MacroNote {
        e.loc
    } else {
        Loc::INVALID
    };
    let location = if loc.is_valid() {
        match (sm.presumed_line_col(loc), raw_line_col(sm, loc)) {
            (Some(p), Some((file, _, column))) => {
                sm.offset_of(loc).map(|(_, offset)| RecordLocation {
                    file,
                    offset,
                    presumed_file: p.file_name.to_owned(),
                    line: p.line,
                    column,
                })
            }
            _ => None,
        }
    } else {
        None
    };
    DiagRecord {
        id: e.id,
        level: e.level,
        message: e.message.clone(),
        group: e.id.and_then(crate::DiagId::display_group).map(crate::Group::name),
        option: e.option.clone(),
        location,
        notes: Vec::new(),
    }
}

/// `records` (as a sink received them) as data (spec/diag.md §10.4): one
/// [`DiagRecord`] per diagnostic, with exactly the content
/// `-fdiagnostics-format=json` prints for it under `opts`' macro
/// backtrace limit and clang level.
#[must_use]
pub fn diag_records(
    sm: &SourceManager,
    opts: &DiagOptions,
    records: &[Diagnostic],
) -> Vec<DiagRecord> {
    records
        .iter()
        .filter_map(|d| {
            let entries = layout::layout(sm, d, opts.macro_backtrace_limit, opts.clang_compat);
            let (head, notes) = entries.split_first()?;
            let mut r = record_of(sm, head);
            r.notes = notes.iter().map(|n| record_of(sm, n)).collect();
            Some(r)
        })
        .collect()
}

/// Renders `d` as one line of JSON (without the newline).
pub(crate) fn render(
    sm: &SourceManager,
    d: &Diagnostic,
    backtrace_limit: u32,
    compat: u32,
) -> String {
    let entries = layout::layout(sm, d, backtrace_limit, compat);
    let mut out = String::new();
    if let Some((head, notes)) = entries.split_first() {
        entry_json(&mut out, sm, head, Some(notes));
    }
    out
}

impl<W: Write> DiagSink for JsonSink<W> {
    fn emit(&mut self, sm: &SourceManager, d: &Diagnostic) {
        let mut line = render(sm, d, self.backtrace_limit, self.clang_compat);
        line.push('\n');
        let _ = self.out.write_all(line.as_bytes());
    }

    fn flush(&mut self) {
        let _ = self.out.flush();
    }
}
