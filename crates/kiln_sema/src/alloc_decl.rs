//! Declaration rules for allocation and deallocation functions
//! ([basic.stc.dynamic.allocation]/1, [basic.stc.dynamic.deallocation]/1-2;
//! WP-1015): clang's `CheckOperatorNewDeclaration` /
//! `CheckOperatorDeleteDeclaration` (spec/sema.md §4.4.1 "Allocation function
//! declarations").

use kiln_ast::DeclId;
use kiln_ast::QualType;
use kiln_ast::decl::{DeclFlags, DeclKind, FnStorage};
use kiln_ast::names::OverloadedOperator;
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// [basic.stc.dynamic.allocation]/1, [basic.stc.dynamic.deallocation]/1:
    /// an allocation or deallocation function is a class member or a global
    /// function; one declared in a namespace other than the global one, or
    /// declared `static` at global scope, is ill-formed. Reported at the
    /// declaration's name; true when it did.
    pub(crate) fn check_new_delete_scope(&mut self, f: DeclId, fname: &str) -> bool {
        let n = self.ast.decls.get(f);
        let Some(ctx) = n.ctx() else { return false };
        let loc = n.loc();
        let rc = self.ast.redecl_context(ctx);
        let id = if self.kind_of(rc) == DeclKind::Namespace {
            DiagId::sema_operator_new_delete_in_namespace
        } else if rc == self.ast.tu
            && self.ast.decls.function(f).is_some_and(|x| x.spec.storage() == FnStorage::Static)
        {
            DiagId::sema_operator_new_delete_static
        } else {
            return false;
        };
        let m = self.diag(id, loc).arg(fname.to_owned());
        self.emit(m);
        true
    }

    /// The signature rules for allocation or deallocation function `f`
    /// (operator `op`, `templated` when it is a function template's
    /// pattern): the return type is `void *` (`void`), not dependent; a
    /// template has at least two parameters; there is at least one
    /// parameter; the first is `std::size_t` (`void *`), not dependent;
    /// an allocation function's first parameter has no default argument.
    /// Reports the first violation (clang's order) and returns false.
    ///
    /// A destroying `operator delete` ([basic.stc.dynamic.deallocation]/5)
    /// takes `C *` first instead (`check_destroying_delete`). Conservative:
    /// a type-aware form taking `std::type_identity<T>` first (P2719, a
    /// clang 22 extension kiln does not implement) is not checked, nor is a
    /// function with an undeduced return type.
    pub(crate) fn check_new_delete_signature(
        &mut self,
        f: DeclId,
        op: OverloadedOperator,
        templated: bool,
        fname: &str,
    ) -> bool {
        use OverloadedOperator as O;
        let destroying = self.ast.is_destroying_operator_delete(f);
        let new = matches!(op, O::New | O::ArrayNew);
        let loc = self.ast.decls.get(f).loc();
        let Some(ty) = self.ast.decls.get(f).ty() else { return true };
        let Some(proto) = self.ast.types.proto(ty).copied() else { return true };
        let ret = proto.ret();
        if self.is_error_type(ret) || self.contains_undeduced_auto(ret) {
            return true;
        }
        let params: Vec<DeclId> = self
            .ast
            .decls
            .function(f)
            .map(|fd| self.ast.lists.iter::<DeclId>(fd.params).collect())
            .unwrap_or_default();
        let first = params.first().and_then(|&p| self.ast.decls.get(p).ty());
        if first.is_some_and(|t| self.is_type_identity(t)) {
            return true;
        }
        let vp = self.ast.types.pointer_to(QualType::VOID);
        let class = self.ast.decls.get(f).ctx().filter(|_| destroying);
        let (want_ret, want_first) = match class {
            // [basic.stc.dynamic.deallocation]/5: within class `C`, the
            // first parameter of a destroying operator delete is `C *`.
            Some(c) => {
                let ct = self.injected_tag_type(c).unwrap_or_else(|| self.tag_type(c));
                (QualType::VOID, self.ast.types.pointer_to(ct))
            }
            None if new => (vp, self.size_type()),
            None => (QualType::VOID, vp),
        };
        if !self.ast.types.same_type(ret, want_ret) {
            let id = if self.ast.types.is_dependent(ret) {
                DiagId::sema_operator_new_delete_dependent_result_type
            } else {
                DiagId::sema_operator_new_delete_invalid_result_type
            };
            return self.new_delete_err(id, loc, fname, want_ret);
        }
        if templated && params.len() < 2 {
            let m = self
                .diag(DiagId::sema_operator_new_delete_template_too_few_parameters, loc)
                .arg(fname.to_owned());
            self.emit(m);
            return false;
        }
        let Some(first) = first else {
            let m = self
                .diag(DiagId::sema_operator_new_delete_too_few_parameters, loc)
                .arg(fname.to_owned());
            self.emit(m);
            return false;
        };
        if self.is_error_type(first) {
            return true;
        }
        if !self.ast.types.same_unqualified(first, want_first) {
            // clang 19 says "first parameter" (the `_19` rows, picked by
            // the engine); both print the canonical `size_t`.
            let dependent = self.ast.types.is_dependent(first);
            let id = match (new, destroying, dependent) {
                (true, _, true) => DiagId::sema_operator_new_dependent_param_type,
                (true, _, false) => DiagId::sema_operator_new_param_type,
                (false, true, true) => DiagId::sema_destroying_operator_delete_dependent_param_type,
                (false, true, false) => DiagId::sema_destroying_operator_delete_param_type,
                (false, false, true) => DiagId::sema_operator_delete_dependent_param_type,
                (false, false, false) => DiagId::sema_operator_delete_param_type,
            };
            // The injected-class-name prints with its parameters' names.
            let want_first =
                if destroying { want_first } else { self.ast.types.canonical(want_first) };
            return self.new_delete_err(id, loc, fname, want_first);
        }
        if let Some(c) = class
            && !self.ast.decls.get(c).flags().contains(DeclFlags::DEPENDENT_CTX)
            && !self.destroying_delete_is_usual(templated, &params)
        {
            let m = self.diag(DiagId::sema_destroying_operator_delete_not_usual, loc);
            self.emit(m);
            return false;
        }
        if new && params.first().is_some_and(|&p| self.param_has_default(p)) {
            let m = self.diag(DiagId::sema_operator_new_default_arg, loc).arg(fname.to_owned());
            self.emit(m);
            return false;
        }
        true
    }

    /// [basic.stc.dynamic.deallocation]/5: a destroying operator delete
    /// is a usual deallocation function: after `C *` and
    /// `std::destroying_delete_t`, an optional `std::size_t` then an
    /// optional `std::align_val_t`, and not a template (clang
    /// `isUsualDeallocationFunction`; a trailing `...` is allowed).
    fn destroying_delete_is_usual(&self, templated: bool, params: &[DeclId]) -> bool {
        if templated {
            return false;
        }
        let tys: Vec<QualType> =
            params.iter().skip(2).filter_map(|&p| self.ast.decls.get(p).ty()).collect();
        if tys.iter().any(|&t| self.is_error_type(t)) {
            return true;
        }
        let size_t = self.size_type();
        let mut rest = tys.as_slice();
        if let [t, tail @ ..] = rest
            && self.ast.types.same_type(*t, size_t)
        {
            rest = tail;
        }
        if let [t, tail @ ..] = rest
            && self.is_align_val_t(*t)
        {
            rest = tail;
        }
        rest.is_empty()
    }

    fn new_delete_err(&mut self, id: DiagId, loc: Loc, fname: &str, want: QualType) -> bool {
        let m = self.diag(id, loc).arg(fname.to_owned()).arg(self.ty_arg(want));
        self.emit(m);
        false
    }

    /// Whether `t` is a specialization of `std::type_identity` (the
    /// leading parameter of a P2719 type-aware allocation function).
    fn is_type_identity(&self, t: QualType) -> bool {
        let t = self.ast.types.canonical(t);
        if let Some(r) = self.ast.types.as_record(t) {
            return self.ast.is_std_named(r, "type_identity");
        }
        self.ty_str(t).contains("type_identity<")
    }
}
