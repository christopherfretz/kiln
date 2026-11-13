//! The snapshot encoding of [`SourceManager`] (spec/source.md
//! "Snapshots", spec/snap.md "Hand-written roots").
//!
//! The manager is a root: it holds locks, a cache keyed by inode numbers
//! and closures, so its encoding is written by hand. Every field is
//! classified below, and `put` and `get` name every field (an exhaustive
//! destructure and a full struct literal), so a new field is a compile
//! error here until it is classified.
//!
//! `SourceManager`:
//!
//! | field | class | how |
//! |---|---|---|
//! | `files` | state | [`FileEntry`] below |
//! | `ranges` | state | as data (buffer ranges and inclusions, in order) |
//! | `expansions` | state | as data (24-byte entries, bulk) |
//! | `macro_names` | state | as data |
//! | `presumed_names` | state | as data |
//! | `kinds` | state | as data |
//! | `by_path` | rebuilt | re-`stat` of every on-disk buffer's path, in id order (inode numbers are per machine) |
//! | `alloc` | state | as data (the allocation frontiers) |
//! | `last_file`, `prev_file`, `last_expansion` | transient | lookup hints, restored as 0 |
//! | `guarded` | state | as data |
//! | `macro_prefix_map` | configuration | not stored: the consumer sets it from its own command line (validated by the snapshot's configuration fingerprint) |
//! | `source_path_style` | configuration | not stored, as `macro_prefix_map` |
//! | `gcc_names` | state | as data |
//!
//! `FileEntry`:
//!
//! | field | class | how |
//! |---|---|---|
//! | `start`, `name`, `len` | state | as data |
//! | `origin` | state | a buffer read from disk is restored as a lazy buffer of its path, checked against the content hash of its text on first use (`Origin::Lazy`, as [`SourceManager::add_lazy_file`]); a virtual buffer as itself |
//! | `origin` (`Lazy::copy`) | rebuilt (lost) | a closure: a lazy buffer whose copy was already used is carried with its text inline; an unread one loses its copy |
//! | `body` | state / rebuilt | a virtual buffer's text inline; a disk buffer's text is re-read from the file on first use, its load report recomputed then |
//! | `lines` | rebuilt | the line-start cache, recomputed on first use |
//! | `line_directives` | state | as data |

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicU32;
use std::sync::{Mutex, OnceLock, RwLock};

use kiln_snap::{Desc, Reader, Schema, Snap, SnapError, Writer};

use super::{
    Alloc, Body, Entry, FILE_SPACE_END, FileEntry, FileKind, FileRange, LazyOutcome, LineDirective,
    MACRO_SPACE_END, Origin, SourceManager, file_key, lock,
};
use crate::hash::content_hash;
use crate::loc::{FileId, Loc, MACRO_BIT, Span};

/// How a buffer's text is carried ([`FileEntry`]'s encoding tag).
const VIRTUAL: u8 = 0;
/// On disk: path and content hash; the text is read on first use.
const ON_DISK: u8 = 1;
/// A lazy buffer whose recorded copy was used: path, hash and the text.
const RECORDED: u8 = 2;

/// Buffers (spec/source.md "Snapshots"); see the module docs for the
/// field classification.
impl Snap for FileEntry {
    fn put(&self, w: &mut Writer) {
        let FileEntry { start, name, len, origin, body, lines: _, line_directives } = self;
        start.put(w);
        name.put(w);
        len.put(w);
        let text = || body.get().map_or("", |b| &b.text);
        match origin {
            Origin::Virtual => {
                w.u8(VIRTUAL);
                w.str(text());
            }
            Origin::Disk => {
                w.u8(ON_DISK);
                PathBuf::from(&**name).put(w);
                content_hash(text().as_bytes()).put(w);
            }
            Origin::Lazy { path, hash, copy: _ } => {
                let copied = body.get().filter(|b| b.outcome == LazyOutcome::Copy);
                w.u8(if copied.is_some() { RECORDED } else { ON_DISK });
                path.put(w);
                hash.put(w);
                if let Some(b) = copied {
                    w.str(&b.text);
                }
            }
        }
        line_directives.put(w);
    }

    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        let start = u32::get(r)?;
        let name = Box::<str>::get(r)?;
        let len = u32::get(r)?;
        if u64::from(start) + u64::from(len) >= FILE_SPACE_END {
            return Err(r.invalid::<Self>("buffer outside the file location space"));
        }
        let text = |r: &mut Reader<'_>, outcome| {
            let t = r.str()?;
            if t.len() != len as usize {
                return Err(r.invalid::<Self>("buffer text of the wrong length"));
            }
            Ok(Body { text: t.into(), report: super::LoadReport::default(), outcome })
        };
        let (origin, body) = match r.u8()? {
            VIRTUAL => (Origin::Virtual, Some(text(r, LazyOutcome::Disk)?)),
            tag @ (ON_DISK | RECORDED) => {
                let path = PathBuf::get(r)?;
                let hash = u64::get(r)?;
                let body = if tag == RECORDED { Some(text(r, LazyOutcome::Copy)?) } else { None };
                (Origin::Lazy { path, hash, copy: None }, body)
            }
            t => return Err(r.bad_tag::<Self>(u64::from(t), 1)),
        };
        let line_directives = RwLock::<Vec<LineDirective>>::get(r)?;
        {
            let dirs = line_directives.read().unwrap_or_else(std::sync::PoisonError::into_inner);
            if dirs.windows(2).any(|p| p[0].offset >= p[1].offset)
                || dirs.iter().any(|d| d.offset > len || d.phys_line == 0)
            {
                return Err(r.invalid::<Self>("line directives out of order or range"));
            }
        }
        let cell = OnceLock::new();
        if let Some(b) = body {
            let _ = cell.set(b);
        }
        Ok(FileEntry {
            start,
            name,
            len,
            origin,
            body: cell,
            lines: OnceLock::new(),
            line_directives,
        })
    }

    fn schema(s: &mut Schema) {
        let d = Desc::new("root")
            .note("FileEntry: tag 0 text | tag 1 path hash | tag 2 path hash text")
            .field::<u32>("start")
            .field::<Box<str>>("name")
            .field::<u32>("len")
            .field::<PathBuf>("path")
            .field::<u64>("hash")
            .field::<String>("text")
            .skip::<OnceLock<Box<[u32]>>>("lines")
            .field::<RwLock<Vec<LineDirective>>>("line_directives");
        if s.register::<Self>(d) {
            u32::schema(s);
            Box::<str>::schema(s);
            PathBuf::schema(s);
            u64::schema(s);
            String::schema(s);
            RwLock::<Vec<LineDirective>>::schema(s);
        }
    }
}

