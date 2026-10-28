//! `SourceManager`: buffers, location mapping, expansions, presumed locations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError, RwLock};
use std::{error, fmt, fs, io};

use crate::arena::AppendVec;
use crate::loc::{FileId, Loc, MACRO_BIT, Span};

/// Exclusive end of the file-location space (`[1, 2^31)`).
const FILE_SPACE_END: u64 = 1 << 31;
/// Exclusive end of the macro-location space (`[2^31, 2^32)`).
const MACRO_SPACE_END: u64 = 1 << 32;

/// The location space (2^31 positions each for buffers and for macro
/// expansions) is exhausted. Fatal for the translation unit, like clang's
/// "ran out of source locations".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocSpaceExhausted;

impl fmt::Display for LocSpaceExhausted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ran out of source locations; translation unit too large")
    }
}

impl error::Error for LocSpaceExhausted {}

/// Why [`SourceManager::load_file`] failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum LoadError {
    /// Canonicalizing or reading the path failed.
    Io(io::Error),
    /// The file does not fit in the remaining location space.
    LocSpaceExhausted,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io(e) => e.fmt(f),
            LoadError::LocSpaceExhausted => LocSpaceExhausted.fmt(f),
        }
    }
}

impl error::Error for LoadError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            LoadError::Io(e) => Some(e),
            LoadError::LocSpaceExhausted => None,
        }
    }
}

impl From<io::Error> for LoadError {
    fn from(e: io::Error) -> Self {
        LoadError::Io(e)
    }
}

impl From<LocSpaceExhausted> for LoadError {
    fn from(_: LocSpaceExhausted) -> Self {
        LoadError::LocSpaceExhausted
    }
}

/// One maximal invalid UTF-8 sequence replaced by U+FFFD at load time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    /// Byte offset of the U+FFFD (3 bytes) in the decoded text.
    pub offset: u32,
    /// The original invalid bytes (1 to 3 bytes).
    pub bytes: Box<[u8]>,
}

/// What happened while decoding a buffer, returned as data so that
/// `kiln_diag` can render it (this crate has no diagnostics dependency).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadReport {
    /// Invalid UTF-8 sequences in file order. Each maximal invalid
    /// subsequence (as defined by [`std::str::Utf8Chunk`]) became one U+FFFD.
    pub replacements: Vec<Replacement>,
}

impl LoadReport {
    /// True if the buffer was valid UTF-8.
    pub fn is_clean(&self) -> bool {
        self.replacements.is_empty()
    }
}

/// A physical line/column position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineCol {
    /// The buffer.
    pub file: FileId,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column, counted in bytes.
    pub col: u32,
}

/// A presumed position: the physical position adjusted by `#line`
/// directives ([cpp.line]); what `__LINE__`, `__FILE__` and diagnostics
/// report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresumedLoc<'a> {
    /// The physical buffer.
    pub file: FileId,
    /// Presumed file name: the last `#line` name in effect, else the
    /// buffer's name.
    pub file_name: &'a str,
    /// Presumed 1-based line number.
    pub line: u32,
    /// 1-based byte column (unaffected by `#line`).
    pub col: u32,
}

/// What an expansion entry represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpansionKind {
    /// Tokens produced by expanding a macro: its replacement list, or the
    /// result of a builtin macro, `#` or `##`. `name` is the span of the
    /// macro-name token at the invocation (used for "expanded from macro"
    /// notes; its text is `SourceManager::span_text(name)`).
    Macro {
        /// Span of the macro-name token of the invocation.
        name: Span,
    },
    /// Tokens of a macro argument substituted for a parameter. The
    /// spelling is where the argument was written; the expansion span is
    /// the parameter token in the macro's replacement list (itself a macro
    /// location of the enclosing [`ExpansionKind::Macro`] entry).
    MacroArg,
}

