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
    /// takes `C *` first instead (`check_destroying_delete`); a type-aware
    /// form taking `std::type_identity<T>` first (P2719) is checked by
    /// `check_type_aware_signature`. A function with an undeduced return
    /// type is not checked.
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
            return self.check_type_aware_signature(f, new, &params, ret, fname);
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

    /// P2719 type-aware allocation and deallocation functions
    /// ([basic.stc.dynamic.allocation]/1, [basic.stc.dynamic.deallocation]/3,
    /// clang 22 `CheckOperatorNewDeleteTypes`): after the leading
    /// `std::type_identity<T>`, `operator new` takes `std::size_t` and
    /// `std::align_val_t`, `operator delete` takes `void *`, `std::size_t`
    /// and `std::align_val_t`; none of these has a default argument. A
    /// type-aware destroying delete is ill-formed. clang 22 warns that the
    /// form is an extension (`-Wext-cxx-type-aware-allocators`) before the
    /// checks. Reports the first violation and returns false.
    fn check_type_aware_signature(
        &mut self,
        f: DeclId,
        new: bool,
        params: &[DeclId],
        ret: QualType,
        fname: &str,
    ) -> bool {
        let loc = self.ast.decls.get(f).loc();
        let ptys: Vec<QualType> =
            params.iter().map(|&p| self.ast.decls.get(p).ty().unwrap_or(QualType::INT)).collect();
        let class = (self.kind_of(f) == DeclKind::CXXMethod)
            .then(|| self.ast.decls.get(f).ctx())
            .flatten();
        let vp = self.ast.types.pointer_to(QualType::VOID);
        let class_ptr = class.map(|c| {
            let ct = self.injected_tag_type(c).unwrap_or_else(|| self.tag_type(c));
            let p = self.ast.types.pointer_to(ct);
            self.ast.types.canonical(p)
        });
        // A type-aware destroying delete: clang names it rather than
        // reporting the parameter types that follow from it.
        if !new
            && let (Some(cp), Some(&p1)) = (class_ptr, ptys.get(1))
        {
            let p1 = self.ast.types.canonical(p1);
            if !self.ast.types.same_type(p1, vp)
                && self.ast.types.same_type(p1, cp)
                && ptys.iter().any(|&t| self.is_destroying_delete_t(t))
            {
                let m = self.diag(DiagId::sema_type_aware_destroying_operator_delete, loc);
                self.emit(m);
                return false;
            }
        }
        let m = self.diag(DiagId::ext_cxx_type_aware_allocators, loc);
        self.emit(m);
        let destroying =
            !new && class.is_some() && ptys.get(2).is_some_and(|&t| self.is_destroying_delete_t(t));
        let d = usize::from(destroying);
        let min = if new { 3 } else { 4 } + d;
        let size_idx = if new { 1 } else { 2 } + d;
        if ptys.len() < min {
            let words = ["three", "four", "five"];
            let m = self
                .diag(DiagId::sema_type_aware_new_delete_too_few_parameters, loc)
                .arg(fname.to_owned())
                .arg(d)
                .arg(words[min - 3]);
            self.emit(m);
            return false;
        }
        if params[..min].iter().any(|&p| self.param_has_default(p)) {
            let m = self.diag(DiagId::sema_operator_new_default_arg, loc).arg(fname.to_owned());
            self.emit(m);
            return false;
        }
        let want_ret = if new { vp } else { QualType::VOID };
        if !self.ast.types.same_type(ret, want_ret) {
            let id = if self.ast.types.is_dependent(ret) {
                DiagId::sema_operator_new_delete_dependent_result_type
            } else {
                DiagId::sema_operator_new_delete_invalid_result_type
            };
            return self.new_delete_err(id, loc, fname, want_ret);
        }
        let size_t = self.size_type();
        let first = if new {
            self.ast.types.canonical(size_t)
        } else if destroying {
            class_ptr.unwrap_or(vp)
        } else {
            vp
        };
        let align = self.declared_std_align_val_t();
        let checks = [(1, Some(first), "size_t"), (size_idx, Some(size_t), "size_t")];
        let checks = checks.into_iter().chain([(size_idx + 1, align, "std::align_val_t")]);
        for (idx, want, fallback) in checks {
            let actual = ptys[idx];
            if self.is_error_type(actual) {
                return true;
            }
            if want.is_some_and(|w| self.ast.types.same_unqualified(actual, w)) {
                continue;
            }
            let dependent = self.ast.types.is_dependent(actual);
            let want = want.map_or_else(|| fallback.to_owned(), |w| self.ty_arg(w));
            let ord = crate::expr::ordinal(idx + 1);
            let m = match (new, dependent) {
                (true, _) => {
                    let id = if dependent {
                        DiagId::sema_type_aware_operator_new_dependent_param_type
                    } else {
                        DiagId::sema_type_aware_operator_new_param_type
                    };
                    self.diag(id, loc).arg(fname.to_owned()).arg(ord).arg(fallback).arg(want)
                }
                (false, true) => self
                    .diag(DiagId::sema_type_aware_operator_delete_dependent_param_type, loc)
                    .arg(fname.to_owned())
                    .arg(d)
                    .arg(ord)
                    .arg(want),
                (false, false) => self
                    .diag(DiagId::sema_type_aware_operator_delete_param_type, loc)
                    .arg(fname.to_owned())
                    .arg(d)
                    .arg(ord)
                    .arg(want),
            };
            self.emit(m);
            return false;
        }
        true
    }

    /// Whether `f` (a function, or a function template's pattern) is a
    /// valid type-aware allocation or deallocation function: its first
    /// parameter is a `std::type_identity` specialization and its
    /// declaration passed `check_type_aware_signature` (clang
    /// `isTypeAwareOperatorNewOrDelete`).
    pub(crate) fn is_type_aware_alloc_fn(&self, f: DeclId) -> bool {
        let f = if self.kind_of(f) == DeclKind::FunctionTemplate {
            match self.templated_function(f) {
                Some(p) => p,
                None => return false,
            }
        } else {
            f
        };
        if self.ast.decls.get(f).has(DeclFlags::INVALID) {
            return false;
        }
        let Some(fd) = self.ast.decls.function(f) else { return false };
        let Some(p0) = self.ast.lists.iter::<DeclId>(fd.params).next() else { return false };
        self.ast.decls.get(p0).ty().is_some_and(|t| self.is_type_identity(t))
    }

    /// P2719 ([class.free]): a class that declares a type-aware
    /// `operator new` (`operator new[]`) also declares a type-aware
    /// `operator delete` (`operator delete[]`), and the converse;
    /// reported at the class, with a note at each unmatched declaration
    /// (clang 22 `CheckCompletedCXXClass`). Run on every completed class,
    /// a template's pattern and each instantiation alike.
    pub(crate) fn check_type_aware_allocator_pairs(&mut self, r: DeclId) {
        use OverloadedOperator as O;
        let mut found: [Vec<DeclId>; 4] = Default::default();
        for m in self.ast.members(r).collect::<Vec<_>>() {
            let Some(op) = self.ast.decls.get(m).name().as_operator() else { continue };
            let slot = match op {
                O::New => 0,
                O::Delete => 1,
                O::ArrayNew => 2,
                O::ArrayDelete => 3,
                _ => continue,
            };
            if self.is_type_aware_alloc_fn(m) {
                found[slot].push(m);
            }
        }
        if found.iter().all(Vec::is_empty) {
            return;
        }
        // A template's pattern prints as `S<T>` (its injected type).
        let ct = match self.ast.described_template(self.ast.first(r)) {
            Some(t) => self.injected_type_of(t),
            None => None,
        };
        let ct = ct.unwrap_or_else(|| self.tag_type(r));
        let class = self.ty_arg(ct);
        let rloc = self.ast.decls.get(r).loc();
        for (n, d, new_op, delete_op) in [(0, 1, O::New, O::Delete), (2, 3, O::ArrayNew, O::ArrayDelete)]
        {
            if found[n].is_empty() == found[d].is_empty() {
                continue;
            }
            let (have, missing) =
                if found[n].is_empty() { (delete_op, new_op) } else { (new_op, delete_op) };
            let mut m = self
                .diag(DiagId::sema_type_aware_allocator_missing_matching_operator, rloc)
                .arg(format!("operator {}", have.spelling()))
                .arg(class.clone())
                .arg(format!("operator {}", missing.spelling()));
            for &f in found[n].iter().chain(&found[d]) {
                let f = self.templated_function(f).unwrap_or(f);
                let at = self.ast.decls.get(f).loc();
                let fname = self.ast.decls.get(f).name().as_operator().map_or("", |o| o.spelling());
                let note = self
                    .diag(DiagId::note_unmatched_type_aware_allocator_declared, at)
                    .arg(format!("operator {fname}"));
                m = m.with_note(note);
            }
            // clang re-checks every instantiation of a class template.
            self.emit(m.per_instantiation());
        }
    }

    /// `std::destroying_delete_t` ([new.syn]).
    fn is_destroying_delete_t(&self, t: QualType) -> bool {
        self.ast.types.as_record(t).is_some_and(|r| self.ast.is_std_named(r, "destroying_delete_t"))
    }

    /// The program's `std::align_val_t`, when declared (clang
    /// `getStdAlignValT`): the implicit one once global `operator new` is
    /// declared, else a lookup in namespace `std`.
    fn declared_std_align_val_t(&mut self) -> Option<QualType> {
        if let Some(t) = self.global_ops.align_val_t {
            return Some(t);
        }
        let tu = self.ast.tu;
        let std = kiln_ast::names::DeclName::ident(self.intern("std"));
        let ns = self
            .ast
            .lookup_local(tu, std)
            .to_vec()
            .into_iter()
            .find(|&d| self.kind_of(d) == DeclKind::Namespace)?;
        let avt = kiln_ast::names::DeclName::ident(self.intern("align_val_t"));
        let e = self
            .ast
            .lookup_local(ns, avt)
            .to_vec()
            .into_iter()
            .find(|&d| self.kind_of(d) == DeclKind::Enum)?;
        Some(self.std_qualified(ns, e))
    }

    /// Whether `t` is a specialization of `std::type_identity` (the
    /// leading parameter of a P2719 type-aware allocation function, clang
    /// `isStdTypeIdentity`): cv-qualified or dependent, but not a
    /// reference or pointer to one.
    pub(crate) fn is_type_identity(&self, t: QualType) -> bool {
        let c = self.ast.types.canonical(t).without_quals();
        if let Some(r) = self.ast.types.as_record(c) {
            return self.ast.is_std_named(r, "type_identity");
        }
        if let kiln_ast::types::TypeView::TemplateSpecialization { name, .. } =
            self.ast.types.view(c.ty())
        {
            return self
                .ast
                .types
                .template_decl(name)
                .is_some_and(|d| self.ast.is_std_named(d, "type_identity"));
        }
        false
    }
}
