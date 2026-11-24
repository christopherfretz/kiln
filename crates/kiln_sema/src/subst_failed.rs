//! The trap ledger: which invalid declarations failed silently
//! (spec/sema-templates.md §5.7.3 "Silent failures", spec/ast.md §4.1;
//! WP-3281).
//!
//! An error raised inside a *trap* is not printed: a SFINAE context counts
//! it (`Sema::emit`), a diagnostic capture holds it until the caller
//! releases or drops it. A declaration kiln makes invalid there failed
//! silently, and [`kiln_ast::decl::DeclArena::subst_failed`] must say so.
//! Every invalidation in Sema and the parser goes through
//! [`Sema::invalidate`] (or [`Sema::invalidate_as`], or
//! [`Sema::note_born_invalid`] for a declaration created invalid; the
//! `xtask` lint `invalid_lint` holds the rule). Each one asks
//! [`Sema::trap_now`], the predicate `emit` uses, and files the
//! declaration with the trap:
//!
//! - a SFINAE context keeps its entries in its [`SfinaeState`]; when it
//!   ends ([`Sema::pop_sfinae_state`], the only way one ends) they are
//!   confirmed if no error was printed while it was open, else dropped
//!   (the error explains them);
//! - a capture keeps its entries under its serial number; releasing the
//!   stash ([`Sema::release`]) re-files them with the trap the
//!   diagnostics go to, or drops them when one prints; a stash that is
//!   never released was discarded, and its entries are confirmed at the
//!   end ([`Sema::flush_subst_failed`]).
//!
//! The confirmed entries are written to the Ast at the end of the
//! translation unit and before a snapshot. An entry without an explicit
//! kind is [`SubstFailedKind::LocalOfFailedSignature`] when it was created
//! while a signature failed to form in the same trap
//! ([`Sema::claim_failed_locals`]) or its owner chain (`ctx`,
//! `closure_owner`) reaches a specialization marked
//! [`SubstFailedKind::FunctionSignature`] or
//! [`SubstFailedKind::ExplicitSpecifier`], else
//! [`SubstFailedKind::OtherTrapped`].
//!
//! [`SfinaeState`]: crate::templ::stack::SfinaeState

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclFlags, DeclKind, SubstFailedKind};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use crate::Sema;
use crate::templ::stack::SfinaeState;

/// An invalidation filed with a trap, waiting for the trap's end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Pending {
    pub(crate) decl: DeclId,
    /// The explicit kind; `None` is decided at the end (module doc).
    pub(crate) kind: Option<SubstFailedKind>,
}

/// The trap an error raised now would go to (module doc).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Trap {
    /// The open diagnostic capture with this serial number.
    Capture(u32),
    /// The innermost SFINAE context.
    Sfinae,
}

/// An open diagnostic capture (`Sema::diag_capture`): the instantiation
/// depth it holds at, what it holds, whether it also holds the checking
/// frames above (the empty-pack sweep), and its serial number.
#[derive(Debug)]
pub(crate) struct DiagCapture {
    pub(crate) depth: usize,
    pub(crate) held: Vec<Diag>,
    pub(crate) checks: bool,
    pub(crate) serial: u32,
}

/// The diagnostics a capture held ([`Sema::capturing`]): the caller
/// releases them ([`Sema::release`]) or drops them.
#[derive(Debug, Default)]
pub(crate) struct Stash {
    pub(crate) diags: Vec<Diag>,
    /// The capture's serial number (0: none, nothing filed with it).
    pub(crate) serial: u32,
}

impl std::ops::Deref for Stash {
    type Target = Vec<Diag>;
    fn deref(&self) -> &Vec<Diag> {
        &self.diags
    }
}

impl std::ops::DerefMut for Stash {
    fn deref_mut(&mut self) -> &mut Vec<Diag> {
        &mut self.diags
    }
}

impl From<Vec<Diag>> for Stash {
    fn from(diags: Vec<Diag>) -> Self {
        Stash { diags, serial: 0 }
    }
}

/// A function template specialization that failed silently, with the
/// declarations created while it was formed (ids `first + 1..=end`), for the
/// checker's ownership rule ([`Sema::verify_subst_failed`]).
#[derive(Clone, Copy, Debug)]
struct Formation {
    spec: DeclId,
    first: u32,
    end: u32,
}