/// One expansion entry (modeled on clang's expansion `SLocEntry`).
///
/// Location `base + n` (for `n <= len`) is spelled at `spelling + n` and
/// was produced by the invocation covering `expansion`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpansionInfo {
    /// First location of the entry's range `[base, base + len]`.
    pub base: Loc,
    /// Length in bytes of the spelled range.
    pub len: u32,
    /// Where `base` is spelled; may itself be a macro location.
    pub spelling: Loc,
    /// The range that was replaced: the macro invocation for
    /// [`ExpansionKind::Macro`], the parameter token for
    /// [`ExpansionKind::MacroArg`].
    pub expansion: Span,
    /// What produced the entry.
    pub kind: ExpansionKind,
}

#[derive(Debug, Clone, Copy)]
struct LineDirective {
    /// Offset of the first byte of the line the directive renumbers.
    offset: u32,
    /// Physical 1-based line containing `offset`.
    phys_line: u32,
    /// Presumed line number of that line.
    new_line: u32,
    /// Index into `presumed_names`, or `None` for the buffer's own name.
    name: Option<u32>,
}

#[derive(Debug)]
struct FileEntry {
    /// Raw value of the buffer's first location.
    start: u32,
    name: Box<str>,
    text: Box<str>,
    report: LoadReport,
    /// Offsets of the first byte of every line; built on first use.
    lines: OnceLock<Box<[u32]>>,
    /// Sorted by `offset`.
    line_directives: RwLock<Vec<LineDirective>>,
}

impl FileEntry {
    fn len(&self) -> u32 {
        self.text.len() as u32
    }

    fn line_starts(&self) -> &[u32] {
        self.lines.get_or_init(|| compute_line_starts(&self.text))
    }
}

#[derive(Debug)]
struct Alloc {
    next_file: u64,
    next_macro: u64,
}

/// Owns every buffer of a translation unit and maps [`Loc`]s back to
/// buffers, lines, columns, spellings and expansions.
///
/// The manager is append-only and every method takes `&self`: the lexer,
/// the preprocessor (which keeps loading files and creating expansion
/// entries while lexers are live) and the diagnostics engine all share one
/// `&SourceManager`. Text returned by [`text`](Self::text) never moves.
/// The type is `Send + Sync`; appends are serialized by an internal mutex
/// and reads are lock-free (except presumed-location lookup, which takes a
/// read lock).
///
/// Methods taking a [`FileId`] panic if the id was not created by this
/// manager; methods taking a [`Loc`] treat unknown locations as invalid.
pub struct SourceManager {
    files: AppendVec<FileEntry>,
    expansions: AppendVec<ExpansionInfo>,
    presumed_names: AppendVec<Box<str>>,
    by_path: Mutex<HashMap<PathBuf, FileId>>,
    alloc: Mutex<Alloc>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Default for SourceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for SourceManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceManager")
            .field("files", &self.files.len())
            .field("expansions", &self.expansions.len())
            .finish_non_exhaustive()
    }
}

impl SourceManager {
    /// Creates an empty manager.
    pub fn new() -> Self {
        Self {
            files: AppendVec::new(),
            expansions: AppendVec::new(),
            presumed_names: AppendVec::new(),
            by_path: Mutex::new(HashMap::new()),
            alloc: Mutex::new(Alloc { next_file: 1, next_macro: u64::from(MACRO_BIT) }),
        }
    }

    // ----- buffers -------------------------------------------------------

    /// Loads the file at `path`.
    ///
    /// Identity is by canonical path: loading the same file again (through
    /// any path that canonicalizes to it) returns the same id without
    /// touching the disk. The buffer's [`file_name`](Self::file_name) is
    /// `path` as given on first load. Invalid UTF-8 is replaced with U+FFFD
    /// and recorded in the [`load_report`](Self::load_report).
    pub fn load_file(&self, path: &Path) -> Result<FileId, LoadError> {
        let canonical = fs::canonicalize(path)?;
        let mut by_path = lock(&self.by_path);
        if let Some(&id) = by_path.get(&canonical) {
            return Ok(id);
        }
        let bytes = fs::read(&canonical)?;
        if bytes.len() as u64 >= FILE_SPACE_END {
            return Err(LoadError::LocSpaceExhausted);
        }
        let (text, report) = decode_utf8(bytes);
        let id = self.push_file(path.to_string_lossy().into(), text, report)?;
        by_path.insert(canonical, id);
        Ok(id)
    }

