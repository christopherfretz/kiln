//! `decltype` ([dcl.type.decltype]), `decltype(auto)` and deduced return
//! types ([dcl.spec.auto], [dcl.type.auto.deduct]; spec/sema.md §4.7).

use kiln_ast::expr::{ExprKind, ValueCategory};
use kiln_ast::types::{AutoKeyword, TypeView};
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
            Some(e) => self.deduce_placeholder(declared, e),
        };
        if !done {
            self.set_deduced_return(fdecl, deduced);
            return deduced;
        }
        if !self.is_error_type(deduced)
            && !self.is_error_type(ret)
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
        if self.undeduced_keyword(ret).is_none() {
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
