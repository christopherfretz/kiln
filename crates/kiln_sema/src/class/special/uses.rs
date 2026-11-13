//! Uses of deleted functions ([dcl.fct.def.delete]/2) and the
//! default-initialization checks of variables of class type
//! ([dcl.init.general]/7–8, [class.dtor]/15; spec/sema-class.md §4.3).
//!
//! Without overload resolution (WP-068a) a use is diagnosed where the
//! name is resolved, except that an unqualified name of a non-member
//! function waits to see whether it is the callee of a call (clang's
//! `call to deleted function` with its candidate note, from overload
//! resolution); every other reference is `attempt to use a deleted
//! function`, reported at the end of the full-expression or
//! declaration.

use kiln_ast::decl::{DeclKind, DefinitionBits, InitStyle, VarSpec};
use kiln_ast::expr::{ConstructionKind, ExprKind, UnaryOp};
use kiln_ast::types::Quals;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use super::SpecialKind;
use super::define::{CtorCall, InitEntity};
use super::deletion::Selected;
use crate::Sema;
use crate::class::{DeletedAs, DiagSites, InitStyle as CtorStyle};

/// A call of a deleted non-member function by its unqualified name,
/// diagnosed at the call unless the call stays dependent.
#[derive(Clone, Copy, Debug, kiln_snap::Snap)]
pub(crate) struct PendingDeleted {
    /// The callee `DeclRefExpr`.
    pub expr: ExprId,
    /// The function as found by lookup.
    pub decl: DeclId,
    /// The name's location.
    pub loc: Loc,
    /// A function template-id resolved without a call or target
    /// ([`Sema::resolve_single_template_id`]): clang keeps it an
    /// overload set until its use, so the entry follows the enclosing
    /// parentheses and `&` and is diagnosed at their start.
    pub template_id: bool,
}

impl Sema<'_, '_> {
    /// Whether function `d` is deleted ([dcl.fct.def.delete]/1; the
    /// definition is the first declaration).
    pub(crate) fn is_deleted_fn(&self, d: DeclId) -> bool {
        let first = self.ast.first(d);
        self.ast.decls.function(first).is_some_and(|x| x.spec.is_deleted())
    }

    /// The `: message` suffix of `= delete("message")` (C++26, P2573).
    pub(crate) fn delete_message_suffix(&self, d: DeclId) -> String {
        let first = self.ast.first(d);
        self.ast
            .decls
            .cold
            .delete_message
            .get(first)
            .and_then(|&m| self.string_msg(m))
            .map(|s| format!(": {s}"))
            .unwrap_or_default()
    }

    /// The id-expression `e` naming function `d` at `loc` was built:
    /// diagnoses a use of a deleted function ([dcl.fct.def.delete]/2).
    /// An `unqualified` name of a non-member function waits for
    /// [`Sema::act_on_call`] (or the end of the full-expression);
    /// non-static members are diagnosed by the member access instead.
    pub(crate) fn note_function_name_use(
        &mut self,
        d: DeclId,
        e: ExprId,
        loc: Loc,
        unqualified: bool,
    ) {
        if !self.ast.decls.get(d).kind().is_function() || self.is_class_member_ref(d) {
            return;
        }
        if !self.is_err_expr(e) && self.diagnose_unsatisfied_use(d, loc) {
            return;
        }
        if !self.is_deleted_fn(d) || self.is_err_expr(e) {
            return;
        }
        if unqualified && self.kind_of(d) == DeclKind::Function {
            self.special.pending_deleted.push(PendingDeleted {
                expr: e,
                decl: d,
                loc,
                template_id: false,
            });
            return;
        }
        self.diagnose_deleted_use(d, loc);
    }

    /// `attempt to use a deleted function` at `loc` with the notes of
    /// clang `NoteDeletedFunction`.
    pub(crate) fn diagnose_deleted_use(&mut self, d: DeclId, loc: Loc) {
        if self.diagnose_unsatisfied_use(d, loc) {
            return;
        }
        if !self.is_deleted_fn(d) {
            return;
        }
        let mut msg =
            self.diag(DiagId::sema_use_deleted_function, loc).arg(self.delete_message_suffix(d));
        for n in self.deleted_function_notes(d, 0) {
            msg = msg.with_note(n);
        }
        self.emit(msg);
    }

