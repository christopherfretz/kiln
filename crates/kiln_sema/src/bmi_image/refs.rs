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
//! without decoding it. The found sets of the unit's dependent entries
//! (spec/ast.md §18.12) follow, with their elements as declaration
//! words: an element the image cannot carry fails the image. Last, the
//! declarator groups the carried shared entries name (§18.3.1, WP-3144),
//! each a list of declaration words. Then the substitution keys
//! (spec/ast.md §18.14, WP-3540): whether the unit recorded them, each
//! key with its template, arguments, parameter or deduced list, outcome,
//! rows and root (§18.14.10, WP-3541: a slot's argument list as written,
//! an expression argument as its expression), and the keys left out. A
//! key is carried whole or left out and counted: the importer then lists
//! it as a gap, since it does not make the key again.

use super::body::PARAM;
use super::{ArgRec, Image, LEX_TU, Locs, NONE, QRef, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;
use kiln_ast::types::TemplateArg;
use kiln_ast::written_refs::{
    AdlSet, AdlSetId, FoundDecl, FoundSetId, PatternNode, QualLink, REF_DEPENDENT, REF_GROUP,
    RefLookup, RefQualifier, RefRole, RootSlot, RootTag, SpecRef, SubstKind, SubstOutcome,
    SubstRef, Unrecorded, WrittenRef, WrittenRefId,
};
use kiln_ast::{ArgListId, DeclId, QualType, RawId as _};
use kiln_source::fx::FxHashMap;
use kiln_source::{FileId, Loc, Span};

/// A qualifier word: a leading `::`.
const Q_GLOBAL: u32 = NONE - 1;
/// A qualifier word: a `decltype(...)::` or splice specifier (WP-2828).
const Q_SPEC: u32 = NONE - 2;

/// The fixed size of an entry, a row and a node (for `count`).
const ENTRY_BYTES: usize = 8 * 4 + 3;
const SPEC_BYTES: usize = 7 * 4 + 2;
const UDIR_BYTES: usize = 3 * 4;
const NODE_BYTES: usize = 1 + 2 * 4;
const LEAD_BYTES: usize = 7 * 4;
const FOUND_SET_BYTES: usize = 2 + 4;
const FOUND_ROW_BYTES: usize = 2 * 4;
/// The smallest group: its count and two declarations.
const GROUP_BYTES: usize = 1 + 2 * 4;
/// The smallest key (no arguments, no deduced list, no root slot) and a
/// keyed row.
const KEY_BYTES: usize = 2 + 4 + 4 + 4 + 1 + 4 + 4;
const KEY_ROW_BYTES: usize = 6 * 4 + 2;

/// One entry: `lo, hi, entity, found, enclosing, qualifier, udir, adl`
/// (declaration words, the qualifier an earlier entry's index,
/// [`Q_GLOBAL`] or [`Q_SPEC`], the set an index into `sets`; [`NONE`] for
/// none), then
/// role, flags and `how`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct EntryRec {
    words: [u32; 8],
    role: u8,
    flags: u8,
    how: u8,
}

/// One per-specialization row: `entry, spec, entity, found, adl, udir,
/// naming_class`, flags, `how`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct SpecRec {
    words: [u32; 7],
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
    /// A dependent operator's token in a body expression
    /// (`PatternNode::Oper`, spec/ast.md §18.13.6).
    Oper {
        entry: u32,
        expr: u32,
        slot: u8,
    },
}

/// A recorded table: argument-dependent sets (namespace and class
/// counts, then the declaration words), entries, rows, nodes, and the
/// using-directives of the dependent entries' ordinary lookups (`entry,
/// declaration, directive`), the entries' leading `::`, `~`, specifier
/// and closing tokens and a friend specialization's `<` and `>` (`entry,
/// colon, tilde, spec, close, langle, rangle`, locations or [`NONE`];
/// WP-2326, WP-2828, WP-2820, WP-3008), the
/// found sets (`how`, flags, `decl, found` word pairs) and the dependent
/// entries' rows (`entry, set`; WP-2472), and the declarator groups of
/// the shared entries (declaration words, the first the entries'
/// `enclosing`; WP-3144).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct RefsRec {
    sets: Vec<(u32, u32, Vec<u32>)>,
    entries: Vec<EntryRec>,
    specs: Vec<SpecRec>,
    nodes: Vec<NodeRec>,
    udirs: Vec<[u32; 3]>,
    leads: Vec<[u32; 7]>,
    found_sets: Vec<FoundRec>,
    found_rows: Vec<[u32; 2]>,
    groups: Vec<Vec<u32>>,
    /// Whether the unit recorded its substitution keys completely.
    keys_on: bool,
    keys: Vec<KeyRec>,
    key_rows: Vec<KeyRowRec>,
    /// The keys left out and their rows.
    keys_left_out: [u32; 2],
}

/// One substitution key (spec/ast.md §18.14.1): kind, outcome, the
/// template's word, the canonical arguments (a pack as [`ArgRec::Pack`]
/// and its elements), a `DefaultArg`'s parameter word (else 0), a
/// `PartialMatch`'s deduced list, its number of rows (the next of
/// `key_rows`) and its root's slots (§18.14.10).
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct KeyRec {
    kind: u8,
    outcome: u8,
    template: u32,
    args: Vec<ArgRec>,
    extra: u32,
    deduced: Option<Vec<ArgRec>>,
    rows: u32,
    root: Vec<SlotRec>,
}

/// One root slot (spec/ast.md §18.14.10) by its tag: an argument list as
/// written (an expression argument as its expression, [`ArgRec::Expr`]),
/// an expression's section, a type.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum SlotRec {
    NotReached,
    Failed,
    Expr(u32),
    Type(QRef),
    Args(Vec<ArgRec>),
}

