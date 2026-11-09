//! Which previous declaration a function declaration redeclares
//! ([basic.scope.scope]/4, [over.load]; clang's `IsOverload`), so that
//! declarations that do not correspond declare overloads
//! (spec/sema-overload.md §2).

use kiln_ast::decl::{DeclFlags, DeclKind, FnStorage, PayloadClass};
use kiln_ast::templates::is_template_kind;
use kiln_ast::types::RefQual;
use kiln_ast::{DeclId, ExprId, QualType, TemplateParamListId};

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
    /// Its `enable_if` conditions (`Sema::enable_if_list`).
    pub enable_if: kiln_ast::ids::ListId,
    /// Compare under the member using-declaration rules (clang's
    /// `UseMemberUsingDeclRules`, [namespace.udecl]/14): the return types
    /// of function templates do not take part.
    pub member_using: bool,
    /// Some parameter has `pass_object_size` (clang `IsOverload`: a
    /// function with such parameters and one without do not correspond;
    /// WP-803).
    pub pass_object_size: bool,
}

impl Sema<'_, '_> {
    /// Picks, among the declarations `cands` found by redeclaration lookup
    /// (declaration order), the one the new function declaration `new`
    /// redeclares: the newest function (or the latest pattern of a
    /// function template) that corresponds to it; else the newest
    /// non-function entity of that name (a conflict); else none (`new`
    /// declares an overload). A corresponding declaration that is invalid
    /// is not on any redeclaration chain: the newest valid one is preferred
    /// and an invalid one is returned only when no valid one corresponds
    /// (the caller then makes `new` silently invalid, clang's
    /// `MergeFunctionDecl` "if the old declaration is invalid, just give
    /// up here").
    #[cfg_attr(feature = "op-stats", track_caller)]
    pub(crate) fn pick_prev_function(&mut self, cands: &[DeclId], new: NewFn) -> Option<DeclId> {
        #[cfg(feature = "op-stats")]
        let (at, before) =
            (std::panic::Location::caller(), crate::templ::deduce_stats::prev_begin());
        let r = self.pick_prev_function_in(cands, new);
        #[cfg(feature = "op-stats")]
        {
            let scope = cands.first().and_then(|&d| self.ast.decls.get(d).ctx());
            let scope = scope.map_or_else(|| "-".to_owned(), |c| format!("{:?}", self.kind_of(c)));
            if cands.len() >= 32 {
                let name = self.decl_name_str(self.ast.decls.get(cands[0]).name());
                let head = if new.head.is_some() { "template" } else { "function" };
                crate::templ::deduce_stats::tally("prev-long", format!("{scope} {head} {name}"));
            }
            crate::templ::deduce_stats::prev_end(at, &scope, cands.len(), before);
        }
        r
    }