/// The ledger's own state (module doc); at rest when empty.
#[derive(Debug, Default)]
pub(crate) struct SubstLedger {
    /// Entries of captures not yet released, by serial number.
    captured: Vec<(u32, Pending)>,
    /// Entries whose trap ended silently.
    confirmed: Vec<Pending>,
    /// The last capture serial number handed out.
    serial: u32,
    /// Formations of failed function template specializations (kept only
    /// while `-fkiln-verify-subst-failed` checks them).
    formations: Vec<Formation>,
    /// The open discard scopes ([`Sema::open_discard`]): the error count
    /// at the open and the entries filed in it under no other trap.
    discards: Vec<(u32, Vec<Pending>)>,
    /// Template parameter lists substituted only to be compared, which no
    /// declaration holds ([`Sema::note_scratch_head`]; kept only while
    /// verifying, as `formations`).
    scratch_heads: Vec<kiln_ast::TemplateParamListId>,
    /// Whether to keep `formations` and `scratch_heads`.
    verifying: bool,
}

impl SubstLedger {
    /// Whether nothing is filed (a snapshot needs it at rest). The
    /// checker's formations are not carried: a consumer checks its own.
    pub(crate) fn is_empty(&self) -> bool {
        self.captured.is_empty() && self.confirmed.is_empty() && self.discards.is_empty()
    }
}

impl Sema<'_, '_> {
    /// Makes `d` invalid (clang `setInvalidDecl`) and files it with the
    /// trap in effect (module doc). An invalid `d` stays as it is.
    pub fn invalidate(&mut self, d: DeclId) {
        self.invalidate_kind(d, None);
    }

    /// [`Self::invalidate`], saying what failed to form if the failure
    /// turns out to be silent.
    pub(crate) fn invalidate_as(&mut self, d: DeclId, kind: SubstFailedKind) {
        self.invalidate_kind(d, Some(kind));
    }

    fn invalidate_kind(&mut self, d: DeclId, kind: Option<SubstFailedKind>) {
        if self.ast.decls.get(d).has(DeclFlags::INVALID) {
            return;
        }
        self.ast.decls.set_flags(d, DeclFlags::INVALID);
        self.file_invalid(d, kind);
    }

    /// Files `d`, created invalid (its `DeclHead` said so), with the trap
    /// in effect.
    pub(crate) fn note_born_invalid(&mut self, d: DeclId) {
        if self.ast.decls.get(d).has(DeclFlags::INVALID) {
            self.file_invalid(d, None);
        }
    }

    fn file_invalid(&self, d: DeclId, kind: Option<SubstFailedKind>) {
        self.file_pending(vec![Pending { decl: d, kind }]);
    }

    /// Files `ps` with the current trap: a SFINAE context, a capture, else
    /// the innermost discard scope; outside all of them an error explains
    /// them (or the checker reports them).
    fn file_pending(&self, ps: Vec<Pending>) {
        if ps.is_empty() {
            return;
        }
        match self.trap_now() {
            Some(Trap::Sfinae) => {
                if let Some(s) = self.inst.sfinae.borrow_mut().last_mut() {
                    s.pending.extend(ps);
                }
            }
            Some(Trap::Capture(n)) => {
                self.subst_ledger.borrow_mut().captured.extend(ps.into_iter().map(|p| (n, p)));
            }
            None => {
                if let Some((_, scope)) = self.subst_ledger.borrow_mut().discards.last_mut() {
                    scope.extend(ps);
                }
            }
        }
    }

    /// `d` was created invalid as the duplicate of a definition merged
    /// from another unit's global module fragment (spec/modules.md §6):
    /// its body is skipped and no diagnostic says so, a silent failure
    /// ([`SubstFailedKind::OtherTrapped`]) wherever it happens. Under a
    /// trap [`Self::note_born_invalid`] has filed it already.
    pub(crate) fn note_merged_duplicate(&self, d: DeclId) {
        if self.ast.decls.get(d).has(DeclFlags::INVALID) && self.trap_now().is_none() {
            let p = Pending { decl: d, kind: Some(SubstFailedKind::OtherTrapped) };
            self.subst_ledger.borrow_mut().confirmed.push(p);
        }
    }

    /// Records template parameter list `l`, substituted only to compare it
    /// with another head and held by no declaration (a friend class
    /// template's head, `Substituter::check_friend_class_head`), for the
    /// checker's rule A: its parameters are the one template-parameter
    /// exception (spec/ast.md §4.1).
    pub(crate) fn note_scratch_head(&self, l: kiln_ast::TemplateParamListId) {
        let mut ledger = self.subst_ledger.borrow_mut();
        if ledger.verifying {
            ledger.scratch_heads.push(l);
        }
    }

