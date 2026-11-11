//! S7: the image's locations (spec/modules.md §11.11 "File table" and
//! "Location records"). Every carried location is one `u32` in the
//! image's own location space: a sequence of location records, each the
//! range of one buffer inclusion or one macro expansion entry of the
//! module's source manager, laid end to end from 0 (record `k` covers
//! `[base_k, base_k + len_k]`, and `base_{k+1} = base_k + len_k + 1`).
//! Record 0 is the module's main file, so a main-file location is its
//! offset. A record names only earlier records, so the importer
//! recreates them in order with `add_inclusion` and `add_expansion`.

use kiln_source::fx::FxHashMap;
use kiln_source::hash::content_hash;
use kiln_source::{ExpansionKind, FileId, FileKind, Loc, SourceManager, Span};

use super::{NONE, W};

/// One buffer the image's locations are in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct FileRec {
    /// The buffer's name, as diagnostics print it.
    pub(super) name: String,
    /// The absolute path the importer reads it from (empty for an
    /// in-memory buffer).
    pub(super) path: String,
    /// The decoded text's length.
    pub(super) len: u32,
    /// [`content_hash`] of the text.
    pub(super) hash: u64,
    /// The buffer was entered as a system header.
    pub(super) system: bool,
    /// An in-memory buffer's text (`<scratch space>`, a module file's
    /// copy of a changed file): carried inline.
    pub(super) text: Option<String>,
}

/// One location record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SlocRec {
    /// One inclusion of file `file`, entered by the `#include` at image
    /// location `from` ([`NONE`]: the buffer's own range). `marks` are
    /// its file-kind marks `(offset, kind)`.
    File { file: u32, from: u32, marks: Vec<(u32, u8)> },
    /// A macro expansion: `len + 1` locations spelled from `spelling`,
    /// produced by the invocation `exp`, of the macro named at `name`.
    Macro { spelling: u32, len: u32, exp: [u32; 2], name: [u32; 2] },
    /// A macro argument substituted for the parameter at `exp`.
    Arg { spelling: u32, len: u32, exp: [u32; 2] },
}

/// The record lengths, given the file table.
pub(super) fn sloc_len(files: &[FileRec], s: &SlocRec) -> Option<u32> {
    match s {
        SlocRec::File { file, .. } => files.get(*file as usize).map(|f| f.len),
        SlocRec::Macro { len, .. } | SlocRec::Arg { len, .. } => Some(*len),
    }
}

/// The writer's tables: the records made so far, by the start of the
/// module's range or entry they copy.
#[derive(Debug)]
pub(super) struct LocTable {
    files: Vec<FileRec>,
    file_ix: FxHashMap<FileId, u32>,
    slocs: Vec<SlocRec>,
    bases: Vec<u32>,
    by_start: FxHashMap<u32, u32>,
    next: u64,
}

/// The end of the image location space (the reserved words start at
/// `LEX_TU`).
const SPACE_END: u64 = (u32::MAX - 1) as u64;

const KIND_USER: u8 = 0;

fn kind_code(k: FileKind) -> u8 {
    match k {
        FileKind::User => KIND_USER,
        FileKind::System => 1,
        FileKind::ExternCSystem => 2,
    }
}

pub(super) fn kind_of(c: u8) -> Option<FileKind> {
    match c {
        0 => Some(FileKind::User),
        1 => Some(FileKind::System),
        2 => Some(FileKind::ExternCSystem),
        _ => None,
    }
}

impl LocTable {
    /// A table whose record 0 is `main`'s own range.
    pub(super) fn new(sm: &SourceManager, main: FileId) -> W<LocTable> {
        let mut t = LocTable {
            files: Vec::new(),
            file_ix: FxHashMap::default(),
            slocs: Vec::new(),
            bases: Vec::new(),
            by_start: FxHashMap::default(),
            next: 0,
        };
        t.carry(sm, sm.loc(main, 0))?;
        Ok(t)
    }

    /// The files and records, in image order.
    pub(super) fn finish(self) -> (Vec<FileRec>, Vec<SlocRec>) {
        (self.files, self.slocs)
    }