    /// The function template-id `e` resolved to the deleted `d` at
    /// `loc` (its start): diagnosed at its use, see [`PendingDeleted`].
    pub(crate) fn defer_deleted_template_id(&mut self, d: DeclId, e: ExprId, loc: Loc) {
        self.special.pending_deleted.push(PendingDeleted {
            expr: e,
            decl: d,
            loc,
            template_id: true,
        });
    }

    /// The function template-id `e` was resolved to a specialization
    /// whose signature's substitution held back the warnings `ds`: they
    /// are reported at its use ([`SpecialState::pending_tid_warnings`]).
    pub(crate) fn defer_template_id_warnings(&mut self, e: ExprId, d: DeclId, ds: Vec<Diag>) {
        self.special.pending_tid_warnings.push((e, d, ds));
    }

    /// The template-id `e` (possibly in parentheses and under `&`) is
    /// not used after all: its specialization keeps the held-back
    /// warnings of its signature's substitution for its first use (those
    /// of this deduction's explicit arguments are dropped with it). clang stores a deduction's warnings
    /// only when it succeeds, so from clang 22 on (where they carry
    /// context notes) the notes name the deduction of that later use, not
    /// this one: their location is recorded to be moved then
    /// ([`Sema::take_suppressed_diags`]).
    pub(crate) fn drop_template_id_warnings(&mut self, e: ExprId) {
        let mut x = e;
        let mut restored = None;
        for _ in 0..64 {
            if let Some(i) = self.special.pending_tid_warnings.iter().position(|w| w.0 == x) {
                let (_, d, mut ds) = self.special.pending_tid_warnings.remove(i);
                // Only the signature substitution's: the next deduction
                // makes its own explicit-argument ones, or none for a call.
                let n = self.templ.deduction_diags.remove(&d).unwrap_or(0);
                let ds = ds.split_off(n.min(ds.len()));
                let slot = self.templ.suppressed_diags.entry(d).or_default();
                if slot.is_empty() {
                    *slot = ds;
                    restored = Some(d);
                }
            }
            let n = *self.ast.exprs.get(x);
            x = match n.kind() {
                ExprKind::ParenExpr => self.ast.exprs.operand(x),
                ExprKind::UnaryOperator if self.ast.exprs.unary_op(x) == UnaryOp::AddrOf => {
                    self.ast.exprs.operand(x)
                }
                _ => break,
            };
        }
        // The notes sit at the template-id itself, the innermost operand.
        if let Some(d) = restored {
            let at = self.ast.exprs.get(x).loc();
            let _ = self.special.dropped_tid_warnings.insert(d, at);
        }
    }

    /// `outer`, a parenthesized expression or `&` built around `inner`:
    /// a pending deleted function template-id `inner` moves to `outer`,
    /// diagnosed at its start (clang resolves the overload set at the
    /// use of the whole operand).
    pub(crate) fn widen_pending_template_id(&mut self, inner: ExprId, outer: ExprId) {
        if let Some(w) = self.special.pending_tid_warnings.iter_mut().find(|w| w.0 == inner) {
            w.0 = outer;
        }
        let Some(i) =
            self.special.pending_deleted.iter().position(|p| p.template_id && p.expr == inner)
        else {
            return;
        };
        let loc = self.begin_loc(outer);
        let p = &mut self.special.pending_deleted[i];
        p.expr = outer;
        p.loc = loc;
    }

