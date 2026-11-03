//! From a [`Diagnostic`] to the sequence of located entries a renderer
//! prints (`spec/diag.md` section "Layout"): the diagnostic itself, its
//! macro-expansion notes, and its attached notes with theirs.
//!
//! A transcription of clang's `DiagnosticRenderer` (`emitDiagnostic`,
//! `emitMacroExpansions`, `emitSingleMacroExpansion`,
//! `mapDiagnosticRanges`, `retrieveMacroLocation`) over kiln's location
//! model, in which every range is half-open (`Span::hi` is one past the
//! end) so clang's token/char range distinction disappears.

use kiln_source::{ExpansionKind, FileId, Loc, SourceManager, Span};

use crate::diag::{Diagnostic, FixIt, Level};
use crate::ids::DiagId;

/// The buffer the "expanded from here" rule looks for: tokens spelled in
/// a buffer of this name (pasted or stringized tokens) have no macro name.
pub const SCRATCH_BUFFER_NAME: &str = "<scratch space>";

/// What an [`Entry`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntryKind {
    /// A diagnostic (or note) with a file location.
    Located,
    /// A diagnostic (or note) without a location.
    Locationless,
    /// clang's `emitBasicNote`: the plain `note: (skipping ...)` line.
    BasicNote,
    /// A macro-expansion note derived from the locations.
    MacroNote,
}

/// A range to underline, mapped to file locations in the caret's file.
#[derive(Clone, Debug)]
pub(crate) struct Range {
    pub span: Span,
    /// The label for ranges from [`Diagnostic::labels`]; `None` for ranges
    /// from fix-its.
    pub label: Option<String>,
}

/// One printed diagnostic or note.
#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub kind: EntryKind,
    pub id: Option<DiagId>,
    pub level: Level,
    pub message: String,
    pub option: Option<String>,
    /// File location of the caret (clang's `getFileLoc`); invalid for
    /// locationless entries.
    pub loc: Loc,
    pub ranges: Vec<Range>,
    pub fixits: Vec<FixIt>,
}

/// The expansion entry (or buffer) a location belongs to: clang's
/// `FileID` of a `SourceLocation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Owner {
    File(FileId),
    Macro(Loc),
    Invalid,
}

fn owner(sm: &SourceManager, loc: Loc) -> Owner {
    if loc.is_macro() {
        sm.expansion_info(loc).map_or(Owner::Invalid, |e| Owner::Macro(e.base))
    } else {
        sm.offset_of(loc).map_or(Owner::Invalid, |(f, _)| Owner::File(f))
    }
}

fn is_macro_arg(sm: &SourceManager, loc: Loc) -> bool {
    sm.expansion_info(loc).is_some_and(|e| e.kind == ExpansionKind::MacroArg)
}

/// clang's `getImmediateExpansionRange`.
fn immediate_expansion(sm: &SourceManager, loc: Loc) -> Span {
    sm.expansion_info(loc).map_or(Span::INVALID, |e| e.expansion)
}

/// clang's `SourceManager::getFileLoc`: argument locations step to where
/// the argument was written, other macro locations to the invocation.
pub(crate) fn file_loc(sm: &SourceManager, mut loc: Loc) -> Loc {
    while loc.is_macro() {
        let Some(e) = sm.expansion_info(loc) else { return Loc::INVALID };
        loc = match e.kind {
            ExpansionKind::MacroArg => sm.immediate_spelling_loc(loc),
            ExpansionKind::Macro { .. } => e.expansion.lo,
        };
    }
    loc
}

/// clang's `getMacroArgExpansionFileIDs`.
fn arg_owners(sm: &SourceManager, mut loc: Loc, is_begin: bool) -> Vec<Owner> {
    let mut out = Vec::new();
    while loc.is_macro() {
        if is_macro_arg(sm, loc) {
            out.push(owner(sm, loc));
            loc = sm.immediate_spelling_loc(loc);
        } else {
            let r = immediate_expansion(sm, loc);
            loc = if is_begin { r.lo } else { r.hi };
        }
    }
    out
}

/// clang's `computeCommonMacroArgExpansionFileIDs`.
fn common_arg_owners(sm: &SourceManager, begin: Loc, end: Loc) -> Vec<Owner> {
    let mut b = arg_owners(sm, begin, true);
    let mut e = arg_owners(sm, end, false);
    b.sort_unstable();
    e.sort_unstable();
    b.retain(|o| e.binary_search(o).is_ok());
    b.dedup();
    b
}