    /// Opens a discard scope: a substitution whose result may be thrown
    /// away without a diagnostic, the pack-expansion probe
    /// (`Substituter::expand_pattern`, `param_pack_len`). Returns the
    /// scope's depth for [`Self::close_discard`].
    pub(crate) fn open_discard(&self) -> usize {
        let mut l = self.subst_ledger.borrow_mut();
        l.discards.push((self.diags.error_count(), Vec::new()));
        l.discards.len()
    }

    /// Closes the discard scope `depth` opened. `discarded`: its result was
    /// thrown away, so what it made invalid failed silently, confirmed when
    /// no error was printed meanwhile; otherwise the result is kept and its
    /// entries go to the enclosing trap.
    pub(crate) fn close_discard(&self, depth: usize, discarded: bool) {
        let popped = {
            let mut l = self.subst_ledger.borrow_mut();
            if l.discards.len() != depth {
                debug_assert!(false, "discard scopes close in order");
                return;
            }
            l.discards.pop()
        };
        let Some((printed, ps)) = popped else { return };
        if !discarded {
            self.file_pending(ps);
        } else if !ps.is_empty() && self.diags.error_count() == printed {
            self.subst_ledger.borrow_mut().confirmed.extend(ps);
        }
    }

    /// The trap an error emitted now goes to: the open capture when it
    /// holds at this depth, else the innermost SFINAE context when it is
    /// one, else none (it prints). [`Self::emit`] decides with the same
    /// two predicates.
    pub(crate) fn trap_now(&self) -> Option<Trap> {
        if let Some(c) = self.diag_capture.borrow().as_ref()
            && self.capture_holds(c)
        {
            return Some(Trap::Capture(c.serial));
        }
        self.sfinae_traps().then_some(Trap::Sfinae)
    }

    /// Whether capture `c` holds an error emitted now: at its own depth,
    /// or above it through checking frames only when it says so.
    pub(crate) fn capture_holds(&self, c: &DiagCapture) -> bool {
        c.depth == self.inst.stack.len() || (c.checks && self.inst.stack.only_checks_above(c.depth))
    }

    /// Whether an error emitted now is a substitution failure: the
    /// innermost frame that says is a SFINAE frame, and no implicit
    /// definition was begun above it (clang `isSFINAEContext`: an
    /// implicitly defined special member or comparison,
    /// `DefiningSynthesizedFunction`, is no SFINAE context, also when a
    /// deduction made it needed).
    pub(crate) fn sfinae_traps(&self) -> bool {
        let Some(i) = self.inst.stack.innermost_sfinae_frame() else { return false };
        self.synth.last().is_none_or(|c| c.inst_depth <= i)
    }

    /// Opens a SFINAE context (§5.7): its errors are counted, not printed.
    pub(crate) fn push_sfinae_state(&self) {
        let st = SfinaeState { printed: self.diags.error_count(), ..SfinaeState::default() };
        self.inst.sfinae.borrow_mut().push(st);
    }

    /// Ends the innermost SFINAE context, confirming the declarations it
    /// made invalid when no error printed while it was open.
    pub(crate) fn pop_sfinae_state(&self) -> SfinaeState {
        let mut st = self.inst.sfinae.borrow_mut().pop().unwrap_or_default();
        let pending = std::mem::take(&mut st.pending);
        if !pending.is_empty() && self.diags.error_count() == st.printed {
            self.subst_ledger.borrow_mut().confirmed.extend(pending);
        }
        st
    }

    /// Opens a capture at the current depth (`checks`: also above it,
    /// through checking frames); returns the one it replaces.
    pub(crate) fn open_capture(&self, checks: bool) -> Option<DiagCapture> {
        let serial = {
            let mut l = self.subst_ledger.borrow_mut();
            l.serial = l.serial.wrapping_add(1).max(1);
            l.serial
        };
        let c = DiagCapture { depth: self.inst.stack.len(), held: Vec::new(), checks, serial };
        self.diag_capture.replace(Some(c))
    }

    /// Closes the capture [`Self::open_capture`] opened, restoring `old`;
    /// what it held, for the caller to release or drop.
    pub(crate) fn close_capture(&self, old: Option<DiagCapture>) -> Stash {
        self.diag_capture
            .replace(old)
            .map(|c| Stash { diags: c.held, serial: c.serial })
            .unwrap_or_default()
    }

