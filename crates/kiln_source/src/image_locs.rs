//! S7: the image's locations (spec/modules.md §11.11 "File table" and
//! "Location records"). Every carried location is one `u32` in the
//! image's own location space: a sequence of location records, each the
//! range of one buffer inclusion or one macro expansion entry of the
//! module's source manager, laid end to end from 0 (record `k` covers
//! `[base_k, base_k + len_k]`, and `base_{k+1} = base_k + len_k + 1`).
//! Record 0 is the module's main file, so a main-file location is its
//! offset. A record names only earlier records, so the importer
//! recreates them in order with `add_inclusion` and `add_expansion`.

use crate::fx::FxHashMap;
use crate::hash::content_hash;
use crate::{ExpansionKind, FileId, FileKind, Loc, SourceManager, Span};

/// The invalid image location.
pub const NONE: u32 = u32::MAX;

/// A writer result: the reason a location cannot be carried.
pub type W<T> = Result<T, &'static str>;

/// One buffer the image's locations are in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileRec {
    /// The buffer's name, as diagnostics print it.
    pub name: String,
    /// The absolute path the importer reads it from (empty for an
    /// in-memory buffer).
    pub path: String,
    /// The decoded text's length.
    pub len: u32,
    /// [`content_hash`] of the text.
    pub hash: u64,
    /// The buffer was entered as a system header.
    pub system: bool,
    /// An in-memory buffer's text (`<scratch space>`, a module file's
    /// copy of a changed file): carried inline.
    pub text: Option<String>,
}

/// One location record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlocRec {
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
pub fn sloc_len(files: &[FileRec], s: &SlocRec) -> Option<u32> {
    match s {
        SlocRec::File { file, .. } => files.get(*file as usize).map(|f| f.len),
        SlocRec::Macro { len, .. } | SlocRec::Arg { len, .. } => Some(*len),
    }
}

/// The writer's tables: the records made so far, by the start of the
/// module's range or entry they copy.
#[derive(Debug)]
pub struct LocTable {
    files: Vec<FileRec>,
    file_ix: FxHashMap<FileId, u32>,
    slocs: Vec<SlocRec>,
    bases: Vec<u32>,
    by_start: FxHashMap<u32, u32>,
    next: u64,
    /// [`LocTable::with_predefined`]: predefined buffers and files with
    /// `#line` directives are carried too.
    predefined: bool,
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

pub fn kind_of(c: u8) -> Option<FileKind> {
    match c {
        0 => Some(FileKind::User),
        1 => Some(FileKind::System),
        2 => Some(FileKind::ExternCSystem),
        _ => None,
    }
}

impl LocTable {
    /// A table whose record 0 is `main`'s own range.
    pub fn new(sm: &SourceManager, main: FileId) -> W<LocTable> {
        LocTable::make_table(sm, main, false)
    }

    /// A table like [`new`](Self::new) that also carries locations in
    /// predefined buffers (`<built-in>`, `<command line>`), as in-memory
    /// files with their text inline, and in files with `#line`
    /// directives, whose presumed lines the records do not keep (the
    /// preprocessor-state record, spec/modules.md §11.13: a macro defined
    /// on the command line or by an `-include`d header still has a
    /// definition location).
    pub fn with_predefined(sm: &SourceManager, main: FileId) -> W<LocTable> {
        LocTable::make_table(sm, main, true)
    }

    fn make_table(sm: &SourceManager, main: FileId, predefined: bool) -> W<LocTable> {
        let mut t = LocTable {
            files: Vec::new(),
            file_ix: FxHashMap::default(),
            slocs: Vec::new(),
            bases: Vec::new(),
            by_start: FxHashMap::default(),
            next: 0,
            predefined,
        };
        t.carry(sm, sm.loc(main, 0))?;
        Ok(t)
    }

    /// The files and records, in image order.
    pub fn finish(self) -> (Vec<FileRec>, Vec<SlocRec>) {
        (self.files, self.slocs)
    }

