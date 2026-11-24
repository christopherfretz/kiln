//! Stage 5: the written-reference table in snapshots (spec/ast.md
//! §18.10.1, §17).
//!
//! The table is written after the AST as one length-prefixed section, so
//! a restoring compilation that does not record skips it without decoding
//! anything. Recording off at the snapshot writes the empty section (its
//! length, 0: one byte), and the snapshot is otherwise the one a compiler
//! without the table writes.
//!
//! | field | class |
//! |---|---|
//! | `entries`, `pending`, `extra`, `adl_sets`, `udir_cache`, `designators` | state: the main table, its pending patches and provenance |
//! | `specs` (dropped rows included, so `SpecRefId`s stay), `spec_nodes`, `spec_udirs` | state: the per-specialization table, its pattern nodes and their using-directives |
//! | `unrecorded` | state |
//! | `leads` | state: the leading `::` and `~` tokens (WP-2326), the closing `]`/`)` of an `operator` entry (WP-2820) and a friend specialization's `<` and `>` (WP-3008) |
//! | `found_rows`, `found_sets`, `found_decls` | state: the found sets of the dependent entries (§18.12, WP-2472) |
//! | `groups`, `group_decls` | state: the declarator groups that share an entry (§18.3.1, WP-3144) |
//! | `subst.on` (as `keys_on`), `subst.keys`, `subst.rows`, `subst.expanded_nodes` | state: the substitution keys and their rows (§18.14, WP-3540); `keys_on` records whether the producer recorded keys |
//! | `adl_index`, `spec_seen`, `found_index`, `subst.index` | rebuilt from `adl_sets`, the live rows of `specs`, `found_sets` and `subst.keys` |
//! | `inst_target`, `spec_pending`, `spec_last`, `spec_key`, `colons`, `subst.pending`, `subst.last` | transient: scratch of one declaration or one instantiation, empty at rest |
//! | `on` | the restoring compilation's own option |

use std::collections::HashMap;

use kiln_snap::{Reader, SnapError, Writer};
use kiln_source::Loc;

use super::{
    AdlSet, AdlSetId, DeclGroupRec, FoundDecl, FoundKey, FoundSetId, FoundSetRec, PatternNode,
    QualLink, REF_DEPENDENT, REF_GROUP, RefExtra, RefLead, RefRole, SPEC_DEAD, SpecRef, SubstKey,
    SubstKeyId, SubstKind, SubstOutcome, SubstRef, SubstTable, Unrecorded, WrittenRef,
    WrittenRefId, WrittenRefs,
};
use crate::Ast;
use crate::ids::{DeclId, ExprId};

/// The snapshot form of a recorded table: its state fields, each map as
/// its entries sorted by key (so the bytes do not depend on hashing).
#[derive(Clone, Debug, Default, kiln_snap::Snap)]
pub struct WrittenRefsSection {
    entries: Vec<WrittenRef>,
    pending: Vec<(ExprId, WrittenRefId)>,
    extra: Vec<(WrittenRefId, RefExtra)>,
    adl_sets: Vec<AdlSet>,
    udir_cache: Vec<((DeclId, DeclId), Option<DeclId>)>,
    designators: Vec<(Loc, WrittenRefId)>,
    specs: Vec<SpecRef>,
    spec_nodes: Vec<(PatternNode, WrittenRefId)>,
    spec_udirs: Vec<((WrittenRefId, DeclId), DeclId)>,
    unrecorded: Vec<Unrecorded>,
    leads: Vec<RefLead>,
    found_rows: Vec<(WrittenRefId, FoundSetId)>,
    found_sets: Vec<FoundSetRec>,
    found_decls: Vec<FoundDecl>,
    groups: Vec<DeclGroupRec>,
    group_decls: Vec<DeclId>,
    /// The producer recorded substitution keys (§18.14).
    keys_on: bool,
    subst_keys: Vec<SubstKey>,
    subst_rows: Vec<SubstRef>,
    /// Sorted.
    expanded_nodes: Vec<PatternNode>,
}

/// The schema hash of the section's payload, which `Sema`'s snapshot root
/// folds after the AST's.
pub const CARRY_SCHEMA_HASH: u64 = <WrittenRefsSection as kiln_snap::Snap>::SCHEMA_HASH;

fn sorted<K: Ord + Copy, V: Copy>(m: &HashMap<K, V>) -> Vec<(K, V)> {
    let mut v: Vec<(K, V)> = m.iter().map(|(&k, &v)| (k, v)).collect();
    v.sort_unstable_by_key(|&(k, _)| k);
    v
}

/// The declarator groups of a section (§18.3.1): by first declaration,
/// once each, two or more declarations each, every shared entry's
/// `enclosing` a group's first declaration.
fn groups_ok(s: &WrittenRefsSection) -> bool {
    let run = |g: &DeclGroupRec| {
        let lo = g.start as usize;
        s.group_decls.get(lo..lo.saturating_add(g.len as usize)).filter(|r| r.len() >= 2)
    };
    let Some(firsts) = s.groups.iter().map(|g| run(g).map(|r| r[0])).collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    firsts.windows(2).all(|w| w[0] < w[1])
        && s.entries
            .iter()
            .filter(|r| r.flags & REF_GROUP != 0)
            .all(|r| r.enclosing.is_some_and(|d| firsts.binary_search(&d).is_ok()))
}