    /// Adds an in-memory buffer named `name` (e.g. `<built-in>`,
    /// `<command line>`, `<scratch space>`). Never deduplicated.
    pub fn add_virtual(&self, name: &str, contents: String) -> Result<FileId, LocSpaceExhausted> {
        self.push_file(name.into(), contents, LoadReport::default())
    }

    fn push_file(
        &self,
        name: Box<str>,
        text: String,
        report: LoadReport,
    ) -> Result<FileId, LocSpaceExhausted> {
        let mut alloc = lock(&self.alloc);
        let start = alloc.next_file;
        let end = start + text.len() as u64 + 1;
        if end > FILE_SPACE_END {
            return Err(LocSpaceExhausted);
        }
        let idx = self.files.push(FileEntry {
            start: start as u32,
            name,
            text: text.into_boxed_str(),
            report,
            lines: OnceLock::new(),
            line_directives: RwLock::new(Vec::new()),
        });
        alloc.next_file = end;
        Ok(FileId(idx))
    }

    fn file(&self, id: FileId) -> &FileEntry {
        self.files
            .get(id.0)
            .unwrap_or_else(|| panic!("{id:?} was not created by this SourceManager"))
    }

    /// Number of buffers created so far.
    pub fn file_count(&self) -> usize {
        self.files.len() as usize
    }

    /// The buffer's text: always valid UTF-8.
    pub fn text(&self, file: FileId) -> &str {
        &self.file(file).text
    }

    /// The buffer's name: the path as first loaded, or the virtual name.
    pub fn file_name(&self, file: FileId) -> &str {
        &self.file(file).name
    }

    /// Decoding report for the buffer (empty for virtual buffers).
    pub fn load_report(&self, file: FileId) -> &LoadReport {
        &self.file(file).report
    }

    /// The location of byte `offset` of `file`. `offset == text.len()` is
    /// the end-of-file location.
    ///
    /// # Panics
    /// If `offset > text(file).len()`.
    pub fn loc(&self, file: FileId, offset: u32) -> Loc {
        let f = self.file(file);
        assert!(offset <= f.len(), "offset {offset} past end of {file:?} (len {})", f.len());
        Loc(f.start + offset)
    }

    /// The span of the whole buffer, `[offset 0, end-of-file)`.
    pub fn file_span(&self, file: FileId) -> Span {
        let f = self.file(file);
        Span::new(Loc(f.start), Loc(f.start + f.len()))
    }

    /// Splits a *file* location into its buffer and byte offset. Returns
    /// `None` for invalid, unknown and macro locations (map those through
    /// [`spelling_loc`](Self::spelling_loc) or
    /// [`expansion_loc`](Self::expansion_loc) first).
    pub fn offset_of(&self, loc: Loc) -> Option<(FileId, u32)> {
        self.find_file(loc).map(|(id, _, off)| (id, off))
    }

    fn find_file(&self, loc: Loc) -> Option<(FileId, &FileEntry, u32)> {
        if !loc.is_file() {
            return None;
        }
        let i = self.files.partition_point(|f| f.start <= loc.0).checked_sub(1)?;
        let f = self.files.get(i)?;
        let off = loc.0 - f.start;
        (off <= f.len()).then_some((FileId(i), f, off))
    }

    // ----- lines and columns ---------------------------------------------

    /// Physical line and column of `loc`. Macro locations are first mapped
    /// to their [`expansion_loc`](Self::expansion_loc) (where the user sees
    /// them); call [`spelling_loc`](Self::spelling_loc) first for the
    /// spelling position. `None` for invalid or unknown locations.
    ///
    /// Line terminators are `\n`, `\r\n` and a lone `\r`. The terminator
    /// belongs to the line it ends. The end-of-file location of a buffer
    /// ending in a terminator is column 1 of the (empty) line after it.
    pub fn line_col(&self, loc: Loc) -> Option<LineCol> {
        let (file, f, off) = self.find_file(self.expansion_loc(loc))?;
        let (line, col) = line_col_in(f.line_starts(), off);
        Some(LineCol { file, line, col })
    }

