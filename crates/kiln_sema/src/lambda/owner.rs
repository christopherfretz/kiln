//! Closure owners (spec/ast.md §5.9 "Closure owners", spec/sema-lambda.md
//! §3.6; WP-2023): the declaration whose source text holds a closure
//! type's lambda-expression, for tooling clients that key a closure as a
//! local type of that declaration. `Ast::closure_owner` answers it.
//!
//! Separate from the mangling context ([`super::ManglingState`]): that
//! follows the Itanium numbering rules and leaves most closures outside
//! function bodies without one; this is set for every closure.
//!
//! The owner is decided in two steps, as the written-reference table's
//! `enclosing` is (spec/ast.md §18.3):
//!
//! - at creation, from what sema knows then ([`Sema::closure_site_owner`]):
//!   the parameter whose default argument, the field whose default member
//!   initializer or the variable whose initializer is being parsed or
//!   instantiated, the function whose body is open, else the context;
//! - later, by claims: a declaration whose text is parsed before its
//!   `DeclId` exists (a function's signature, a parameter, a template
//!   parameter, an alias, a template head's requires-clause) claims the
//!   closures created since its start whose owner existed then
//!   ([`Sema::claim_closures`]), so the innermost claimer wins. A
//!   function whose body holds the closure is narrowed only by a
//!   parameter: a closure in a function body belongs to that function,
//!   not to a block-scope variable, but one in the type or default
//!   argument of a parameter of a lambda there belongs to that parameter.
//!   Instantiation claims too: a substituted class member and a function
//!   template specialization's signature.
//!
//! A claim by a template is a claim by its templated declaration.

use kiln_ast::DeclId;
use kiln_ast::decl::DeclKind;
use kiln_ast::ids::LambdaId;

use crate::Sema;

/// The kind classes [`Sema::instantiated_closure_owner`] matches a
/// pattern owner with.
#[derive(PartialEq, Eq)]
enum OwnerClass {
    Function,
    Param,
    Field,
    Var,
    Record,
    Alias,
    Other,
}

/// A class, including a class template specialization.
fn is_record(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
    )
}

fn owner_class(k: DeclKind) -> OwnerClass {
    if k.is_function() {
        return OwnerClass::Function;
    }
    match k {
        DeclKind::ParmVar => OwnerClass::Param,
        DeclKind::Field => OwnerClass::Field,
        DeclKind::Var
        | DeclKind::VarTemplateSpecialization
        | DeclKind::VarTemplatePartialSpecialization => OwnerClass::Var,
        DeclKind::Typedef | DeclKind::TypeAlias => OwnerClass::Alias,
        k if is_record(k) => OwnerClass::Record,
        _ => OwnerClass::Other,
    }
}

impl Sema<'_, '_> {
    /// The owner of a closure created now, from the numbering sites sema
    /// tracks: the parameter whose default argument is parsed or
    /// instantiated, the field whose default member initializer is, the
    /// function whose body is open, the variable whose initializer is
    /// (at namespace or class scope), else the current context (claims
    /// narrow it later).
    pub(crate) fn closure_site_owner(&self) -> DeclId {
        let mut ctx = self.ctx;
        while self.kind_of(ctx) == DeclKind::RequiresExprBody
            && let Some(c) = self.ast.decls.get(ctx).ctx()
        {
            ctx = c;
        }
        let kind = self.kind_of(ctx);
        let in_ctx = |d: DeclId| self.ast.decls.get(d).ctx() == Some(ctx);
        if let Some(p) = self.lambda_mangle.default_arg
            && (in_ctx(p) || is_record(kind))
        {
            return p;
        }
        if is_record(kind)
            && let Some(&(field, _)) = self.nsdmi_this.last()
            && in_ctx(field)
        {
            return field;
        }
        if kind.is_function() {
            return ctx;
        }
        if let Some(v) = self.lambda_mangle.var
            && in_ctx(v)
        {
            return v;
        }
        ctx
    }

    /// The number of declarations now: the mark of a sema-side claim
    /// ([`Self::claim_closures`]).
    pub(crate) fn decl_count(&self) -> u32 {
        u32::try_from(self.ast.decls.len()).unwrap_or(u32::MAX)
    }

    /// Records the owner of closure type `closure`, created now
    /// ([`Self::closure_site_owner`]).
    pub(crate) fn note_closure_owner(&mut self, closure: DeclId) {
        let o = self.closure_site_owner();
        self.ast.set_closure_owner(closure, o);
    }

    /// The owner of `closure`, rebuilt from the pattern closure `pattern`
    /// by a substitution whose local map gives `local` for the pattern's
    /// owner ([temp.inst]; spec/sema-lambda.md §3.6): that local (a
    /// parameter, a variable, an enclosing lambda's call operator); else
    /// the owner sema's state gives now when it is the same kind of
    /// declaration as the pattern's (the instantiated function whose body,
    /// parameter whose default argument, field whose default member
    /// initializer, variable whose initializer, class whose member is
    /// substituted); else the pattern's owner itself (a function template's
    /// signature during deduction, a default template argument substituted
    /// at a use, an alias template), which a declaration the substitution
    /// creates may still claim.
    pub(crate) fn inherit_closure_owner(
        &mut self,
        closure: DeclId,
        pattern: DeclId,
        local: impl FnOnce(DeclId) -> Option<DeclId>,
    ) {
        self.note_subst_closure(closure);
        let Some(po) = self.ast.closure_owner(pattern) else { return };
        let o = local(po).unwrap_or_else(|| {
            let state = self.closure_site_owner();
            if owner_class(self.kind_of(state)) == owner_class(self.kind_of(po))
                && owner_class(self.kind_of(po)) != OwnerClass::Other
            {
                state
            } else {
                po
            }
        });
        self.ast.set_closure_owner(closure, o);
    }