impl WrittenRefs {
    fn section(&self) -> WrittenRefsSection {
        WrittenRefsSection {
            entries: self.entries.clone(),
            pending: sorted(&self.pending),
            extra: sorted(&self.extra),
            adl_sets: self.adl_sets.clone(),
            udir_cache: sorted(&self.udir_cache),
            designators: sorted(&self.designators),
            specs: self.specs.clone(),
            spec_nodes: sorted(&self.spec_nodes),
            spec_udirs: sorted(&self.spec_udirs),
            unrecorded: self.unrecorded.clone(),
            leads: self.leads.clone(),
            found_rows: self.found_rows.clone(),
            found_sets: self.found_sets.clone(),
            found_decls: self.found_decls.clone(),
            groups: self.groups.clone(),
            group_decls: self.group_decls.clone(),
            keys_on: self.subst_keys_on(),
            subst_keys: self.subst.keys.clone(),
            subst_rows: self.subst.rows.clone(),
            expanded_nodes: {
                let mut v: Vec<PatternNode> = self.subst.expanded_nodes.iter().copied().collect();
                v.sort_unstable();
                v
            },
        }
    }

    /// The keyed table a section decodes to (§18.14.5), checked against
    /// itself: the keys' runs tile the rows in order, an identity is
    /// interned once, `Unsatisfied` only on a satisfaction key, every row
    /// of a `DEPENDENT` entry in range with a known set, every expanded
    /// node a pattern node.
    fn subst_from_section(s: &mut WrittenRefsSection) -> Result<SubstTable, &'static str> {
        let n = s.entries.len();
        let mut end = 0usize;
        for k in &s.subst_keys {
            if k.rows.0 as usize != end {
                return Err("a substitution key's rows out of order or overlapping");
            }
            end = end.saturating_add(k.rows.1 as usize);
            if end > s.subst_rows.len() {
                return Err("a substitution key's rows out of range");
            }
            if k.outcome == SubstOutcome::Unsatisfied && k.kind != SubstKind::Satisfaction {
                return Err("an unsatisfied substitution key that is no satisfaction check");
            }
        }
        if end != s.subst_rows.len() {
            return Err("a keyed row of no substitution key");
        }
        let nsets = s.adl_sets.len();
        let rows_ok = s.subst_rows.iter().all(|r| {
            r.entry.index() < n
                && s.entries[r.entry.index()].flags & REF_DEPENDENT != 0
                && r.adl.is_none_or(|a| (a.0.get() as usize) <= nsets)
        });
        if !rows_ok {
            return Err("a keyed row of an entry out of range or not dependent");
        }
        let mut index = HashMap::with_capacity(s.subst_keys.len());
        for (i, k) in s.subst_keys.iter().enumerate() {
            if index.insert(k.identity(), SubstKeyId::from_index(i)).is_some() {
                return Err("a substitution key interned twice");
            }
        }
        let nodes: std::collections::HashSet<PatternNode> =
            s.spec_nodes.iter().map(|&(x, _)| x).collect();
        if !s.expanded_nodes.iter().all(|x| nodes.contains(x)) {
            return Err("an expanded node that is no pattern node");
        }
        Ok(SubstTable {
            on: s.keys_on,
            keys: std::mem::take(&mut s.subst_keys),
            rows: std::mem::take(&mut s.subst_rows),
            index,
            expanded_nodes: s.expanded_nodes.drain(..).collect(),
            pending: HashMap::new(),
            last: None,
        })
    }

    /// The table a section decodes to, checked against itself (every
    /// entry, row and set id in range, every qualifier before the entry
    /// it qualifies), with recording `on`.
    fn from_section(mut s: WrittenRefsSection, on: bool) -> Result<WrittenRefs, &'static str> {
        let subst = Self::subst_from_section(&mut s)?;
        Ok(WrittenRefs { subst, ..Self::from_section_main(s, on)? })
    }

    /// [`from_section`](Self::from_section) but the keyed table (left
    /// empty).
    fn from_section_main(s: WrittenRefsSection, on: bool) -> Result<WrittenRefs, &'static str> {
        let n = s.entries.len();
        let entry_ok = |id: WrittenRefId| id.index() < n;
        let set_ok =
            |a: Option<AdlSetId>| a.is_none_or(|a| (a.0.get() as usize) <= s.adl_sets.len());
        for (i, r) in s.entries.iter().enumerate() {
            if let QualLink::Entry(q) = r.qualifier.link()
                && q.index() >= i
            {
                return Err("a written-reference qualifier that does not precede its entry");
            }
        }
        // Leads: by entry, once each, a `::` only on a global qualifier,
        // a specifier's first token only on a specifier qualifier, a
        // closing token only on an `operator` entry, a template argument
        // list's `<` with its `>`.
        let leads_ok = s.leads.windows(2).all(|w| w[0].entry < w[1].entry)
            && s.leads.iter().all(|l| {
                entry_ok(l.entry) && {
                    let e = &s.entries[l.entry.index()];
                    let link = e.qualifier.link();
                    (!l.colon.is_valid() || link == QualLink::Global)
                        && (!l.spec.is_valid() || link == QualLink::Specifier)
                        && (!l.close.is_valid() || e.role == RefRole::Operator)
                        && l.langle.is_valid() == l.rangle.is_valid()
                }
            });
        if !leads_ok {
            return Err("a written-reference lead out of order or on the wrong entry");
        }
        let ok = s.pending.iter().all(|&(_, id)| entry_ok(id))
            && s.extra.iter().all(|&(id, x)| entry_ok(id) && set_ok(x.adl))
            && s.designators.iter().all(|&(_, id)| entry_ok(id))
            && s.spec_nodes.iter().all(|&(_, id)| entry_ok(id))
            && s.spec_udirs.iter().all(|&((id, _), _)| entry_ok(id))
            && s.specs.iter().all(|r| entry_ok(r.entry) && set_ok(r.adl));
        if !ok {
            return Err("a written-reference id out of range");
        }
        let mut adl_index = HashMap::with_capacity(s.adl_sets.len());
        for (i, set) in s.adl_sets.iter().enumerate() {
            let id = AdlSetId(std::num::NonZeroU32::new(i as u32 + 1).expect("i + 1 is never 0"));
            if adl_index.insert(set.clone(), id).is_some() {
                return Err("an argument-dependent set interned twice");
            }
        }
        // Found sets: rows by entry, once each, of known sets; each set a
        // run of the elements, interned once.
        let rows_ok = s.found_rows.windows(2).all(|w| w[0].0 < w[1].0)
            && s.found_rows.iter().all(|&(id, f)| entry_ok(id) && f.index() < s.found_sets.len());
        if !rows_ok {
            return Err("a found-set row out of order or out of range");
        }
        let mut found_index: HashMap<FoundKey, FoundSetId> =
            HashMap::with_capacity(s.found_sets.len());
        for (i, f) in s.found_sets.iter().enumerate() {
            let (lo, len) = (f.start as usize, f.len as usize);
            let Some(list) = s.found_decls.get(lo..lo.saturating_add(len)) else {
                return Err("a found set past its elements");
            };
            if found_index
                .insert((f.how, f.flags, list.into()), FoundSetId::from_index(i))
                .is_some()
            {
                return Err("a found set interned twice");
            }
        }
        if !groups_ok(&s) {
            return Err(
                "a declarator group out of order, of one, past its declarations, or a shared entry without its group",
            );
        }
        let mut spec_seen = HashMap::new();
        for (i, r) in s.specs.iter().enumerate() {
            if r.flags & SPEC_DEAD == 0 && spec_seen.insert(*r, i as u32).is_some() {
                return Err("a per-specialization row kept twice");
            }
        }
        Ok(WrittenRefs {
            on,
            no_nodes: false,
            raw_dump: false,
            entries: s.entries,
            pending: s.pending.into_iter().collect(),
            extra: s.extra.into_iter().collect(),
            adl_sets: s.adl_sets,
            adl_index,
            udir_cache: s.udir_cache.into_iter().collect(),
            designators: s.designators.into_iter().collect(),
            inst_target: None,
            specs: s.specs,
            spec_nodes: s.spec_nodes.into_iter().collect(),
            spec_udirs: s.spec_udirs.into_iter().collect(),
            found_rows: s.found_rows,
            found_sets: s.found_sets,
            found_decls: s.found_decls,
            found_index,
            spec_pending: HashMap::new(),
            spec_seen,
            spec_last: None,
            spec_key: None,
            unrecorded: s.unrecorded,
            leads: s.leads,
            groups: s.groups,
            group_decls: s.group_decls,
            ..WrittenRefs::default()
        })
    }
}