    /// Number of lines in the buffer, counting the empty line after a
    /// trailing terminator (so an empty buffer has 1 line).
    pub fn line_count(&self, file: FileId) -> u32 {
        self.file(file).line_starts().len() as u32
    }

    /// Text of 1-based `line` without its terminator, or `None` if the
    /// line does not exist.
    pub fn line_text(&self, file: FileId, line: u32) -> Option<&str> {
        let f = self.file(file);
        let starts = f.line_starts();
        let i = (line as usize).checked_sub(1)?;
        let start = *starts.get(i)? as usize;
        let end = starts.get(i + 1).map_or(f.text.len(), |&e| e as usize);
        let s = &f.text[start..end];
        let s = s.strip_suffix('\n').unwrap_or(s);
        Some(s.strip_suffix('\r').unwrap_or(s))
    }

    /// The source text spelled by `span`: both ends are mapped through
    /// [`spelling_loc`](Self::spelling_loc). `None` if they do not land in
    /// the same buffer in order, or not on character boundaries.
    pub fn span_text(&self, span: Span) -> Option<&str> {
        let (lo_file, f, lo) = self.find_file(self.spelling_loc(span.lo))?;
        let (hi_file, _, hi) = self.find_file(self.spelling_loc(span.hi))?;
        if lo_file != hi_file || lo > hi {
            return None;
        }
        f.text.get(lo as usize..hi as usize)
    }

    // ----- macro expansions ----------------------------------------------

    /// Creates an expansion entry covering `len` bytes spelled starting at
    /// `spelling`, and returns its base location: location
    /// `base.with_offset(n)` (for `n <= len`) is spelled at
    /// `spelling.with_offset(n)` and was produced by `expansion`.
    ///
    /// For a macro body, pass the location of the start of the replacement
    /// list as `spelling` and its length as `len`: one entry then serves
    /// every token of that expansion. For a run of argument tokens, pass
    /// the argument run's start and length with [`ExpansionKind::MacroArg`].
    ///
    /// # Panics
    /// If `spelling`, `expansion` or the macro-name span are not locations
    /// already allocated by this manager. (This guarantees that every
    /// walk through expansion entries terminates.) In debug builds, also if
    /// `[spelling, spelling + len]` crosses an entry boundary.
    pub fn add_expansion(
        &self,
        spelling: Loc,
        len: u32,
        expansion: Span,
        kind: ExpansionKind,
    ) -> Result<Loc, LocSpaceExhausted> {
        assert!(self.is_allocated(spelling), "add_expansion: bad spelling {spelling:?}");
        assert!(
            self.is_allocated(expansion.lo) && self.is_allocated(expansion.hi),
            "add_expansion: bad expansion span {expansion:?}"
        );
        if let ExpansionKind::Macro { name } = kind {
            assert!(
                self.is_allocated(name.lo) && self.is_allocated(name.hi),
                "add_expansion: bad macro-name span {name:?}"
            );
        }
        debug_assert!(
            spelling.0.checked_add(len).is_some_and(|end| self.same_entry(spelling, Loc(end))),
            "add_expansion: spelled range {spelling:?}+{len} crosses an entry boundary"
        );
        let mut alloc = lock(&self.alloc);
        let base = alloc.next_macro;
        let end = base + u64::from(len) + 1;
        if end > MACRO_SPACE_END {
            return Err(LocSpaceExhausted);
        }
        let base = Loc(base as u32);
        self.expansions.push(ExpansionInfo { base, len, spelling, expansion, kind });
        alloc.next_macro = end;
        Ok(base)
    }

    fn find_expansion(&self, loc: Loc) -> Option<&ExpansionInfo> {
        if !loc.is_macro() {
            return None;
        }
        let i = self.expansions.partition_point(|e| e.base <= loc).checked_sub(1)?;
        let e = self.expansions.get(i)?;
        (loc.0 - e.base.0 <= e.len).then_some(e)
    }