    /// Re-files the entries of `stash`'s capture after its diagnostics
    /// were emitted again: dropped when one printed (`printed`), else
    /// filed with the trap they went to, else dropped.
    pub(crate) fn refile_stash(&self, serial: u32, printed: bool) {
        if serial == 0 {
            return;
        }
        let moved: Vec<Pending> = {
            let mut l = self.subst_ledger.borrow_mut();
            if !l.captured.iter().any(|&(n, _)| n == serial) {
                return;
            }
            let (mine, rest): (Vec<_>, Vec<_>) =
                std::mem::take(&mut l.captured).into_iter().partition(|&(n, _)| n == serial);
            l.captured = rest;
            mine.into_iter().map(|(_, p)| p).collect()
        };
        if printed {
            return;
        }
        self.file_pending(moved);
    }

    /// A signature failed to form (kind 1 or 2) inside the current trap:
    /// the entries that trap holds without a kind for declarations created
    /// after `first` (the last id before the formation) are the signature's locals
    /// ([`SubstFailedKind::LocalOfFailedSignature`]) whatever their `ctx`
    /// (a statement-expression's local keeps its pattern's context).
    pub(crate) fn claim_failed_locals(&self, first: usize) {
        let mine = |p: &mut Pending| {
            if p.kind.is_none() && p.decl.index() as usize > first {
                p.kind = Some(SubstFailedKind::LocalOfFailedSignature);
            }
        };
        match self.trap_now() {
            None => {}
            Some(Trap::Sfinae) => {
                if let Some(s) = self.inst.sfinae.borrow_mut().last_mut() {
                    s.pending.iter_mut().for_each(mine);
                }
            }
            Some(Trap::Capture(n)) => {
                let mut l = self.subst_ledger.borrow_mut();
                l.captured.iter_mut().filter(|(m, _)| *m == n).for_each(|(_, p)| mine(p));
            }
        }
    }

    /// Records that function template specialization `spec`, whose
    /// formation created the declarations from `first` on, failed (for the
    /// checker's ownership rule; nothing unless it runs).
    pub(crate) fn note_failed_formation(&self, spec: DeclId, first: usize) {
        let mut l = self.subst_ledger.borrow_mut();
        if l.verifying {
            let end = self.ast.decls.len() as u32;
            l.formations.push(Formation { spec, first: first as u32, end });
        }
    }

    /// `-fkiln-verify-subst-failed`: keep what [`Self::verify_subst_failed`]
    /// checks.
    pub fn set_verify_subst_failed(&mut self, on: bool) {
        self.subst_ledger.get_mut().verifying = on;
    }

    /// Writes the confirmed silent failures to the Ast (module doc): at the
    /// end of the translation unit and of a snapshot prefix. A capture
    /// still filed was never released: its diagnostics were dropped.
    pub fn flush_subst_failed(&mut self) {
        let all: Vec<Pending> = {
            let l = self.subst_ledger.get_mut();
            let mut all = std::mem::take(&mut l.confirmed);
            all.extend(std::mem::take(&mut l.captured).into_iter().map(|(_, p)| p));
            all
        };
        if all.is_empty() {
            return;
        }
        let mut signature = Vec::new();
        for p in &all {
            if let Some(k) = p.kind
                && self.ast.decls.get(p.decl).has(DeclFlags::INVALID)
                && self.ast.decls.subst_failed(p.decl).is_none()
            {
                self.ast.decls.set_subst_failed(p.decl, k);
                if k.is_signature() {
                    signature.push(p.decl);
                }
            }
        }
        for p in &all {
            if p.kind.is_none()
                && self.ast.decls.get(p.decl).has(DeclFlags::INVALID)
                && self.ast.decls.subst_failed(p.decl).is_none()
            {
                let k = if self.owned_by_failed_signature(p.decl) {
                    SubstFailedKind::LocalOfFailedSignature
                } else {
                    SubstFailedKind::OtherTrapped
                };
                self.ast.decls.set_subst_failed(p.decl, k);
            }
        }
        // A template whose pattern failed silently is invalid with it (its
        // head copies the pattern's flags, `Builder::template_decl`): the
        // same mark.
        for p in &all {
            if let Some(k) = self.ast.decls.subst_failed(p.decl)
                && let Some(t) = self.ast.described_template(p.decl)
                && self.ast.decls.get(t).has(DeclFlags::INVALID)
                && self.ast.decls.subst_failed(t).is_none()
            {
                self.ast.decls.set_subst_failed(t, k);
            }
        }
        // A specialization that was not formed is named by no recorded
        // reference (spec/ast.md §18.9.2): what its signature resolved
        // belongs to nothing.
        if !signature.is_empty() && self.records_written_refs() {
            let set: kiln_source::fx::FxHashSet<DeclId> = signature.into_iter().collect();
            self.ast.written_refs.drop_specs_since(0, |k| set.contains(&k));
        }
    }

