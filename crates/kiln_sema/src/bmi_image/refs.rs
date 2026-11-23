//! The written-reference table of a module unit (spec/ast.md §18.10.2,
//! spec/modules.md §11.11 "Written references"; WP-1934).
//!
//! When the unit was compiled with the table on, the last section carries
//! the entries of the names written in the unit's own sources after its
//! preamble (the importer replays the preamble, and records its names by
//! parsing them), their provenance, the per-specialization rows of the
//! unit's carried specializations and the pattern nodes of its dependent
//! entries. A declaration is a word as in a body (a record, [`LEX_TU`],
//! or `PARAM | g`). An importer that records appends the entries to its
//! own table when it builds the unit, as a replay of the unit records
//! them; its later instantiations of the unit's templates then give their
//! rows, as they do after a replay. An entry whose declaration, location,
//! qualifier or pattern node the image cannot carry fails the image (the
//! importer replays). A per-specialization row whose specialization (or
//! entity) is not a record is left out: the importer makes it again when
//! it instantiates that specialization. With the table off the section
//! is `len = 0`, and an importer that does not record skips the section
//! without decoding it.

use super::body::PARAM;
use super::{Image, LEX_TU, Locs, NONE, QRef, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;
use kiln_ast::written_refs::{
    AdlSet, AdlSetId, PatternNode, QualLink, RefLookup, RefQualifier, RefRole, SpecRef, Unrecorded,
    WrittenRef, WrittenRefId,
};
use kiln_ast::{DeclId, QualType};
use kiln_source::fx::FxHashMap;
use kiln_source::{FileId, Loc, Span};

/// A qualifier word: a leading `::`.
const Q_GLOBAL: u32 = NONE - 1;

/// The fixed size of an entry, a row and a node (for `count`).
const ENTRY_BYTES: usize = 8 * 4 + 3;
const SPEC_BYTES: usize = 6 * 4 + 2;
const UDIR_BYTES: usize = 3 * 4;
const NODE_BYTES: usize = 1 + 2 * 4;

/// One entry: `lo, hi, entity, found, enclosing, qualifier, udir, adl`
/// (declaration words, the qualifier an earlier entry's index or
/// [`Q_GLOBAL`], the set an index into `sets`; [`NONE`] for none), then
/// role, flags and `how`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct EntryRec {
    words: [u32; 8],
    role: u8,
    flags: u8,
    how: u8,
}

/// One per-specialization row: `entry, spec, entity, found, adl, udir`,
/// flags, `how`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct SpecRec {
    words: [u32; 6],
    flags: u8,
    how: u8,
}

/// A pattern node: its entry, and a body expression or a dependent type
/// written at a location.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum NodeRec {
    Expr {
        entry: u32,
        expr: u32,
    },
    Type {
        entry: u32,
        ty: QRef,
        loc: u32,
    },
    /// A dependent using-declarator's member (`PatternNode::Decl`).
    Decl {
        entry: u32,
        decl: u32,
    },
}

/// A recorded table: argument-dependent sets (namespace and class
/// counts, then the declaration words), entries, rows, nodes, and the
/// using-directives of the dependent entries' ordinary lookups (`entry,
/// declaration, directive`).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct RefsRec {
    sets: Vec<(u32, u32, Vec<u32>)>,
    entries: Vec<EntryRec>,
    specs: Vec<SpecRec>,
    nodes: Vec<NodeRec>,
    udirs: Vec<[u32; 3]>,
}

/// The section as decoded.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) enum Refs {
    /// The unit was compiled with the table off (`len = 0`).
    #[default]
    Off,
    /// The importer does not record: the section was skipped.
    Skipped,
    Table(RefsRec),
}