impl Ast {
    /// Writes the written-reference section after the AST (spec/ast.md
    /// §18.10.1): the table when recording is on, else the empty section
    /// (one byte).
    pub fn put_written_refs(&self, w: &mut Writer) {
        if !self.written_refs.is_on() {
            w.length(0);
            return;
        }
        let mut body = if w.is_canonical() { Writer::canonical() } else { Writer::new() };
        body.put(&self.written_refs.section());
        let bytes = body.into_bytes();
        w.length(bytes.len());
        w.bytes(&bytes);
    }

    /// Reads the section [`Ast::put_written_refs`] wrote into this AST
    /// (restored from the same snapshot), whose table then records when
    /// `on` (the restoring compilation's option). Off, the section is
    /// skipped undecoded. On, an empty section (recording was off at the
    /// snapshot) leaves the table empty with an [`Unrecorded::Prefix`]
    /// row of `files` files.
    ///
    /// # Errors
    /// A truncated or corrupt section, or one whose ids do not fit this
    /// AST (`ErrorKind::Invalid`, never a panic).
    pub fn get_written_refs(
        &mut self,
        r: &mut Reader<'_>,
        (on, keys): (bool, bool),
        files: u32,
    ) -> Result<(), SnapError> {
        let n = r.length()?;
        self.written_refs = WrittenRefs::default();
        self.written_refs.set_on(on);
        self.written_refs.set_keys_on(keys);
        if !on {
            return if n == 0 { Ok(()) } else { r.skip(n) };
        }
        if n == 0 {
            self.written_refs.note_unrecorded(Unrecorded::Prefix { files });
            return Ok(());
        }
        let bytes = r.bytes(n)?;
        let mut sub = Reader::new(bytes);
        let s: WrittenRefsSection = sub.get()?;
        let checked = self
            .check_ids(&sub)
            .and_then(|()| self.check_subst_extras(&s))
            .and_then(|()| WrittenRefs::from_section(s, on));
        let finished = sub.finish();
        match (checked, finished) {
            (Ok(mut t), Ok(())) => {
                t.restore_keys(keys, files);
                self.written_refs = t;
                Ok(())
            }
            (Err(why), _) => Err(r.invalid::<WrittenRefsSection>(why)),
            (_, Err(e)) => Err(e),
        }
    }
}