/// clang's `retrieveMacroLocation`: walks `loc` out of macro expansions
/// until it lands in `caret`'s entry.
fn retrieve(sm: &SourceManager, loc: Loc, caret: Owner, common: &[Owner], is_begin: bool) -> Loc {
    let own = owner(sm, loc);
    if own == caret {
        return loc;
    }
    if !loc.is_macro() || own == Owner::Invalid {
        return Loc::INVALID;
    }
    let (macro_range, arg_range) = if is_macro_arg(sm, loc) {
        let isl = sm.immediate_spelling_loc(loc);
        let mr =
            if common.binary_search(&own).is_ok() { Span::new(isl, isl) } else { Span::INVALID };
        (mr, immediate_expansion(sm, loc))
    } else {
        let isl = sm.immediate_spelling_loc(loc);
        (immediate_expansion(sm, loc), Span::new(isl, isl))
    };
    let m = if is_begin { macro_range.lo } else { macro_range.hi };
    if m.is_valid() {
        let r = retrieve(sm, m, caret, common, is_begin);
        if r.is_valid() {
            return r;
        }
    }
    let a = if is_begin { arg_range.lo } else { arg_range.hi };
    if !a.is_valid() {
        return Loc::INVALID;
    }
    retrieve(sm, a, caret, common, is_begin)
}

/// clang's `mapDiagnosticRange` for one range: maps `span` into the entry
/// of `caret`, then to spelling locations. `None` if it cannot be shown.
fn map_range(sm: &SourceManager, caret: Loc, span: Span) -> Option<Span> {
    if !span.lo.is_valid() || !span.hi.is_valid() {
        return None;
    }
    let caret_owner = owner(sm, caret);
    let (mut begin, mut end) = (span.lo, span.hi);
    let mut bo = owner(sm, begin);
    let mut eo = owner(sm, end);
    let mut begin_map: Vec<(Owner, Loc)> = Vec::new();
    while begin.is_macro() && bo != eo {
        begin_map.push((bo, begin));
        begin = immediate_expansion(sm, begin).lo;
        bo = owner(sm, begin);
    }
    if bo != eo {
        let find =
            |o: Owner, m: &[(Owner, Loc)]| m.iter().rev().find(|(k, _)| *k == o).map(|&(_, l)| l);
        while end.is_macro() && find(eo, &begin_map).is_none() {
            end = immediate_expansion(sm, end).hi;
            eo = owner(sm, end);
        }
        if end.is_macro() {
            begin = find(eo, &begin_map).unwrap_or(Loc::INVALID);
            bo = eo;
        }
    }
    if !begin.is_valid() || !end.is_valid() || bo != eo || bo == Owner::Invalid {
        return None;
    }
    let common = common_arg_owners(sm, begin, end);
    let begin = retrieve(sm, begin, caret_owner, &common, true);
    let end = retrieve(sm, end, caret_owner, &common, false);
    if !begin.is_valid() || !end.is_valid() {
        return None;
    }
    Some(Span::new(sm.spelling_loc(begin), sm.spelling_loc(end)))
}

fn map_ranges(sm: &SourceManager, caret: Loc, ranges: &[Range]) -> Vec<Range> {
    ranges
        .iter()
        .filter_map(|r| {
            map_range(sm, caret, r.span).map(|span| Range { span, label: r.label.clone() })
        })
        .collect()
}

/// clang's `checkRangesForMacroArgExpansion`: whether the expansion note
/// for macro location `l` is dropped as adding nothing. clang 19 compares
/// the ranges *after* `mapDiagnosticRanges`, which yields spelling (file)
/// locations that are never macro-argument expansions; so the note is
/// dropped only for a diagnostic without valid ranges whose location `l`
/// is a macro-argument expansion (e.g. `use of undeclared identifier` on
/// a macro argument), and kept whenever a range is shown.
///
/// clang 22 (`rangesInsideSameMacroArgExpansion`) checks the unmapped
/// ranges, including the invalid ones a diagnostic streams (an unset
/// `R2`): every range must be a single token spelled in a macro argument
/// (a longer range never qualifies, as its locations' argument expansion
/// start is not `l`), so an invalid range keeps the notes (spec/diag.md
/// §3.3).
fn ranges_in_same_arg(sm: &SourceManager, l: Loc, ranges: &[Range], compat: u32) -> bool {
    if !sm.expansion_info(l).is_some_and(|e| e.kind == ExpansionKind::MacroArg) {
        return false;
    }
    if compat < 22 {
        return ranges.iter().all(|r| !r.span.lo.is_valid());
    }
    let valid: Vec<Range> = ranges.iter().filter(|r| r.span.lo.is_valid()).cloned().collect();
    if map_ranges(sm, l, &valid).len() < valid.len() {
        return false;
    }
    ranges.iter().all(|r| {
        r.span.lo.is_valid()
            && sm.expansion_info(r.span.lo).is_some_and(|e| e.kind == ExpansionKind::MacroArg)
            && single_token(sm, r.span)
    })
}

