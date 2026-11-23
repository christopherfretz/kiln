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
//! | `leads` | state: the leading `::` and `~` tokens (WP-2326) |
//! | `adl_index`, `spec_seen` | rebuilt from `adl_sets` and the live rows of `specs` |
//! | `inst_target`, `spec_pending`, `spec_last`, `spec_key`, `colons` | transient: scratch of one declaration or one instantiation, empty at rest |
//! | `on` | the restoring compilation's own option |

use std::collections::HashMap;

use kiln_snap::{Reader, SnapError, Writer};
use kiln_source::Loc;

use super::{
    AdlSet, AdlSetId, PatternNode, QualLink, RefExtra, RefLead, SPEC_DEAD, SpecRef, Unrecorded,
    WrittenRef, WrittenRefId, WrittenRefs,
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
}

/// The schema hash of the section's payload, which `Sema`'s snapshot root
/// folds after the AST's.
pub const CARRY_SCHEMA_HASH: u64 = <WrittenRefsSection as kiln_snap::Snap>::SCHEMA_HASH;

fn sorted<K: Ord + Copy, V: Copy>(m: &HashMap<K, V>) -> Vec<(K, V)> {
    let mut v: Vec<(K, V)> = m.iter().map(|(&k, &v)| (k, v)).collect();
    v.sort_unstable_by_key(|&(k, _)| k);
    v
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
        }
    }

    /// The table a section decodes to, checked against itself (every
    /// entry, row and set id in range, every qualifier before the entry
    /// it qualifies), with recording `on`.
    fn from_section(s: WrittenRefsSection, on: bool) -> Result<WrittenRefs, &'static str> {
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
        // Leads: by entry, once each, a `::` only on a global qualifier.
        let leads_ok = s.leads.windows(2).all(|w| w[0].entry < w[1].entry)
            && s.leads.iter().all(|l| {
                entry_ok(l.entry)
                    && (!l.colon.is_valid()
                        || s.entries[l.entry.index()].qualifier.link() == QualLink::Global)
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
        let mut spec_seen = HashMap::new();
        for (i, r) in s.specs.iter().enumerate() {
            if r.flags & SPEC_DEAD == 0 && spec_seen.insert(*r, i as u32).is_some() {
                return Err("a per-specialization row kept twice");
            }
        }
        Ok(WrittenRefs {
            on,
            no_nodes: false,
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
            spec_pending: HashMap::new(),
            spec_seen,
            spec_last: None,
            spec_key: None,
            unrecorded: s.unrecorded,
            leads: s.leads,
            colons: Vec::new(),
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
        on: bool,
        files: u32,
    ) -> Result<(), SnapError> {
        let n = r.length()?;
        self.written_refs = WrittenRefs::default();
        self.written_refs.set_on(on);
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
        let checked = self.check_ids(&sub).and_then(|()| WrittenRefs::from_section(s, on));
        let finished = sub.finish();
        match (checked, finished) {
            (Ok(t), Ok(())) => {
                self.written_refs = t;
                Ok(())
            }
            (Err(why), _) => Err(r.invalid::<WrittenRefsSection>(why)),
            (_, Err(e)) => Err(e),
        }
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
        back.get_written_refs(&mut r, false, 3).expect("skipped");
        assert_eq!(r.remaining(), 0);
        assert!(back.written_refs.is_empty() && back.written_refs.unrecorded().is_empty());
        // One that does gets the entries back.
        let mut r = Reader::new(&bytes);
        back.get_written_refs(&mut r, true, 3).expect("read");
        assert_eq!(back.written_refs.len(), 1);
        assert!(back.written_refs.is_on());
        // An empty section read with recording on is a noted gap.
        let mut r = Reader::new(&[0]);
        back.get_written_refs(&mut r, true, 3).expect("read");
        assert_eq!(back.written_refs.unrecorded(), &[Unrecorded::Prefix { files: 3 }]);
    }

    #[test]
    fn a_qualifier_after_its_entry_is_rejected() {
        let mut ast = new_ast();
        ast.written_refs.set_on(true);
        let first = WrittenRefId::from_index(0);
        let _ = ast.written_refs.push(entry(RefQualifier::entry(first)));
        let bytes = put(&ast);
        let mut r = Reader::new(&bytes);
        assert!(ast.get_written_refs(&mut r, true, 0).is_err());
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
        back.get_written_refs(&mut rd, true, 0).expect("read");
        assert_eq!(back.written_refs.leads(), ast.written_refs.leads());
        assert_eq!(back.written_refs.global_colon(ea), Some(Loc::from_raw(3)));
        assert_eq!(back.written_refs.tilde(eb), Some(Loc::from_raw(4)));
        // A `::` on an entry that is not globally qualified.
        let mut bad = new_ast();
        bad.written_refs.set_on(true);
        let ec = bad.written_refs.push(entry(RefQualifier::NONE)).unwrap();
        bad.written_refs.set_lead(ec, Some(Loc::from_raw(3)), None);
        let bytes = put(&bad);
        let mut rd = Reader::new(&bytes);
        assert!(new_ast().get_written_refs(&mut rd, true, 0).is_err());
    }
}