    fn is_allocated(&self, loc: Loc) -> bool {
        if loc.is_macro() {
            self.find_expansion(loc).is_some()
        } else {
            self.find_file(loc).is_some()
        }
    }

    fn same_entry(&self, a: Loc, b: Loc) -> bool {
        if a.is_macro() {
            matches!((self.find_expansion(a), self.find_expansion(b)), (Some(x), Some(y)) if x.base == y.base)
        } else {
            matches!((self.find_file(a), self.find_file(b)), (Some(x), Some(y)) if x.0 == y.0)
        }
    }

    /// True if `loc` is a macro-expansion location. O(1).
    pub fn is_macro_loc(&self, loc: Loc) -> bool {
        loc.is_macro()
    }

    /// The expansion entry containing `loc`, or `None` for file, invalid
    /// and unknown locations. Walk outward with
    /// [`immediate_macro_caller_loc`](Self::immediate_macro_caller_loc)
    /// to produce "expanded from macro" notes.
    pub fn expansion_info(&self, loc: Loc) -> Option<ExpansionInfo> {
        self.find_expansion(loc).copied()
    }

    /// One step of spelling resolution: where `loc` is spelled according
    /// to its own entry (may still be a macro location). Identity for file
    /// locations; [`Loc::INVALID`] for unknown macro locations.
    pub fn immediate_spelling_loc(&self, loc: Loc) -> Loc {
        match self.find_expansion(loc) {
            Some(e) => e.spelling.with_offset(loc.0 - e.base.0),
            None if loc.is_macro() => Loc::INVALID,
            None => loc,
        }
    }

    /// Where the characters of `loc` were written: follows spelling links
    /// until a file location. Identity for file locations.
    pub fn spelling_loc(&self, mut loc: Loc) -> Loc {
        while loc.is_macro() {
            loc = self.immediate_spelling_loc(loc);
        }
        loc
    }

    /// The file location of the outermost macro invocation that produced
    /// `loc` (start of its expansion range). Identity for file locations.
    pub fn expansion_loc(&self, mut loc: Loc) -> Loc {
        while loc.is_macro() {
            loc = self.find_expansion(loc).map_or(Loc::INVALID, |e| e.expansion.lo);
        }
        loc
    }

    /// The file range of the outermost macro invocation that produced
    /// `loc`: the start walks expansion starts, the end walks expansion
    /// ends. `Span::empty(loc)` for file locations; [`Span::INVALID`] for
    /// unknown macro locations.
    pub fn expansion_span(&self, loc: Loc) -> Span {
        let Some(e) = self.find_expansion(loc) else {
            return if loc.is_macro() { Span::INVALID } else { Span::empty(loc) };
        };
        let lo = self.expansion_loc(e.expansion.lo);
        let mut hi = e.expansion.hi;
        while hi.is_macro() {
            hi = self.find_expansion(hi).map_or(Loc::INVALID, |e| e.expansion.hi);
        }
        Span::new(lo, hi)
    }

    /// One step outward toward the code that caused `loc` (clang's
    /// `getImmediateMacroCallerLoc`): for a macro-argument location, where
    /// the argument was written; for other macro locations, the start of
    /// the invocation. Identity for file locations.
    pub fn immediate_macro_caller_loc(&self, loc: Loc) -> Loc {
        match self.find_expansion(loc) {
            Some(e) if e.kind == ExpansionKind::MacroArg => self.immediate_spelling_loc(loc),
            Some(e) => e.expansion.lo,
            None if loc.is_macro() => Loc::INVALID,
            None => loc,
        }
    }

    // ----- presumed locations ([cpp.line]) -------------------------------

