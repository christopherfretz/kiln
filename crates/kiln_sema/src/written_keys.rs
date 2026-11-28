//! Key scopes of the written-reference table's substitution keys
//! (spec/ast.md §18.14.3, WP-3540).
//!
//! A key scope is the transient record of one substitution of a template
//! pattern in progress (a partial specialization match, a satisfaction
//! check, a default template argument, an alias template) with concrete
//! arguments. It is opened where the substitution's frame gets its
//! arguments and closed where its outcome is known. While it is the
//! innermost row target ([`Sema::written_row_keys`]) the rows of the
//! dependent names its substitution resolves go to its private buffer;
//! at a completed close the buffer is committed as the key's rows, once
//! per identity, and on failure it is dropped. A scope opened for an
//! identity already committed, or already open further out, records
//! nothing (`Skip`): the first completed run's rows are the key's.
//!
//! With `-fkiln-written-ref-keys` off every entry point returns after one
//! bool test, and the scope stack, its pool and the table stay empty.
//!
//! The substitution sites open their scopes kind by kind (§18.14.3):
//! partial specialization matching (`templ/order.rs`) is the first. A
//! scope also holds its key's root (§18.14.10), set once by the site and
//! committed with the rows.

use kiln_ast::written_refs::{
    PatternNode, RootSlot, SUBST_DEPENDENT, ScopeRef, SubstKeyId, SubstKind, SubstOutcome,
    SubstRef, WrittenRefId,
};
use kiln_ast::{ArgListId, DeclId, RawId};

use crate::Sema;
use crate::templ::stack::FrameKind;

/// An open key scope: its index on the scope stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KeyScopeId(pub(crate) u32);

/// Whether a scope records rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyMode {
    /// The first run of its identity: rows go to its buffer.
    Record,
    /// The identity is committed, or open further out: nothing recorded.
    Skip,
}

/// One substitution in progress (§18.14.3).
#[derive(Debug)]
pub(crate) struct KeyScope {
    pub(crate) kind: SubstKind,
    /// The template (first declaration) the key names.
    pub(crate) template: DeclId,
    /// The canonical arguments.
    pub(crate) args: ArgListId,
    /// A `DefaultArg`'s parameter (raw), else 0: part of the identity.
    pub(crate) extra: u32,
    /// A `PartialMatch`'s deduced argument list (the key's payload).
    pub(crate) payload: Option<ArgListId>,
    /// The instantiation frame that owns the scope (its index), or
    /// `u32::MAX` when none was on the stack.
    pub(crate) frame: u32,
    /// The declaration context at the open: a context built inside the
    /// substitution other than this one is a pattern of its own
    /// ([`Sema::written_ctx_dependent`]).
    pub(crate) home_ctx: DeclId,
    pub(crate) mode: KeyMode,
    /// A name of the pattern failed to resolve: the substitution fails.
    pub(crate) poisoned: bool,
    /// The error count at the open: an error diagnosed inside the scope
    /// keeps the key from being committed.
    pub(crate) errors: u32,
    /// Tells the scope from a later one at the same index.
    pub(crate) serial: u32,
    /// The rows, in the order the substitution reached the names.
    pub(crate) rows: Vec<SubstRef>,
    /// The key's root (§18.14.10), set once by the opening site.
    pub(crate) root: Vec<RootSlot>,
}

/// The scope stack and its buffer pool (a field of [`Sema`], transient).
#[derive(Debug, Default)]
pub(crate) struct KeyScopes {
    pub(crate) scopes: Vec<KeyScope>,
    /// The open suspensions, innermost last: each the number of scopes
    /// it silences (those open when it began, and those an outer
    /// suspension silences). A scope opened during a suspension records
    /// (a check inside a substitution that is not part of it, such as
    /// partial ordering, can itself run a keyed substitution).
    pub(crate) suspend: Vec<u32>,
    /// The declaration images being built (§18.14.5): a match their
    /// records re-run is the unit's own, whose key the image carries, so
    /// a scope opened meanwhile commits nothing.
    pub(crate) image_builds: u32,
    /// The next scope's serial.
    serial: u32,
    /// Cleared buffers for the next scopes.
    pool: Vec<Vec<SubstRef>>,
    /// Cleared root buffers for the next scopes.
    root_pool: Vec<Vec<RootSlot>>,
}

