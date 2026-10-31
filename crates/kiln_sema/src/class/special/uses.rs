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
use kiln_ast::expr::ConstructionKind;
use kiln_ast::types::Quals;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::SpecialKind;
use super::define::InitEntity;
use super::deletion::Selected;
use crate::Sema;

/// A call of a deleted non-member function by its unqualified name,
/// diagnosed at the call unless the call stays dependent.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PendingDeleted {
    /// The callee `DeclRefExpr`.
    pub expr: ExprId,
    /// The function as found by lookup.
    pub decl: DeclId,
    /// The name's location.
    pub loc: Loc,
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
            .and_then(|&m| self.string_text(m))
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
            self.special.pending_deleted.push(PendingDeleted { expr: e, decl: d, loc });
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

    /// At a call whose callee is `callee`: a pending deleted function
    /// named by it is diagnosed as `call to deleted function` unless
    /// `dependent` (or `invalid`, an argument being in error).
    pub(crate) fn act_on_deleted_callee(&mut self, callee: ExprId, dependent: bool, invalid: bool) {
        let Some(i) = self.special.pending_deleted.iter().position(|p| p.expr == callee) else {
            return;
        };
        let p = self.special.pending_deleted.swap_remove(i);
        if dependent || invalid {
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
        if let Selected::Missing =
            self.select_special(class, SpecialKind::DefaultCtor, Quals::NONE, Quals::NONE)
        {
            self.unsupported("default-initialization without a default constructor", loc);
            ok = false;
        } else {
            let e = self.construct_special(
                ty,
                class,
                SpecialKind::DefaultCtor,
                None,
                ConstructionKind::Complete,
                loc,
                InitEntity::Var,
            );
            match e {
                Some(e) => {
                    let e = self.finish_cleanups(e);
                    if self.ast.decls.var(v).is_some_and(|x| x.init.is_none()) {
                        let _ = self.builder().set_var_init(v, e);
                        self.ast.decls.mark_var(v, VarSpec::new().with_init_style(InitStyle::Call));
                    }
                }
                None => ok = false,
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
        if ok {
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