/// Whether `span` covers one token: a token range whose begin is its end
/// (as sema streams an expression's range), or a half-open range over an
/// identifier, a number or a literal.
fn single_token(sm: &SourceManager, span: Span) -> bool {
    if span.lo == span.hi {
        return true;
    }
    let Some(d) = sm.same_entry_distance(span.lo, span.hi) else { return false };
    let lo = sm.spelling_loc(span.lo);
    let Some(t) = sm.span_text(Span::new(lo, lo.with_offset(d))) else { return false };
    let t = t.trim_end();
    let Some(c) = t.chars().next() else { return false };
    if c == '"' || c == '\'' {
        return !t.contains(char::is_whitespace);
    }
    let digit = c.is_ascii_digit();
    t.chars().all(|x| x.is_alphanumeric() || x == '_' || (digit && (x == '.' || x == '\'')))
}

/// clang's `Lexer::getImmediateMacroNameForDiagnostics`: the name of the
/// macro whose expansion produced `loc`, or `None` for tokens spelled in
/// the scratch buffer.
fn macro_name(sm: &SourceManager, mut loc: Loc) -> Option<&str> {
    loop {
        let e = sm.expansion_info(loc)?;
        match e.kind {
            ExpansionKind::MacroArg => loc = e.expansion.lo,
            ExpansionKind::Macro { name } => {
                let spell = sm.spelling_loc(loc);
                let (file, _) = sm.offset_of(spell)?;
                if sm.file_name(file) == SCRATCH_BUFFER_NAME {
                    return None;
                }
                return sm.span_text(name).filter(|s| !s.is_empty());
            }
        }
    }
}

/// Lays out `d` and its notes.
pub(crate) fn layout(
    sm: &SourceManager,
    d: &Diagnostic,
    backtrace_limit: u32,
    compat: u32,
) -> Vec<Entry> {
    let mut out = Vec::new();
    push_diagnostic(sm, d, backtrace_limit, compat, &mut out);
    for n in &d.notes {
        push_diagnostic(sm, n, backtrace_limit, compat, &mut out);
    }
    out
}

fn push_diagnostic(
    sm: &SourceManager,
    d: &Diagnostic,
    limit: u32,
    compat: u32,
    out: &mut Vec<Entry>,
) {
    let loc = d.span.lo;
    let mut raw: Vec<Range> = d
        .labels
        .iter()
        .filter(|l| l.span.is_valid())
        .map(|l| Range { span: l.span, label: Some(l.text.clone()) })
        .collect();
    raw.extend(
        d.fixits.iter().filter(|f| f.span.is_valid()).map(|f| Range { span: f.span, label: None }),
    );
    let floc = if loc.is_valid() { file_loc(sm, loc) } else { Loc::INVALID };
    if !floc.is_valid() {
        out.push(Entry {
            kind: EntryKind::Locationless,
            id: d.id,
            level: d.level,
            message: d.message.clone(),
            option: d.option.clone(),
            loc: Loc::INVALID,
            ranges: Vec::new(),
            fixits: Vec::new(),
        });
        return;
    }
    out.push(Entry {
        kind: EntryKind::Located,
        id: d.id,
        level: d.level,
        message: d.message.clone(),
        option: d.option.clone(),
        loc: floc,
        ranges: map_ranges(sm, floc, &raw),
        fixits: d.fixits.clone(),
    });
    if loc.is_macro() {
        // clang 22 also sees the invalid ranges a diagnostic streams.
        let all: Vec<Range> = if compat >= 22 {
            let mut all: Vec<Range> = d
                .labels
                .iter()
                .map(|l| Range { span: l.span, label: Some(l.text.clone()) })
                .collect();
            all.extend(merged_fixit_ranges(sm, &d.fixits));
            all
        } else {
            Vec::new()
        };
        macro_expansions(sm, loc, &raw, &all, limit, compat, out);
    }
}