/// Adds `field` to a decoding error's path.
fn at<T>(field: &'static str, r: Result<T, SnapError>) -> Result<T, SnapError> {
    r.map_err(|e| e.within(field))
}

/// The source manager as a snapshot root (spec/source.md "Snapshots");
/// see the module docs for the field classification. Decoding builds a
/// fresh manager in which every [`FileId`] and [`Loc`] is unchanged, then
/// re-identifies the on-disk buffers (`by_path`) by `stat`ing their paths.
impl Snap for SourceManager {
    fn put(&self, w: &mut Writer) {
        let SourceManager {
            files,
            ranges,
            expansions,
            macro_names,
            presumed_names,
            kinds,
            by_path: _,
            alloc,
            last_file: _,
            prev_file: _,
            last_expansion: _,
            guarded,
            macro_prefix_map: _,
            source_path_style: _,
            gcc_names,
        } = self;
        presumed_names.put(w);
        files.put(w);
        ranges.put(w);
        expansions.put(w);
        macro_names.put(w);
        kinds.put(w);
        guarded.put(w);
        gcc_names.put(w);
        alloc.put(w);
    }

    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        let presumed_names = at("SourceManager.presumed_names", r.get())?;
        let files: super::AppendVec<FileEntry> = at("SourceManager.files", r.get())?;
        let ranges: super::AppendVec<FileRange> = at("SourceManager.ranges", r.get())?;
        let expansions: super::AppendVec<Entry> = at("SourceManager.expansions", r.get())?;
        let macro_names = at("SourceManager.macro_names", r.get())?;
        let kinds = at("SourceManager.kinds", r.get())?;
        let guarded: Mutex<HashMap<FileId, Box<str>>> = at("SourceManager.guarded", r.get())?;
        let gcc_names: Mutex<HashMap<FileId, Box<str>>> = at("SourceManager.gcc_names", r.get())?;
        let alloc: Mutex<Alloc> = at("SourceManager.alloc", r.get())?;
        let sm = SourceManager {
            files,
            ranges,
            expansions,
            macro_names,
            presumed_names,
            kinds,
            by_path: Mutex::new(HashMap::new()),
            alloc,
            last_file: AtomicU32::new(0),
            prev_file: AtomicU32::new(0),
            last_expansion: AtomicU32::new(0),
            guarded,
            macro_prefix_map: OnceLock::new(),
            source_path_style: OnceLock::new(),
            gcc_names,
        };
        if let Err(why) = sm.check_restored() {
            return Err(r.invalid::<Self>(why));
        }
        sm.rebuild_by_path();
        Ok(sm)
    }

    fn schema(s: &mut Schema) {
        let d = Desc::new("root")
            .field::<super::AppendVec<Box<str>>>("presumed_names")
            .field::<super::AppendVec<FileEntry>>("files")
            .field::<super::AppendVec<FileRange>>("ranges")
            .field::<super::AppendVec<Entry>>("expansions")
            .field::<super::AppendVec<(Loc, Span)>>("macro_names")
            .field::<RwLock<Vec<(u32, FileKind)>>>("kinds")
            .field::<Mutex<HashMap<FileId, Box<str>>>>("guarded")
            .field::<Mutex<HashMap<FileId, Box<str>>>>("gcc_names")
            .field::<Mutex<Alloc>>("alloc");
        if s.register::<Self>(d) {
            super::AppendVec::<Box<str>>::schema(s);
            super::AppendVec::<FileEntry>::schema(s);
            super::AppendVec::<FileRange>::schema(s);
            super::AppendVec::<Entry>::schema(s);
            super::AppendVec::<(Loc, Span)>::schema(s);
            RwLock::<Vec<(u32, FileKind)>>::schema(s);
            Mutex::<HashMap<FileId, Box<str>>>::schema(s);
            Mutex::<Alloc>::schema(s);
        }
    }
}