impl KeyScopes {
    /// No scope open and none suspended (the pool is scratch).
    pub(crate) fn at_rest(&self) -> bool {
        self.scopes.is_empty() && self.suspend.is_empty() && self.image_builds == 0
    }

    /// The scopes below this index are silenced by a suspension.
    fn silenced(&self) -> u32 {
        self.suspend.last().copied().unwrap_or(0)
    }

    /// The scope `r` names, while it is open and records.
    fn recording(&self, r: ScopeRef) -> Option<&KeyScope> {
        let s = self.scopes.get(r.index as usize)?;
        (s.serial == r.serial && s.mode == KeyMode::Record && r.index >= self.silenced())
            .then_some(s)
    }

    /// Whether the scope `r` names is open and records.
    pub(crate) fn records(&self, r: ScopeRef) -> bool {
        self.recording(r).is_some()
    }

    /// Row `index` of the buffer of the scope `r` names, while it records.
    pub(crate) fn row(&self, r: ScopeRef, index: u32) -> Option<SubstRef> {
        self.recording(r)?.rows.get(index as usize).copied()
    }

    /// The declaration context the scope `r` names was opened in, while
    /// it is open.
    pub(crate) fn home_ctx(&self, r: ScopeRef) -> Option<DeclId> {
        let s = self.scopes.get(r.index as usize)?;
        (s.serial == r.serial).then_some(s.home_ctx)
    }
}

/// Where the rows of a dependent name resolved now go (§18.14.3): the
/// per-specialization table's key (§18.9.2) and the key scope that
/// records, either or both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RowKeys {
    pub(crate) spec: Option<DeclId>,
    pub(crate) key: Option<ScopeRef>,
}

impl Sema<'_, '_> {
    /// Opens a key scope for the substitution of `template`'s pattern with
    /// the canonical arguments `args` (§18.14.3), owned by the innermost
    /// instantiation frame. `extra` is a `DefaultArg`'s parameter (0 for
    /// the other kinds), `payload` a `PartialMatch`'s deduced list. `None`
    /// when keys are off.
    pub(crate) fn key_scope_open(
        &mut self,
        kind: SubstKind,
        template: DeclId,
        args: ArgListId,
        extra: u32,
        payload: Option<ArgListId>,
    ) -> Option<KeyScopeId> {
        if !self.ast.written_refs.subst_keys_on() {
            return None;
        }
        let extra = if kind == SubstKind::DefaultArg { extra } else { 0 };
        let committed = self.ast.written_refs.subst_key_of(kind, template, args, extra).is_some();
        let open = self.key_scopes.scopes.iter().any(|s| {
            s.kind == kind && s.template == template && s.args == args && s.extra == extra
        });
        let image = self.key_scopes.image_builds != 0;
        let mode = if committed || open || image { KeyMode::Skip } else { KeyMode::Record };
        let index = self.key_scopes.scopes.len();
        let id = u32::try_from(index).expect("key scope stack exhausted");
        let frame = match self.inst.stack.frames.last_mut() {
            Some(f) => {
                f.key_scope = id + 1;
                u32::try_from(self.inst.stack.frames.len() - 1).unwrap_or(u32::MAX)
            }
            None => u32::MAX,
        };
        let serial = self.key_scopes.serial;
        self.key_scopes.serial = serial.wrapping_add(1);
        let rows = self.key_scopes.pool.pop().unwrap_or_default();
        let root = self.key_scopes.root_pool.pop().unwrap_or_default();
        self.key_scopes.scopes.push(KeyScope {
            kind,
            template,
            args,
            extra,
            payload,
            frame,
            home_ctx: self.ctx,
            mode,
            poisoned: false,
            errors: self.diags.error_count(),
            serial,
            rows,
            root,
        });
        Some(KeyScopeId(id))
    }

    /// Whether scope `id` records (its site builds a root only then):
    /// open in `Record` mode and not silenced by a suspension.
    pub(crate) fn key_scope_records(&self, id: KeyScopeId) -> bool {
        self.key_scopes
            .scopes
            .get(id.0 as usize)
            .is_some_and(|s| s.mode == KeyMode::Record && id.0 >= self.key_scopes.silenced())
    }