    /// Records a `#line` directive ([cpp.line]) in `file`: the line
    /// starting at byte `at_offset` (the first byte *after* the directive's
    /// newline) has presumed line number `new_line`, and following lines
    /// count up from it. `new_file_name` replaces the presumed file name;
    /// `None` keeps the name in effect at `at_offset`.
    ///
    /// Directives may be added in any order. Adding one at an offset that
    /// already has a directive replaces it (a file without include guards
    /// entered twice re-adds its directives).
    ///
    /// # Panics
    /// If `at_offset > text(file).len()`.
    pub fn add_line_directive(
        &self,
        file: FileId,
        at_offset: u32,
        new_line: u32,
        new_file_name: Option<&str>,
    ) {
        let f = self.file(file);
        assert!(at_offset <= f.len(), "line directive offset {at_offset} past end of {file:?}");
        let (phys_line, _) = line_col_in(f.line_starts(), at_offset);
        let name = new_file_name.map(|n| {
            let _alloc = lock(&self.alloc);
            self.presumed_names.push(n.into())
        });
        let mut dirs = f.line_directives.write().unwrap_or_else(PoisonError::into_inner);
        let pos = dirs.partition_point(|d| d.offset < at_offset);
        let name = name.or_else(|| pos.checked_sub(1).and_then(|p| dirs[p].name));
        let d = LineDirective { offset: at_offset, phys_line, new_line, name };
        if dirs.get(pos).is_some_and(|old| old.offset == at_offset) {
            dirs[pos] = d;
        } else {
            dirs.insert(pos, d);
        }
    }

    /// The presumed position of `loc`: like [`line_col`](Self::line_col)
    /// (macro locations use their expansion location), with the line
    /// number and file name adjusted by the last `#line` directive at or
    /// before it. A line number that would exceed `u32::MAX` saturates.
    pub fn presumed_line_col(&self, loc: Loc) -> Option<PresumedLoc<'_>> {
        let (file, f, off) = self.find_file(self.expansion_loc(loc))?;
        let (line, col) = line_col_in(f.line_starts(), off);
        let directive = {
            let dirs = f.line_directives.read().unwrap_or_else(PoisonError::into_inner);
            dirs.partition_point(|d| d.offset <= off).checked_sub(1).map(|i| dirs[i])
        };
        let Some(d) = directive else {
            return Some(PresumedLoc { file, file_name: &f.name, line, col });
        };
        let file_name = d.name.and_then(|n| self.presumed_names.get(n)).map_or(&*f.name, |n| &**n);
        Some(PresumedLoc {
            file,
            file_name,
            line: d.new_line.saturating_add(line - d.phys_line),
            col,
        })
    }
}

/// 1-based (line, column) of `off` given line start offsets.
fn line_col_in(starts: &[u32], off: u32) -> (u32, u32) {
    // starts[0] == 0 <= off, so the partition point is at least 1.
    let i = starts.partition_point(|&s| s <= off) - 1;
    (i as u32 + 1, off - starts[i] + 1)
}

/// Offsets of the first byte of each line. Terminators: `\n`, `\r\n`,
/// lone `\r`.
fn compute_line_starts(text: &str) -> Box<[u32]> {
    let b = text.as_bytes();
    let mut starts = vec![0u32];
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\n' => starts.push(i as u32 + 1),
            b'\r' => {
                if b.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                starts.push(i as u32 + 1);
            }
            _ => {}
        }
        i += 1;
    }
    starts.into_boxed_slice()
}

/// Decodes `bytes` as UTF-8, replacing each maximal invalid subsequence
/// with U+FFFD.
fn decode_utf8(bytes: Vec<u8>) -> (String, LoadReport) {
    let bytes = match String::from_utf8(bytes) {
        Ok(s) => return (s, LoadReport::default()),
        Err(e) => e.into_bytes(),
    };
    let mut text = String::with_capacity(bytes.len() + 16);
    let mut replacements = Vec::new();
    for chunk in bytes.utf8_chunks() {
        text.push_str(chunk.valid());
        if !chunk.invalid().is_empty() {
            replacements
                .push(Replacement { offset: text.len() as u32, bytes: chunk.invalid().into() });
            text.push(char::REPLACEMENT_CHARACTER);
        }
    }
    (text, LoadReport { replacements })
}

const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SourceManager>();
};