impl Writer<'_, '_, '_> {
    /// The word of `d`: its record, the translation unit, or a parameter
    /// of a function record.
    fn ref_word(&self, d: DeclId) -> Option<u32> {
        if let Some(&r) = self.recs.get(&d) {
            Some(r)
        } else if d == self.s.ast.tu {
            Some(LEX_TU)
        } else {
            self.bs.params.get(&d).map(|&g| PARAM | g)
        }
    }

    fn ref_opt(&self, d: Option<DeclId>) -> Option<u32> {
        d.map_or(Some(NONE), |d| self.ref_word(d))
    }

    /// The word of set `s`, interned in `out`.
    fn ref_set(
        &self,
        s: AdlSetId,
        memo: &mut FxHashMap<AdlSetId, u32>,
        out: &mut RefsRec,
    ) -> Option<u32> {
        if let Some(&i) = memo.get(&s) {
            return Some(i);
        }
        let set = self.s.ast.written_refs.adl_set_of(s);
        let all = set.namespaces().iter().chain(set.classes()).chain(set.enums());
        let list = all.map(|&d| self.ref_word(d)).collect::<Option<Vec<u32>>>()?;
        let i = out.sets.len() as u32;
        out.sets.push((set.namespaces().len() as u32, set.classes().len() as u32, list));
        memo.insert(s, i);
        Some(i)
    }

    /// Whether the importer records the name at `l` itself, replaying the
    /// preamble (entered before its end, or from an import's buffers).
    fn ref_in_preamble(&self, l: Loc) -> bool {
        self.img.preamble != NONE
            && super::locs::main_pos(self.s.diags.sm(), self.main, l)
                .is_none_or(|p| p < self.img.preamble)
    }

    fn ref_loc(&self, l: Loc) -> W<u32> {
        if l.is_valid() { self.loc(l) } else { Ok(NONE) }
    }

    /// The section (after every other: its declarations are records
    /// already written, its expressions those of bodies already walked).
    pub(super) fn refs_section(&mut self) -> W<()> {
        const UNCARRIED: &str = "a written name of a declaration the image does not carry";
        let t = &self.s.ast.written_refs;
        if !t.is_on() {
            return Ok(());
        }
        if !t.unrecorded().is_empty() {
            return Err("a written-reference table with unrecorded sources");
        }
        let mut out = RefsRec::default();
        let mut memo = FxHashMap::default();
        // Per entry of the unit's table, its index in the section, or
        // `NONE` for one the importer records replaying the preamble.
        let mut map = vec![NONE; t.len()];
        for (id, r) in t.iter() {
            if self.ref_in_preamble(r.range.lo) {
                continue;
            }
            let qual = match r.qualifier.link() {
                QualLink::None => NONE,
                QualLink::Global => Q_GLOBAL,
                QualLink::Entry(q) => match map[q.index()] {
                    NONE => return Err("a written name qualified by a name of the preamble"),
                    w => w,
                },
            };
            let adl = match t.adl_set_id(id) {
                Some(s) => self.ref_set(s, &mut memo, &mut out).ok_or(UNCARRIED)?,
                None => NONE,
            };
            let words = [
                self.ref_loc(r.range.lo)?,
                self.ref_loc(r.range.hi)?,
                self.ref_opt(r.entity).ok_or(UNCARRIED)?,
                self.ref_opt(r.found).ok_or(UNCARRIED)?,
                self.ref_opt(r.enclosing).ok_or(UNCARRIED)?,
                qual,
                self.ref_opt(t.using_directive(id)).ok_or(UNCARRIED)?,
                adl,
            ];
            map[id.index()] = out.entries.len() as u32;
            out.entries.push(EntryRec {
                words,
                role: r.role as u8,
                flags: r.flags,
                how: r.how as u8,
            });
        }
        for (_, s) in t.specs() {
            let entry = map[s.entry.index()];
            if entry == NONE {
                continue;
            }
            let adl = match s.adl {
                Some(a) => self.ref_set(a, &mut memo, &mut out),
                None => Some(NONE),
            };
            let words = (|| {
                Some([
                    entry,
                    self.ref_word(s.spec)?,
                    self.ref_opt(s.entity)?,
                    self.ref_opt(s.found)?,
                    adl?,
                    self.ref_opt(s.udir)?,
                ])
            })();
            // A specialization the image does not carry: the importer
            // makes its rows again when it instantiates it.
            if let Some(words) = words {
                out.specs.push(SpecRec { words, flags: s.flags, how: s.how as u8 });
            }
        }
        self.ref_nodes(&map, &mut out)?;
        self.img.refs = Refs::Table(out);
        Ok(())
    }

    /// The pattern nodes of the section's dependent entries and the
    /// using-directives their candidates were found through (§18.9.1),
    /// by `map` (the unit's entry to its index in the section).
    fn ref_nodes(&mut self, map: &[u32], out: &mut RefsRec) -> W<()> {
        const UNCARRIED: &str = "a written name of a declaration the image does not carry";
        let t = &self.s.ast.written_refs;
        let mut nodes: Vec<(PatternNode, WrittenRefId)> = t.pattern_nodes().collect();
        nodes.sort_unstable();
        for (n, id) in nodes {
            let entry = map[id.index()];
            if entry == NONE {
                continue;
            }
            out.nodes.push(match n {
                PatternNode::Expr(e) => NodeRec::Expr {
                    entry,
                    expr: self
                        .expr_word(e)
                        .map_err(|_| "a dependent written name outside the image's bodies")?,
                },
                PatternNode::Type(q, l) => {
                    let loc = self.ref_loc(l)?;
                    NodeRec::Type { entry, ty: self.qref(q)?, loc }
                }
                PatternNode::Decl(d) => {
                    NodeRec::Decl { entry, decl: self.ref_word(d).ok_or(UNCARRIED)? }
                }
            });
        }
        let mut udirs: Vec<((WrittenRefId, DeclId), DeclId)> = t.pattern_udirs().collect();
        udirs.sort_unstable();
        for ((id, d), u) in udirs {
            let entry = map[id.index()];
            if entry == NONE {
                continue;
            }
            let words =
                [entry, self.ref_word(d).ok_or(UNCARRIED)?, self.ref_word(u).ok_or(UNCARRIED)?];
            out.udirs.push(words);
        }
        Ok(())
    }
}