    /// The image location of `l`, carrying its range or entry and
    /// everything that names (the include chain, the spelling and
    /// expansion of a macro location) first.
    pub fn loc(&mut self, sm: &SourceManager, l: Loc) -> W<u32> {
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

    /// The file table index of `file`, adding it to the table (with no
    /// location record) if no location in it was carried: the
    /// preprocessor-state record keys include guards and `#pragma once`
    /// files by it.
    pub fn file_index(&mut self, sm: &SourceManager, file: FileId) -> W<u32> {
        self.file(sm, file)
    }

    /// The file table index of `file`.
    fn file(&mut self, sm: &SourceManager, file: FileId) -> W<u32> {
        if let Some(&i) = self.file_ix.get(&file) {
            return Ok(i);
        }
        let name = sm.file_name(file);
        if name.is_empty() || !self.predefined && name.starts_with('<') && name != "<scratch space>"
        {
            return Err("a location in a predefined buffer (<built-in>, <command line>)");
        }
        if !self.predefined && sm.has_line_directives(file) {
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
pub fn main_pos(sm: &SourceManager, main: FileId, l: Loc) -> Option<u32> {
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
pub fn validate(files: &[FileRec], slocs: &[SlocRec]) -> Result<Vec<u32>, &'static str> {
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
                check_exp(&bases, &lens, *spelling, *len, *exp, &ok)?;
                if !name.iter().all(|&o| ok(o)) {
                    return Err("a macro name location out of range");
                }
            }
            SlocRec::Arg { spelling, len, exp } => {
                check_exp(&bases, &lens, *spelling, *len, *exp, &ok)?;
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
    exp: [u32; 2],
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
#[derive(Clone, Debug, Default)]
pub struct Locs {
    /// `(image base, length, importer location of the base)`, by base.
    runs: Vec<(u32, u32, Loc)>,
}

impl Locs {
    /// Recreates the records in the importer's source manager, with the
    /// file table's buffers `ids`.
    pub fn build(
        sm: &SourceManager,
        files: &[FileRec],
        slocs: &[SlocRec],
        bases: &[u32],
        ids: &[FileId],
    ) -> Result<Locs, &'static str> {
        Locs::build_rooted(sm, files, slocs, bases, ids, Loc::INVALID)
    }

    /// As [`build`](Self::build), the image's own buffers (those with no
    /// includer) entered by an `#include` at `root` when it is valid (a
    /// precompiled header's, from the predefines; WP-1036).
    pub fn build_rooted(
        sm: &SourceManager,
        files: &[FileRec],
        slocs: &[SlocRec],
        bases: &[u32],
        ids: &[FileId],
        root: Loc,
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
                    let at = if *from == NONE && root.is_valid() {
                        sm.add_inclusion(id, root).map_err(|_| "no location space")?
                    } else if *from == NONE {
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
                    sm.add_expansion(m.get(*spelling), *len, m.span(*exp), kind)
                        .map_err(|_| "no location space")?
                }
                SlocRec::Arg { spelling, len, exp } => sm
                    .add_expansion(m.get(*spelling), *len, m.span(*exp), ExpansionKind::MacroArg)
                    .map_err(|_| "no location space")?,
            };
            m.runs.push((bases[k], len, base));
        }
        Ok(m)
    }

    fn span(&self, e: [u32; 2]) -> Span {
        Span::new(self.get(e[0]), self.get(e[1]))
    }

    /// The importer location of image location `o` ([`NONE`]: invalid).
    pub fn get(&self, o: u32) -> Loc {
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
    pub fn in_space(bases: &[u32], files: &[FileRec], slocs: &[SlocRec], o: u32) -> bool {
        if o == NONE {
            return true;
        }
        let Some(i) = bases.partition_point(|&b| b <= o).checked_sub(1) else { return false };
        sloc_len(files, &slocs[i]).is_some_and(|len| o - bases[i] <= len)
    }
}

/// Appends the file table and the location records in the module-file
/// encoding (spec/modules.md §11.11 "File table", "Location records"):
/// little-endian `u32` counts and fields, `u64` hashes, strings as a
/// `u32` byte length and UTF-8.
pub fn encode(files: &[FileRec], slocs: &[SlocRec], out: &mut Vec<u8>) {
    let u32 = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    let len = |out: &mut Vec<u8>, n: usize| u32(out, u32::try_from(n).unwrap_or(u32::MAX));
    let str = |out: &mut Vec<u8>, s: &str| {
        len(out, s.len());
        out.extend_from_slice(s.as_bytes());
    };
    len(out, files.len());
    for f in files {
        str(out, &f.name);
        str(out, &f.path);
        u32(out, f.len);
        out.extend_from_slice(&f.hash.to_le_bytes());
        out.push(u8::from(f.system) | (u8::from(f.text.is_some()) << 1));
        if let Some(t) = &f.text {
            str(out, t);
        }
    }
    len(out, slocs.len());
    for s in slocs {
        match s {
            SlocRec::File { file, from, marks } => {
                out.push(0);
                u32(out, *file);
                u32(out, *from);
                len(out, marks.len());
                for &(o, k) in marks {
                    u32(out, o);
                    out.push(k);
                }
            }
            SlocRec::Macro { spelling, len: n, exp, name } => {
                out.push(1);
                for v in [*spelling, *n, exp[0], exp[1], name[0], name[1]] {
                    u32(out, v);
                }
            }
            SlocRec::Arg { spelling, len: n, exp } => {
                out.push(2);
                for v in [*spelling, *n, exp[0], exp[1]] {
                    u32(out, v);
                }
            }
        }
    }
}

/// A bounds-checked little-endian reader over a module-file section.
#[derive(Debug)]
pub struct Reader<'a>(pub &'a [u8]);

impl<'a> Reader<'a> {
    /// The next `n` bytes.
    pub fn take(&mut self, n: usize) -> W<&'a [u8]> {
        if n > self.0.len() {
            return Err("truncated");
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    /// A byte.
    pub fn u8(&mut self) -> W<u8> {
        Ok(self.take(1)?[0])
    }
    /// A `u32`.
    pub fn u32(&mut self) -> W<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    /// A `u64`.
    pub fn u64(&mut self) -> W<u64> {
        let mut a = [0u8; 8];
        a.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(a))
    }
    /// A count whose elements take at least `min` bytes each, checked
    /// against the bytes left before anything is sized by it.
    pub fn count(&mut self, min: usize) -> W<usize> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min.max(1)) > self.0.len() {
            return Err("length exceeds the section");
        }
        Ok(n)
    }
    /// A string.
    pub fn str(&mut self) -> W<String> {
        let n = self.count(1)?;
        std::str::from_utf8(self.take(n)?).map(str::to_owned).map_err(|_| "invalid UTF-8")
    }
}

/// Decodes what [`encode`] wrote and checks it ([`validate`]); returns
/// the files, the records and their bases.
pub fn decode(r: &mut Reader<'_>) -> W<(Vec<FileRec>, Vec<SlocRec>, Vec<u32>)> {
    let n = r.count(21)?;
    let mut files = Vec::with_capacity(n);
    for _ in 0..n {
        let (name, path, len, hash) = (r.str()?, r.str()?, r.u32()?, r.u64()?);
        let flags = r.u8()?;
        if flags > 3 {
            return Err("bad file flags");
        }
        let text = if flags & 2 != 0 { Some(r.str()?) } else { None };
        files.push(FileRec { name, path, len, hash, system: flags & 1 != 0, text });
    }
    let n = r.count(13)?;
    let mut slocs = Vec::with_capacity(n);
    for _ in 0..n {
        let s = match r.u8()? {
            0 => {
                let (file, from) = (r.u32()?, r.u32()?);
                let m = r.count(5)?;
                let mut marks = Vec::with_capacity(m);
                for _ in 0..m {
                    marks.push((r.u32()?, r.u8()?));
                }
                SlocRec::File { file, from, marks }
            }
            1 => SlocRec::Macro {
                spelling: r.u32()?,
                len: r.u32()?,
                exp: [r.u32()?, r.u32()?],
                name: [r.u32()?, r.u32()?],
            },
            2 => SlocRec::Arg { spelling: r.u32()?, len: r.u32()?, exp: [r.u32()?, r.u32()?] },
            _ => return Err("bad location record"),
        };
        slocs.push(s);
    }
    let bases = validate(&files, &slocs)?;
    Ok((files, slocs, bases))
}