/// The ranges of `fixits` as clang's `mergeFixits` leaves them for the
/// macro-backtrace check: each end moved out of macro arguments to where
/// the argument is spelled, and through the start of a macro expansion
/// to its invocation (`edit::Commit::canInsert`); when an end stays in a
/// macro the commit fails and no fix-it range remains.
fn merged_fixit_ranges(sm: &SourceManager, fixits: &[FixIt]) -> Vec<Range> {
    let commit = |mut l: Loc| -> Option<Loc> {
        while sm.expansion_info(l).is_some_and(|e| e.kind == ExpansionKind::MacroArg) {
            l = sm.immediate_spelling_loc(l);
        }
        while l.is_macro() {
            let e = sm.expansion_info(l)?;
            if e.kind == ExpansionKind::MacroArg || l != e.base {
                return None;
            }
            l = e.expansion.lo;
        }
        l.is_valid().then_some(l)
    };
    let mut out = Vec::new();
    for f in fixits.iter().filter(|f| f.span.is_valid()) {
        let (Some(lo), Some(hi)) = (commit(f.span.lo), commit(f.span.hi)) else {
            return Vec::new();
        };
        out.push(Range { span: Span::new(lo, hi), label: None });
    }
    out
}

/// clang's `emitMacroExpansions`.
fn macro_expansions(
    sm: &SourceManager,
    start: Loc,
    ranges: &[Range],
    all: &[Range],
    limit: u32,
    compat: u32,
    out: &mut Vec<Entry>,
) {
    let mut stack: Vec<Loc> = Vec::new();
    let mut ignored_end = 0;
    let mut l = start;
    while l.is_macro() {
        let Some(e) = sm.expansion_info(l) else { break };
        stack.push(if e.kind == ExpansionKind::MacroArg { e.expansion.lo } else { l });
        if ranges_in_same_arg(sm, l, if compat >= 22 { all } else { ranges }, compat) {
            ignored_end = stack.len();
        }
        l = sm.immediate_macro_caller_loc(l);
        if l.is_file() {
            l = stack.last().map_or(Loc::INVALID, |&b| sm.immediate_macro_caller_loc(b));
        }
        if !l.is_valid() {
            break;
        }
    }
    stack.drain(..ignored_end);
    let depth = stack.len();
    let limit = limit as usize;
    if limit == 0 || depth <= limit {
        for &l in stack.iter().rev() {
            single_expansion(sm, l, ranges, out);
        }
        return;
    }
    let first = limit / 2;
    let last = limit / 2 + limit % 2;
    for &l in stack.iter().rev().take(first) {
        single_expansion(sm, l, ranges, out);
    }
    out.push(Entry {
        kind: EntryKind::BasicNote,
        id: Some(DiagId::diag_backtrace_skipped),
        level: Level::Note,
        message: crate::template::render(
            DiagId::diag_backtrace_skipped.template(),
            &[(depth - limit).to_string()],
        ),
        option: None,
        loc: Loc::INVALID,
        ranges: Vec::new(),
        fixits: Vec::new(),
    });
    for &l in stack[..last].iter().rev() {
        single_expansion(sm, l, ranges, out);
    }
}

/// clang's `emitSingleMacroExpansion`.
fn single_expansion(sm: &SourceManager, l: Loc, ranges: &[Range], out: &mut Vec<Entry>) {
    let spelling = sm.spelling_loc(l);
    let in_entry = map_ranges(sm, l, ranges);
    let (id, message) = match macro_name(sm, l) {
        Some(name) => (
            DiagId::diag_expanded_from_macro,
            crate::template::render(
                DiagId::diag_expanded_from_macro.template(),
                &[name.to_string()],
            ),
        ),
        None => (
            DiagId::diag_expanded_from_here,
            crate::template::render(DiagId::diag_expanded_from_here.template(), &[]),
        ),
    };
    // emitDiagnostic at the (file) spelling location maps the ranges
    // again, which drops those in other buffers.
    let ranges = map_ranges(sm, spelling, &in_entry);
    out.push(Entry {
        kind: EntryKind::MacroNote,
        id: Some(id),
        level: Level::Note,
        message,
        option: None,
        loc: spelling,
        ranges,
        fixits: Vec::new(),
    });
}
