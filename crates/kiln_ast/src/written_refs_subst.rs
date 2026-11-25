//! Substitution keys (spec/ast.md §18.14, WP-3540): the keyed table of
//! [`WrittenRefs`], what each dependent entry of a template pattern bound
//! to under one completed substitution of that pattern (a partial
//! specialization match, a satisfaction check, a default template
//! argument, an alias template), keyed by the substitution itself.
//!
//! Opt-in under the table (`-fkiln-written-ref-keys`): off, nothing here
//! allocates and the recording sites test one bool. Keys are committed
//! whole, once per identity, by the substituter's key scopes
//! (`kiln_sema::written_keys`); this module is the storage, the read API,
//! the dump and the memory row.

use std::collections::HashMap;
use std::fmt::Write as _;

use kiln_source::Loc;

use super::{
    AdlSet, PatternNode, RefMark, RootSlot, RootTag, SPEC_BUILTIN, SUBST_DEPENDENT, SUBST_EXPANDED,
    SubstIdentity, SubstKey, SubstKeyId, SubstKind, SubstOutcome, SubstRef, Unrecorded, WrittenRef,
    WrittenRefId, WrittenRefs, dump_adl, dump_rewrite_flags, dump_udir,
};
use crate::ids::{ArgListId, DeclId};
use crate::mem::MemRow;
use crate::types::TypeNames;

impl WrittenRefs {
    /// True when substitution keys are recorded: the sub-switch is on and
    /// so is the table.
    #[inline]
    pub fn subst_keys_on(&self) -> bool {
        self.on && self.subst.on
    }

    /// Turns the sub-switch on or off (effective only while the table is
    /// on; keys already committed stay).
    pub fn set_keys_on(&mut self, on: bool) {
        self.subst.on = on;
    }

    /// Commits the key (`kind`, `template`, canonical `args`, `extra`)
    /// with `outcome`, `rows` as one contiguous run (possibly empty: a
    /// substitution that completed and bound nothing still has its key)
    /// and its root `root` as another (§18.14.10). `None` when keys are
    /// off or the identity is already committed (the first completed
    /// run's rows and root are the key's). The caller decides that the
    /// substitution is eligible for a key (§18.14.3).
    #[allow(clippy::too_many_arguments, reason = "a key's fields, its rows and its root")]
    pub fn commit_subst(
        &mut self,
        kind: SubstKind,
        outcome: SubstOutcome,
        template: DeclId,
        args: ArgListId,
        extra: u32,
        rows: &[SubstRef],
        root: &[RootSlot],
    ) -> Option<SubstKeyId> {
        if !self.subst_keys_on() {
            return None;
        }
        debug_assert!(
            outcome == SubstOutcome::Substituted || kind == SubstKind::Satisfaction,
            "only a satisfaction check completes unsatisfied"
        );
        let identity = SubstIdentity::new(kind, template, args, extra);
        if self.subst.index.contains_key(&identity) {
            return None;
        }
        let start = u32::try_from(self.subst.rows.len()).expect("keyed row table exhausted");
        let len = u32::try_from(rows.len()).expect("keyed row table exhausted");
        self.subst.rows.extend_from_slice(rows);
        let slot = u32::try_from(self.subst.root_slots.len()).expect("root table exhausted");
        let slots = u32::try_from(root.len()).expect("root table exhausted");
        self.subst.root_slots.extend_from_slice(root);
        self.subst.root_spans.push((slot, slots));
        let id = SubstKeyId::from_index(self.subst.keys.len());
        self.subst.keys.push(SubstKey { kind, outcome, template, args, extra, rows: (start, len) });
        self.subst.index.insert(identity, id);
        Some(id)
    }

