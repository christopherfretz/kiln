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

use kiln_ast::decl::{DeclKind, FnStorage};
use kiln_ast::{DeclId, ExprId, TemplateParamListId};

use crate::Sema;
use crate::overload::redecl::NewFn;

impl Sema<'_, '_> {
    /// [temp.over.link]/5: whether expressions `a` and `b` are equivalent
    /// (their canonical profiles are equal). Used for requires-clauses,
    /// type-constraints, and atomic constraints.
    pub(crate) fn exprs_equivalent(&self, a: ExprId, b: ExprId) -> bool {
        a == b || self.ast.exprs_equivalent(a, b)
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
        self.fn_corresponds(fa, ha, NewFn { ty, is_static, head: hb, requires })
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
}