/// One keyed row: `entry, entity, found, adl, udir, naming_class`, flags,
/// `how`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct KeyRowRec {
    words: [u32; 6],
    flags: u8,
    how: u8,
}

/// One found set: its lookup kind, flags and `decl, found` words.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct FoundRec {
    how: u8,
    flags: u8,
    decls: Vec<[u32; 2]>,
}

/// The section as decoded.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) enum Refs {
    /// The unit was compiled with the table off (`len = 0`).
    #[default]
    Off,
    /// The importer does not record: the section was skipped.
    Skipped,
    Table(Box<RefsRec>),
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
        if t.unrecorded().iter().any(|u| !u.keys_only()) {
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
                QualLink::Specifier => Q_SPEC,
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
                    self.ref_opt(s.naming_class)?,
                ])
            })();
            // A specialization the image does not carry: the importer
            // makes its rows again when it instantiates it.
            if let Some(words) = words {
                out.specs.push(SpecRec { words, flags: s.flags, how: s.how as u8 });
            }
        }
        self.ref_nodes(&map, &mut out)?;
        self.ref_found(&map, &mut out)?;
        self.ref_groups(&mut out)?;
        for l in t.leads() {
            let entry = map[l.entry.index()];
            if entry != NONE {
                let (colon, tilde) = (self.ref_loc(l.colon)?, self.ref_loc(l.tilde)?);
                let (spec, close) = (self.ref_loc(l.spec)?, self.ref_loc(l.close)?);
                let (langle, rangle) = (self.ref_loc(l.langle)?, self.ref_loc(l.rangle)?);
                out.leads.push([entry, colon, tilde, spec, close, langle, rangle]);
            }
        }
        self.ref_keys(&map, &mut memo, &mut out);
        self.img.refs = Refs::Table(Box::new(out));
        Ok(())
    }

    /// The substitution keys (spec/ast.md §18.14): carried when the unit
    /// recorded them without a gap; a key whose template, arguments or
    /// rows the image cannot carry is left out whole and counted.
    fn ref_keys(&mut self, map: &[u32], memo: &mut FxHashMap<AdlSetId, u32>, out: &mut RefsRec) {
        let s = self.s;
        let t = &s.ast.written_refs;
        out.keys_on = t.subst_keys_on() && t.unrecorded().is_empty();
        if !out.keys_on {
            return;
        }
        for (id, k) in t.subst_keys() {
            let rows = t.subst_rows(id);
            let rec = (|| {
                let mut recs = Vec::with_capacity(rows.len());
                for r in rows {
                    let adl = match r.adl {
                        Some(a) => self.ref_set(a, memo, out)?,
                        None => NONE,
                    };
                    let entry = map[r.entry.index()];
                    if entry == NONE {
                        return None;
                    }
                    let words = [
                        entry,
                        self.ref_opt(r.entity)?,
                        self.ref_opt(r.found)?,
                        adl,
                        self.ref_opt(r.udir)?,
                        self.ref_opt(r.naming_class)?,
                    ];
                    recs.push(KeyRowRec { words, flags: r.flags, how: r.how as u8 });
                }
                let extra = match k.parameter() {
                    Some(p) => self.ref_word(p)?,
                    None => 0,
                };
                let deduced = match k.deduced() {
                    Some(l) => Some(self.spec_arg_recs(l).ok()?),
                    None => None,
                };
                let mut root = Vec::with_capacity(t.subst_root(id).len());
                for slot in t.subst_root(id) {
                    root.push(self.slot_rec(slot)?);
                }
                let key = KeyRec {
                    kind: k.kind as u8,
                    outcome: k.outcome as u8,
                    template: self.ref_word(k.template)?,
                    args: self.spec_arg_recs(k.args).ok()?,
                    extra,
                    deduced,
                    rows: u32::try_from(recs.len()).ok()?,
                    root,
                };
                Some((key, recs))
            })();
            if let Some((key, recs)) = rec {
                out.keys.push(key);
                out.key_rows.extend(recs);
            } else {
                out.keys_left_out[0] += 1;
                out.keys_left_out[1] += k.row_len();
            }
        }
    }

    /// The record of root slot `x` (spec/ast.md §18.14.10); `None` when
    /// the image cannot carry what it holds (its key is then left out).
    fn slot_rec(&mut self, x: &RootSlot) -> Option<SlotRec> {
        Some(match x.tag {
            RootTag::NotReached => SlotRec::NotReached,
            RootTag::Failed => SlotRec::Failed,
            RootTag::Expr => SlotRec::Expr(self.texpr(x.as_expr()?).ok()?),
            RootTag::Type => SlotRec::Type(self.qref(x.as_type()?).ok()?),
            RootTag::Args => SlotRec::Args(self.written_arg_recs(x.as_args()?).ok()?),
            _ => return None,
        })
    }

    /// The declarator groups the section's shared entries name
    /// (spec/ast.md §18.3.1); a declaration the image does not carry
    /// fails it.
    fn ref_groups(&self, out: &mut RefsRec) -> W<()> {
        let mut firsts: Vec<u32> =
            out.entries.iter().filter(|e| e.flags & REF_GROUP != 0).map(|e| e.words[4]).collect();
        if firsts.is_empty() {
            return Ok(());
        }
        firsts.sort_unstable();
        firsts.dedup();
        for g in self.s.ast.written_refs.groups() {
            // A group none of the section's entries names: the importer's
            // replay of the preamble makes it.
            let first = g.first().and_then(|&d| self.ref_word(d));
            if first.is_none_or(|f| firsts.binary_search(&f).is_err()) {
                continue;
            }
            let words = g.iter().map(|&d| self.ref_word(d)).collect::<Option<Vec<u32>>>();
            out.groups
                .push(words.ok_or("a declarator group of a declaration the image does not carry")?);
        }
        Ok(())
    }

    /// The found sets of the section's dependent entries (spec/ast.md
    /// §18.12), by `map`; an element the image does not carry fails it.
    fn ref_found(&self, map: &[u32], out: &mut RefsRec) -> W<()> {
        const UNCARRIED: &str = "a found declaration the image does not carry";
        let t = &self.s.ast.written_refs;
        let mut memo: FxHashMap<FoundSetId, u32> = FxHashMap::default();
        for &(id, _) in t.found_rows() {
            let entry = map[id.index()];
            let Some(s) = t.found_set_id(id) else { continue };
            if entry == NONE {
                continue;
            }
            let set = if let Some(&i) = memo.get(&s) {
                i
            } else {
                let f = t.found_set(s);
                let mut decls = Vec::with_capacity(f.decls.len());
                for e in f.decls {
                    let d = self.ref_word(e.decl).ok_or(UNCARRIED)?;
                    let g = if e.found == e.decl {
                        d
                    } else {
                        self.ref_word(e.found).ok_or(UNCARRIED)?
                    };
                    decls.push([d, g]);
                }
                let i = out.found_sets.len() as u32;
                out.found_sets.push(FoundRec { how: f.how as u8, flags: f.flags, decls });
                memo.insert(s, i);
                i
            };
            out.found_rows.push([entry, set]);
        }
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
                PatternNode::Oper(e, slot) => NodeRec::Oper {
                    entry,
                    expr: self
                        .expr_word(e)
                        .map_err(|_| "a dependent written operator outside the image's bodies")?,
                    slot,
                },
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
            NodeRec::Oper { entry, expr, slot } => {
                b.u8(3);
                b.u32(entry);
                b.u32(expr);
                b.u8(slot);
            }
        }
    }
    b.len(t.udirs.len());
    for u in &t.udirs {
        for &x in u {
            b.u32(x);
        }
    }
    b.len(t.leads.len());
    for l in &t.leads {
        for &x in l {
            b.u32(x);
        }
    }
    b.len(t.found_sets.len());
    for f in &t.found_sets {
        b.u8(f.how);
        b.u8(f.flags);
        b.len(f.decls.len());
        for &[d, g] in &f.decls {
            b.u32(d);
            b.u32(g);
        }
    }
    b.len(t.found_rows.len());
    for &[entry, set] in &t.found_rows {
        b.u32(entry);
        b.u32(set);
    }
    b.len(t.groups.len());
    for g in &t.groups {
        b.len(g.len());
        for &d in g {
            b.u32(d);
        }
    }
    encode_keys(&mut b, t);
    // A recorded table is never empty (its six counts): the length
    // tells it from the table off.
    w.len(b.0.len());
    w.0.extend_from_slice(&b.0);
}