impl WrittenRefs {
    /// After a restore (§18.14.5): the consumer's sub-switch is `keys`. A
    /// table made without keys, restored by a consumer that records them,
    /// lists the prefix as [`Unrecorded::PrefixKeys`] and starts with no
    /// keys; a consumer that does not record keys keeps none (and no key
    /// gap rows).
    fn restore_keys(&mut self, keys: bool, files: u32) {
        let producer = self.subst.on;
        if !keys {
            self.subst = SubstTable::default();
            self.unrecorded.retain(|u| !u.keys_only());
            return;
        }
        if !producer {
            self.subst = SubstTable { on: true, ..SubstTable::default() };
            self.unrecorded.push(Unrecorded::PrefixKeys { files });
        }
    }
}

impl Ast {
    /// The untyped payload of each carried substitution key (§18.14.5): a
    /// `DefaultArg`'s parameter is a declaration, a `PartialMatch`'s
    /// deduced list 0 or an argument list, both in this AST's arenas.
    fn check_subst_extras(&self, s: &WrittenRefsSection) -> Result<(), &'static str> {
        let args = self.types.max_table_ids()[2];
        let ok = s.subst_keys.iter().all(|k| match k.kind {
            SubstKind::DefaultArg => k.extra != 0 && k.extra as usize <= self.decls.len(),
            SubstKind::PartialMatch => k.extra as usize <= args,
            SubstKind::Satisfaction | SubstKind::Alias => k.extra == 0,
        });
        if ok { Ok(()) } else { Err("a substitution key's payload out of range") }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::written_refs::{RefLookup, RefQualifier, RefRole};
    use kiln_source::Span;

    fn entry(q: RefQualifier) -> WrittenRef {
        WrittenRef {
            range: Span::new(Loc::from_raw(5), Loc::from_raw(6)),
            entity: None,
            found: None,
            enclosing: None,
            qualifier: q,
            role: RefRole::Expr,
            flags: 0,
            how: RefLookup::None,
        }
    }

    fn new_ast() -> Ast {
        Ast::new(
            std::sync::Arc::new(kiln_source::symbol::Interner::new()),
            crate::types::TypeContext::default(),
        )
    }

    /// The index of a new AST's translation unit, a declaration in range.
    fn ast_tu_index() -> u32 {
        new_ast().tu.index()
    }

    fn put(ast: &Ast) -> Vec<u8> {
        let mut w = Writer::new();
        ast.put_written_refs(&mut w);
        w.into_bytes()
    }

    #[test]
    fn off_writes_one_byte_and_reads_nothing() {
        let ast = new_ast();
        assert_eq!(put(&ast), vec![0]);
        let mut on = new_ast();
        on.written_refs.set_on(true);
        let _ = on.written_refs.push(entry(RefQualifier::NONE));
        let bytes = put(&on);
        assert!(bytes.len() > 1);
        // A consumer that does not record skips the section.
        let mut back = new_ast();
        let mut r = Reader::new(&bytes);
        back.get_written_refs(&mut r, (false, false), 3).expect("skipped");
        assert_eq!(r.remaining(), 0);
        assert!(back.written_refs.is_empty() && back.written_refs.unrecorded().is_empty());
        // One that does gets the entries back.
        let mut r = Reader::new(&bytes);
        back.get_written_refs(&mut r, (true, false), 3).expect("read");
        assert_eq!(back.written_refs.len(), 1);
        assert!(back.written_refs.is_on());
        // An empty section read with recording on is a noted gap.
        let mut r = Reader::new(&[0]);
        back.get_written_refs(&mut r, (true, false), 3).expect("read");
        assert_eq!(back.written_refs.unrecorded(), &[Unrecorded::Prefix { files: 3 }]);
    }

