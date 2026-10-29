//! `decltype` ([dcl.type.decltype]), `decltype(auto)` and deduced return
//! types ([dcl.spec.auto], [dcl.type.auto.deduct]; spec/sema.md §4.3.1).

use kiln_ast::expr::{ExprKind, ValueCategory};
use kiln_ast::types::{AutoKeyword, Quals, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// `decltype ( e )` ([dcl.type.decltype]/1): the type `decltype(e)`
    /// sugar over the computed type, or `None` when `e` is erroneous.
    /// `e` was parsed as an unevaluated operand.
    pub fn act_on_decltype(&mut self, e: ExprId, _loc: Loc, _rparen: Loc) -> Option<QualType> {
        if self.is_err_expr(e) {
            return None;
        }
        let u = self.decltype_of(e);
        Some(self.ast.types.decltype_type(e, u, None))
    }

    /// The type `decltype(e)` denotes ([dcl.type.decltype]/1): the
    /// declared type of an unparenthesized id-expression or class member
    /// access; otherwise `T&&` for an xvalue, `T&` for an lvalue and `T`
    /// for a prvalue.
    pub(crate) fn decltype_of(&mut self, e: ExprId) -> QualType {
        let n = *self.ast.exprs.get(e);
        if matches!(n.kind(), ExprKind::DeclRefExpr | ExprKind::MemberExpr) {
            let d = self.ast.exprs.decl(e);
            if let Some(t) = self.ast.decls.get(d).ty() {
                return t;
            }
        }
        match n.value_category() {
            ValueCategory::LValue => self.ast.types.lvalue_ref_to(n.ty()),
            ValueCategory::XValue => self.ast.types.rvalue_ref_to(n.ty()),
            ValueCategory::PRValue => n.ty(),
        }
    }

    /// The keyword of an undeduced placeholder type `q` (`auto`,
    /// `decltype(auto)`), if `q` is one.
    pub(crate) fn undeduced_keyword(&self, q: QualType) -> Option<AutoKeyword> {
        match self.ast.types.view(self.ast.types.canonical(q).ty()) {
            TypeView::Auto { keyword, deduced: None, .. } => Some(keyword),
            _ => None,
        }
    }

    /// The type deduced for the placeholder `declared` from `e`
    /// ([dcl.type.auto.deduct]/4): for `auto`, the decayed type of `e`
    /// without top-level cv-qualifiers, plus the cv-qualifiers written
    /// on `declared`; for `decltype(auto)`, `decltype(e)`.
    pub(crate) fn deduce_placeholder(&mut self, declared: QualType, e: ExprId) -> QualType {
        if self.is_err_expr(e) {
            return QualType::ERROR;
        }
        if self.undeduced_keyword(declared) == Some(AutoKeyword::DecltypeAuto) {
            return self.decltype_of(e);
        }
        let ety = self.ast.exprs.get(e).ty();
        let t = &self.ast.types;
        let base = if t.is_array(ety) {
            let el = t.element(ety).unwrap_or(QualType::ERROR);
            self.ast.types.pointer_to(el)
        } else if t.is_function(ety) {
            self.ast.types.pointer_to(ety)
        } else {
            let c = self.ast.types.canonical(ety);
            if self.ast.types.effective_quals(ety).is_empty() { ety } else { c.without_quals() }
        };
        let q = declared.quals();
        if q.is_empty() { base } else { self.ast.types.qualify(base, q) }
    }

    /// True if `q` is an undeduced `auto` under zero or more pointer and
    /// reference declarators (`auto`, `const auto &`, `auto **`, ...;
    /// [dcl.spec.auto.general]/3).
    pub(crate) fn contains_undeduced_auto(&self, q: QualType) -> bool {
        let mut cur = q;
        for _ in 0..256 {
            match self.ast.types.canonical_view(cur) {
                TypeView::Auto { deduced: None, .. } => return true,
                TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => cur = p,
                _ => return false,
            }
        }
        false
    }

    /// The type deduced for a declared type `declared` containing a
    /// placeholder from initializer `e` ([dcl.type.auto.deduct]/4): a
    /// top-level placeholder as [`Self::deduce_placeholder`]; otherwise
    /// template argument deduction from a call ([temp.deduct.call]/2-4)
    /// with `auto` as the template parameter, restricted to the pointer
    /// and reference declarators [`Self::contains_undeduced_auto`]
    /// admits. `None` when deduction fails (the caller reports).
    pub(crate) fn deduce_from(&mut self, declared: QualType, e: ExprId) -> Option<QualType> {
        if self.undeduced_keyword(declared).is_some() {
            return Some(self.deduce_placeholder(declared, e));
        }
        if self.is_err_expr(e) {
            return Some(QualType::ERROR);
        }
        let n = *self.ast.exprs.get(e);
        let ety = n.ty();
        let t = &self.ast.types;
        match t.canonical_view(declared) {
            TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                let rref = matches!(t.canonical_view(declared), TypeView::RValueRef(_));
                // A forwarding reference `auto &&` bound to an lvalue
                // deduces `A &` ([temp.deduct.call]/3); the reference
                // collapses ([dcl.ref]/7).
                if rref
                    && n.value_category() == ValueCategory::LValue
                    && self.undeduced_keyword(p).is_some()
                    && t.canonical(p).quals().is_empty()
                {
                    return Some(self.ast.types.lvalue_ref_to(ety));
                }
                let inner = self.deduce_match(p, ety)?;
                Some(if rref {
                    self.ast.types.rvalue_ref_to(inner)
                } else {
                    self.ast.types.lvalue_ref_to(inner)
                })
            }
            _ => {
                // A non-reference P: A decays and loses its top-level cv
                // ([temp.deduct.call]/2).
                let a = if t.is_array(ety) {
                    let el = t.element(ety).unwrap_or(QualType::ERROR);
                    self.ast.types.pointer_to(el)
                } else if t.is_function(ety) {
                    self.ast.types.pointer_to(ety)
                } else {
                    crate::conv::strip_cv(self, ety)
                };
                self.deduce_match(declared, a)
            }
        }
    }

    /// Matches `p` (pointers down to a placeholder) against `a`, returning
    /// `p` with the placeholder replaced; the placeholder deduces `a`
    /// without the cv-qualifiers written on it ([temp.deduct.call]/4.2
    /// allows the extra qualification). `p`'s own top-level cv is the
    /// caller's.
    fn deduce_match(&mut self, p: QualType, a: QualType) -> Option<QualType> {
        let t = &self.ast.types;
        let pq = t.canonical(p).quals();
        match t.canonical_view(p) {
            TypeView::Auto { deduced: None, .. } => {
                let aq = self.ast.types.effective_quals(a);
                let rest = Quals::from_bits(aq.bits() & !pq.bits());
                let base = crate::conv::strip_cv(self, a);
                let leaf = if rest.is_empty() { base } else { self.ast.types.qualify(base, rest) };
                Some(if pq.is_empty() { leaf } else { self.ast.types.qualify(leaf, pq) })
            }
            TypeView::Pointer(pp) => {
                let TypeView::Pointer(ap) = t.canonical_view(a) else { return None };
                let inner = self.deduce_match(pp, ap)?;
                let r = self.ast.types.pointer_to(inner);
                Some(if pq.is_empty() { r } else { self.ast.types.qualify(r, pq) })
            }
            _ => None,
        }
    }

    /// `return e;` (or `return;` when `value` is `None`) in a function
    /// with a deduced return type ([dcl.spec.auto.general]/9,
    /// [dcl.type.auto.deduct]/2.3). Deduces the return type from the
    /// first return statement and checks later ones against it; returns
    /// the (deduced) return type to convert `value` to. `value` has not
    /// been through the implicit move yet.
    pub(crate) fn deduce_return(&mut self, value: Option<ExprId>, loc: Loc) -> QualType {
        let Some(state) = self.func.as_ref() else { return QualType::ERROR };
        let (fdecl, ret, done) = (state.decl, state.ret, state.deduced);
        let Some(declared) = state.auto_ret else { return ret };
        let kw = self.undeduced_keyword(declared).unwrap_or(AutoKeyword::Auto);
        // A type-dependent operand defers the deduction to instantiation
        // ([dcl.spec.auto.general]/12): the declared type keeps its
        // placeholder (clang prints `auto`).
        if let Some(e) = value
            && self.expr_is_type_dependent(e)
        {
            if let Some(st) = self.func.as_mut()
                && !st.deduced
            {
                st.deduced = true;
                st.ret = QualType::DEPENDENT;
            }
            return QualType::DEPENDENT;
        }
        let deduced = match value {
            None => QualType::VOID,
            Some(e) if kw == AutoKeyword::DecltypeAuto => {
                // An implicitly movable entity named by a parenthesized
                // id-expression is an xvalue ([expr.prim.id.unqual]/4,
                // C++23): `decltype((x))` is then `T&&`.
                let moved = self.implicit_move(e);
                if self.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr {
                    self.decltype_of(e)
                } else {
                    self.decltype_of(moved)
                }
            }
            Some(e) => {
                if let Some(t) = self.deduce_from(declared, e) {
                    t
                } else {
                    let msg = self
                        .diag(DiagId::sema_auto_fn_deduction_failure, self.ast.exprs.get(e).loc())
                        .arg(self.ty_arg(declared))
                        .arg(self.ty_arg(self.ast.exprs.get(e).ty()));
                    self.emit(msg);
                    QualType::ERROR
                }
            }
        };
        if !done {
            self.set_deduced_return(fdecl, deduced);
            return deduced;
        }
        if !self.is_error_type(deduced)
            && !self.is_error_type(ret)
            && !self.ast.types.is_dependent(ret)
            && self.ast.types.canonical(deduced) != self.ast.types.canonical(ret)
        {
            let sel = u8::from(kw == AutoKeyword::DecltypeAuto);
            let msg = self
                .diag(DiagId::sema_auto_fn_different_deductions, loc)
                .arg(sel)
                .arg(self.ty_arg(deduced))
                .arg(self.ty_arg(ret));
            self.emit(msg);
        }
        ret
    }

    /// Records `ret` as the deduced return type of function `f` and of
    /// all its redeclarations (clang updates every redeclaration's type).
    pub(crate) fn set_deduced_return(&mut self, f: DeclId, ret: QualType) {
        if let Some(state) = self.func.as_mut()
            && state.decl == f
        {
            state.ret = ret;
            state.deduced = true;
        }
        let Some(fty) = self.ast.decls.get(f).ty() else { return };
        let TypeView::Function(p) = self.ast.types.view(fty.ty()) else { return };
        let params = self.ast.types.params(&p).to_vec();
        let exc = self.ast.types.exc_spec(&p);
        let nty = self.ast.types.function(ret, &params, p.info(), exc);
        let redecls: Vec<DeclId> = self.ast.redecls_of(f).collect();
        for d in redecls {
            self.ast.decls.set_ty(d, nty);
        }
    }

    /// Uses of a function whose return type is still undeduced
    /// ([dcl.spec.auto.general]/11): reports and returns true.
    pub(crate) fn check_undeduced_use(&mut self, d: DeclId, loc: Loc) -> bool {
        let Some(fty) = self.ast.decls.get(d).ty() else { return false };
        let Some(ret) = self.ast.types.proto(fty).map(kiln_ast::types::FuncProto::ret) else {
            return false;
        };
        if !self.contains_undeduced_auto(ret) {
            return false;
        }
        let name = self.decl_name(d);
        let note = self
            .diag(DiagId::note_entity_declared_at, self.ast.decls.get(d).loc())
            .arg(format!("'{name}'"));
        let msg =
            self.diag(DiagId::sema_auto_fn_used_before_defined, loc).arg(name).with_note(note);
        self.emit(msg);
        true
    }
}