/// The section's substitution keys and their rows (spec/ast.md §18.14).
fn encode_keys(b: &mut Wr, t: &RefsRec) {
    b.u8(u8::from(t.keys_on));
    b.len(t.keys.len());
    for k in &t.keys {
        b.u8(k.kind);
        b.u8(k.outcome);
        b.u32(k.template);
        b.args(&k.args);
        b.u32(k.extra);
        match &k.deduced {
            None => b.u8(0),
            Some(l) => {
                b.u8(1);
                b.args(l);
            }
        }
        b.u32(k.rows);
        b.len(k.root.len());
        for x in &k.root {
            match x {
                SlotRec::NotReached => b.u8(0),
                SlotRec::Failed => b.u8(1),
                SlotRec::Expr(e) => {
                    b.u8(2);
                    b.u32(*e);
                }
                SlotRec::Type(q) => {
                    b.u8(3);
                    b.q(*q);
                }
                SlotRec::Args(l) => {
                    b.u8(4);
                    b.args(l);
                }
            }
        }
    }
    b.len(t.key_rows.len());
    for r in &t.key_rows {
        for &x in &r.words {
            b.u32(x);
        }
        b.u8(r.flags);
        b.u8(r.how);
    }
    b.u32(t.keys_left_out[0]);
    b.u32(t.keys_left_out[1]);
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
        let mut words = [0; 7];
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
            3 => NodeRec::Oper { entry: r.u32()?, expr: r.u32()?, slot: r.u8()? },
            _ => return bad("bad written-reference pattern node"),
        });
    }
    for _ in 0..r.count(UDIR_BYTES)? {
        t.udirs.push([r.u32()?, r.u32()?, r.u32()?]);
    }
    for _ in 0..r.count(LEAD_BYTES)? {
        t.leads.push([r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?]);
    }
    for _ in 0..r.count(FOUND_SET_BYTES)? {
        let (how, flags) = (r.u8()?, r.u8()?);
        let m = r.count(8)?;
        let mut decls = Vec::with_capacity(m);
        for _ in 0..m {
            decls.push([r.u32()?, r.u32()?]);
        }
        t.found_sets.push(FoundRec { how, flags, decls });
    }
    for _ in 0..r.count(FOUND_ROW_BYTES)? {
        t.found_rows.push([r.u32()?, r.u32()?]);
    }
    for _ in 0..r.count(GROUP_BYTES)? {
        let m = r.count(4)?;
        let mut g = Vec::with_capacity(m);
        for _ in 0..m {
            g.push(r.u32()?);
        }
        t.groups.push(g);
    }
    decode_keys(&mut r, &mut t)?;
    if !r.0.is_empty() {
        return bad("trailing bytes in the written references");
    }
    Ok(Refs::Table(Box::new(t)))
}