    /// spec/ast.md §18.12: the found sets round trip (the content index
    /// rebuilt); a row out of order and a set interned twice are rejected.
    #[test]
    fn found_sets_round_trip_and_are_validated() {
        use crate::written_refs::{FOUND_ADL, REF_DEPENDENT};
        let el = |d: u32| FoundDecl { decl: DeclId::new(d), found: DeclId::new(d) };
        let tu = ast_tu_index();
        let mut ast = new_ast();
        ast.written_refs.set_on(true);
        let dep = WrittenRef { flags: REF_DEPENDENT, ..entry(RefQualifier::NONE) };
        let a = ast.written_refs.push(dep).unwrap();
        let b = ast.written_refs.push(dep).unwrap();
        let s = ast.written_refs.intern_found(&[el(tu)], RefLookup::Unqualified, FOUND_ADL);
        ast.written_refs.set_found(a, s);
        ast.written_refs.set_found(b, s);
        let bytes = put(&ast);
        let mut back = new_ast();
        let mut rd = Reader::new(&bytes);
        back.get_written_refs(&mut rd, (true, false), 0).expect("read");
        assert_eq!(
            back.written_refs.found_at_definition(a),
            ast.written_refs.found_at_definition(a)
        );
        let mut section = ast.written_refs.section();
        let ok = WrittenRefs::from_section(section.clone(), true).expect("valid");
        assert_eq!(ok.found_at_definition(b), ast.written_refs.found_at_definition(b));
        let mut t = ok;
        assert_eq!(
            t.intern_found(&[el(tu)], RefLookup::Unqualified, FOUND_ADL),
            s,
            "the index is rebuilt"
        );
        section.found_rows.swap(0, 1);
        assert!(WrittenRefs::from_section(section.clone(), true).is_err(), "rows out of order");
        section.found_rows.swap(0, 1);
        section.found_sets.push(section.found_sets[0]);
        assert!(WrittenRefs::from_section(section.clone(), true).is_err(), "a set twice");
        section.found_sets.pop();
        section.found_sets[0].len = 2;
        assert!(WrittenRefs::from_section(section, true).is_err(), "a set past its elements");
    }

    /// spec/ast.md §18.3.1 (WP-3144): the declarator groups round trip;
    /// a group of one, groups out of order and a shared entry without its
    /// group are rejected.
    #[test]
    fn groups_round_trip_and_are_validated() {
        // A new AST has one declaration in range: the group repeats it.
        let tu = DeclId::new(ast_tu_index());
        let mut ast = new_ast();
        ast.written_refs.set_on(true);
        let e = WrittenRef { enclosing: Some(tu), ..entry(RefQualifier::NONE) };
        let id = ast.written_refs.push(e).unwrap();
        ast.written_refs.share_group((0, 1), &[tu, tu]);
        assert_eq!(ast.written_refs.get(id).flags, REF_GROUP);
        let bytes = put(&ast);
        let mut back = new_ast();
        let mut rd = Reader::new(&bytes);
        back.get_written_refs(&mut rd, (true, false), 0).expect("read");
        assert_eq!(back.written_refs.holders(id), &[tu, tu]);
        assert_eq!(back.written_refs.group(tu), Some(&[tu, tu][..]));
        let section = ast.written_refs.section();
        let mut one = section.clone();
        one.groups[0].len = 1;
        assert!(WrittenRefs::from_section(one, true).is_err(), "a group of one");
        let mut twice = section.clone();
        twice.groups.push(twice.groups[0]);
        assert!(WrittenRefs::from_section(twice, true).is_err(), "groups out of order");
        let mut none = section;
        none.groups.clear();
        assert!(WrittenRefs::from_section(none, true).is_err(), "a shared entry alone");
    }

    #[test]
    fn a_qualifier_after_its_entry_is_rejected() {
        let mut ast = new_ast();
        ast.written_refs.set_on(true);
        let first = WrittenRefId::from_index(0);
        let _ = ast.written_refs.push(entry(RefQualifier::entry(first)));
        let bytes = put(&ast);
        let mut r = Reader::new(&bytes);
        assert!(ast.get_written_refs(&mut r, (true, false), 0).is_err());
    }

    #[test]
    fn leads_round_trip_and_a_colon_off_a_global_is_rejected() {
        let mut ast = new_ast();
        ast.written_refs.set_on(true);
        let link = ast.written_refs.global_link(Loc::from_raw(3));
        let ea = ast.written_refs.push(entry(link)).unwrap();
        let eb = ast.written_refs.push(entry(RefQualifier::entry(ea))).unwrap();
        ast.written_refs.set_tilde(eb, Loc::from_raw(4));
        let bytes = put(&ast);
        let mut back = new_ast();
        let mut rd = Reader::new(&bytes);
        back.get_written_refs(&mut rd, (true, false), 0).expect("read");
        assert_eq!(back.written_refs.leads(), ast.written_refs.leads());
        assert_eq!(back.written_refs.global_colon(ea), Some(Loc::from_raw(3)));
        assert_eq!(back.written_refs.tilde(eb), Some(Loc::from_raw(4)));
        // A `::` on an entry that is not globally qualified.
        let mut bad = new_ast();
        bad.written_refs.set_on(true);
        let ec = bad.written_refs.push(entry(RefQualifier::NONE)).unwrap();
        bad.written_refs.set_lead(ec, Some(Loc::from_raw(3)), None, None);
        let bytes = put(&bad);
        let mut rd = Reader::new(&bytes);
        assert!(new_ast().get_written_refs(&mut rd, (true, false), 0).is_err());
    }