    /// Whether `d`'s owner chain (`ctx`, and a closure's `closure_owner`)
    /// reaches a function template specialization that was not formed.
    fn owned_by_failed_signature(&self, d: DeclId) -> bool {
        self.failed_signature_owner(d).is_some()
    }

    /// The not-formed specialization `d`'s owner chain reaches.
    fn failed_signature_owner(&self, d: DeclId) -> Option<DeclId> {
        let mut cur = self.decl_owner(d)?;
        for _ in 0..256 {
            if self.ast.decls.subst_failed(cur).is_some_and(SubstFailedKind::is_signature) {
                return Some(cur);
            }
            cur = self.decl_owner(cur)?;
        }
        None
    }

    /// `d`'s owner: a closure type's `closure_owner`, else its `ctx`.
    fn decl_owner(&self, d: DeclId) -> Option<DeclId> {
        self.ast.decls.cold.closure_owner.get(d).copied().or_else(|| self.ast.decls.get(d).ctx())
    }

    /// The checker of `-fkiln-verify-subst-failed` (spec/ast.md §4.1), run
    /// after [`Self::flush_subst_failed`]: in a translation unit with no
    /// error, every invalid declaration carries a mark; every declaration
    /// created while a not-formed specialization was formed belongs to it
    /// (rule A of spec/ast.md §4.1); no recorded reference names a
    /// not-formed specialization. One line per violation.
    pub fn verify_subst_failed(&self) -> Vec<String> {
        let mut out = Vec::new();
        let decls = &self.ast.decls;
        if self.diags.error_count() == 0 {
            for i in 1..=decls.len() as u32 {
                let d = DeclId::new(i);
                if decls.get(d).has(DeclFlags::INVALID) && decls.subst_failed(d).is_none() {
                    out.push(format!(
                        "invalid declaration #{i} ({}) carries no subst-failed mark in a translation unit without errors",
                        self.verify_name(d)
                    ));
                }
            }
        }
        let l = self.subst_ledger.borrow();
        let parm_owners = if l.formations.is_empty() {
            kiln_source::fx::FxHashMap::default()
        } else {
            self.template_parm_owners(&l.scratch_heads)
        };
        for f in &l.formations {
            if !decls.subst_failed(f.spec).is_some_and(SubstFailedKind::is_signature) {
                continue;
            }
            for i in (f.first + 1).max(f.spec.index() + 1)..=f.end {
                let d = DeclId::new(i);
                if let Some(why) = self.orphan_of_formation(d, f, &parm_owners) {
                    out.push(format!(
                        "declaration #{i} ({}), created while #{} ({}) failed to form, {why}",
                        self.verify_name(d),
                        f.spec.index(),
                        self.verify_name(f.spec)
                    ));
                }
            }
        }
        for k in self.ast.written_refs.spec_keys_since(0) {
            if decls.subst_failed(k).is_some_and(SubstFailedKind::is_signature) {
                out.push(format!(
                    "the written references are keyed by #{} ({}), which failed to form",
                    k.index(),
                    self.verify_name(k)
                ));
            }
        }
        out
    }