/// The substitution keys at the end of the section (spec/ast.md
/// §18.14.5): `keys_on`, the keys with their roots, their rows and the
/// counts of the keys left out.
fn decode_keys(r: &mut Rd<'_>, t: &mut RefsRec) -> Res<()> {
    t.keys_on = match r.u8()? {
        0 => false,
        1 => true,
        _ => return bad("bad written-reference keys"),
    };
    for _ in 0..r.count(KEY_BYTES)? {
        let (kind, outcome, template) = (r.u8()?, r.u8()?, r.u32()?);
        let args = r.args()?;
        let extra = r.u32()?;
        let deduced = match r.u8()? {
            0 => None,
            1 => Some(r.args()?),
            _ => return bad("bad written-reference key"),
        };
        let rows = r.u32()?;
        let mut root = Vec::new();
        for _ in 0..r.count(1)? {
            root.push(match r.u8()? {
                0 => SlotRec::NotReached,
                1 => SlotRec::Failed,
                2 => SlotRec::Expr(r.u32()?),
                3 => SlotRec::Type(r.q()?),
                4 => SlotRec::Args(r.args()?),
                _ => return bad("bad written-reference root slot"),
            });
        }
        t.keys.push(KeyRec { kind, outcome, template, args, extra, deduced, rows, root });
    }
    for _ in 0..r.count(KEY_ROW_BYTES)? {
        let mut words = [0; 6];
        for x in &mut words {
            *x = r.u32()?;
        }
        t.key_rows.push(KeyRowRec { words, flags: r.u8()?, how: r.u8()? });
    }
    t.keys_left_out = [r.u32()?, r.u32()?];
    Ok(())
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
            && (qual == NONE || qual == Q_GLOBAL || qual == Q_SPEC || (qual as usize) < i)
            && RefRole::from_u8(e.role).is_some()
            && RefLookup::from_u8(e.how).is_some()
    });
    let ne = t.entries.len() as u32;
    let ok_specs = t.specs.iter().all(|s| {
        let [entry, spec, entity, found, adl, udir, naming] = s.words;
        entry < ne
            && word_ok(spec)
            && d_ok(entity)
            && d_ok(found)
            && d_ok(udir)
            && d_ok(naming)
            && (adl == NONE || adl < ns)
            && RefLookup::from_u8(s.how).is_some()
    });
    let nx = img.bodies.exprs.len() as u32;
    let ok_nodes = t.nodes.iter().all(|n| match *n {
        NodeRec::Expr { entry, expr } => entry < ne && expr < nx,
        NodeRec::Type { entry, ty, .. } => entry < ne && qref_ok(ty),
        NodeRec::Decl { entry, decl } => entry < ne && word_ok(decl),
        NodeRec::Oper { entry, expr, slot } => entry < ne && expr < nx && slot < 2,
    });
    let ok_udirs = t.udirs.iter().all(|&[entry, d, u]| entry < ne && word_ok(d) && word_ok(u));
    // By entry, once each; a `::` only on an entry after a leading `::`,
    // a specifier's token only on an entry after a specifier, a closing
    // token only on an `operator` entry.
    let ok_leads = t.leads.windows(2).all(|w| w[0][0] < w[1][0])
        && t.leads.iter().all(|&[entry, colon, _, spec, close, langle, rangle]| {
            entry < ne && {
                let e = &t.entries[entry as usize];
                let qual = e.words[5];
                (colon == NONE || qual == Q_GLOBAL)
                    && (spec == NONE || qual == Q_SPEC)
                    && (close == NONE || e.role == RefRole::Operator as u8)
                    && (langle == NONE) == (rangle == NONE)
            }
        });
    // Found sets of known kinds over records; rows by entry, once each.
    let nf = t.found_sets.len() as u32;
    let ok_found = t.found_sets.iter().all(|f| {
        RefLookup::from_u8(f.how).is_some()
            && f.decls.iter().all(|&[d, g]| word_ok(d) && word_ok(g))
    }) && t.found_rows.windows(2).all(|w| w[0][0] < w[1][0])
        && t.found_rows.iter().all(|&[entry, set]| entry < ne && set < nf);
    // Groups of two or more records; every shared entry's `enclosing`
    // the first of one.
    let ok_groups = t.groups.iter().all(|g| g.len() >= 2 && g.iter().all(|&w| word_ok(w)))
        && t.entries
            .iter()
            .filter(|e| e.flags & REF_GROUP != 0)
            .all(|e| t.groups.iter().any(|g| g[0] == e.words[4]));
    let ok_keys = keys_ok(img, t, qref_ok, &word_ok);
    if ok_sets
        && ok_entries
        && ok_specs
        && ok_nodes
        && ok_udirs
        && ok_leads
        && ok_found
        && ok_groups
        && ok_keys
    {
        Ok(())
    } else {
        bad("bad written references")
    }
}

