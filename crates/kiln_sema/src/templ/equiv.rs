//! Equivalence of expressions and of function template signatures
//! ([temp.over.link]; spec/sema-templates.md §11.2, WP-116).
//!
//! Two expressions involving template parameters are *equivalent*
//! ([temp.over.link]/5) exactly when their canonical profiles
//! ([`kiln_ast::Ast::profile_expr`], spec/types.md §7.2) are equal:
//! template parameters compare by (depth, index), function parameters by
//! position, unresolved names by spelling. Expressions that are only
//! *functionally* equivalent ([temp.over.link]/6: `N + 1` vs `1 + N`) are
//! not equivalent; a program relying on their being the same is
//! ill-formed, no diagnostic required, and kiln treats them as different
//! (as clang does).

use kiln_ast::decl::{DeclKind, FnStorage, FriendKind};
use kiln_ast::{DeclId, ExprId, TemplateParamListId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::overload::redecl::NewFn;

impl Sema<'_, '_> {
    /// [temp.over.link]/5: whether expressions `a` and `b` are equivalent
    /// (their canonical profiles are equal). Used for requires-clauses,
    /// type-constraints, and atomic constraints.
    pub(crate) fn exprs_equivalent(&self, a: ExprId, b: ExprId) -> bool {
        if a == b {
            return true;
        }
        let m = &self.friend_cmp_constraints;
        let (a, b) = if m.is_empty() {
            (a, b)
        } else {
            (m.get(&a).copied().unwrap_or(a), m.get(&b).copied().unwrap_or(b))
        };
        if a == b {
            return true;
        }
        let mut s = self.equiv_scratch.take();
        let r = self.ast.exprs_equivalent_in(a, b, &mut s);
        self.equiv_scratch.replace(s);
        r
    }

    /// [`Self::exprs_equivalent`] over optional expressions: both absent,
    /// or both present and equivalent.
    pub(crate) fn opt_exprs_equivalent(&self, a: Option<ExprId>, b: Option<ExprId>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => self.exprs_equivalent(a, b),
            _ => false,
        }
    }

    /// [temp.over.link]/7: whether function (template) declarations `a`
    /// and `b` have equivalent signatures — the same parameter-type-lists
    /// (after adjustment, [dcl.fct]/5), the same object parameters
    /// (cv- and ref-qualifiers of non-static members), equivalent
    /// trailing requires-clauses, and for templates the same return type
    /// and equivalent template heads ([temp.over.link]/6). Dependent
    /// types and expressions in them compare by their canonical profiles.
    ///
    /// Either argument may be a `FunctionTemplate`, the pattern of one,
    /// or a plain function. Scope and name are the caller's (this is the
    /// "corresponds" half of [basic.scope.scope]/4). Shared with
    /// deduction for explicit specializations and partial ordering
    /// (WP-106).
    pub fn equivalent_signatures(&mut self, a: DeclId, b: DeclId) -> bool {
        self.signatures_correspond(a, b, false)
    }

    /// [`Self::equivalent_signatures`]; `member_using`: under the member
    /// using-declaration rules ([namespace.udecl]/14, clang's
    /// `UseMemberUsingDeclRules`), where the return types of function
    /// templates are not compared.
    pub(crate) fn signatures_correspond(
        &mut self,
        a: DeclId,
        b: DeclId,
        member_using: bool,
    ) -> bool {
        let (Some((fa, ha)), Some((fb, hb))) = (self.fn_and_head(a), self.fn_and_head(b)) else {
            return false;
        };
        if fa == fb {
            return true;
        }
        let Some(ty) = self.declared_fn_type(fb) else { return false };
        let is_static = self.kind_of(fb) == DeclKind::CXXMethod
            && self.ast.decls.function(fb).is_some_and(|f| f.spec.storage() == FnStorage::Static);
        let requires = self.ast.templates.trailing_requires.get(fb).copied();
        let enable_if = self.enable_if_list_of(fb);
        self.fn_corresponds(
            fa,
            ha,
            NewFn { ty, is_static, head: hb, requires, enable_if, member_using },
        )
    }

    /// The function declaration and template head that `d` names: a
    /// function template's pattern and head, a pattern's own template
    /// head, or a plain function with none.
    fn fn_and_head(&self, d: DeclId) -> Option<(DeclId, Option<TemplateParamListId>)> {
        if self.kind_of(d) == DeclKind::FunctionTemplate {
            let head = self.ast.template_info(d).map(|i| i.params);
            return Some((self.pattern_of(d)?, head));
        }
        let head = self
            .ast
            .described_template(d)
            .and_then(|t| self.ast.template_info(t))
            .map(|i| i.params);
        self.ast.decls.function(d).map(|_| (d, head))
    }

    /// Whether class template `t` was declared by a friend declaration
    /// in a dependent context (`template<C U> friend class X;` in a class
    /// template): its head is deeper than the namespace scope template's
    /// and may name the enclosing template's parameters, so it is not
    /// compared with other declarations' heads ([temp.friend]; clang's
    /// `CheckClassTemplate` skips such a friend's head).
    pub(crate) fn is_dependent_friend_template(&self, t: DeclId) -> bool {
        self.kind_of(t) == DeclKind::ClassTemplate
            && self
                .pattern_of(t)
                .is_some_and(|p| self.ast.decls.get(p).misc().friend() != FriendKind::None)
            && self
                .ast
                .template_info(t)
                .is_some_and(|i| self.ast.templates.param_list(i.params).depth > 0)
    }

    /// The latest of `t` and its previous declarations (a class
    /// template's) that is not a friend in a dependent context
    /// ([`Self::is_dependent_friend_template`]): the declaration whose
    /// head a redeclaration's head is compared with.
    pub(crate) fn head_comparable_decl(&self, mut t: DeclId) -> Option<DeclId> {
        while self.is_dependent_friend_template(t) {
            t = self.ast.prev(self.pattern_of(t)?).and_then(|p| self.ast.described_template(p))?;
        }
        Some(t)
    }

    /// clang's `TemplateParameterListsAreEqual` (`TPL_TemplateMatch`):
    /// whether the template head `new` of a class, variable or alias
    /// template redeclaration is equivalent to the previous head `old`
    /// ([temp.over.link]/6: same length, kinds, packs, non-type
    /// parameter types, type-constraints and requires-clause). Diagnoses
    /// the first difference.
    pub(crate) fn check_template_heads_match(
        &mut self,
        new: TemplateParamListId,
        old: TemplateParamListId,
    ) -> bool {
        self.heads_match_in(new, old, false)
    }

    /// [`Self::check_template_heads_match`]; `nested`: the lists of a
    /// template template parameter (clang's
    /// `TPL_TemplateTemplateParmMatch`), which carry no requires-clause
    /// check of their own here.
    fn heads_match_in(
        &mut self,
        new: TemplateParamListId,
        old: TemplateParamListId,
        nested: bool,
    ) -> bool {
        if new == old {
            return true;
        }
        let which = u8::from(nested);
        let (ln, lo) = (*self.ast.templates.param_list(new), *self.ast.templates.param_list(old));
        let pn: Vec<DeclId> = self.ast.lists.iter::<DeclId>(ln.params).collect();
        let po: Vec<DeclId> = self.ast.lists.iter::<DeclId>(lo.params).collect();
        if pn.len() != po.len() {
            let d = self
                .diag(DiagId::sema_template_param_list_different_arity, ln.template_loc)
                .arg(u8::from(pn.len() > po.len()))
                .arg(which);
            let note =
                self.diag(DiagId::note_template_prev_declaration, lo.template_loc).arg(which);
            self.emit(d.with_note(note));
            return false;
        }
        for (&x, &y) in pn.iter().zip(&po) {
            if !self.params_match(x, y, which) {
                return false;
            }
        }
        if nested {
            return true;
        }
        if !self.opt_exprs_equivalent(ln.requires, lo.requires) {
            let at_n = ln.requires.map_or(ln.template_loc, |e| self.begin_loc(e));
            let at_o = lo.requires.map_or(lo.template_loc, |e| self.begin_loc(e));
            let d = self.diag(DiagId::sema_template_different_requires_clause, at_n);
            let note = self.diag(DiagId::note_template_prev_declaration, at_o).arg(0);
            self.emit(d.with_note(note));
            return false;
        }
        true
    }

    /// clang's `MatchTemplateParameterKind` for parameter `x` of a
    /// redeclaration against `y` of the previous declaration.
    fn params_match(&mut self, x: DeclId, y: DeclId, which: u8) -> bool {
        let (kx, ky) = (self.kind_of(x), self.kind_of(y));
        let (lx, ly) = (self.ast.decls.get(x).loc(), self.ast.decls.get(y).loc());
        // P2841R7 [temp.over.link]/6.1: a type, concept and variable
        // template template parameter are three different kinds.
        if kx != ky || self.ttp_kind(x) != self.ttp_kind(y) {
            let d = self.diag(DiagId::sema_template_param_different_kind, lx).arg(which);
            let note = self.diag(DiagId::note_template_prev_declaration, ly).arg(which);
            self.emit(d.with_note(note));
            return false;
        }
        let (Some(ix), Some(iy)) =
            (self.ast.template_parm(x).copied(), self.ast.template_parm(y).copied())
        else {
            return true;
        };
        let kind_index = match kx {
            DeclKind::TemplateTypeParm => 0u8,
            DeclKind::NonTypeTemplateParm => 1,
            _ => 2,
        };
        if ix.pack != iy.pack {
            let d = self
                .diag(DiagId::sema_template_parameter_pack_non_pack, lx)
                .arg(kind_index)
                .arg(u8::from(ix.pack));
            let note = self
                .diag(DiagId::note_template_parameter_pack_here, ly)
                .arg(kind_index)
                .arg(u8::from(iy.pack));
            self.emit(d.with_note(note));
            return false;
        }
        match kx {
            DeclKind::NonTypeTemplateParm => {
                let (tx, ty) = (self.ast.decls.get(x).ty(), self.ast.decls.get(y).ty());
                if let (Some(tx), Some(ty)) = (tx, ty)
                    && !self.ast.types.same_type(tx, ty)
                {
                    let d = self
                        .diag(DiagId::sema_template_nontype_parm_different_type, lx)
                        .arg(self.ty_arg(tx))
                        .arg(which);
                    let note = self
                        .diag(DiagId::note_template_nontype_parm_prev_declaration, ly)
                        .arg(self.ty_arg(ty));
                    self.emit(d.with_note(note));
                    return false;
                }
            }
            DeclKind::TemplateTemplateParm => {
                if let (Some(a), Some(b)) = (ix.params, iy.params)
                    && !self.heads_match_in(a, b, true)
                {
                    return false;
                }
            }
            _ => {}
        }
        if which == 0 && !self.opt_exprs_equivalent(ix.constraint, iy.constraint) {
            // An unconstrained parameter's diagnostic is at its first
            // token, the `class`/`typename` key (clang's `getBeginLoc`).
            let at = |s: &Self, c: Option<ExprId>, d: DeclId, l: Loc| {
                c.map_or_else(
                    || s.ast.decls.cold.begin_loc.get(d).copied().unwrap_or(l),
                    |e| s.begin_loc(e),
                )
            };
            let (at_x, at_y) = (at(self, ix.constraint, x, lx), at(self, iy.constraint, y, ly));
            let d = self.diag(DiagId::sema_template_different_type_constraint, at_x);
            let note = self.diag(DiagId::note_template_prev_declaration, at_y).arg(0);
            self.emit(d.with_note(note));
            return false;
        }
        true
    }
}