impl SourceManager {
    /// The cross-table checks of a decoded manager: the invariants the
    /// creation paths establish and the queries rely on without a check
    /// (ranges and entries tile their halves of the location space in
    /// order, every link of an entry points below it, every id and index is
    /// in range). The snapshot comes from the same build, so this guards
    /// against corruption, not a different layout.
    fn check_restored(&self) -> Result<(), &'static str> {
        let nfiles = self.files.len();
        let names = self.presumed_names.len();
        // Buffer ranges: each buffer's own range in id order, inclusion
        // ranges of earlier buffers between them, contiguous from 1.
        let (mut next_file, mut own) = (1u64, 0u32);
        for i in 0..self.ranges.len() {
            let Some(r) = self.ranges.get(i) else { return Err("missing range") };
            let Some(f) = self.files.get(r.file.0) else { return Err("range of an unknown file") };
            let is_own = r.file.0 == own;
            if r.file.0 > own
                || r.len != f.len
                || u64::from(r.start) != next_file
                || (is_own && (f.start != r.start || r.included_from.is_valid()))
            {
                return Err("buffer ranges do not tile the file location space");
            }
            own += u32::from(is_own);
            next_file += u64::from(r.len) + 1;
        }
        if own != nfiles {
            return Err("a buffer without its own range");
        }
        // Expansion entries: contiguous from `MACRO_BIT`; every location an
        // entry holds was allocated before it; one macro name per macro
        // entry, in order.
        let mut next_macro = u64::from(MACRO_BIT);
        let mut name_i = 0;
        let allocated = |l: Loc, below: u64| {
            l.is_valid() && u64::from(l.0) < if l.is_macro() { below } else { next_file }
        };
        for i in 0..self.expansions.len() {
            let Some(e) = self.expansions.get(i) else { return Err("missing expansion entry") };
            let base = u64::from(e.base.0);
            let len = u64::from(e.len());
            let spelled = |l: Loc| {
                let end = u64::from(l.0) + len;
                allocated(l, base) && if l.is_macro() { end < base } else { end < next_file }
            };
            if base != next_macro
                || !spelled(e.spelling)
                || e.file_spelling.is_macro()
                || !spelled(e.file_spelling)
                || !allocated(e.expansion.lo, base)
                || !allocated(e.expansion.hi, base)
            {
                return Err("expansion entries out of order or pointing forward");
            }
            if e.is_macro_kind() {
                let Some(&(b, name)) = self.macro_names.get(name_i) else {
                    return Err("macro entry without a name");
                };
                if b != e.base || !allocated(name.lo, base) || !allocated(name.hi, base) {
                    return Err("macro name of the wrong entry");
                }
                name_i += 1;
            }
            next_macro = base + len + 1;
        }
        if name_i != self.macro_names.len() || next_macro > MACRO_SPACE_END {
            return Err("macro names without entries");
        }
        let alloc = lock(&self.alloc);
        if alloc.next_file != next_file || alloc.next_macro != next_macro {
            return Err("allocation frontier inconsistent with the tables");
        }
        drop(alloc);
        let kinds = self.kinds.read().unwrap_or_else(std::sync::PoisonError::into_inner);
        if kinds.windows(2).any(|p| p[0].0 > p[1].0)
            || kinds.iter().any(|&(l, _)| u64::from(l) >= next_file)
        {
            return Err("file kinds out of order or range");
        }
        drop(kinds);
        for i in 0..nfiles {
            let Some(f) = self.files.get(i) else { return Err("missing buffer") };
            let dirs = f.line_directives.read().unwrap_or_else(std::sync::PoisonError::into_inner);
            if dirs.iter().any(|d| d.name.is_some_and(|n| n >= names)) {
                return Err("line directive names an unknown presumed name");
            }
        }
        for map in [&self.guarded, &self.gcc_names] {
            if lock(map).keys().any(|f| f.0 >= nfiles) {
                return Err("table keyed by an unknown file");
            }
        }
        Ok(())
    }

    /// Re-identifies every on-disk buffer by its path, in id order (a
    /// later buffer of the same file wins, as when it was loaded). A file
    /// that is gone is left out: loading it again fails as it would.
    fn rebuild_by_path(&self) {
        let mut by_path = lock(&self.by_path);
        for i in 0..self.files.len() {
            let Some(f) = self.files.get(i) else { continue };
            let Origin::Lazy { path, .. } = &f.origin else { continue };
            let Ok(meta) = fs::metadata(path) else { continue };
            if let Ok(key) = file_key(path, &meta) {
                by_path.insert(key, FileId(i));
            }
        }
    }
}