    /// Sets the root of scope `id` (§18.14.10): all its slots, once, by
    /// the site that opened it. Nothing for a scope that does not record.
    pub(crate) fn key_scope_set_root(&mut self, id: KeyScopeId, slots: &[RootSlot]) {
        if !self.key_scope_records(id) {
            return;
        }
        let root = &mut self.key_scopes.scopes[id.0 as usize].root;
        root.clear();
        root.extend_from_slice(slots);
    }

    /// Closes scope `id` (scopes close in stack order), the substitution
    /// having completed (`ok`) or failed (§18.14.3). It commits the key
    /// with its rows (possibly none) when it completed, recorded, was not
    /// poisoned and no error was diagnosed since the open; otherwise the
    /// rows are dropped. Whether a substitution gets a key at all is the
    /// opening site's judgement. Returns the key committed.
    pub(crate) fn key_scope_close(&mut self, id: KeyScopeId, ok: bool) -> Option<SubstKeyId> {
        self.key_scope_close_as(id, ok, SubstOutcome::Substituted)
    }

    /// [`Self::key_scope_close`] with the outcome of a completed run
    /// (`Unsatisfied` for a satisfaction check that completed unsatisfied).
    pub(crate) fn key_scope_close_as(
        &mut self,
        id: KeyScopeId,
        ok: bool,
        outcome: SubstOutcome,
    ) -> Option<SubstKeyId> {
        let index = id.0 as usize;
        debug_assert_eq!(
            index + 1,
            self.key_scopes.scopes.len(),
            "key scopes close in stack order"
        );
        // An inner scope left open fails with this one.
        while self.key_scopes.scopes.len() > index + 1 {
            let inner = KeyScopeId((self.key_scopes.scopes.len() - 1) as u32);
            let _ = self.key_scope_close_as(inner, false, SubstOutcome::Substituted);
        }
        let mut s = self.key_scopes.scopes.pop()?;
        if let Some(f) = self.inst.stack.frames.get_mut(s.frame as usize)
            && f.key_scope == id.0 + 1
        {
            f.key_scope = 0;
        }
        let commit =
            ok && s.mode == KeyMode::Record && !s.poisoned && self.diags.error_count() == s.errors;
        let key = if commit {
            let extra = match s.kind {
                SubstKind::PartialMatch => s.payload.map_or(0, RawId::raw),
                _ => s.extra,
            };
            self.ast
                .written_refs
                .commit_subst(s.kind, outcome, s.template, s.args, extra, &s.rows, &s.root)
        } else {
            None
        };
        // A recording run that committed nothing (§18.14.10).
        if s.mode == KeyMode::Record && key.is_none() {
            self.ast.written_refs.note_subst_not_completed(s.kind);
        }
        s.rows.clear();
        s.root.clear();
        self.key_scopes.pool.push(std::mem::take(&mut s.rows));
        self.key_scopes.root_pool.push(std::mem::take(&mut s.root));
        key
    }

    /// Adds `row` to the buffer of scope `scope` (kept once: an equal row
    /// already there gives its index). Nothing for a scope that does not
    /// record or while scopes are suspended.
    pub(crate) fn key_scope_push(&mut self, scope: ScopeRef, row: SubstRef) -> Option<u32> {
        self.key_scopes.recording(scope)?;
        let s = &mut self.key_scopes.scopes[scope.index as usize];
        let row = SubstRef { flags: row.flags | scope.flags, ..row };
        if let Some(i) = s.rows.iter().position(|r| *r == row) {
            return u32::try_from(i).ok();
        }
        s.rows.push(row);
        u32::try_from(s.rows.len() - 1).ok()
    }

    /// Changes row `index` of scope `scope` by `f` (an overload
    /// resolution's facts); when another row of the buffer then equals
    /// it, it is dropped and that one's index returned.
    pub(crate) fn key_scope_update(
        &mut self,
        scope: ScopeRef,
        index: u32,
        f: impl FnOnce(&mut SubstRef),
    ) -> Option<u32> {
        self.key_scopes.recording(scope)?;
        let rows = &mut self.key_scopes.scopes[scope.index as usize].rows;
        let i = index as usize;
        let mut new = *rows.get(i)?;
        f(&mut new);
        if let Some(j) = rows.iter().enumerate().position(|(j, r)| j != i && *r == new) {
            rows.remove(i);
            return u32::try_from(if j > i { j - 1 } else { j }).ok();
        }
        rows[i] = new;
        Some(index)
    }