pub(super) fn encode_refs(w: &mut Wr, s: &Refs) {
    let Refs::Table(t) = s else {
        w.len(0);
        return;
    };
    let mut b = Wr(Vec::new());
    b.len(t.sets.len());
    for (ns, cls, list) in &t.sets {
        b.u32(*ns);
        b.u32(*cls);
        b.len(list.len());
        for &d in list {
            b.u32(d);
        }
    }
    b.len(t.entries.len());
    for e in &t.entries {
        for &x in &e.words {
            b.u32(x);
        }
        b.u8(e.role);
        b.u8(e.flags);
        b.u8(e.how);
    }
    b.len(t.specs.len());
    for s in &t.specs {
        for &x in &s.words {
            b.u32(x);
        }
        b.u8(s.flags);
        b.u8(s.how);
    }
    b.len(t.nodes.len());
    for n in &t.nodes {
        match *n {
            NodeRec::Expr { entry, expr } => {
                b.u8(0);
                b.u32(entry);
                b.u32(expr);
            }
            NodeRec::Type { entry, ty, loc } => {
                b.u8(1);
                b.u32(entry);
                b.q(ty);
                b.u32(loc);
            }
            NodeRec::Decl { entry, decl } => {
                b.u8(2);
                b.u32(entry);
                b.u32(decl);
            }
        }
    }
    b.len(t.udirs.len());
    for u in &t.udirs {
        for &x in u {
            b.u32(x);
        }
    }
    // A recorded table is never empty (its five counts): the length
    // tells it from the table off.
    w.len(b.0.len());
    w.0.extend_from_slice(&b.0);
}

/// Reads the section; `want`: the importer records (otherwise the
/// section is skipped undecoded).
pub(super) fn decode_refs(r: &mut Rd, want: bool) -> Res<Refs> {
    let n = r.count(1)?;
    if n == 0 {
        return Ok(Refs::Off);
    }
    let bytes = r.take(n)?;
    if !want {
        return Ok(Refs::Skipped);
    }
    let mut r = Rd(bytes);
    let mut t = RefsRec::default();
    for _ in 0..r.count(12)? {
        let (ns, cls) = (r.u32()?, r.u32()?);
        let m = r.count(4)?;
        let mut list = Vec::with_capacity(m);
        for _ in 0..m {
            list.push(r.u32()?);
        }
        t.sets.push((ns, cls, list));
    }
    for _ in 0..r.count(ENTRY_BYTES)? {
        let mut words = [0; 8];
        for x in &mut words {
            *x = r.u32()?;
        }
        t.entries.push(EntryRec { words, role: r.u8()?, flags: r.u8()?, how: r.u8()? });
    }
    for _ in 0..r.count(SPEC_BYTES)? {
        let mut words = [0; 6];
        for x in &mut words {
            *x = r.u32()?;
        }
        t.specs.push(SpecRec { words, flags: r.u8()?, how: r.u8()? });
    }
    for _ in 0..r.count(NODE_BYTES)? {
        t.nodes.push(match r.u8()? {
            0 => NodeRec::Expr { entry: r.u32()?, expr: r.u32()? },
            1 => NodeRec::Type { entry: r.u32()?, ty: r.q()?, loc: r.u32()? },
            2 => NodeRec::Decl { entry: r.u32()?, decl: r.u32()? },
            _ => return bad("bad written-reference pattern node"),
        });
    }
    for _ in 0..r.count(UDIR_BYTES)? {
        t.udirs.push([r.u32()?, r.u32()?, r.u32()?]);
    }
    if !r.0.is_empty() {
        return bad("trailing bytes in the written references");
    }
    Ok(Refs::Table(t))
}