    /// The image location of `l`, carrying its range or entry and
    /// everything that names (the include chain, the spelling and
    /// expansion of a macro location) first.
    pub(super) fn loc(&mut self, sm: &SourceManager, l: Loc) -> W<u32> {
        if !l.is_valid() {
            return Ok(NONE);
        }
        let (start, _) = sm.entry_bounds(l).ok_or("a location the source manager does not know")?;
        let k = self.carry(sm, start)?;
        Ok(self.bases[k as usize] + (l.to_raw() - start.to_raw()))
    }

    /// The image location of `l`, whose entry is carried.
    fn carried(&self, sm: &SourceManager, l: Loc) -> u32 {
        if !l.is_valid() {
            return NONE;
        }
        let Some((start, _)) = sm.entry_bounds(l) else { return NONE };
        self.by_start
            .get(&start.to_raw())
            .map_or(NONE, |&k| self.bases[k as usize] + (l.to_raw() - start.to_raw()))
    }

    /// Carries the range or entry starting at `start`, its dependencies
    /// first: each names only locations allocated before it (the source
    /// manager allocates in order), so the walk ends.
    fn carry(&mut self, sm: &SourceManager, start: Loc) -> W<u32> {
        if let Some(&k) = self.by_start.get(&start.to_raw()) {
            return Ok(k);
        }
        let mut stack = vec![start];
        while let Some(&e) = stack.last() {
            if self.by_start.contains_key(&e.to_raw()) {
                stack.pop();
                continue;
            }
            let deps = deps_of(sm, e)?;
            let missing: Vec<Loc> = deps
                .into_iter()
                .filter(|d| d.is_valid())
                .filter_map(|d| sm.entry_bounds(d).map(|(s, _)| s))
                .filter(|s| !self.by_start.contains_key(&s.to_raw()))
                .collect();
            if missing.iter().any(|m| m.to_raw() >= e.to_raw()) {
                return Err("a location record naming a later location");
            }
            if missing.is_empty() {
                stack.pop();
                self.make(sm, e)?;
            } else {
                stack.extend(missing);
            }
        }
        self.by_start.get(&start.to_raw()).copied().ok_or("a location the image cannot carry")
    }

    /// Makes the record for the range or entry at `e`, whose
    /// dependencies are carried.
    fn make(&mut self, sm: &SourceManager, e: Loc) -> W<()> {
        let (rec, len) = if e.is_macro() {
            let x = sm.expansion_info(e).ok_or("an unknown macro location")?;
            let spelling = self.carried(sm, x.spelling);
            let exp = [self.carried(sm, x.expansion.lo), self.carried(sm, x.expansion.hi)];
            let rec = match x.kind {
                ExpansionKind::Macro { name } => SlocRec::Macro {
                    spelling,
                    len: x.len,
                    exp,
                    name: [self.carried(sm, name.lo), self.carried(sm, name.hi)],
                },
                ExpansionKind::MacroArg => SlocRec::Arg { spelling, len: x.len, exp },
            };
            (rec, x.len)
        } else {
            let (file, off) = sm.offset_of(e).ok_or("an unknown file location")?;
            if off != 0 {
                return Err("a file location outside its range");
            }
            let fi = self.file(sm, file)?;
            let marks: Vec<(u32, u8)> =
                sm.file_kind_marks(e).into_iter().map(|(o, k)| (o, kind_code(k))).collect();
            if marks.first().is_some_and(|&(o, k)| o == 0 && k != KIND_USER) {
                self.files[fi as usize].system = true;
            }
            let from = self.carried(sm, sm.include_loc(e));
            let len = self.files[fi as usize].len;
            (SlocRec::File { file: fi, from, marks }, len)
        };
        let base = self.next;
        let end = base + u64::from(len) + 1;
        if end > SPACE_END {
            return Err("more source than the image's location space holds");
        }
        let k = self.slocs.len() as u32;
        self.slocs.push(rec);
        self.bases.push(base as u32);
        self.by_start.insert(e.to_raw(), k);
        self.next = end;
        Ok(())
    }