/// Keys of known kinds and outcomes over records and argument lists
/// the image carries; their rows tile `key_rows`, each of a dependent
/// entry; nothing when the unit did not record keys.
fn keys_ok(
    img: &Image,
    t: &RefsRec,
    qref_ok: &dyn Fn(QRef) -> bool,
    word_ok: &dyn Fn(u32) -> bool,
) -> bool {
    let d_ok = |w: u32| w == NONE || word_ok(w);
    let (ns, ne) = (t.sets.len() as u32, t.entries.len() as u32);
    let one_ok = |a: &ArgRec| match *a {
        ArgRec::Type(q) => qref_ok(q),
        ArgRec::Value(q, _) => super::spec::value_ty_ok(img, q),
        ArgRec::Template(d, 0) => (d as usize) < img.decls.len() && super::tttp::tname_rec(img, d),
        _ => false,
    };
    let list_ok = |l: &[ArgRec]| {
        let mut k = 0;
        while k < l.len() {
            match l[k] {
                ArgRec::Pack(n) => {
                    let Some(elems) = l.get(k + 1..k + 1 + n as usize) else { return false };
                    if !elems.iter().all(one_ok) {
                        return false;
                    }
                    k += 1 + n as usize;
                }
                ref a if one_ok(a) => k += 1,
                _ => return false,
            }
        }
        true
    };
    // A root slot's argument list may hold an expression argument as
    // written (§18.14.10); a `PartialMatch` root holds argument lists only.
    let nt = img.texprs.len() as u32;
    let slot_arg_ok = |a: &ArgRec| match *a {
        ArgRec::Expr(e, _) => e < nt,
        ref a => one_ok(a),
    };
    let root_ok = |k: &KeyRec, kind: Option<SubstKind>| {
        k.root.iter().all(|x| match x {
            SlotRec::Args(l) => l.iter().all(slot_arg_ok),
            _ if kind == Some(SubstKind::PartialMatch) => false,
            SlotRec::NotReached | SlotRec::Failed => true,
            SlotRec::Expr(e) => *e < nt,
            SlotRec::Type(q) => qref_ok(*q),
        })
    };
    t.keys.iter().all(|k| {
        let kind = SubstKind::from_u8(k.kind);
        root_ok(k, kind)
            && kind.is_some()
            && match SubstOutcome::from_u8(k.outcome) {
                Some(SubstOutcome::Substituted) => true,
                Some(SubstOutcome::Unsatisfied) => kind == Some(SubstKind::Satisfaction),
                None => false,
            }
            && word_ok(k.template)
            && list_ok(&k.args)
            && if kind == Some(SubstKind::DefaultArg) { word_ok(k.extra) } else { k.extra == 0 }
            && match &k.deduced {
                Some(l) => kind == Some(SubstKind::PartialMatch) && list_ok(l),
                None => true,
            }
    }) && t.keys.iter().map(|k| u64::from(k.rows)).sum::<u64>() == t.key_rows.len() as u64
        && t.key_rows.iter().all(|r| {
            let [entry, entity, found, adl, udir, naming] = r.words;
            entry < ne
                && t.entries[entry as usize].flags & REF_DEPENDENT != 0
                && d_ok(entity)
                && d_ok(found)
                && d_ok(udir)
                && d_ok(naming)
                && (adl == NONE || adl < ns)
                && RefLookup::from_u8(r.how).is_some()
        })
        && (t.keys_on || (t.keys.is_empty() && t.keys_left_out == [0, 0]))
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
    for l in &t.leads {
        offs.extend(l[1..].iter().copied().filter(|&o| o != NONE));
    }
}