    /// Stage 6 (WP-2820): an `operator` entry's closing token and its
    /// rewritten flags round trip; a closing token on an entry of another
    /// role is rejected.
    #[test]
    fn operator_closes_round_trip_and_a_close_off_an_operator_is_rejected() {
        use crate::written_refs::{REF_REVERSED, REF_REWRITTEN};
        let mut ast = new_ast();
        ast.written_refs.set_on(true);
        let flags = REF_REWRITTEN | REF_REVERSED;
        let op = WrittenRef { role: RefRole::Operator, flags, ..entry(RefQualifier::NONE) };
        let eo = ast.written_refs.push(op).unwrap();
        ast.written_refs.set_close(eo, Loc::from_raw(7));
        let ud = WrittenRef { role: RefRole::UdSuffix, ..entry(RefQualifier::NONE) };
        let eu = ast.written_refs.push(ud).unwrap();
        let bytes = put(&ast);
        let mut back = new_ast();
        let mut rd = Reader::new(&bytes);
        back.get_written_refs(&mut rd, (true, false), 0).expect("read");
        assert_eq!(back.written_refs.close(eo), Some(Loc::from_raw(7)));
        assert_eq!(back.written_refs.get(eo).flags, flags);
        assert_eq!(back.written_refs.get(eu).role, RefRole::UdSuffix);
        let mut bad = new_ast();
        bad.written_refs.set_on(true);
        let ec = bad.written_refs.push(entry(RefQualifier::NONE)).unwrap();
        bad.written_refs.set_close(ec, Loc::from_raw(3));
        let bytes = put(&bad);
        let mut rd = Reader::new(&bytes);
        assert!(new_ast().get_written_refs(&mut rd, (true, false), 0).is_err());
    }

    /// WP-3008: a friend specialization's written `<` and `>` round trip
    /// beside the entry.
    #[test]
    fn template_args_leads_round_trip() {
        let mut ast = new_ast();
        ast.written_refs.set_on(true);
        let ef = ast.written_refs.push(entry(RefQualifier::NONE)).unwrap();
        let eg = ast.written_refs.push(entry(RefQualifier::NONE)).unwrap();
        ast.written_refs.set_args(ef, Loc::from_raw(5), Loc::from_raw(6));
        assert_eq!(ast.written_refs.template_args(eg), None);
        let bytes = put(&ast);
        let mut back = new_ast();
        let mut rd = Reader::new(&bytes);
        back.get_written_refs(&mut rd, (true, false), 0).expect("read");
        assert_eq!(back.written_refs.leads(), ast.written_refs.leads());
        let args = Some((Loc::from_raw(5), Loc::from_raw(6)));
        assert_eq!(back.written_refs.template_args(ef), args);
        assert_eq!(back.written_refs.template_args(eg), None);
    }

    /// spec/ast.md §18.13.6: a dependent operator's pattern nodes (both
    /// slots of a binary fold, apart from the expression's own node) and a
    /// specialization's built-in row round trip.
    #[test]
    fn operator_pattern_nodes_and_builtin_rows_round_trip() {
        use crate::expr::{ExprDeps, ExprFlags, ExprKind, ExprNode, ObjectKind, ValueCategory};
        use crate::written_refs::{PatternNode, REF_DEPENDENT, SPEC_BUILTIN, SpecRef};
        // An AST with the pattern expression the nodes name (ids are
        // range-checked against the reading AST).
        let with_expr = || {
            let mut ast = new_ast();
            let x = ast.exprs.push(ExprNode::new(
                ExprKind::IntegerLiteral,
                ValueCategory::PRValue,
                ObjectKind::Ordinary,
                ExprFlags::NONE,
                ExprDeps::NONE,
                0,
                crate::ids::QualType::unqual(crate::ids::TypeId::new(1)),
                Loc::INVALID,
                0,
                0,
            ));
            (ast, x)
        };
        let (mut ast, x) = with_expr();
        ast.written_refs.set_on(true);
        let op = WrittenRef {
            role: RefRole::Operator,
            flags: REF_DEPENDENT,
            ..entry(RefQualifier::NONE)
        };
        let first = ast.written_refs.push(op).unwrap();
        let second = ast.written_refs.push(op).unwrap();
        ast.written_refs.note_pattern_node(PatternNode::Oper(x, 0), first);
        ast.written_refs.note_pattern_node(PatternNode::Oper(x, 1), second);
        let row = SpecRef {
            entry: first,
            spec: DeclId::new(ast_tu_index()),
            entity: None,
            found: None,
            adl: None,
            udir: None,
            flags: SPEC_BUILTIN,
            how: RefLookup::Builtin,
        };
        ast.written_refs.push_spec(row);
        let bytes = put(&ast);
        let (mut back, _) = with_expr();
        let mut rd = Reader::new(&bytes);
        back.get_written_refs(&mut rd, (true, false), 0).expect("read");
        let t = &back.written_refs;
        assert_eq!(t.pattern_entry(PatternNode::Oper(x, 0)), Some(first));
        assert_eq!(t.pattern_entry(PatternNode::Oper(x, 1)), Some(second));
        assert_eq!(t.pattern_entry(PatternNode::Expr(x)), None);
        let rows: Vec<_> = t.specs_of(first).map(|(_, r)| *r).collect();
        assert_eq!(rows, [row]);
    }