    /// The file table index of `file`.
    fn file(&mut self, sm: &SourceManager, file: FileId) -> W<u32> {
        if let Some(&i) = self.file_ix.get(&file) {
            return Ok(i);
        }
        let name = sm.file_name(file);
        if name.starts_with('<') && name != "<scratch space>" || name.is_empty() {
            return Err("a location in a predefined buffer (<built-in>, <command line>)");
        }
        if sm.has_line_directives(file) {
            return Err("a file with #line directives or line markers");
        }
        if !sm.load_report(file).is_clean() {
            return Err("a file that is not valid UTF-8");
        }
        let text = sm.text(file);
        let path = if sm.is_virtual(file) {
            String::new()
        } else {
            std::path::absolute(name)
                .ok()
                .and_then(|p| p.to_str().map(str::to_owned))
                .ok_or("a file whose path is not absolute-able UTF-8")?
        };
        let rec = FileRec {
            name: name.to_owned(),
            path,
            len: text.len() as u32,
            hash: content_hash(text.as_bytes()),
            system: false,
            text: sm.is_virtual(file).then(|| text.to_owned()),
        };
        let i = self.files.len() as u32;
        self.files.push(rec);
        self.file_ix.insert(file, i);
        Ok(i)
    }
}

/// The locations the record for the range or entry at `e` names.
fn deps_of(sm: &SourceManager, e: Loc) -> W<Vec<Loc>> {
    if e.is_macro() {
        let x = sm.expansion_info(e).ok_or("an unknown macro location")?;
        let mut v = vec![x.spelling, x.expansion.lo, x.expansion.hi];
        if let ExpansionKind::Macro { name } = x.kind {
            v.extend([name.lo, name.hi]);
        }
        if v.iter().any(|l| !l.is_valid()) {
            return Err("a macro expansion record with an invalid location");
        }
        Ok(v)
    } else {
        Ok(vec![sm.include_loc(e)])
    }
}

/// The main-file offset at which `l` was entered: its expansion
/// location, then the `#include` chain up to `main`.
pub(super) fn main_pos(sm: &SourceManager, main: FileId, l: Loc) -> Option<u32> {
    let mut l = sm.expansion_loc(l);
    for _ in 0..4096 {
        match sm.offset_of(l) {
            Some((f, o)) if f == main && !sm.include_loc(l).is_valid() => return Some(o),
            _ => {}
        }
        let up = sm.include_loc(l);
        if !up.is_valid() {
            return None;
        }
        l = up;
    }
    None
}

/// Checks the records against each other and the file table: every
/// record names earlier records, a spelled range stays inside one
/// record, record 0 is the main file's own range. Returns the bases.
pub(super) fn validate(files: &[FileRec], slocs: &[SlocRec]) -> Result<Vec<u32>, &'static str> {
    if files.is_empty()
        || !matches!(slocs.first(), Some(SlocRec::File { file: 0, from, .. }) if *from == NONE)
    {
        return Err("no main file record");
    }
    for f in files {
        if let Some(t) = &f.text
            && (t.len() != f.len as usize || content_hash(t.as_bytes()) != f.hash)
        {
            return Err("an in-memory file whose text does not match its hash");
        }
        if f.text.is_none() && f.path.is_empty() {
            return Err("a file without a path or text");
        }
    }
    let mut bases: Vec<u32> = Vec::with_capacity(slocs.len());
    let mut next = 0u64;
    let find = |bases: &[u32], o: u32| -> Option<usize> {
        let i = bases.partition_point(|&b| b <= o).checked_sub(1)?;
        Some(i)
    };
    for s in slocs {
        let len = sloc_len(files, s).ok_or("a location record naming a missing file")?;
        let lens = |i: usize| sloc_len(files, &slocs[i]).unwrap_or(0);
        // A location named by the record: earlier and inside a record.
        let ok = |o: u32| o != NONE && u64::from(o) < next && find(&bases, o).is_some();
        match s {
            SlocRec::File { from, marks, .. } => {
                if *from != NONE && !ok(*from) {
                    return Err("an include location out of range");
                }
                if marks.windows(2).any(|w| w[0].0 >= w[1].0)
                    || marks.iter().any(|&(o, k)| o > len || kind_of(k).is_none())
                {
                    return Err("bad file-kind marks");
                }
            }
            SlocRec::Macro { spelling, len, exp, name } => {
                check_exp(&bases, &lens, *spelling, *len, exp, &ok)?;
                if !name.iter().all(|&o| ok(o)) {
                    return Err("a macro name location out of range");
                }
            }
            SlocRec::Arg { spelling, len, exp } => {
                check_exp(&bases, &lens, *spelling, *len, exp, &ok)?;
            }
        }
        bases.push(next as u32);
        next += u64::from(len) + 1;
        if next > SPACE_END {
            return Err("location records past the location space");
        }
    }
    Ok(bases)
}