    /// At a call whose callee is `callee`: a pending deleted function
    /// named by it is diagnosed as `call to deleted function` unless
    /// `dependent` (or `invalid`, an argument being in error).
    pub(crate) fn act_on_deleted_callee(&mut self, callee: ExprId, dependent: bool, invalid: bool) {
        // A called template-id is used.
        if let Some(i) = self.special.pending_tid_warnings.iter().position(|w| w.0 == callee) {
            let (_, _, ds) = self.special.pending_tid_warnings.remove(i);
            self.emit_held_warnings(ds);
        }
        let Some(i) = self.special.pending_deleted.iter().position(|p| p.expr == callee) else {
            return;
        };
        let p = self.special.pending_deleted.swap_remove(i);
        if dependent || invalid {
            return;
        }
        if p.template_id
            && let Some((_, info)) = self.ast.spec_info(p.decl)
        {
            // clang names the template-id's name and notes the
            // specialization's bindings (`candidate function [with N = 2]`).
            let kind = format!("function {}", self.deduced_args_desc(info.template, info.args));
            let (at, spec) = self.deleted_note_decl(p.decl);
            let msg = self
                .diag(DiagId::sema_call_deleted_function, p.loc)
                .arg(self.decl_arg(info.template))
                .arg(self.delete_message_suffix(p.decl))
                .with_note(
                    self.diag(DiagId::note_ovl_candidate_deleted, self.ast.decls.get(at).loc())
                        .arg(kind)
                        .arg(u8::from(spec)),
                );
            self.emit(msg);
            return;
        }
        // clang's candidate note names the declaration found; on a
        // redeclaration of a deleted function it says "implicitly".
        let own = self.ast.decls.function(p.decl).is_some_and(|x| x.spec.is_deleted());
        let msg = self
            .diag(DiagId::sema_call_deleted_function, p.loc)
            .arg(self.decl_arg(p.decl))
            .arg(self.delete_message_suffix(p.decl))
            .with_note(
                self.diag(DiagId::note_candidate_deleted, self.ast.decls.get(p.decl).loc())
                    .arg(u8::from(!own)),
            );
        self.emit(msg);
    }

    /// At the end of a full-expression: a pending callee that no call
    /// consumed is a plain use.
    pub(crate) fn flush_pending_deleted(&mut self) {
        for (_, _, ds) in std::mem::take(&mut self.special.pending_tid_warnings) {
            self.emit_held_warnings(ds);
        }
        let pending = std::mem::take(&mut self.special.pending_deleted);
        for p in pending {
            self.diagnose_deleted_use(p.decl, p.loc);
        }
    }

    /// Default-initialization of variable `v` of (array of) class type
    /// `ty` ([dcl.init.general]/7–8) and the destructor it potentially
    /// invokes ([class.dtor]/15): builds the default constructor call
    /// (the variable's `callinit` initializer, spec/sema-class.md §5.6),
    /// diagnosing a deleted, ambiguous or inaccessible default
    /// constructor and a `const` object without a user-provided one; then
    /// the destructor must be accessible and not deleted, and is odr-used
    /// (§4.8).
    pub(crate) fn check_class_default_init(&mut self, v: DeclId, ty: QualType, loc: Loc) {
        let elem = self.base_element(ty);
        let Some(class) = self.element_class(elem) else { return };
        let class = self.class_def(class);
        let cflags = self.ast.decls.get(class).flags();
        if cflags.contains(kiln_ast::decl::DeclFlags::INVALID)
            || cflags.contains(kiln_ast::decl::DeclFlags::DEPENDENT_CTX)
        {
            return;
        }
        let mut ok = true;
        {
            let e = if let Selected::Missing =
                self.select_special(class, SpecialKind::DefaultCtor, Quals::NONE, Quals::NONE)
            {
                // No usable default constructor: clang's "no matching
                // constructor for initialization" with the candidates of
                // [over.match.ctor] for no arguments, kept in `ovl.failed`.
                let sel = self.resolve_ctor(class, &[], CtorStyle::Direct);
                let call = CtorCall::new(&[]);
                let sites = DiagSites { access: loc, deleted: loc, deleted_as: DeletedAs::Call };
                self.use_ctor(
                    sel,
                    ty,
                    class,
                    SpecialKind::DefaultCtor,
                    call,
                    loc,
                    InitEntity::Var,
                    sites,
                )
            } else {
                self.construct_special(
                    ty,
                    class,
                    SpecialKind::DefaultCtor,
                    None,
                    ConstructionKind::Complete,
                    loc,
                    InitEntity::Var,
                )
            };
            if let Some(e) = e {
                // A full-expression ([intro.execution]/5.4): its
                // immediate invocations are evaluated.
                self.note_var_init_full_expr(v);
                self.finish_immediate_invocations(e);
                let e = self.finish_cleanups(e);
                if self.ast.decls.var(v).is_some_and(|x| x.init.is_none()) {
                    let _ = self.builder().set_var_init(v, e);
                    self.ast.decls.mark_var(v, VarSpec::new().with_init_style(InitStyle::Call));
                }
            } else {
                // clang `InitializationSequence::Diagnose` makes the
                // variable invalid: a constant evaluation reaching its
                // definition fails with no note (spec/oven.md).
                self.ast.decls.set_flags(v, kiln_ast::decl::DeclFlags::INVALID);
                ok = false;
            }
        }
        if ok
            && self.ast.types.effective_quals(ty).is_const()
            && !self.defn_bits(class).contains(DefinitionBits::CAN_CONST_DEFAULT_INIT)
        {
            let msg = self.diag(DiagId::sema_default_init_const_no_ctor, loc).arg(self.ty_arg(ty));
            self.emit(msg);
            ok = false;
        }
        if ok && self.ast.types.is_array(ty) {
            // Default-initializing the elements potentially invokes
            // their destructor (clang's check for the construction of an
            // array entity, "temporary of type" wording); the variable
            // itself has none to finalize (WP-227).
            let _ = self.check_temporary_dtor(class, elem, loc);
        } else if ok {
            self.finalize_var_dtor(class, elem, loc);
        }
    }