/// Every word of the section is in range: declarations are records, the
/// translation unit or parameters, entries and sets the section's, a
/// qualifier precedes its entry, roles and lookups are known, pattern
/// nodes name body expressions and types.
pub(super) fn validate_refs(img: &Image, qref_ok: &dyn Fn(QRef) -> bool) -> Res<()> {
    let Refs::Table(t) = &img.refs else { return Ok(()) };
    let nparams = super::body::param_count(img) as u32;
    let word_ok = |w: u32| {
        w == LEX_TU
            || if w & PARAM != 0 { (w & !PARAM) < nparams } else { (w as usize) < img.decls.len() }
    };
    let d_ok = |w: u32| w == NONE || word_ok(w);
    let ns = t.sets.len() as u32;
    let ok_sets = t.sets.iter().all(|(n, c, list)| {
        u64::from(*n) + u64::from(*c) <= list.len() as u64 && list.iter().all(|&w| word_ok(w))
    });
    let ok_entries = t.entries.iter().enumerate().all(|(i, e)| {
        let [_, _, entity, found, enclosing, qual, udir, adl] = e.words;
        d_ok(entity)
            && d_ok(found)
            && d_ok(enclosing)
            && d_ok(udir)
            && (adl == NONE || adl < ns)
            && (qual == NONE || qual == Q_GLOBAL || (qual as usize) < i)
            && RefRole::from_u8(e.role).is_some()
            && RefLookup::from_u8(e.how).is_some()
    });
    let ne = t.entries.len() as u32;
    let ok_specs = t.specs.iter().all(|s| {
        let [entry, spec, entity, found, adl, udir] = s.words;
        entry < ne
            && word_ok(spec)
            && d_ok(entity)
            && d_ok(found)
            && d_ok(udir)
            && (adl == NONE || adl < ns)
            && RefLookup::from_u8(s.how).is_some()
    });
    let nx = img.bodies.exprs.len() as u32;
    let ok_nodes = t.nodes.iter().all(|n| match *n {
        NodeRec::Expr { entry, expr } => entry < ne && expr < nx,
        NodeRec::Type { entry, ty, .. } => entry < ne && qref_ok(ty),
        NodeRec::Decl { entry, decl } => entry < ne && word_ok(decl),
    });
    let ok_udirs = t.udirs.iter().all(|&[entry, d, u]| entry < ne && word_ok(d) && word_ok(u));
    if ok_sets && ok_entries && ok_specs && ok_nodes && ok_udirs {
        Ok(())
    } else {
        bad("bad written references")
    }
}

/// The section's locations, for the decoder's offset check.
pub(super) fn refs_offsets(img: &Image, offs: &mut Vec<u32>) {
    let Refs::Table(t) = &img.refs else { return };
    for e in &t.entries {
        offs.extend([e.words[0], e.words[1]].into_iter().filter(|&o| o != NONE));
    }
    for n in &t.nodes {
        if let NodeRec::Type { loc, .. } = *n
            && loc != NONE
        {
            offs.push(loc);
        }
    }
}

/// What [`Sema::image_refs`] maps the section's words with.
pub(super) struct RefsCtx<'x> {
    pub(super) ids: &'x [DeclId],
    pub(super) params: &'x [DeclId],
    pub(super) built: &'x super::body::Built,
    pub(super) main: &'x Locs,
    pub(super) files: &'x [FileId],
}