    /// WP-2828: `decltype(s)::m`, the specifier's first token round
    /// trips with its `SPECIFIER` qualifier; one on an entry with another
    /// qualifier is rejected.
    #[test]
    fn a_specifier_lead_round_trips_and_one_off_a_specifier_is_rejected() {
        let mut ast = new_ast();
        ast.written_refs.set_on(true);
        let link = ast.written_refs.specifier_link(Loc::from_raw(3));
        let em = ast.written_refs.push(entry(link)).unwrap();
        let bytes = put(&ast);
        let mut back = new_ast();
        let mut rd = Reader::new(&bytes);
        back.get_written_refs(&mut rd, (true, false), 0).expect("read");
        assert_eq!(back.written_refs.leads(), ast.written_refs.leads());
        assert_eq!(back.written_refs.get(em).qualifier, RefQualifier::SPECIFIER);
        assert_eq!(back.written_refs.specifier_lo(em), Some(Loc::from_raw(3)));
        let mut bad = new_ast();
        bad.written_refs.set_on(true);
        let link = bad.written_refs.global_link(Loc::from_raw(2));
        let ec = bad.written_refs.push(entry(link)).unwrap();
        bad.written_refs.set_specifier_lo(ec, Loc::from_raw(3));
        let bytes = put(&bad);
        let mut rd = Reader::new(&bytes);
        assert!(new_ast().get_written_refs(&mut rd, (true, false), 0).is_err());
    }

    /// An AST with one argument list (`<>`), for keys (ids are
    /// range-checked against the reading AST).
    fn with_args() -> (Ast, crate::ids::ArgListId) {
        let mut ast = new_ast();
        let l = ast.types.arg_list(&[]);
        (ast, l)
    }

    /// spec/ast.md §18.14.5: three keys (one unsatisfied, one without
    /// rows, one with a payload), their rows and an expanded node round
    /// trip, the identity index rebuilt; a
    /// consumer without keys keeps none.
    #[test]
    fn subst_keys_round_trip() {
        use crate::written_refs::{REF_DEPENDENT, SubstKind as K, SubstOutcome as O};
        let tu = DeclId::new(ast_tu_index());
        let (mut ast, l) = with_args();
        ast.written_refs.set_on(true);
        ast.written_refs.set_keys_on(true);
        let dep = WrittenRef { flags: REF_DEPENDENT, ..entry(RefQualifier::NONE) };
        let a = ast.written_refs.push(dep).unwrap();
        let row = SubstRef {
            entry: a,
            entity: Some(tu),
            found: None,
            adl: None,
            udir: None,
            flags: 0,
            how: RefLookup::Unqualified,
        };
        ast.written_refs.note_pattern_node(PatternNode::Decl(tu), a);
        ast.written_refs.note_expanded_node(PatternNode::Decl(tu));
        let t = &mut ast.written_refs;
        let k1 = t.commit_subst(K::Alias, O::Substituted, tu, l, 0, &[row]).unwrap();
        let k2 = t.commit_subst(K::Satisfaction, O::Unsatisfied, tu, l, 0, &[]).unwrap();
        let k3 = t.commit_subst(K::DefaultArg, O::Substituted, tu, l, tu.index(), &[row]).unwrap();
        let bytes = put(&ast);
        let (mut back, _) = with_args();
        let mut rd = Reader::new(&bytes);
        back.get_written_refs(&mut rd, (true, true), 0).expect("read");
        let b = &back.written_refs;
        assert!(b.subst_keys_on() && b.subst_keys_complete());
        let keys: Vec<SubstKey> = b.subst_keys().map(|(_, k)| *k).collect();
        let want: Vec<SubstKey> = ast.written_refs.subst_keys().map(|(_, k)| *k).collect();
        assert_eq!(keys, want);
        assert_eq!(b.subst_rows(k1), &[row]);
        assert!(b.subst_rows(k2).is_empty());
        assert_eq!(b.subst_key(k2).outcome, O::Unsatisfied);
        assert_eq!(b.subst_rows(k3), &[row]);
        assert_eq!(b.subst_key_of(K::DefaultArg, tu, l, tu.index()), Some(k3));
        assert!(b.is_expanded_node(PatternNode::Decl(tu)));
        // A consumer that records the table but not keys: none kept.
        let (mut plain, _) = with_args();
        let mut rd = Reader::new(&bytes);
        plain.get_written_refs(&mut rd, (true, false), 0).expect("read");
        assert_eq!(plain.written_refs.subst_key_len(), 0);
        assert_eq!(plain.written_refs.subst_mem(), (0, 0));
        assert_eq!(plain.written_refs.len(), 1);
    }

