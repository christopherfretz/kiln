//! Which previous declaration a function declaration redeclares
//! ([basic.scope.scope]/4, [over.load]; clang's `IsOverload`), so that
//! declarations that do not correspond declare overloads
//! (spec/sema-overload.md §2).

use kiln_ast::decl::{DeclKind, FnStorage, PayloadClass};
use kiln_ast::expr::ExprKind;
use kiln_ast::ids::RawId;
use kiln_ast::templates::is_template_kind;
use kiln_ast::types::ArgView;
use kiln_ast::{ArgListId, DeclId, ExprId, QualType, TemplateParamListId};

use crate::Sema;
use crate::lookup::is_tag_kind;

/// The new declaration being matched against previous ones.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NewFn {
    /// Its (declared) function type.
    pub ty: QualType,
    /// Declared `static` (a member function).
    pub is_static: bool,
    /// Its template head, for a function template.
    pub head: Option<TemplateParamListId>,
    /// Its trailing requires-clause.
    pub requires: Option<ExprId>,
}

impl Sema<'_, '_> {
    /// Picks, among the declarations `cands` found by redeclaration lookup
    /// (declaration order), the one the new function declaration `new`
    /// redeclares: the newest function (or the latest pattern of a
    /// function template) that corresponds to it; else the newest
    /// non-function entity of that name (a conflict); else none (`new`
    /// declares an overload).
    pub(crate) fn pick_prev_function(&mut self, cands: &[DeclId], new: NewFn) -> Option<DeclId> {
        let mut other = None;
        for &d in cands.iter().rev() {
            let k = self.kind_of(d);
            if is_tag_kind(k) {
                continue;
            }
            if is_template_kind(k) {
                if new.head.is_none() {
                    continue;
                }
                let Some(p) = self.pattern_of(d).map(|p| self.ast.latest(p)) else { continue };
                if k == DeclKind::FunctionTemplate {
                    let head = self.ast.template_info(d).map(|i| i.params);
                    if self.fn_corresponds(p, head, new) {
                        return Some(p);
                    }
                } else if other.is_none() {
                    other = Some(p);
                }
                continue;
            }
            if new.head.is_some() {
                continue;
            }
            if k.payload() == PayloadClass::Function {
                if self.fn_corresponds(d, None, new) {
                    return Some(d);
                }
            } else if other.is_none() {
                other = Some(d);
            }
        }
        other
    }