    /// Every committed key, in commit order (not part of the contract:
    /// compare sorted).
    pub fn subst_keys(&self) -> impl Iterator<Item = (SubstKeyId, &SubstKey)> + '_ {
        self.subst.keys.iter().enumerate().map(|(i, k)| (SubstKeyId::from_index(i), k))
    }

    /// Key `id`.
    pub fn subst_key(&self, id: SubstKeyId) -> &SubstKey {
        &self.subst.keys[id.index()]
    }

    /// The number of committed keys.
    pub fn subst_key_len(&self) -> usize {
        self.subst.keys.len()
    }

    /// The committed key with this identity (`extra` counts only for a
    /// `DefaultArg`).
    pub fn subst_key_of(
        &self,
        kind: SubstKind,
        template: DeclId,
        args: ArgListId,
        extra: u32,
    ) -> Option<SubstKeyId> {
        if self.subst.index.is_empty() {
            return None;
        }
        self.subst.index.get(&SubstIdentity::new(kind, template, args, extra)).copied()
    }

    /// The rows of key `id`, in the order its substitution reached the
    /// names (§18.14.4).
    pub fn subst_rows(&self, id: SubstKeyId) -> &[SubstRef] {
        let (start, len) = self.subst.keys[id.index()].rows;
        let lo = start as usize;
        &self.subst.rows[lo..lo + len as usize]
    }

    /// The root of key `id` (§18.14.10): its slots, in the order of the
    /// key's pattern (one per written argument for a `PartialMatch`).
    pub fn subst_root(&self, id: SubstKeyId) -> &[RootSlot] {
        let (start, len) = self.subst.root_spans[id.index()];
        let lo = start as usize;
        &self.subst.root_slots[lo..lo + len as usize]
    }

    /// The recording runs of `kind` that closed without committing a key
    /// (§18.14.10): matches whose deduction succeeded and that then did
    /// not complete. It counts runs, not identities, and is this parse's
    /// own (not carried).
    pub fn subst_not_completed(&self, kind: SubstKind) -> u32 {
        self.subst.not_completed[kind as usize - 1]
    }

    /// Counts one recording run of `kind` that closed without committing
    /// (nothing when keys are off).
    pub fn note_subst_not_completed(&mut self, kind: SubstKind) {
        if self.subst_keys_on() {
            let n = &mut self.subst.not_completed[kind as usize - 1];
            *n = n.saturating_add(1);
        }
    }

    /// Every keyed row of pattern entry `entry`, with its key.
    pub fn subst_rows_of(
        &self,
        entry: WrittenRefId,
    ) -> impl Iterator<Item = (SubstKeyId, &SubstRef)> + '_ {
        self.subst_all_rows().filter(move |(_, r)| r.entry == entry)
    }

    /// Every keyed row that names `d`, as its entity or as the
    /// declaration lookup found, with its key.
    pub fn subst_rows_naming(
        &self,
        d: DeclId,
    ) -> impl Iterator<Item = (SubstKeyId, &SubstRef)> + '_ {
        self.subst_all_rows().filter(move |(_, r)| r.entity == Some(d) || r.found == Some(d))
    }

    fn subst_all_rows(&self) -> impl Iterator<Item = (SubstKeyId, &SubstRef)> + '_ {
        self.subst_keys().flat_map(move |(id, _)| self.subst_rows(id).iter().map(move |r| (id, r)))
    }

    /// A keyed row's argument-dependent set (§18.7.4).
    pub fn subst_adl_set(&self, r: &SubstRef) -> Option<&AdlSet> {
        r.adl.map(|s| self.adl_set_of(s))
    }

    /// True when every substitution the compilation completed can have
    /// its key here: no prefix or module unit is listed as unrecorded,
    /// for the table or for its keys (§18.14.5).
    pub fn subst_keys_complete(&self) -> bool {
        self.unrecorded.is_empty()
    }

    /// The pattern node `n` was reached through an alias template-id kept
    /// only as its expansion (nothing when keys are off).
    pub fn note_expanded_node(&mut self, n: PatternNode) {
        if self.subst_keys_on() {
            self.subst.expanded_nodes.insert(n);
        }
    }

    /// Whether pattern node `n` was noted by
    /// [`note_expanded_node`](Self::note_expanded_node).
    #[inline]
    pub fn is_expanded_node(&self, n: PatternNode) -> bool {
        !self.subst.expanded_nodes.is_empty() && self.subst.expanded_nodes.contains(&n)
    }

    /// The nodes noted by [`note_expanded_node`](Self::note_expanded_node),
    /// unordered.
    pub fn expanded_nodes(&self) -> impl Iterator<Item = PatternNode> + '_ {
        self.subst.expanded_nodes.iter().copied()
    }

    /// [`truncate`](Self::truncate)'s part (§18.14.5): a key with a row
    /// whose entry is dropped goes whole, the rows are compacted; an
    /// expanded node whose entry is dropped goes.
    pub(super) fn truncate_subst(&mut self, m: RefMark) {
        let n = m.entries as usize;
        let t = &mut self.subst;
        if t.keys.is_empty() && t.expanded_nodes.is_empty() {
            return;
        }
        // (The pending scratch is dropped with the entries, by `truncate`.)
        let gone = |k: &SubstKey, rows: &[SubstRef]| {
            let lo = k.rows.0 as usize;
            rows[lo..lo + k.rows.1 as usize].iter().any(|r| r.entry.index() >= n)
        };
        if t.keys.iter().any(|k| gone(k, &t.rows)) {
            let mut keys = Vec::with_capacity(t.keys.len());
            let mut rows = Vec::with_capacity(t.rows.len());
            let mut spans = Vec::with_capacity(t.root_spans.len());
            let mut slots = Vec::with_capacity(t.root_slots.len());
            for (k, &(lo, len)) in t.keys.iter().zip(&t.root_spans) {
                if gone(k, &t.rows) {
                    continue;
                }
                let r = k.rows.0 as usize;
                let start = rows.len() as u32;
                rows.extend_from_slice(&t.rows[r..r + k.rows.1 as usize]);
                keys.push(SubstKey { rows: (start, k.rows.1), ..*k });
                // Its root goes with it; the slots are compacted.
                spans.push((slots.len() as u32, len));
                slots.extend_from_slice(&t.root_slots[lo as usize..(lo + len) as usize]);
            }
            t.index = keys
                .iter()
                .enumerate()
                .map(|(i, k)| (k.identity(), SubstKeyId::from_index(i)))
                .collect();
            t.keys = keys;
            t.rows = rows;
            t.root_spans = spans;
            t.root_slots = slots;
        }
        // No key is committed inside a tentative parse that is then rolled
        // back past its template.
        debug_assert!(t.keys.iter().all(|k| k.template.index() <= m.decls));
        let nodes = &self.spec_nodes;
        t.expanded_nodes.retain(|x| nodes.get(x).is_some_and(|id| id.index() < n));
    }

    /// The bytes (live, reserved) of the keyed table (§18.14.7): keys,
    /// rows, the identity index, the expanded nodes and the pending
    /// overload sets (a hash slot counted twice its element, as in
    /// [`provenance_mem`](Self::provenance_mem)).
    pub fn subst_mem(&self) -> (u64, u64) {
        let t = &self.subst;
        let key = size_of::<SubstKey>() as u64;
        let row = size_of::<SubstRef>() as u64;
        let slot = (size_of::<(SubstIdentity, SubstKeyId)>() as u64) * 2;
        let node = (size_of::<PatternNode>() as u64) * 2;
        let pend = (size_of::<(crate::ids::ExprId, (WrittenRefId, super::ScopeRef))>() as u64) * 2;
        let live = t.keys.len() as u64 * key
            + t.rows.len() as u64 * row
            + t.index.len() as u64 * slot
            + t.expanded_nodes.len() as u64 * node
            + t.pending.len() as u64 * pend;
        let reserved = t.keys.capacity() as u64 * key
            + t.rows.capacity() as u64 * row
            + t.index.capacity() as u64 * slot
            + t.expanded_nodes.capacity() as u64 * node
            + t.pending.capacity() as u64 * pend;
        (live, reserved)
    }

    /// The memory row `written_refs.subst` (count: keyed rows), only when
    /// keys are recorded. Its bytes are not in the `written_refs` row.
    pub fn subst_mem_row(&self) -> Option<MemRow> {
        if !self.subst_keys_on() {
            return None;
        }
        let (live, reserved) = self.subst_mem();
        Some(MemRow::new("written_refs.subst", self.subst.rows.len() as u64, live, reserved))
    }

    /// The bytes (live, reserved) of the keys' roots (§18.14.10): the
    /// spans and the slots (the argument lists they name are the type
    /// context's).
    pub fn roots_mem(&self) -> (u64, u64) {
        let t = &self.subst;
        let span = size_of::<(u32, u32)>() as u64;
        let slot = size_of::<RootSlot>() as u64;
        (
            t.root_spans.len() as u64 * span + t.root_slots.len() as u64 * slot,
            t.root_spans.capacity() as u64 * span + t.root_slots.capacity() as u64 * slot,
        )
    }

    /// The memory rows `written_refs.roots` (count: slots) and
    /// `written_refs.subst.not_completed` (count: the runs of every kind
    /// that did not complete; no bytes), only when keys are recorded.
    pub fn roots_mem_rows(&self) -> Option<[MemRow; 2]> {
        if !self.subst_keys_on() {
            return None;
        }
        let (live, reserved) = self.roots_mem();
        let slots = self.subst.root_slots.len() as u64;
        let failed: u64 = self.subst.not_completed.iter().map(|&n| u64::from(n)).sum();
        Some([
            MemRow::new("written_refs.roots", slots, live, reserved),
            MemRow::new("written_refs.subst.not_completed", failed, 0, 0),
        ])
    }

    /// The keyed rows grouped by pattern entry, for the dump.
    pub(super) fn keyed_by_entry(&self) -> HashMap<WrittenRefId, Vec<(SubstKeyId, &SubstRef)>> {
        let mut m: HashMap<WrittenRefId, Vec<(SubstKeyId, &SubstRef)>> = HashMap::new();
        for (id, r) in self.subst_all_rows() {
            m.entry(r.entry).or_default().push((id, r));
        }
        m
    }

    /// The dump's name of key `k` (§18.14.6): `<kind>:<template>@<pos>`
    /// and the canonical arguments; a `PartialMatch` names the primary
    /// template at the partial specialization's position, a `DefaultArg`
    /// adds `#<parameter index>`.
    fn write_key(
        k: &SubstKey,
        ast: &crate::Ast,
        names: &crate::diag_names::DiagNames<'_>,
        pos: &dyn Fn(Loc) -> String,
        out: &mut String,
    ) {
        out.push_str(k.kind.spelling());
        out.push(':');
        let shown = match k.kind {
            SubstKind::PartialMatch => {
                ast.partial_spec_info(k.template).map_or(k.template, |(_, p)| p.template)
            }
            _ => k.template,
        };
        let mut s = String::new();
        names.write_decl_name(shown, true, &mut s);
        if s.is_empty() {
            s.push_str("(anonymous)");
        }
        out.push_str(&s);
        out.push('@');
        out.push_str(&pos(ast.decls.get(k.template).loc()));
        crate::types::write_arg_list_scoped_of(&ast.types, names, shown, k.args, out);
        if let Some(p) = k.parameter() {
            let i = ast.template_parm(p).map_or(0, |i| i.index);
            let _ = write!(out, "#{i}");
        }
    }

    /// The keyed lines of entry `r` (`rows`), each starting with `head`
    /// (the entry's positions, role and spelling), sorted and without
    /// duplicates (§18.14.6).
    #[allow(clippy::too_many_arguments, reason = "the dump's shared printers")]
    pub(super) fn dump_keyed(
        &self,
        r: &WrittenRef,
        rows: &[(SubstKeyId, &SubstRef)],
        head: &str,
        ast: &crate::Ast,
        names: &crate::diag_names::DiagNames<'_>,
        pos: &dyn Fn(Loc) -> String,
        decl: &dyn Fn(DeclId, &mut String),
        out: &mut String,
    ) {
        let mut lines: Vec<String> = rows
            .iter()
            .map(|&(id, sr)| {
                let k = self.subst_key(id);
                let mut l = head.to_owned();
                match sr.entity {
                    Some(d) => decl(d, &mut l),
                    None if sr.flags & SUBST_DEPENDENT != 0 => l.push_str("(dependent)"),
                    None if sr.flags & SPEC_BUILTIN != 0 => l.push_str("(builtin)"),
                    None => l.push_str("(unresolved)"),
                }
                if let Some(f) = sr.found {
                    l.push_str(" found=");
                    decl(f, &mut l);
                }
                l.push_str(" key=");
                Self::write_key(k, ast, names, pos, &mut l);
                dump_rewrite_flags(sr.flags, &mut l);
                if sr.how != sr.default_how(r) {
                    l.push_str(" how=");
                    l.push_str(sr.how.spelling());
                }
                if let Some(u) = sr.udir {
                    dump_udir(u, ast, names, pos, &mut l);
                }
                if let Some(set) = self.subst_adl_set(sr) {
                    dump_adl(set, ast, pos, decl, &mut l);
                }
                if k.outcome == SubstOutcome::Unsatisfied {
                    l.push_str(" unsatisfied");
                }
                if sr.flags & SUBST_EXPANDED != 0 {
                    l.push_str(" expanded");
                }
                l.push('\n');
                l
            })
            .collect();
        lines.sort();
        lines.dedup();
        for l in lines {
            out.push_str(&l);
        }
    }

    /// The `keys` block (§18.14.6), after the entries: per key one line
    /// `keys  key=<...> rows=<n>`, then one line per slot of its root
    /// (`root[<i>]=<text>`, as written: [`Self::write_root_slot`]); the
    /// blocks sorted by their first line, each once.
    pub(super) fn dump_key_blocks(
        &self,
        ast: &crate::Ast,
        canon: &crate::diag_names::DiagNames<'_>,
        written: &crate::diag_names::DiagNames<'_>,
        pos: &dyn Fn(Loc) -> String,
        out: &mut String,
    ) {
        let mut blocks: Vec<(String, String)> = self
            .subst_keys()
            .map(|(id, k)| {
                let mut key = String::from("key=");
                Self::write_key(k, ast, canon, pos, &mut key);
                let mut l = format!("keys  {key} rows={}", k.rows.1);
                if k.outcome == SubstOutcome::Unsatisfied {
                    l.push_str(" unsatisfied");
                }
                l.push('\n');
                let mut body = String::new();
                for (i, slot) in self.subst_root(id).iter().enumerate() {
                    let _ = write!(body, "keys  {key} root[{i}]=");
                    Self::write_root_slot(slot, ast, written, &mut body);
                    body.push('\n');
                }
                (l, body)
            })
            .collect();
        blocks.sort();
        blocks.dedup();
        for (l, body) in blocks {
            out.push_str(&l);
            out.push_str(&body);
        }
    }

    /// A root slot's dump text (§18.14.6): an `Args` slot's elements as
    /// written (not canonical), comma-separated, `<empty>` for none; `-`
    /// for `NotReached`, `failed` for `Failed`; an `Expr` or `Type` slot
    /// as written. No source position.
    fn write_root_slot(
        slot: &RootSlot,
        ast: &crate::Ast,
        names: &crate::diag_names::DiagNames<'_>,
        out: &mut String,
    ) {
        match slot.tag {
            RootTag::NotReached => out.push('-'),
            RootTag::Failed => out.push_str("failed"),
            RootTag::Args => match slot.as_args() {
                Some(l) if !ast.types.args(l).is_empty() => {
                    let mut s = String::new();
                    crate::types::write_arg_list_scoped(&ast.types, names, l, &mut s);
                    // `<a, b>` without its brackets.
                    let inner = s
                        .strip_prefix('<')
                        .and_then(|x| x.strip_suffix('>'))
                        .unwrap_or(&s)
                        .trim_end_matches(' ');
                    out.push_str(inner);
                }
                _ => out.push_str("<empty>"),
            },
            RootTag::Type => match slot.as_type() {
                Some(q) => crate::types::write_type(
                    &ast.types,
                    names,
                    q,
                    crate::types::PrintOptions::default(),
                    out,
                ),
                None => out.push('?'),
            },
            RootTag::Expr => match slot.as_expr() {
                Some(e) => names.write_expr(e, out),
                None => out.push('?'),
            },
        }
    }

    /// Notes a gap of the keyed table (nothing when keys are off, or when
    /// the table already lists the source as unrecorded as a whole).
    pub fn note_unrecorded_keys(&mut self, u: Unrecorded) {
        debug_assert!(u.keys_only());
        if self.subst_keys_on() {
            self.unrecorded.push(u);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::written_refs::{REF_DEPENDENT, RefLookup, RefQualifier, RefRole};
    use kiln_source::Span;

    fn table(keys: bool) -> WrittenRefs {
        let mut t = WrittenRefs::default();
        t.set_on(true);
        t.set_keys_on(keys);
        t
    }

    fn dep_entry(t: &mut WrittenRefs) -> WrittenRefId {
        t.push(WrittenRef {
            range: Span::new(Loc::from_raw(5), Loc::from_raw(6)),
            entity: None,
            found: None,
            enclosing: None,
            qualifier: RefQualifier::NONE,
            role: RefRole::Expr,
            flags: REF_DEPENDENT,
            how: RefLookup::None,
        })
        .expect("on")
    }

    fn row(entry: WrittenRefId, entity: u32) -> SubstRef {
        SubstRef {
            entry,
            entity: Some(DeclId::new(entity)),
            found: None,
            adl: None,
            udir: None,
            flags: 0,
            how: RefLookup::Unqualified,
        }
    }

    fn args(n: u32) -> ArgListId {
        ArgListId::new(n)
    }

    /// spec/ast.md §18.14.1: a key and a keyed row are 24 bytes each.
    #[test]
    fn sizes() {
        assert_eq!(size_of::<SubstKey>(), 24);
        assert_eq!(size_of::<SubstRef>(), 24);
        // §18.14.10: a root slot is 12 bytes, a root's span 8.
        assert_eq!(size_of::<RootSlot>(), 12);
    }

    /// §18.14.10: a root is committed with its key, one span per key (an
    /// empty root too), read back by `subst_root`; a refused commit keeps
    /// no slot; a rollback keeps the kept keys' roots and drops the
    /// others'; the memory row counts slots.
    #[test]
    fn roots_are_committed_with_their_keys() {
        let mut t = table(true);
        let first = dep_entry(&mut t);
        let mark = t.mark(5);
        let second = dep_entry(&mut t);
        let (pm, done) = (SubstKind::PartialMatch, SubstOutcome::Substituted);
        let r1 = [RootSlot::args(args(4)), RootSlot::args(args(5))];
        let r3 = [
            RootSlot::NOT_REACHED,
            RootSlot::FAILED,
            RootSlot::ty(crate::ids::QualType::unqual(crate::ids::TypeId::new(12))),
        ];
        let k1 = t.commit_subst(pm, done, DeclId::new(3), args(1), 0, &[row(second, 7)], &r1);
        let k2 = t.commit_subst(pm, done, DeclId::new(3), args(2), 0, &[], &[]);
        let k3 = t.commit_subst(pm, done, DeclId::new(3), args(3), 0, &[row(first, 8)], &r3);
        let (k1, k2, k3) = (k1.unwrap(), k2.unwrap(), k3.unwrap());
        assert_eq!(t.subst_root(k1), &r1);
        assert!(t.subst_root(k2).is_empty());
        assert_eq!(t.subst_root(k3), &r3);
        assert_eq!(t.subst_root(k1)[1].as_args(), Some(args(5)));
        assert_eq!(t.subst_root(k3)[0].as_args(), None);
        assert_eq!(t.commit_subst(pm, done, DeclId::new(3), args(1), 0, &[], &r3), None);
        let rows = t.roots_mem_rows().expect("keys on");
        assert_eq!((rows[0].name.as_str(), rows[0].count), ("written_refs.roots", 5));
        assert_eq!(rows[0].live_bytes, 3 * 8 + 5 * 12);
        // Rolled back past the second entry: the first key goes, its root
        // with it.
        t.truncate(mark);
        let kept: Vec<(u32, Vec<RootSlot>)> =
            t.subst_keys().map(|(id, k)| (k.args.index(), t.subst_root(id).to_vec())).collect();
        assert_eq!(kept, [(2, vec![]), (3, r3.to_vec())]);
        assert_eq!(t.roots_mem_rows().expect("keys on")[0].count, 3);
    }

    /// §18.14.10: the runs that did not complete are counted by kind (keys
    /// on only), into a row without bytes.
    #[test]
    fn not_completed_runs_are_counted() {
        let mut t = table(true);
        t.note_subst_not_completed(SubstKind::PartialMatch);
        t.note_subst_not_completed(SubstKind::PartialMatch);
        t.note_subst_not_completed(SubstKind::Alias);
        assert_eq!(t.subst_not_completed(SubstKind::PartialMatch), 2);
        assert_eq!(t.subst_not_completed(SubstKind::Alias), 1);
        assert_eq!(t.subst_not_completed(SubstKind::DefaultArg), 0);
        let rows = t.roots_mem_rows().expect("keys on");
        assert_eq!(
            (rows[1].name.as_str(), rows[1].count, rows[1].live_bytes),
            ("written_refs.subst.not_completed", 3, 0)
        );
        let mut off = table(false);
        off.note_subst_not_completed(SubstKind::PartialMatch);
        assert_eq!(off.subst_not_completed(SubstKind::PartialMatch), 0);
        assert!(off.roots_mem_rows().is_none());
    }

    /// §18.14.4: a commit makes one contiguous run, read back by
    /// `subst_rows`; an empty run is a key too, between two keys with rows
    /// and as the last key; an identity is committed once.
    #[test]
    fn commit_makes_one_run_per_key_and_refuses_a_second() {
        let mut t = table(true);
        let (first, second) = (dep_entry(&mut t), dep_entry(&mut t));
        let alias = SubstKind::Alias;
        let done = SubstOutcome::Substituted;
        let k1 = t.commit_subst(
            alias,
            done,
            DeclId::new(3),
            args(1),
            0,
            &[row(first, 7), row(second, 8)],
            &[],
        );
        let k2 = t.commit_subst(SubstKind::DefaultArg, done, DeclId::new(3), args(1), 9, &[], &[]);
        let k3 = t.commit_subst(alias, done, DeclId::new(3), args(2), 0, &[row(second, 9)], &[]);
        let k4 =
            t.commit_subst(SubstKind::PartialMatch, done, DeclId::new(4), args(2), 5, &[], &[]);
        let (k1, k2, k3, k4) = (k1.unwrap(), k2.unwrap(), k3.unwrap(), k4.unwrap());
        assert_eq!(t.subst_rows(k1), &[row(first, 7), row(second, 8)]);
        assert!(t.subst_rows(k2).is_empty());
        assert_eq!(t.subst_rows(k3), &[row(second, 9)]);
        assert!(t.subst_rows(k4).is_empty());
        assert_eq!(t.subst_key_len(), 4);
        assert_eq!(t.subst_key(k2).parameter(), Some(DeclId::new(9)));
        assert_eq!(t.subst_key(k4).deduced(), Some(args(5)));
        // Refused: the same identity, whatever the rows; a `PartialMatch`'s
        // payload is not part of its identity, a `DefaultArg`'s is.
        assert_eq!(t.commit_subst(alias, done, DeclId::new(3), args(1), 0, &[], &[]), None);
        assert_eq!(
            t.commit_subst(SubstKind::PartialMatch, done, DeclId::new(4), args(2), 6, &[], &[]),
            None
        );
        assert!(
            t.commit_subst(SubstKind::DefaultArg, done, DeclId::new(3), args(1), 10, &[], &[])
                .is_some()
        );
        assert_eq!(t.subst_key_of(alias, DeclId::new(3), args(2), 0), Some(k3));
        assert_eq!(t.subst_key_of(SubstKind::PartialMatch, DeclId::new(4), args(2), 77), Some(k4));
        assert_eq!(t.subst_rows_of(second).map(|(id, _)| id).collect::<Vec<_>>(), [k1, k3]);
        assert_eq!(t.subst_rows_naming(DeclId::new(8)).count(), 1);
        let ids: Vec<SubstKeyId> = t.subst_keys().map(|(id, _)| id).collect();
        assert_eq!(ids.len(), 5);
    }

    /// §18.14.5: rolling back the table drops every key with a dropped
    /// entry's row whole, compacts the rows, keeps the others' ids
    /// readable, and drops the expanded nodes of dropped entries.
    #[test]
    fn truncate_drops_whole_keys() {
        let mut t = table(true);
        let first = dep_entry(&mut t);
        let mark = t.mark(5);
        let second = dep_entry(&mut t);
        let done = SubstOutcome::Substituted;
        let alias = SubstKind::Alias;
        let _ = t.commit_subst(
            alias,
            done,
            DeclId::new(3),
            args(1),
            0,
            &[row(first, 7), row(second, 8)],
            &[],
        );
        let _ = t.commit_subst(alias, done, DeclId::new(3), args(2), 0, &[], &[]);
        let _ = t.commit_subst(alias, done, DeclId::new(3), args(3), 0, &[row(first, 9)], &[]);
        t.note_pattern_node(PatternNode::Decl(DeclId::new(2)), second);
        t.note_expanded_node(PatternNode::Decl(DeclId::new(2)));
        t.truncate(mark);
        assert_eq!(t.subst_key_len(), 2);
        let kept: Vec<(u32, Vec<SubstRef>)> =
            t.subst_keys().map(|(id, key)| (key.args.index(), t.subst_rows(id).to_vec())).collect();
        assert_eq!(kept, [(2, vec![]), (3, vec![row(first, 9)])]);
        assert_eq!(
            t.subst_key_of(alias, DeclId::new(3), args(3), 0).map(SubstKeyId::index),
            Some(1)
        );
        assert_eq!(t.subst_key_of(alias, DeclId::new(3), args(1), 0), None);
        assert!(!t.is_expanded_node(PatternNode::Decl(DeclId::new(2))));
    }

    /// §18.14.7: with the table on and keys off nothing is kept or
    /// allocated, and there is no memory row; keys on, the row is there
    /// with 0 rows until a key with rows commits.
    #[test]
    fn keys_off_allocates_nothing() {
        let mut t = table(false);
        let a = dep_entry(&mut t);
        let s = SubstOutcome::Substituted;
        assert_eq!(
            t.commit_subst(SubstKind::Alias, s, DeclId::new(3), args(1), 0, &[row(a, 7)], &[]),
            None
        );
        t.note_expanded_node(PatternNode::Decl(DeclId::new(2)));
        assert_eq!(t.subst_mem(), (0, 0));
        assert!(t.subst_mem_row().is_none());
        // The sub-switch needs the table.
        let mut off = WrittenRefs::default();
        off.set_keys_on(true);
        assert!(!off.subst_keys_on());
        let on = table(true);
        let r = on.subst_mem_row().expect("keys on");
        assert_eq!((r.name.as_str(), r.count, r.live_bytes), ("written_refs.subst", 0, 0));
    }

    /// §18.14.5: the key gaps are listed (keys on only) and make the keyed
    /// table incomplete; they do not make the main table's gap rows.
    #[test]
    fn key_gaps_are_listed() {
        let mut t = table(true);
        assert!(t.subst_keys_complete());
        t.note_unrecorded_keys(Unrecorded::PrefixKeys { files: 2 });
        t.set_prefix_files(4);
        assert_eq!(t.unrecorded(), &[Unrecorded::PrefixKeys { files: 4 }]);
        assert!(!t.subst_keys_complete());
        assert!(t.unrecorded()[0].keys_only());
        let mut off = table(false);
        off.note_unrecorded_keys(Unrecorded::ModuleKeysLeftOut {
            module: "M".into(),
            keys: 1,
            rows: 2,
        });
        assert!(off.unrecorded().is_empty());
    }
}