    fn pick_prev_function_in(&mut self, cands: &[DeclId], new: NewFn) -> Option<DeclId> {
        let mut other = None;
        let mut invalid = None;
        let mut found = |s: &Self, d: DeclId| {
            if s.ast.decls.get(d).has(DeclFlags::INVALID) {
                invalid = invalid.or(Some(d));
                false
            } else {
                true
            }
        };
        for (i, &d) in cands.iter().rev().enumerate() {
            // The position of a hit; cleared below when there is none.
            #[cfg(feature = "op-stats")]
            crate::templ::deduce_stats::prev_hit(Some(i));
            #[cfg(not(feature = "op-stats"))]
            let _ = i;
            let k = self.kind_of(d);
            // A dependent using-declarator (`using T::f;`) is checked
            // against the class's functions only at instantiation
            // ([namespace.udecl]/14; clang's `CheckUsingShadowDecl`).
            if is_tag_kind(k) || k == DeclKind::UnresolvedUsingValue {
                continue;
            }
            if is_template_kind(k) {
                if new.head.is_none() {
                    continue;
                }
                let Some(p) = self.pattern_of(d).map(|p| self.ast.latest(p)) else { continue };
                if k == DeclKind::FunctionTemplate {
                    let head = self.ast.template_info(d).map(|i| i.params);
                    if self.fn_corresponds(p, head, new) && found(self, p) {
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
                if self.fn_corresponds(d, None, new) && found(self, d) {
                    return Some(d);
                }
            } else if other.is_none() {
                other = Some(d);
            }
        }
        #[cfg(feature = "op-stats")]
        crate::templ::deduce_stats::prev_hit(None);
        invalid.or(other)
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
        let r = self.fn_corresponds_in(old, old_head, new, false);
        #[cfg(feature = "op-stats")]
        crate::templ::deduce_stats::compared(r);
        r
    }

    /// The member function among `cands` (redeclaration lookup in the
    /// class, declaration order) that the non-static member function
    /// `new` would correspond to but for its ref-qualifier: same
    /// parameter-type-list, template head and requires-clause, ref-
    /// qualifiers differing with one of them absent ([over.load]/2.3 of
    /// C++20; clang `IsOverload`'s `DiagnoseInconsistentRefQualifiers`).
    /// Neither has an explicit object parameter; cv-qualifiers are not
    /// compared (the caller does, by language mode).
    pub(crate) fn ref_qualifier_mix_prev(
        &mut self,
        cands: &[DeclId],
        new: NewFn,
    ) -> Option<DeclId> {
        for &d in cands {
            let k = self.kind_of(d);
            let (p, head) = if k == DeclKind::FunctionTemplate {
                if new.head.is_none() {
                    continue;
                }
                let Some(p) = self.pattern_of(d) else { continue };
                (p, self.ast.template_info(d).map(|i| i.params))
            } else if k == DeclKind::CXXMethod && new.head.is_none() {
                (d, None)
            } else {
                continue;
            };
            if self.kind_of(p) != DeclKind::CXXMethod
                || self.has_explicit_object(p)
                || self.ast.decls.get(p).has(DeclFlags::INVALID)
            {
                continue;
            }
            if self.fn_corresponds_in(p, head, new, true) {
                return Some(p);
            }
        }
        None
    }

    /// `ref_mix`: compare as [`Sema::ref_qualifier_mix_prev`] does
    /// (object parameters ignored but for a ref-qualifier present on
    /// exactly one side) instead of requiring corresponding ones.
    fn fn_corresponds_in(
        &mut self,
        old: DeclId,
        old_head: Option<TemplateParamListId>,
        new: NewFn,
        ref_mix: bool,
    ) -> bool {
        // The parameters are those of the declared type, whose return type
        // only differs from it when deduced: that is formed (interned)
        // only for the template return-type comparison below (WP-657).
        let Some(oty) = self.ast.decls.get(old).ty() else { return false };
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
        let both_non_static =
            self.kind_of(old) == DeclKind::CXXMethod && !old_static && !new.is_static;
        if ref_mix {
            let (ro, rn) = (po.info().ref_qual(), pn.info().ref_qual());
            if !both_non_static || ro == rn || (ro != RefQual::None && rn != RefQual::None) {
                return false;
            }
        } else if both_non_static
            && (po.info().this_quals() != pn.info().this_quals()
                || po.info().ref_qual() != pn.info().ref_qual())
        {
            return false;
        }
        match (old_head, new.head) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                if !new.member_using {
                    let oret = self
                        .declared_fn_type(old)
                        .and_then(|t| self.ast.types.proto(t).map(kiln_ast::types::FuncProto::ret))
                        .unwrap_or(po.ret());
                    if !self.ast.types.same_type(oret, pn.ret()) {
                        return false;
                    }
                }
                if !self.heads_equivalent(a, b) {
                    return false;
                }
            }
            _ => return false,
        }
        let old_req = self.ast.templates.trailing_requires.get(old).copied();
        self.opt_exprs_equivalent(old_req, new.requires)
            && self.same_enable_if(old, new.enable_if)
            && self.ast.pass_object_size_params(old).is_empty() != new.pass_object_size
    }

    /// [temp.over.link]/6: equivalent template heads (same kinds, packs,
    /// non-type parameter types, type-constraints and requires-clauses).
    pub(crate) fn heads_equivalent(&self, a: TemplateParamListId, b: TemplateParamListId) -> bool {
        if a == b {
            return true;
        }
        let (la, lb) = (*self.ast.templates.param_list(a), *self.ast.templates.param_list(b));
        let pa = self.ast.lists.iter::<DeclId>(la.params);
        let pb = self.ast.lists.iter::<DeclId>(lb.params);
        if pa.len() != pb.len() || !self.opt_exprs_equivalent(la.requires, lb.requires) {
            return false;
        }
        pa.zip(pb).all(|(x, y)| {
            let (kx, ky) = (self.kind_of(x), self.kind_of(y));
            if kx != ky || self.ttp_kind(x) != self.ttp_kind(y) {
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
}