fn check_exp(
    bases: &[u32],
    lens: &dyn Fn(usize) -> u32,
    spelling: u32,
    len: u32,
    exp: &[u32; 2],
    ok: &dyn Fn(u32) -> bool,
) -> Result<(), &'static str> {
    if !ok(spelling) || !exp.iter().all(|&o| ok(o)) {
        return Err("a macro expansion location out of range");
    }
    let i = bases.partition_point(|&b| b <= spelling) - 1;
    if u64::from(spelling) + u64::from(len) > u64::from(bases[i]) + u64::from(lens(i)) {
        return Err("a spelled range crossing a location record");
    }
    Ok(())
}

/// The importer's side: image location to importer location.
#[derive(Debug, Default)]
pub(crate) struct Locs {
    /// `(image base, length, importer location of the base)`, by base.
    runs: Vec<(u32, u32, Loc)>,
}

impl Locs {
    /// Recreates the records in the importer's source manager, with the
    /// file table's buffers `ids`.
    pub(super) fn build(
        sm: &SourceManager,
        files: &[FileRec],
        slocs: &[SlocRec],
        bases: &[u32],
        ids: &[FileId],
    ) -> Result<Locs, &'static str> {
        let mut m = Locs { runs: Vec::with_capacity(slocs.len()) };
        for (k, s) in slocs.iter().enumerate() {
            let len = sloc_len(files, s).unwrap_or(0);
            let base = match s {
                SlocRec::File { file, from, marks } => {
                    let id = *ids.get(*file as usize).ok_or("a file the importer has not")?;
                    // By the range, not the text: a lazy buffer stays unread.
                    let span = sm.file_span(id);
                    if span.hi.to_raw() - span.lo.to_raw() != len {
                        return Err("a file whose length differs from its record");
                    }
                    let at = if *from == NONE {
                        sm.loc(id, 0)
                    } else {
                        sm.add_inclusion(id, m.get(*from)).map_err(|_| "no location space")?
                    };
                    for &(o, kd) in marks {
                        if let Some(kd) = kind_of(kd) {
                            sm.set_file_kind(at.with_offset(o), kd);
                        }
                    }
                    at
                }
                SlocRec::Macro { spelling, len, exp, name } => {
                    let kind =
                        ExpansionKind::Macro { name: Span::new(m.get(name[0]), m.get(name[1])) };
                    sm.add_expansion(m.get(*spelling), *len, m.span(exp), kind)
                        .map_err(|_| "no location space")?
                }
                SlocRec::Arg { spelling, len, exp } => sm
                    .add_expansion(m.get(*spelling), *len, m.span(exp), ExpansionKind::MacroArg)
                    .map_err(|_| "no location space")?,
            };
            m.runs.push((bases[k], len, base));
        }
        Ok(m)
    }

    fn span(&self, e: &[u32; 2]) -> Span {
        Span::new(self.get(e[0]), self.get(e[1]))
    }

    /// The importer location of image location `o` ([`NONE`]: invalid).
    pub(crate) fn get(&self, o: u32) -> Loc {
        if o == NONE {
            return Loc::INVALID;
        }
        let Some(i) = self.runs.partition_point(|r| r.0 <= o).checked_sub(1) else {
            return Loc::INVALID;
        };
        let (b, len, at) = self.runs[i];
        if o - b > len { Loc::INVALID } else { at.with_offset(o - b) }
    }

    /// True if `o` is [`NONE`] or a location of the image's space.
    pub(super) fn in_space(bases: &[u32], files: &[FileRec], slocs: &[SlocRec], o: u32) -> bool {
        if o == NONE {
            return true;
        }
        let Some(i) = bases.partition_point(|&b| b <= o).checked_sub(1) else { return false };
        sloc_len(files, &slocs[i]).is_some_and(|len| o - bases[i] <= len)
    }
}