    /// Whether function `old` (with template head `old_head`) and `new`
    /// correspond ([basic.scope.scope]/4): same parameter-type-lists,
    /// object parameters and trailing requires-clauses, and for templates
    /// the same return type and equivalent template heads
    /// ([temp.over.link]/6).
    pub(crate) fn fn_corresponds(
        &mut self,
        old: DeclId,
        old_head: Option<TemplateParamListId>,
        new: NewFn,
    ) -> bool {
        let Some(oty) = self.declared_fn_type(old) else { return false };
        let t = &self.ast.types;
        let (Some(po), Some(pn)) = (t.proto(oty).copied(), t.proto(new.ty).copied()) else {
            // A non-prototype (error) type: treat as the same entity.
            return true;
        };
        let (ao, an) = (t.params(&po), t.params(&pn));
        if ao.len() != an.len()
            || po.info().variadic() != pn.info().variadic()
            || ao.iter().zip(an).any(|(x, y)| !t.same_type(*x, *y))
        {
            return false;
        }
        let old_static =
            self.ast.decls.function(old).is_some_and(|f| f.spec.storage() == FnStorage::Static);
        if self.kind_of(old) == DeclKind::CXXMethod
            && !old_static
            && !new.is_static
            && (po.info().this_quals() != pn.info().this_quals()
                || po.info().ref_qual() != pn.info().ref_qual())
        {
            return false;
        }
        match (old_head, new.head) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                if !self.ast.types.same_type(po.ret(), pn.ret()) || !self.heads_equivalent(a, b) {
                    return false;
                }
            }
            _ => return false,
        }
        let old_req = self.ast.templates.trailing_requires.get(old).copied();
        self.opt_exprs_equivalent(old_req, new.requires)
    }

    /// [temp.over.link]/6: equivalent template heads (same kinds, packs,
    /// non-type parameter types, type-constraints and requires-clauses).
    pub(crate) fn heads_equivalent(&self, a: TemplateParamListId, b: TemplateParamListId) -> bool {
        if a == b {
            return true;
        }
        let (la, lb) = (*self.ast.templates.param_list(a), *self.ast.templates.param_list(b));
        let pa: Vec<DeclId> = self.ast.lists.iter::<DeclId>(la.params).collect();
        let pb: Vec<DeclId> = self.ast.lists.iter::<DeclId>(lb.params).collect();
        if pa.len() != pb.len() || !self.opt_exprs_equivalent(la.requires, lb.requires) {
            return false;
        }
        pa.iter().zip(&pb).all(|(&x, &y)| {
            let (kx, ky) = (self.kind_of(x), self.kind_of(y));
            if kx != ky {
                return false;
            }
            let (ix, iy) = (self.ast.template_parm(x).copied(), self.ast.template_parm(y).copied());
            let (Some(ix), Some(iy)) = (ix, iy) else { return ix.is_none() && iy.is_none() };
            if ix.pack != iy.pack || !self.opt_exprs_equivalent(ix.constraint, iy.constraint) {
                return false;
            }
            match kx {
                DeclKind::NonTypeTemplateParm => {
                    let (tx, ty) = (self.ast.decls.get(x).ty(), self.ast.decls.get(y).ty());
                    match (tx, ty) {
                        (Some(tx), Some(ty)) => self.ast.types.same_type(tx, ty),
                        _ => false,
                    }
                }
                DeclKind::TemplateTemplateParm => match (ix.params, iy.params) {
                    (Some(a), Some(b)) => self.heads_equivalent(a, b),
                    (a, b) => a.is_none() && b.is_none(),
                },
                _ => true,
            }
        })
    }

    fn opt_exprs_equivalent(&self, a: Option<ExprId>, b: Option<ExprId>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => self.exprs_equivalent(a, b, 0),
            _ => false,
        }
    }

    /// A structural approximation of expression equivalence
    /// ([temp.over.link]/5) for comparing requires-clauses and
    /// type-constraints of redeclarations. Template parameters compare by
    /// position. todo(WP-073): profile-based equivalence.
    pub(crate) fn exprs_equivalent(&self, a: ExprId, b: ExprId, depth: u32) -> bool {
        if a == b {
            return true;
        }
        if depth > 256 {
            return false;
        }
        let e = &self.ast.exprs;
        let (na, nb) = (*e.get(a), *e.get(b));
        if na.kind() != nb.kind() || !self.ast.types.same_type(na.ty(), nb.ty()) {
            return false;
        }
        match na.kind() {
            ExprKind::DeclRefExpr => {
                let (da, db) = (e.decl(a), e.decl(b));
                if da == db || self.ast.first(da) == self.ast.first(db) {
                    return true;
                }
                match (self.ast.template_parm(da), self.ast.template_parm(db)) {
                    (Some(x), Some(y)) => x.depth == y.depth && x.index == y.index,
                    _ => false,
                }
            }
            ExprKind::ConceptSpecializationExpr => {
                let (ca, cb) = (na.a(), nb.a());
                if ca != cb {
                    return false;
                }
                let t = &self.ast.types;
                let (Some(la), Some(lb)) =
                    (ArgListId::from_raw(na.b()), ArgListId::from_raw(nb.b()))
                else {
                    return na.b() == nb.b();
                };
                let (xa, xb) = (t.args(la), t.args(lb));
                xa.len() == xb.len()
                    && xa.iter().zip(xb).all(|(x, y)| match (x.view(), y.view()) {
                        (ArgView::Type(p), ArgView::Type(q)) => t.same_type(p, q),
                        (ArgView::Expr { expr: p, .. }, ArgView::Expr { expr: q, .. }) => {
                            self.exprs_equivalent(p, q, depth + 1)
                        }
                        _ => t.canonical_arg(*x) == t.canonical_arg(*y),
                    })
            }
            // `sizeof(T)`/`alignof(T)`: the type operand by canonical type
            // (template parameters by position).
            ExprKind::UnaryExprOrTypeTraitExpr
                if e.trait_type_arg(a).is_some() || e.trait_type_arg(b).is_some() =>
            {
                na.aux() == nb.aux()
                    && match (e.trait_type_arg(a), e.trait_type_arg(b)) {
                        (Some(x), Some(y)) => self.ast.types.same_type(x, y),
                        _ => false,
                    }
            }
            ExprKind::TypeTraitExpr => {
                let l = &self.ast.lists;
                let xa: Vec<QualType> = l.iter::<QualType>(e.type_trait_args(a)).collect();
                let xb: Vec<QualType> = l.iter::<QualType>(e.type_trait_args(b)).collect();
                na.aux() == nb.aux()
                    && xa.len() == xb.len()
                    && xa.iter().zip(&xb).all(|(&x, &y)| self.ast.types.same_type(x, y))
            }
            ExprKind::BinaryOperator if e.binary_op(a) != e.binary_op(b) => false,
            ExprKind::UnaryOperator if e.unary_op(a) != e.unary_op(b) => false,
            _ => {
                let (ca, cb) = (e.children(a, &self.ast.lists), e.children(b, &self.ast.lists));
                if ca.is_empty() && cb.is_empty() {
                    return na.a() == nb.a() && na.b() == nb.b() && na.aux() == nb.aux();
                }
                ca.len() == cb.len()
                    && ca.iter().zip(&cb).all(|(&x, &y)| self.exprs_equivalent(x, y, depth + 1))
            }
        }
    }
}