    /// Declaration `d`, whose text was parsed after the declaration count
    /// was `after`, claims the closures created since: each whose owner
    /// existed then ([`Self::claimable`]) gets `d` (spec/ast.md §5.9),
    /// except those substitution created (an instantiation the text
    /// triggered: their lambda-expressions are elsewhere). One comparison
    /// when no closure was created.
    pub(crate) fn claim_closures(&mut self, after: u32, d: DeclId) {
        self.claim_closures_in(after, d, None);
    }

    /// Declaration `d`, substituted from `pattern` after the declaration
    /// count was `after`, claims the closures created since whose owner is
    /// `pattern` or inside it (its parameters): those of its substituted
    /// signature (spec/sema-lambda.md §3.6).
    pub(crate) fn claim_substituted_closures(&mut self, after: u32, d: DeclId, pattern: DeclId) {
        self.claim_closures_in(after, d, Some(pattern));
    }

    fn claim_closures_in(&mut self, after: u32, d: DeclId, pattern: Option<DeclId>) {
        // A template's closures belong to its templated declaration.
        let templated = |x: DeclId| match self.ast.template_info(x) {
            Some(i) => i.pattern.unwrap_or(x),
            None => x,
        };
        let d = templated(d);
        let pattern = pattern.map(templated);
        // A parameter's text is its type and default argument, never a
        // block: it narrows a closure of the function whose body is open
        // too (a parameter of a lambda in that body, [dcl.fct.default]/3).
        let param = self.kind_of(d) == DeclKind::ParmVar;
        let mut i = self.ast.exprs.lambdas.len();
        while i > 0 {
            let id = LambdaId::new(u32::try_from(i).unwrap_or(u32::MAX));
            let c = self.ast.exprs.lambdas.get(id).closure;
            if c.index() <= after {
                break;
            }
            i -= 1;
            if !self.claimable(c, after, param) {
                continue;
            }
            let ok = match pattern {
                None => self.subst_closures.binary_search(&c).is_err(),
                Some(p) => self.ast.closure_owner(c).is_some_and(|o| self.within(o, p)),
            };
            if ok {
                self.ast.set_closure_owner(c, d);
            }
        }
    }

    /// Whether `d` is `outer` or declared inside it (its context chain).
    fn within(&self, d: DeclId, outer: DeclId) -> bool {
        let mut x = Some(d);
        while let Some(y) = x {
            if y == outer {
                return true;
            }
            x = self.ast.decls.get(y).ctx();
        }
        false
    }

    /// Notes closure type `c`, created by substitution (the parser's claims
    /// skip it).
    pub(crate) fn note_subst_closure(&mut self, c: DeclId) {
        // Ids grow, so the list stays sorted.
        if self.subst_closures.last().is_none_or(|&l| l < c) {
            self.subst_closures.push(c);
        }
    }

    /// The end of a top-level declaration: no claim is open any more.
    pub fn end_top_level_closures(&mut self) {
        self.subst_closures.clear();
    }

    /// The end of the template parameter parsed since the declaration
    /// count was `after`: it claims the closures of its default argument
    /// and type ([temp.param]/10).
    pub fn claim_template_param_closures(&mut self, after: u32) {
        if let Some(p) = self.templ.last_param()
            && p.index() > after
        {
            self.claim_closures(after, p);
        }
    }

    /// Before the innermost template head closes: the templated
    /// declaration it declared (found among the declarations created
    /// since the count was `after`) claims the closures of the head's
    /// requires-clause ([temp.pre]/9) that no inner declaration claimed.
    pub fn claim_template_head_closures(&mut self, after: u32) {
        if !self.has_unclaimed_closures(after) {
            return;
        }
        let Some(list) = self.templ.innermost_list() else { return };
        let len = self.decl_count();
        // The template is created right after its templated declaration,
        // near the start of the range.
        let owner = (after + 1..=len).map(DeclId::new).find_map(|t| {
            let i = self.ast.template_info(t)?;
            (i.params == list && i.decl == t).then(|| i.pattern.unwrap_or(t))
        });
        if let Some(o) = owner {
            self.claim_closures(after, o);
        }
    }

    /// Whether a closure created since the declaration count was `after`
    /// still has an owner that existed then and is not a function.
    fn has_unclaimed_closures(&self, after: u32) -> bool {
        let lambdas = &self.ast.exprs.lambdas;
        (1..=lambdas.len())
            .rev()
            .map(|i| lambdas.get(LambdaId::new(u32::try_from(i).unwrap_or(u32::MAX))).closure)
            .take_while(|c| c.index() > after)
            .any(|c| {
                self.claimable(c, after, false) && self.subst_closures.binary_search(&c).is_err()
            })
    }

    /// Whether a claim from the declaration count `after` may narrow the
    /// owner of closure `c`: one that existed then, unless it is a
    /// function whose body holds `c` (a block-scope declaration never
    /// owns a closure; a function's own signature closure, whose context
    /// is outside it, may be claimed by its specialization). A claim by a
    /// parameter (`param`) narrows that function too: the parameter of a
    /// lambda or of a block-scope function declaration in its body owns
    /// the closures of its type and default argument.
    fn claimable(&self, c: DeclId, after: u32, param: bool) -> bool {
        let Some(o) = self.ast.closure_owner(c) else { return false };
        if o.index() > after {
            return false;
        }
        if param || !self.kind_of(o).is_function() {
            return true;
        }
        self.ast.decls.get(c).ctx().is_none_or(|x| !self.within(x, o))
    }
}