    /// A name of scope `scope`'s pattern failed to resolve: its
    /// substitution fails, and the key is not committed.
    pub(crate) fn key_scope_poison(&mut self, scope: ScopeRef) {
        if self.key_scopes.recording(scope).is_some() {
            self.key_scopes.scopes[scope.index as usize].poisoned = true;
        }
    }

    /// Stops the scopes open now from recording until
    /// [`Self::key_scope_resume`] (§18.14.3: a suspension is a depth). A
    /// scope opened meanwhile records as usual.
    pub(crate) fn key_scope_suspend(&mut self) {
        if self.ast.written_refs.subst_keys_on() {
            let depth = u32::try_from(self.key_scopes.scopes.len()).unwrap_or(u32::MAX);
            let below = depth.max(self.key_scopes.silenced());
            self.key_scopes.suspend.push(below);
        }
    }

    /// Undoes one [`Self::key_scope_suspend`].
    pub(crate) fn key_scope_resume(&mut self) {
        if self.ast.written_refs.subst_keys_on() {
            let _ = self.key_scopes.suspend.pop();
        }
    }

    /// The scope of `id` as the recording sites hold it, with the flags
    /// of a row made from pattern node `node` (§18.14.2: `SUBST_EXPANDED`
    /// for a node reached through an alias expansion, unless the scope
    /// is the alias template's own and holds the entry `w`).
    pub(crate) fn key_scope_ref(
        &self,
        id: KeyScopeId,
        w: WrittenRefId,
        node: Option<PatternNode>,
    ) -> ScopeRef {
        let s = &self.key_scopes.scopes[id.0 as usize];
        let expanded = node.is_some_and(|n| self.ast.written_refs.is_expanded_node(n))
            && !(s.kind == SubstKind::Alias
                && self.ast.written_refs.get(w).enclosing.is_some_and(|e| {
                    self.ast.first(e) == s.template
                        || self
                            .ast
                            .described_template(e)
                            .is_some_and(|t| self.ast.first(t) == s.template)
                }));
        let flags = if expanded { kiln_ast::written_refs::SUBST_EXPANDED } else { 0 };
        ScopeRef { index: id.0, serial: s.serial, flags }
    }

    /// The key scope the rows of names resolved now go to (§18.14.3):
    /// one walk from the innermost frame. Lambda substitutions, binding
    /// initializers and requirement instantiations are transparent; a
    /// frame that owns a scope gives it; a constraint substitution
    /// continues outward only to the satisfaction check of the same
    /// entity; any other frame (every real instantiation) gives none. A
    /// scope that does not record gives none. `None` after one bool test
    /// when keys are off or no scope is open.
    pub(crate) fn written_key_scope(&self) -> Option<KeyScopeId> {
        if self.key_scopes.scopes.is_empty() {
            return None;
        }
        let mut sat_of: Option<DeclId> = None;
        for f in self.inst.stack.frames.iter().rev() {
            if f.key_scope != 0 {
                if sat_of.is_some_and(|e| !(f.kind.is_satisfaction_check() && f.entity == e)) {
                    return None;
                }
                let id = KeyScopeId(f.key_scope - 1);
                return self.key_scope_records(id).then_some(id);
            }
            match f.kind {
                FrameKind::LambdaSubstitution
                | FrameKind::BindingInit
                | FrameKind::RequirementInstantiation => {}
                k if k.is_constraint_substitution() && sat_of.is_none() => sat_of = Some(f.entity),
                _ => return None,
            }
        }
        None
    }

    /// The row targets of a name resolved now: [`Self::written_spec_key`]
    /// and [`Self::written_key_scope`], `None` when neither records.
    pub(crate) fn written_row_keys(
        &self,
        w: WrittenRefId,
        node: Option<PatternNode>,
    ) -> Option<RowKeys> {
        let spec = self.written_spec_key();
        let key = self.written_key_scope().map(|id| self.key_scope_ref(id, w, node));
        (spec.is_some() || key.is_some()).then_some(RowKeys { spec, key })
    }