impl Sema<'_, '_> {
    /// Appends the unit's entries to the table when it records (after
    /// every record, body and type is built): the names a replay of the
    /// unit would record parsing it. A unit compiled without the table
    /// is listed as unrecorded (spec/ast.md §18.10.3).
    pub(super) fn image_refs(&mut self, img: &Image, tys: &mut Vec<QualType>, cx: &RefsCtx<'_>) {
        if !self.ast.written_refs.is_on() {
            return;
        }
        let t = match &img.refs {
            Refs::Table(t) => t,
            Refs::Skipped => return,
            Refs::Off => {
                let u = Unrecorded::Module { module: img.module.clone(), files: cx.files.into() };
                self.ast.written_refs.note_unrecorded(u);
                return;
            }
        };
        let tu = self.ast.tu;
        let dec = |w: u32| -> Option<DeclId> {
            match w {
                NONE => None,
                LEX_TU => Some(tu),
                w if w & PARAM != 0 => cx.params.get((w & !PARAM) as usize).copied(),
                w => cx.ids.get(w as usize).copied(),
            }
        };
        let set = |w: u32| -> Option<AdlSet> {
            let (ns, cls, list) = t.sets.get(w as usize)?;
            let list = list.iter().map(|&x| dec(x)).collect::<Option<Vec<DeclId>>>()?;
            let (ns, cls) = (*ns as usize, *cls as usize);
            Some(AdlSet::new(&list[..ns], &list[ns..ns + cls], &list[ns + cls..]))
        };
        let loc = |o: u32| if o == NONE { Loc::INVALID } else { cx.main.get(o) };
        let mut pushed: Vec<WrittenRefId> = Vec::with_capacity(t.entries.len());
        for e in &t.entries {
            let [lo, hi, entity, found, enclosing, qual, udir, adl] = e.words;
            let qualifier = match qual {
                NONE => RefQualifier::NONE,
                Q_GLOBAL => RefQualifier::GLOBAL,
                q => RefQualifier::entry(pushed[q as usize]),
            };
            let r = WrittenRef {
                range: Span::new(loc(lo), loc(hi)),
                entity: dec(entity),
                found: dec(found),
                enclosing: dec(enclosing),
                qualifier,
                role: RefRole::from_u8(e.role).unwrap_or(RefRole::Expr),
                flags: e.flags,
                how: RefLookup::from_u8(e.how).unwrap_or_default(),
            };
            let Some(id) = self.ast.written_refs.push(r) else { return };
            pushed.push(id);
            if let Some(u) = dec(udir) {
                self.ast.written_refs.set_using_directive(id, Some(u));
            }
            if adl != NONE
                && let Some(s) = set(adl)
            {
                self.ast.written_refs.set_adl(id, s);
            }
        }
        for s in &t.specs {
            let [entry, spec, entity, found, adl, udir] = s.words;
            let Some(spec) = dec(spec) else { continue };
            let adl = if adl == NONE { None } else { set(adl) };
            let adl = adl.map(|a| self.ast.written_refs.intern_adl(a));
            self.ast.written_refs.push_spec(SpecRef {
                entry: pushed[entry as usize],
                spec,
                entity: dec(entity),
                found: dec(found),
                adl,
                udir: dec(udir),
                flags: s.flags,
                how: RefLookup::from_u8(s.how).unwrap_or_default(),
            });
        }
        for n in &t.nodes {
            let (entry, node) = match *n {
                NodeRec::Expr { entry, expr } => (entry, PatternNode::Expr(cx.built.expr(expr))),
                NodeRec::Type { entry, ty, loc: l } => {
                    let q = self.image_ty(img, tys, cx.ids, ty);
                    (entry, PatternNode::Type(q, loc(l)))
                }
                NodeRec::Decl { entry, decl } => match dec(decl) {
                    Some(d) => (entry, PatternNode::Decl(d)),
                    None => continue,
                },
            };
            self.ast.written_refs.note_pattern_node(node, pushed[entry as usize]);
        }
        for &[entry, d, u] in &t.udirs {
            if let (Some(d), Some(u)) = (dec(d), dec(u)) {
                let d = self.ast.first(d);
                self.ast.written_refs.note_pattern_udir(pushed[entry as usize], d, u);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::sample;
    use super::super::{decode, decode_with, encode};
    use super::*;

    /// `sample` with a table: entry 0 names record 1 (with a type pattern
    /// node), entry 1 the first parameter, qualified by entry 0, with a
    /// set naming record 0, a row for record 7, a using-declarator node
    /// (record 1) and a using-directive (record 0) for record 1.
    fn with_table() -> Image {
        let mut img = sample();
        let int = QRef { ty: kiln_ast::types::BuiltinType::Int as u32, quals: 0 };
        let entry = |words, role: RefRole| EntryRec { words, role: role as u8, flags: 0, how: 1 };
        img.refs = Refs::Table(RefsRec {
            sets: vec![(1, 0, vec![0])],
            entries: vec![
                entry([0, 1, 1, NONE, LEX_TU, NONE, NONE, NONE], RefRole::Nns),
                entry([2, 3, PARAM, NONE, 7, 0, NONE, 0], RefRole::Expr),
            ],
            specs: vec![SpecRec { words: [1, 7, 1, NONE, NONE, NONE], flags: 0, how: 1 }],
            nodes: vec![
                NodeRec::Expr { entry: 1, expr: 0 },
                NodeRec::Type { entry: 0, ty: int, loc: 5 },
                NodeRec::Decl { entry: 1, decl: 1 },
            ],
            udirs: vec![[1, 1, 0]],
        });
        img
    }

    fn table(img: &mut Image) -> &mut RefsRec {
        match &mut img.refs {
            Refs::Table(t) => t,
            _ => unreachable!("with_table has a table"),
        }
    }

    /// spec/modules.md §11.11 "Written references": the section round
    /// trips; off, it is one empty-length word after the image a
    /// compiler without it writes; an importer that does not record skips
    /// it undecoded.
    #[test]
    fn written_refs_section() {
        let img = with_table();
        let on = encode(&img);
        assert_eq!(decode(&on).as_ref(), Ok(&img));
        let off = encode(&sample());
        let head = off.len() - 4;
        assert_eq!(off[head..], [0, 0, 0, 0]);
        assert_eq!(on[..head], off[..head]);
        assert!(on.len() > off.len());
        let skipped = decode_with(&on, false).expect("skipped");
        assert_eq!(skipped.refs, Refs::Skipped);
        assert_eq!(Image { refs: Refs::Off, ..skipped }, decode(&off).expect("off"));
    }

    #[test]
    fn written_refs_section_is_validated() {
        type Corrupt = (&'static str, fn(&mut RefsRec));
        let corrupt: [Corrupt; 13] = [
            ("qualifier after its entry", |t| t.entries[0].words[5] = 0),
            ("unknown role", |t| t.entries[0].role = 99),
            ("unknown lookup", |t| t.entries[1].how = 99),
            ("entity past the records", |t| t.entries[0].words[2] = 1000),
            ("parameter past the parameters", |t| t.entries[1].words[2] = PARAM | 0x3e8),
            ("set past the sets", |t| t.entries[1].words[7] = 1),
            ("set counts past its list", |t| t.sets[0].0 = 2),
            ("row of no entry", |t| t.specs[0].words[0] = 2),
            ("row of no specialization", |t| t.specs[0].words[1] = NONE),
            ("node past the expressions", |t| t.nodes[0] = NodeRec::Expr { entry: 1, expr: 9999 }),
            ("row directive past the records", |t| t.specs[0].words[5] = 1000),
            ("directive of no entry", |t| t.udirs[0][0] = 2),
            ("using node past the records", |t| {
                t.nodes[2] = NodeRec::Decl { entry: 1, decl: 1000 }
            }),
        ];
        for (what, f) in corrupt {
            let mut img = with_table();
            f(table(&mut img));
            assert!(decode(&encode(&img)).is_err(), "{what}");
            // Skipped, the section is not looked at.
            assert!(decode_with(&encode(&img), false).is_ok(), "{what}");
        }
        // A section whose length runs past the image.
        let mut bytes = encode(&with_table());
        bytes.truncate(bytes.len() - 1);
        assert!(decode_with(&bytes, false).is_err());
    }
}