    /// Why `d`, created while `f` was formed, is an orphan; `None` when it
    /// belongs to the specialization (its owner chain reaches it), is an
    /// independent entity (a specialization or an implicit deduction
    /// guide, or inside one), is listed in an older context (a pattern
    /// through its template, a template parameter through the parameter
    /// list holding it), is itself marked kind 5, or is one of the two
    /// known scratch gaps (spec/ast.md §4.1 rule A).
    fn orphan_of_formation(
        &self,
        d: DeclId,
        f: &Formation,
        parm_owners: &kiln_source::fx::FxHashMap<DeclId, Option<DeclId>>,
    ) -> Option<String> {
        if self.ast.decls.subst_failed(d) == Some(SubstFailedKind::LocalOfFailedSignature) {
            return None;
        }
        let mut cur = d;
        for _ in 0..256 {
            // Independent entities: a specialization, and an implicit
            // deduction guide (kept with its class template).
            if cur == f.spec
                || self.ast.spec_info(cur).is_some()
                || self.kind_of(cur) == DeclKind::CXXDeductionGuide
            {
                return None;
            }
            // A template belongs where its pattern does.
            if let Some(p) = self.ast.template_info(cur).and_then(|i| i.pattern)
                && p != cur
            {
                cur = p;
                continue;
            }
            // A template parameter belongs to the declaration whose
            // parameter list holds it (a child link): one older than the
            // formation lists it, a newer one is followed.
            if matches!(
                self.kind_of(cur),
                DeclKind::TemplateTypeParm
                    | DeclKind::NonTypeTemplateParm
                    | DeclKind::TemplateTemplateParm
            ) {
                match parm_owners.get(&cur) {
                    // A comparison head's: the known gap.
                    Some(None) => return None,
                    Some(&Some(o)) if o.index() <= f.first => return None,
                    Some(&Some(o)) => {
                        cur = o;
                        continue;
                    }
                    None => return Some("is in no template parameter list".to_owned()),
                }
            }
            let owner = match self.ast.decls.cold.closure_owner.get(cur) {
                Some(&o) => o,
                // A friend belongs to the class it is declared in.
                None => self.ast.decls.lexical_ctx(cur)?,
            };
            if owner.index() <= f.first && owner != f.spec {
                // `cur` is the outermost declaration of the formation on
                // the chain: it must be a listed member of `owner` (or
                // one of its parameters).
                // The one known gap (spec/ast.md §4.1 rule A): constraint
                // satisfaction substitutes a function's parameters for its
                // requires-clause into new `ParmVar`s that no list holds
                // (`seed_function_params`); one named like a parameter of
                // its older function is accepted.
                // A template's pattern is listed through its template.
                let name = self.ast.decls.get(cur).name();
                let wrapper = self.ast.described_template(cur);
                let listed = self.ast.decls.members(owner).any(|m| m == cur || Some(m) == wrapper)
                    || self.ast.decls.function(owner).is_some_and(|fd| {
                        self.ast.lists.iter::<DeclId>(fd.params).any(|q| {
                            q == cur
                                || self.kind_of(cur) == DeclKind::ParmVar
                                    && name != kiln_ast::names::DeclName::EMPTY
                                    && self.ast.decls.get(q).name() == name
                        })
                    });
                return if listed {
                    None
                } else {
                    Some(format!(
                        "belongs to no declaration (#{} {} is older and does not list it)",
                        owner.index(),
                        self.verify_name(owner)
                    ))
                };
            }
            cur = owner;
        }
        None
    }

    /// For the checker: each template parameter, mapped to the declaration
    /// whose parameter list holds it (a template, a partial
    /// specialization, or a template template parameter). A parameter of
    /// a recorded comparison head (`scratch`) maps to `None`: the known gap
    /// of rule A.
    fn template_parm_owners(
        &self,
        scratch: &[kiln_ast::TemplateParamListId],
    ) -> kiln_source::fx::FxHashMap<DeclId, Option<DeclId>> {
        let mut out = kiln_source::fx::FxHashMap::default();
        for &l in scratch {
            for q in self.ast.lists.iter::<DeclId>(self.ast.templates.param_list(l).params) {
                out.insert(q, None);
            }
        }
        for i in 1..=self.ast.decls.len() as u32 {
            let d = DeclId::new(i);
            let list = if let Some(t) = self.ast.template_info(d) {
                Some(t.params)
            } else if let Some((_, p)) = self.ast.partial_spec_info(d) {
                Some(p.params)
            } else {
                self.ast.template_parm(d).and_then(|p| p.params)
            };
            if let Some(l) = list {
                for q in self.ast.lists.iter::<DeclId>(self.ast.templates.param_list(l).params) {
                    out.entry(q).or_insert(Some(d));
                }
            }
        }
        out
    }

    /// `name` (kind) of `d`, for a checker line.
    fn verify_name(&self, d: DeclId) -> String {
        format!("{:?} {}", self.kind_of(d), self.decl_arg(d))
    }

    /// Emits the checker's findings as errors (`-fkiln-verify-subst-failed`).
    pub fn report_subst_failed_violations(&self) {
        for v in self.verify_subst_failed() {
            self.diag(DiagId::drv_verify_subst_failed, Loc::INVALID).arg(v).emit(self.diags);
        }
    }
}