    /// `keys` for a row of entry `w` made from pattern node `node` (the
    /// scope's flags recomputed).
    pub(crate) fn rekey(&self, keys: RowKeys, w: WrittenRefId, node: PatternNode) -> RowKeys {
        RowKeys {
            spec: keys.spec,
            key: keys.key.map(|k| self.key_scope_ref(KeyScopeId(k.index), w, Some(node))),
        }
    }

    /// The keyed row of an entry substituted and still dependent
    /// (§18.14.2), and the expanded mark of the node it now stands for:
    /// `old` stood for entry `w`, `new` does now.
    pub(crate) fn note_keyed_still_dependent(
        &mut self,
        w: WrittenRefId,
        key: Option<ScopeRef>,
        old: PatternNode,
        new: PatternNode,
    ) {
        if !self.ast.written_refs.subst_keys_on() {
            return;
        }
        let in_alias =
            self.inst.stack.frames.last().is_some_and(|f| f.kind == FrameKind::AliasTemplate);
        if in_alias || self.ast.written_refs.is_expanded_node(old) {
            self.ast.written_refs.note_expanded_node(new);
        }
        if let Some(k) = key {
            let row = SubstRef {
                entry: w,
                entity: None,
                found: None,
                adl: None,
                udir: None,
                naming_class: None,
                flags: SUBST_DEPENDENT,
                how: kiln_ast::written_refs::RefLookup::None,
            };
            let _ = self.key_scope_push(k, row);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Sema;
    use crate::templ::stack::FrameKind;
    use crate::written_keys::KeyMode;
    use kiln_ast::written_refs::{
        REF_DEPENDENT, RefLookup, RefQualifier, RefRole, ScopeRef, SubstKind, SubstRef, WrittenRef,
        WrittenRefId,
    };
    use kiln_ast::{ArgListId, DeclId};
    use kiln_source::{Loc, Span};

    fn with_sema(f: impl FnOnce(&mut Sema<'_, '_>)) {
        use kiln_ast::Ast;
        use kiln_ast::types::TypeContext;
        use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
        use kiln_source::symbol::Interner;
        use std::sync::Arc;

        use crate::target::{Arch, Target};
        let sm = kiln_source::SourceManager::new();
        let diags = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
        let target = Target::for_arch(Arch::X86_64);
        let ast = Ast::new(Arc::new(Interner::new()), TypeContext::new(target.types.clone()));
        let mut sema = Sema::for_target(ast, &diags, target);
        sema.ast.written_refs.set_on(true);
        sema.ast.written_refs.set_keys_on(true);
        f(&mut sema);
    }

    fn entry(sema: &mut Sema<'_, '_>) -> WrittenRefId {
        sema.ast
            .written_refs
            .push(WrittenRef {
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

    fn row(w: WrittenRefId, d: DeclId) -> SubstRef {
        SubstRef {
            entry: w,
            entity: Some(d),
            found: None,
            adl: None,
            udir: None,
            naming_class: None,
            flags: 0,
            how: RefLookup::Unqualified,
        }
    }

    fn sref(sema: &Sema<'_, '_>, i: u32) -> ScopeRef {
        ScopeRef { index: i, serial: sema.key_scopes.scopes[i as usize].serial, flags: 0 }
    }

    /// spec/ast.md §18.14.3: a completed close commits the buffer (rows
    /// kept once), a scope with no rows commits a key without rows, and a
    /// default argument's key is told apart by its parameter.
    #[test]
    fn a_completed_close_commits() {
        with_sema(|s| {
            let t = s.ast.tu;
            let w = entry(s);
            let args = s.ast.types.arg_list(&[]);
            let id = s.key_scope_open(SubstKind::Alias, t, args, 0, None).expect("on");
            let r = sref(s, id.0);
            assert_eq!(s.key_scope_push(r, row(w, t)), Some(0));
            assert_eq!(s.key_scope_push(r, row(w, t)), Some(0), "kept once");
            let k = s.key_scope_close(id, true).expect("committed");
            assert_eq!(s.ast.written_refs.subst_rows(k), &[row(w, t)]);
            // No rows: a key all the same.
            let id = s.key_scope_open(SubstKind::Satisfaction, t, args, 0, None).unwrap();
            let k = s.key_scope_close(id, true).expect("a key without rows");
            assert!(s.ast.written_refs.subst_rows(k).is_empty());
            // The same template and arguments, a default argument: its own key.
            let id = s.key_scope_open(SubstKind::DefaultArg, t, args, t.index(), None).unwrap();
            let k = s.key_scope_close(id, true).expect("a default argument's key");
            assert_eq!(s.ast.written_refs.subst_key(k).parameter(), Some(t));
            assert!(s.key_scopes.at_rest());
        });
    }

    /// §18.14.3: nothing is committed on failure, on poison, or when an
    /// error was diagnosed inside the scope; the buffer goes back to the
    /// pool.
    #[test]
    fn a_failed_close_drops_the_rows() {
        with_sema(|s| {
            let t = s.ast.tu;
            let w = entry(s);
            let args = s.ast.types.arg_list(&[]);
            let id = s.key_scope_open(SubstKind::Alias, t, args, 0, None).unwrap();
            let _ = s.key_scope_push(sref(s, id.0), row(w, t));
            assert_eq!(s.key_scope_close(id, false), None, "failed");
            let id = s.key_scope_open(SubstKind::Alias, t, args, 0, None).unwrap();
            s.key_scope_poison(sref(s, id.0));
            assert_eq!(s.key_scope_close(id, true), None, "poisoned");
            let id = s.key_scope_open(SubstKind::Alias, t, args, 0, None).unwrap();
            let before = s.diags.error_count();
            s.error(kiln_diag::DiagId::sema_cleanup_arg_not_function, Loc::INVALID);
            assert!(s.diags.error_count() > before);
            assert_eq!(s.key_scope_close(id, true), None, "an error diagnosed inside");
            assert_eq!(s.ast.written_refs.subst_key_len(), 0);
            assert_eq!(s.key_scopes.pool.len(), 1);
        });
    }

    /// §18.14.3: a committed identity and one open further out open
    /// `Skip` and record nothing; suspended scopes record nothing; a
    /// stale scope reference is ignored.
    #[test]
    fn skip_and_suspend_record_nothing() {
        with_sema(|s| {
            let t = s.ast.tu;
            let w = entry(s);
            let args = s.ast.types.arg_list(&[]);
            let outer = s.key_scope_open(SubstKind::Alias, t, args, 0, None).unwrap();
            let inner = s.key_scope_open(SubstKind::Alias, t, args, 0, None).unwrap();
            assert_eq!(s.key_scopes.scopes[inner.0 as usize].mode, KeyMode::Skip, "re-entrant");
            assert_eq!(s.key_scope_push(sref(s, inner.0), row(w, t)), None);
            assert_eq!(s.key_scope_close(inner, true), None);
            s.key_scope_suspend();
            assert_eq!(s.key_scope_push(sref(s, outer.0), row(w, t)), None, "suspended");
            s.key_scope_resume();
            let stale = sref(s, outer.0);
            let k = s.key_scope_close(outer, true).expect("committed");
            assert!(s.ast.written_refs.subst_rows(k).is_empty());
            let again = s.key_scope_open(SubstKind::Alias, t, args, 0, None).unwrap();
            assert_eq!(s.key_scopes.scopes[again.0 as usize].mode, KeyMode::Skip, "committed");
            assert_eq!(s.key_scope_push(stale, row(w, t)), None, "a closed scope's reference");
            assert_eq!(s.key_scope_close(again, true), None);
        });
    }

    /// §18.14.3: a suspension is a depth: it silences the scopes open
    /// when it starts, not one opened meanwhile, which records and commits
    /// its rows while a push to the outer scope records nothing; nested
    /// suspensions silence the deepest depth until the last resume.
    #[test]
    fn suspension_is_a_depth() {
        with_sema(|s| {
            let t = s.ast.tu;
            let w = entry(s);
            let args = s.ast.types.arg_list(&[]);
            let outer = s.key_scope_open(SubstKind::Alias, t, args, 0, None).unwrap();
            s.key_scope_suspend();
            assert!(!s.key_scope_records(outer));
            let inner = s.key_scope_open(SubstKind::PartialMatch, t, args, 0, Some(args)).unwrap();
            assert!(s.key_scope_records(inner));
            assert_eq!(s.key_scope_push(sref(s, inner.0), row(w, t)), Some(0));
            assert_eq!(s.key_scope_push(sref(s, outer.0), row(w, t)), None, "silenced");
            // A suspension inside the inner scope silences both; its
            // resume gives the inner scope back, not the outer.
            s.key_scope_suspend();
            assert!(!s.key_scope_records(inner));
            s.key_scope_resume();
            assert!(s.key_scope_records(inner) && !s.key_scope_records(outer));
            let k = s.key_scope_close(inner, true).expect("the inner scope commits");
            assert_eq!(s.ast.written_refs.subst_rows(k), &[row(w, t)]);
            assert!(!s.key_scopes.at_rest(), "still suspended");
            s.key_scope_resume();
            assert_eq!(s.key_scope_push(sref(s, outer.0), row(w, t)), Some(0), "resumed");
            let k = s.key_scope_close(outer, true).expect("committed");
            assert_eq!(s.ast.written_refs.subst_rows(k), &[row(w, t)]);
            assert!(s.key_scopes.at_rest());
        });
    }

    /// §18.14.10: the root the opening site sets is committed with the
    /// key; a scope that does not record takes none; a recording run that
    /// commits nothing is counted, a skipped one is not.
    #[test]
    fn a_root_is_committed_with_its_key() {
        use kiln_ast::written_refs::RootSlot;
        with_sema(|s| {
            let t = s.ast.tu;
            let args = s.ast.types.arg_list(&[]);
            let pm = SubstKind::PartialMatch;
            let root = [RootSlot::args(args), RootSlot::args(args)];
            let id = s.key_scope_open(pm, t, args, 0, Some(args)).unwrap();
            s.key_scope_set_root(id, &[RootSlot::args(args)]);
            s.key_scope_set_root(id, &root);
            let k = s.key_scope_close(id, true).expect("committed");
            assert_eq!(s.ast.written_refs.subst_root(k), &root);
            assert_eq!(s.ast.written_refs.subst_key(k).deduced(), Some(args));
            // The same identity again: skipped, no root, not counted.
            let id = s.key_scope_open(pm, t, args, 0, Some(args)).unwrap();
            assert!(!s.key_scope_records(id));
            s.key_scope_set_root(id, &root);
            assert!(s.key_scopes.scopes[id.0 as usize].root.is_empty());
            assert_eq!(s.key_scope_close(id, true), None);
            assert_eq!(s.ast.written_refs.subst_not_completed(pm), 0);
            // A run that did not complete: counted, its root dropped.
            let id = s.key_scope_open(SubstKind::Alias, t, args, 0, None).unwrap();
            s.key_scope_set_root(id, &root);
            assert_eq!(s.key_scope_close(id, false), None);
            assert_eq!(s.ast.written_refs.subst_not_completed(SubstKind::Alias), 1);
            assert!(!s.key_scopes.root_pool.is_empty());
            assert!(s.key_scopes.root_pool.iter().all(Vec::is_empty));
        });
    }

    /// §18.14.3: the owning frame routes rows to its scope; a real
    /// instantiation frame inside it does not; closing unlinks the frame.
    /// Keys off, nothing opens.
    #[test]
    fn the_innermost_frame_decides() {
        with_sema(|s| {
            let t = s.ast.tu;
            let args = s.ast.types.arg_list(&[]);
            assert!(s.push_inst_frame(FrameKind::AliasTemplate, t, t, Loc::INVALID));
            let id = s.key_scope_open(SubstKind::Alias, t, args, 0, None).unwrap();
            assert_eq!(s.written_key_scope(), Some(id));
            assert!(s.push_inst_frame(FrameKind::LambdaSubstitution, t, t, Loc::INVALID));
            assert_eq!(s.written_key_scope(), Some(id), "transparent");
            assert!(s.push_inst_frame(FrameKind::ClassInstantiation, t, t, Loc::INVALID));
            assert_eq!(s.written_key_scope(), None, "a real instantiation");
            s.pop_inst_frame();
            s.pop_inst_frame();
            let _ = s.key_scope_close(id, true);
            assert_eq!(s.inst.stack.frames.last().map(|f| f.key_scope), Some(0));
            assert_eq!(s.written_key_scope(), None);
            s.pop_inst_frame();
            s.ast.written_refs.set_keys_on(false);
            assert!(s.key_scope_open(SubstKind::Alias, t, args, 0, None).is_none());
            let _ = ArgListId::new(1);
        });
    }
}