    /// §18.14.5: every check of a carried keyed table.
    #[test]
    fn subst_keys_are_validated() {
        use crate::written_refs::{REF_DEPENDENT, SubstKind as K, SubstOutcome as O};
        type Corrupt = (&'static str, fn(&mut WrittenRefsSection));
        let tu = DeclId::new(ast_tu_index());
        let (mut ast, l) = with_args();
        ast.written_refs.set_on(true);
        ast.written_refs.set_keys_on(true);
        let dep = WrittenRef { flags: REF_DEPENDENT, ..entry(RefQualifier::NONE) };
        let a = ast.written_refs.push(dep).unwrap();
        let _ = ast.written_refs.push(entry(RefQualifier::NONE)).unwrap();
        let row = SubstRef {
            entry: a,
            entity: Some(tu),
            found: None,
            adl: None,
            udir: None,
            flags: 0,
            how: RefLookup::Unqualified,
        };
        ast.written_refs.note_pattern_node(PatternNode::Decl(tu), a);
        let t = &mut ast.written_refs;
        let _ = t.commit_subst(K::Alias, O::Substituted, tu, l, 0, &[row, row]);
        let _ = t.commit_subst(K::Satisfaction, O::Unsatisfied, tu, l, 0, &[]);
        let _ = t.commit_subst(K::DefaultArg, O::Substituted, tu, l, tu.index(), &[row]);
        let section = ast.written_refs.section();
        assert!(WrittenRefs::from_section(section.clone(), true).is_ok());
        let corrupt: [Corrupt; 9] = [
            ("a run that starts past the previous one's end", |s| s.subst_keys[1].rows.0 = 3),
            ("overlapping runs", |s| s.subst_keys[2].rows.0 = 1),
            ("a run past the rows", |s| s.subst_keys[2].rows.1 = 2),
            ("a row of no key", |s| s.subst_rows.push(s.subst_rows[0])),
            ("an identity twice", |s| {
                let mut k = s.subst_keys[1];
                k.rows = (3, 0);
                s.subst_keys.push(k);
            }),
            ("unsatisfied on an alias key", |s| {
                s.subst_keys[0].outcome = SubstOutcome::Unsatisfied;
            }),
            ("a row of an entry that is not dependent", |s| {
                s.subst_rows[0].entry = WrittenRefId::from_index(1);
            }),
            ("a row of an entry out of range", |s| {
                s.subst_rows[1].entry = WrittenRefId::from_index(9);
            }),
            ("an expanded node that is no pattern node", |s| {
                s.expanded_nodes.push(PatternNode::Expr(ExprId::new(1)));
            }),
        ];
        for (what, f) in corrupt {
            let mut bad = section.clone();
            f(&mut bad);
            assert!(WrittenRefs::from_section(bad, true).is_err(), "{what}");
        }
        // The untyped payloads, checked against the reading AST.
        for (k, extra) in [(2, 0), (2, u32::MAX >> 1), (0, 1)] {
            let keep = ast.written_refs.subst.keys[k].extra;
            ast.written_refs.subst.keys[k].extra = extra;
            let bytes = put(&ast);
            ast.written_refs.subst.keys[k].extra = keep;
            let (mut back, _) = with_args();
            let mut rd = Reader::new(&bytes);
            assert!(
                back.get_written_refs(&mut rd, (true, true), 0).is_err(),
                "payload {k} {extra}"
            );
        }
    }

    /// §18.14.5: a table made without keys, restored by a consumer that
    /// records them, is a key gap of the prefix's files; a table made
    /// without recording is only the whole-table gap.
    #[test]
    fn a_prefix_without_keys_is_a_key_gap() {
        let mut ast = new_ast();
        ast.written_refs.set_on(true);
        let _ = ast.written_refs.push(entry(RefQualifier::NONE));
        let bytes = put(&ast);
        let mut back = new_ast();
        let mut rd = Reader::new(&bytes);
        back.get_written_refs(&mut rd, (true, true), 3).expect("read");
        assert_eq!(back.written_refs.unrecorded(), &[Unrecorded::PrefixKeys { files: 3 }]);
        assert!(back.written_refs.subst_keys_on());
        assert!(!back.written_refs.subst_keys_complete());
        let mut rd = Reader::new(&bytes);
        back.get_written_refs(&mut rd, (true, false), 3).expect("read");
        assert!(back.written_refs.unrecorded().is_empty());
        let mut rd = Reader::new(&[0]);
        back.get_written_refs(&mut rd, (true, true), 3).expect("read");
        assert_eq!(back.written_refs.unrecorded(), &[Unrecorded::Prefix { files: 3 }]);
    }
}