    /// The destructor variable `v` of (array of) class type `ty`
    /// potentially invokes ([class.dtor]/15, clang's
    /// `FinalizeVarWithDestructor`; spec/sema-class.md §4.8): it must be
    /// accessible and not deleted, and is odr-used unless trivial. `loc`
    /// is the variable's name.
    pub(crate) fn check_var_destructor(&mut self, v: DeclId, ty: QualType, loc: Loc) {
        if self.ast.decls.get(v).has(kiln_ast::decl::DeclFlags::INVALID)
            || self.ast.types.is_reference(ty)
            || self.is_dependent_type(ty)
        {
            return;
        }
        let elem = self.base_element(ty);
        let Some(class) = self.element_class(elem) else { return };
        let class = self.class_def(class);
        let cflags = self.ast.decls.get(class).flags();
        if cflags.contains(kiln_ast::decl::DeclFlags::INVALID)
            || cflags.contains(kiln_ast::decl::DeclFlags::DEPENDENT_CTX)
        {
            return;
        }
        if self.ast.types.is_array(ty) {
            // clang's `FinalizeVarWithDestructor` skips an array: the
            // initialization already checked the element destructor
            // ([dcl.init.aggr]/8, `init_dtor.rs`); it stays odr-used.
            if let Selected::Found(m) =
                self.select_special(class, SpecialKind::Dtor, Quals::NONE, Quals::NONE)
                && !self.defn_bits(class).contains(DefinitionBits::TRIVIAL_DTOR)
            {
                self.mark_function_referenced(m, loc);
            }
            return;
        }
        self.finalize_var_dtor(class, elem, loc);
    }

    fn finalize_var_dtor(&mut self, class: DeclId, elem: QualType, loc: Loc) {
        match self.select_special(class, SpecialKind::Dtor, Quals::NONE, Quals::NONE) {
            Selected::Found(m) => {
                let msg = self.special_member_access(m, class, |s, prot| {
                    s.diag(DiagId::sema_dtor_access_var, loc)
                        .arg(s.ty_arg(elem))
                        .arg(u8::from(prot))
                });
                if let Some(msg) = msg {
                    self.emit(msg);
                }
                if !self.defn_bits(class).contains(DefinitionBits::TRIVIAL_DTOR) {
                    self.mark_function_referenced(m, loc);
                }
            }
            Selected::Deleted(m) => self.diagnose_deleted_use(m, loc),
            Selected::Missing | Selected::Ambiguous | Selected::Unsupported => {}
        }
    }
}