/// The section's declaration words and argument-dependent sets, mapped
/// to the importer's.
type RefMaps<'a> = (&'a dyn Fn(u32) -> Option<DeclId>, &'a dyn Fn(u32) -> Option<AdlSet>);

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
                Q_SPEC => RefQualifier::SPECIFIER,
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
        self.image_specs(t, &pushed, (&dec, &set));
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
                NodeRec::Oper { entry, expr, slot } => {
                    (entry, PatternNode::Oper(cx.built.expr(expr), slot))
                }
            };
            self.ast.written_refs.note_pattern_node(node, pushed[entry as usize]);
        }
        for &[entry, d, u] in &t.udirs {
            if let (Some(d), Some(u)) = (dec(d), dec(u)) {
                let d = self.ast.first(d);
                self.ast.written_refs.note_pattern_udir(pushed[entry as usize], d, u);
            }
        }
        self.image_leads(t, &pushed, &loc);
        self.image_found(t, &pushed, &dec);
        self.image_groups(t, &dec);
        self.image_keys(img, tys, cx, t, &pushed, (&dec, &set));
    }

    /// The unit's per-specialization rows (spec/ast.md §18.9).
    fn image_specs(&mut self, t: &RefsRec, pushed: &[WrittenRefId], (dec, set): RefMaps<'_>) {
        for s in &t.specs {
            let [entry, spec, entity, found, adl, udir, naming] = s.words;
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
                naming_class: dec(naming),
                flags: s.flags,
                how: RefLookup::from_u8(s.how).unwrap_or_default(),
            });
        }
    }

    /// The unit's substitution keys (spec/ast.md §18.14), when the
    /// importer records them: each committed as the unit committed it
    /// (an identity the importer has already is kept as it is). A unit
    /// that did not record keys, and keys the image left out or the
    /// importer cannot map, are listed as gaps.
    fn image_keys(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        cx: &RefsCtx<'_>,
        t: &RefsRec,
        pushed: &[WrittenRefId],
        (dec, set): RefMaps<'_>,
    ) {
        if !self.ast.written_refs.subst_keys_on() {
            return;
        }
        if !t.keys_on {
            let u = Unrecorded::ModuleKeys { module: img.module.clone(), files: cx.files.into() };
            self.ast.written_refs.note_unrecorded_keys(u);
            return;
        }
        let [mut left_keys, mut left_rows] = t.keys_left_out;
        let mut at = 0usize;
        let mut rows = Vec::new();
        for k in &t.keys {
            let recs = &t.key_rows[at..at + k.rows as usize];
            at += k.rows as usize;
            rows.clear();
            let ok = (|| {
                let kind = SubstKind::from_u8(k.kind)?;
                let outcome = SubstOutcome::from_u8(k.outcome)?;
                let template = self.ast.first(dec(k.template)?);
                let args = self.image_key_args(img, tys, cx.ids, &k.args)?;
                let extra = match (kind, &k.deduced) {
                    (SubstKind::DefaultArg, _) => dec(k.extra)?.raw(),
                    (SubstKind::PartialMatch, Some(l)) => {
                        self.image_key_args(img, tys, cx.ids, l)?.raw()
                    }
                    _ => 0,
                };
                let mut root = Vec::with_capacity(k.root.len());
                for x in &k.root {
                    root.push(self.image_root_slot(img, tys, cx.ids, x)?);
                }
                for r in recs {
                    let [entry, entity, found, adl, udir, naming] = r.words;
                    let adl = if adl == NONE { None } else { Some(set(adl)?) };
                    rows.push(SubstRef {
                        entry: pushed[entry as usize],
                        entity: dec(entity),
                        found: dec(found),
                        adl: adl.map(|a| self.ast.written_refs.intern_adl(a)),
                        udir: dec(udir),
                        naming_class: dec(naming),
                        flags: r.flags,
                        how: RefLookup::from_u8(r.how).unwrap_or_default(),
                    });
                }
                let _ = self
                    .ast
                    .written_refs
                    .commit_subst(kind, outcome, template, args, extra, &rows, &root);
                Some(())
            })();
            if ok.is_none() {
                left_keys += 1;
                left_rows += k.rows;
            }
        }
        if left_keys != 0 {
            let u = Unrecorded::ModuleKeysLeftOut {
                module: img.module.clone(),
                keys: left_keys,
                rows: left_rows,
            };
            self.ast.written_refs.note_unrecorded_keys(u);
        }
    }

    /// The canonical argument list of image arguments `recs` (a pack's
    /// elements one pack); `None` for an argument the importer cannot
    /// build.
    fn image_key_args(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        recs: &[ArgRec],
    ) -> Option<ArgListId> {
        let mut list = Vec::with_capacity(recs.len());
        let mut k = 0;
        while k < recs.len() {
            if let ArgRec::Pack(n) = recs[k] {
                let elems = recs.get(k + 1..k + 1 + n as usize)?;
                let mut v = Vec::with_capacity(elems.len());
                for a in elems {
                    let ty = self.image_ty(img, tys, ids, a.q());
                    v.push(self.image_targ(ids, ty, a));
                }
                if v.contains(&TemplateArg::NULL) {
                    return None;
                }
                list.push(TemplateArg::pack(self.ast.types.arg_list(&v)));
                k += 1 + n as usize;
                continue;
            }
            let ty = self.image_ty(img, tys, ids, recs[k].q());
            let a = self.image_targ(ids, ty, &recs[k]);
            if a == TemplateArg::NULL {
                return None;
            }
            list.push(a);
            k += 1;
        }
        let sugared = self.ast.types.arg_list(&list);
        Some(self.ast.types.canonical_args(sugared))
    }

    /// Root slot `x` of a carried key (spec/ast.md §18.14.10): an
    /// argument list as written (not canonical); `None` for a slot the
    /// importer cannot build (the key is then counted as left out).
    fn image_root_slot(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        x: &SlotRec,
    ) -> Option<RootSlot> {
        let error = TemplateArg::ty(QualType::ERROR);
        Some(match x {
            SlotRec::NotReached => RootSlot::NOT_REACHED,
            SlotRec::Failed => RootSlot::FAILED,
            SlotRec::Expr(e) => RootSlot::expr(self.image_texpr(*e)?),
            SlotRec::Type(q) => {
                let q = self.image_ty(img, tys, ids, *q);
                if q == QualType::ERROR {
                    return None;
                }
                RootSlot::ty(q)
            }
            SlotRec::Args(recs) => {
                let mut v = Vec::with_capacity(recs.len());
                for a in recs {
                    let ty = self.image_ty(img, tys, ids, a.q());
                    let arg = self.image_targ(ids, ty, a);
                    if arg == TemplateArg::NULL || arg == error {
                        return None;
                    }
                    v.push(arg);
                }
                RootSlot::args(self.ast.types.arg_list(&v))
            }
        })
    }

    /// The declarator groups of the unit's shared entries (spec/ast.md
    /// §18.3.1): one the importer cannot map is left out, its entries
    /// then held by their first declaration.
    fn image_groups(&mut self, t: &RefsRec, dec: &dyn Fn(u32) -> Option<DeclId>) {
        for g in &t.groups {
            if let Some(decls) = g.iter().map(|&w| dec(w)).collect::<Option<Vec<DeclId>>>() {
                self.ast.written_refs.push_group(&decls);
            }
        }
    }

    /// The tokens of the unit's entries outside their ranges (spec/ast.md
    /// §18.11.2): a leading `::`, a destructor's `~`, a specifier's first
    /// token, the `]` or `)` closing an `operator` entry (§18.13).
    fn image_leads(&mut self, t: &RefsRec, pushed: &[WrittenRefId], loc: &dyn Fn(u32) -> Loc) {
        for &[entry, colon, tilde, spec, close, langle, rangle] in &t.leads {
            let some = |o: u32| (o != NONE).then(|| loc(o));
            let id = pushed[entry as usize];
            self.ast.written_refs.set_lead(id, some(colon), some(tilde), some(close));
            if let Some(s) = some(spec) {
                self.ast.written_refs.set_specifier_lo(id, s);
            }
            if let (Some(l), Some(r)) = (some(langle), some(rangle)) {
                self.ast.written_refs.set_args(id, l, r);
            }
        }
    }

    /// The found sets of the unit's dependent entries (spec/ast.md
    /// §18.12): `decl` a first declaration, `found` the same unless it was
    /// a using-shadow; a set with an element the importer cannot map is
    /// left out.
    fn image_found(
        &mut self,
        t: &RefsRec,
        pushed: &[WrittenRefId],
        dec: &dyn Fn(u32) -> Option<DeclId>,
    ) {
        let mut sets: Vec<Option<FoundSetId>> = vec![None; t.found_sets.len()];
        for &[entry, set] in &t.found_rows {
            let s = if let Some(s) = sets[set as usize] {
                s
            } else {
                let f = &t.found_sets[set as usize];
                let decls = f.decls.iter().map(|&[d, g]| {
                    let decl = self.ast.first(dec(d)?);
                    Some(FoundDecl { decl, found: if g == d { decl } else { dec(g)? } })
                });
                let Some(decls) = decls.collect::<Option<Vec<FoundDecl>>>() else { continue };
                let how = RefLookup::from_u8(f.how).unwrap_or_default();
                let s = self.ast.written_refs.intern_found(&decls, how, f.flags);
                sets[set as usize] = Some(s);
                s
            };
            self.ast.written_refs.set_found(pushed[entry as usize], s);
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
    /// (record 1), an operator token node and a using-directive (record 0)
    /// for record 1; entry 1 is dependent, with substitution keys of
    /// record 7: an alias key with two rows, a default-argument key
    /// (parameter record 1) without rows and a partial-match key with a
    /// deduced list; the alias key's root has a type, a not-reached and a
    /// failed slot, the default-argument key's none, the partial-match
    /// key's one argument list (spec/ast.md §18.14.10).
    fn with_table() -> Image {
        let mut img = sample();
        let int = QRef { ty: kiln_ast::types::BuiltinType::Int as u32, quals: 0 };
        let entry = |words, role: RefRole| EntryRec { words, role: role as u8, flags: 0, how: 1 };
        img.refs = Refs::Table(Box::new(RefsRec {
            sets: vec![(1, 0, vec![0])],
            entries: vec![
                entry([0, 1, 1, NONE, LEX_TU, NONE, NONE, NONE], RefRole::Nns),
                EntryRec {
                    words: [2, 3, PARAM, NONE, 7, 0, NONE, 0],
                    role: RefRole::Expr as u8,
                    flags: REF_DEPENDENT,
                    how: 1,
                },
                // `decltype(e)::m` (WP-2828): its specifier's first token.
                entry([2, 3, 1, NONE, LEX_TU, Q_SPEC, NONE, NONE], RefRole::Expr),
                // A name the declarator group of records 7 and 1 shares
                // (WP-3144).
                EntryRec {
                    words: [0, 1, 1, NONE, 7, NONE, NONE, NONE],
                    role: RefRole::Type as u8,
                    flags: REF_GROUP,
                    how: 1,
                },
            ],
            specs: vec![SpecRec { words: [1, 7, 1, NONE, NONE, NONE, 7], flags: 0, how: 1 }],
            nodes: vec![
                NodeRec::Expr { entry: 1, expr: 0 },
                NodeRec::Type { entry: 0, ty: int, loc: 5 },
                NodeRec::Decl { entry: 1, decl: 1 },
                // A binary fold's second operator token (WP-2910).
                NodeRec::Oper { entry: 1, expr: 0, slot: 1 },
            ],
            udirs: vec![[1, 1, 0]],
            leads: vec![[1, NONE, 4, NONE, NONE, NONE, NONE], [2, NONE, NONE, 0, NONE, NONE, NONE]],
            found_sets: vec![FoundRec { how: 1, flags: 1, decls: vec![[1, 1], [0, 1]] }],
            found_rows: vec![[1, 0]],
            groups: vec![vec![7, 1]],
            keys_on: true,
            keys: vec![
                KeyRec {
                    kind: SubstKind::Alias as u8,
                    outcome: 1,
                    template: 7,
                    args: vec![ArgRec::Type(int)],
                    extra: 0,
                    deduced: None,
                    rows: 2,
                    root: vec![SlotRec::Type(int), SlotRec::NotReached, SlotRec::Failed],
                },
                KeyRec {
                    kind: SubstKind::DefaultArg as u8,
                    outcome: 1,
                    template: 7,
                    args: vec![ArgRec::Pack(2), ArgRec::Type(int), ArgRec::Type(int)],
                    extra: 1,
                    deduced: None,
                    rows: 0,
                    root: Vec::new(),
                },
                KeyRec {
                    kind: SubstKind::PartialMatch as u8,
                    outcome: 1,
                    template: 7,
                    args: vec![ArgRec::Type(int)],
                    extra: 0,
                    deduced: Some(vec![ArgRec::Type(int)]),
                    rows: 0,
                    root: vec![SlotRec::Args(vec![ArgRec::Type(int)])],
                },
            ],
            key_rows: vec![
                KeyRowRec { words: [1, 1, NONE, 0, NONE, 7], flags: 0, how: 1 },
                KeyRowRec { words: [1, NONE, NONE, NONE, NONE, NONE], flags: 1, how: 0 },
            ],
            keys_left_out: [1, 3],
        }));
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
        let corrupt: [Corrupt; 50] = [
            ("qualifier after its entry", |t| t.entries[0].words[5] = 0),
            ("unknown role", |t| t.entries[0].role = 99),
            ("unknown lookup", |t| t.entries[1].how = 99),
            ("entity past the records", |t| t.entries[0].words[2] = 1000),
            ("parameter past the parameters", |t| t.entries[1].words[2] = PARAM | 0x3e8),
            ("set past the sets", |t| t.entries[1].words[7] = 1),
            ("set counts past its list", |t| t.sets[0].0 = 2),
            ("row of no entry", |t| t.specs[0].words[0] = 4),
            ("row of no specialization", |t| t.specs[0].words[1] = NONE),
            ("node past the expressions", |t| t.nodes[0] = NodeRec::Expr { entry: 1, expr: 9999 }),
            ("row directive past the records", |t| t.specs[0].words[5] = 1000),
            ("row naming class past the records", |t| t.specs[0].words[6] = 1000),
            ("directive of no entry", |t| t.udirs[0][0] = 4),
            ("operator node past the expressions", |t| {
                t.nodes[3] = NodeRec::Oper { entry: 1, expr: 9999, slot: 0 }
            }),
            ("operator node slot past a fold's two", |t| {
                t.nodes[3] = NodeRec::Oper { entry: 1, expr: 0, slot: 2 }
            }),
            ("using node past the records", |t| {
                t.nodes[2] = NodeRec::Decl { entry: 1, decl: 1000 }
            }),
            ("lead of no entry", |t| t.leads[1][0] = 4),
            ("a `::` on an entry without one", |t| t.leads[0][1] = 4),
            ("a specifier on an entry without one", |t| t.leads[0][3] = 4),
            ("leads out of order", |t| t.leads.insert(0, [1, NONE, 4, NONE, NONE, NONE, NONE])),
            ("a closing token on an entry that is no operator", |t| t.leads[0][4] = 4),
            ("a `<` without its `>`", |t| t.leads[0][5] = 4),
            ("found set of an unknown lookup", |t| t.found_sets[0].how = 99),
            ("found declaration past the records", |t| t.found_sets[0].decls[0][1] = 1000),
            ("found row of no set", |t| t.found_rows[0][1] = 1),
            ("found rows out of order", |t| t.found_rows.insert(0, [1, 0])),
            ("a group of one", |t| t.groups[0].truncate(1)),
            ("group declaration past the records", |t| t.groups[0][1] = 1000),
            ("a shared entry without its group", |t| t.groups[0][0] = 1),
            // Substitution keys (spec/ast.md §18.14, WP-3540).
            ("unknown key kind", |t| t.keys[0].kind = 9),
            ("unknown key outcome", |t| t.keys[0].outcome = 3),
            ("unsatisfied outside a satisfaction key", |t| t.keys[0].outcome = 2),
            ("key template past the records", |t| t.keys[0].template = 1000),
            ("key argument of no type", |t| {
                t.keys[0].args[0] = ArgRec::Type(QRef { ty: 999_999, quals: 0 });
            }),
            ("key pack past its elements", |t| t.keys[1].args.truncate(2)),
            ("default-argument parameter past the records", |t| t.keys[1].extra = 1000),
            ("parameter on an alias key", |t| t.keys[0].extra = 1),
            ("deduced list on an alias key", |t| t.keys[0].deduced = Some(Vec::new())),
            ("key rows past the keyed rows", |t| t.keys[0].rows = 3),
            ("keyed row of no entry", |t| t.key_rows[0].words[0] = 4),
            ("keyed row of an entry that is not dependent", |t| t.key_rows[0].words[0] = 0),
            ("keyed row set past the sets", |t| t.key_rows[0].words[3] = 1),
            ("keyed row naming class past the records", |t| t.key_rows[0].words[5] = 1000),
            ("keyed row of an unknown lookup", |t| t.key_rows[1].how = 99),
            ("keys from a unit that did not record them", |t| t.keys_on = false),
            // Roots (spec/ast.md §18.14.10, WP-3541).
            ("a partial-match root slot that is no argument list", |t| {
                t.keys[2].root[0] = SlotRec::NotReached;
            }),
            ("root type of no type", |t| {
                t.keys[0].root[0] = SlotRec::Type(QRef { ty: 999_999, quals: 0 });
            }),
            ("root expression past the sections", |t| t.keys[0].root[1] = SlotRec::Expr(0)),
            ("root argument of no type", |t| {
                t.keys[2].root[0] =
                    SlotRec::Args(vec![ArgRec::Type(QRef { ty: 999_999, quals: 0 })]);
            }),
            ("root expression argument past the sections", |t| {
                t.keys[2].root[0] = SlotRec::Args(vec![ArgRec::Expr(0, false)]);
            }),
        ];
        for (what, f) in corrupt {
            let mut img = with_table();
            f(table(&mut img));
            assert!(decode(&encode(&img)).is_err(), "{what}");
            // Skipped, the section is not looked at.
            assert!(decode_with(&encode(&img), false).is_ok(), "{what}");
        }
        // A unit without keys: nothing of them but the counts.
        let mut img = with_table();
        let t = table(&mut img);
        (t.keys_on, t.keys_left_out) = (false, [0, 0]);
        t.keys.clear();
        t.key_rows.clear();
        let off = encode(&img);
        assert_eq!(decode(&off).as_ref(), Ok(&img));
        assert!(off.len() < encode(&with_table()).len());
        // An unknown root slot tag: the one byte a not-reached and a
        // failed slot differ in, set to a tag no slot has.
        let mut failed = with_table();
        table(&mut failed).keys[0].root[1] = SlotRec::Failed;
        let (mut bytes, other) = (encode(&with_table()), encode(&failed));
        assert_eq!(bytes.len(), other.len());
        let at: Vec<usize> = (0..bytes.len()).filter(|&i| bytes[i] != other[i]).collect();
        assert_eq!((at.len(), bytes[at[0]], other[at[0]]), (1, 0, 1));
        bytes[at[0]] = 9;
        assert!(decode(&bytes).is_err());
        assert!(decode_with(&bytes, false).is_ok());
        // A section whose length runs past the image.
        let mut bytes = encode(&with_table());
        bytes.truncate(bytes.len() - 1);
        assert!(decode_with(&bytes, false).is_err());
    }
}
