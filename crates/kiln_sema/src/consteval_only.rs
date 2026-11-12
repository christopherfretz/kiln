//! Consteval-only values (P4101R1, the working draft's [expr.const.const]
//! and [expr.const.imm]; spec/reflection-repr.md §6).
//!
//! What may not reach run time is a *value*: a reflection other than the
//! null reflection, or a pointer (or pointer to member) to an immediate
//! function or into an immediate object ([expr.const.const]/1). An object
//! whose complete object holds one is an *immediate object* (/2) and is a
//! `constexpr` variable's, a template parameter object or one that lives
//! within a constant evaluation (/3); a potentially-evaluated expression
//! that is an *immediate constant expression* (a glvalue that refers to an
//! immediate object, a prvalue whose result object is one, /5) is
//! immediate-escalating ([expr.const.imm]/2.1). The values are frozen
//! constants and the predicate is `kiln_ast`'s
//! [`Ast::has_consteval_only_value`](kiln_ast::ast::Ast), shared with
//! codegen.
//!
//! Expressions are tracked through their *leaves*: a `^^` operand, a
//! reference to a variable, structured binding, static data member or
//! template parameter object whose value is consteval-only, a substituted
//! non-type template argument whose value is, and an immediate invocation
//! whose result is; and a default argument or default member initializer
//! holding one, at each use (where it joins the using full-expression, or
//! the constructor's body, [expr.const.imm]/4 Note 1). Each rides the
//! immediate-reference list of its
//! full-expression ([`Sema::note_consteval_only`]) and is classified with
//! the immediate invocations in `consteval.rs`: allowed in a manifestly
//! constant-evaluated initializer, escalating an immediate-escalating
//! function, an error elsewhere. A non-`constexpr` variable whose constant
//! initializer holds one is then checked by its value
//! ([`Sema::check_consteval_only_var`]). Nothing here looks at a type,
//! except to skip values that cannot hold a pointer or a reflection.
//!
//! Functions are not restricted by their type: a non-`consteval`
//! function may take and return `std::meta::info` (or a class holding
//! one), which exists at run time holding the null reflection
//! (spec/codegen.md "Run-time reflections"). What its body may not do is
//! decided by the same leaves: one outside an immediate function context
//! escalates an immediate-escalating function and is an error in any
//! other (WP-1361).

use kiln_ast::decl::{DeclFlags, DeclKind, VarEvalState};
use kiln_ast::expr::ExprKind;
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;

use crate::Sema;

impl Sema<'_, '_> {
    /// Whether a value of type `t` can be or hold a consteval-only value
    /// (a cheap filter before any value is looked at): everything but an
    /// arithmetic, enumeration, `bool`, `nullptr_t` or `void` type.
    pub(crate) fn may_hold_consteval_only(&self, t: QualType) -> bool {
        !matches!(
            self.ast.types.canonical_view(t),
            TypeView::Builtin(b) if b != BuiltinType::MetaInfo && b != BuiltinType::Dependent
        ) && self.ast.types.as_enum(self.ast.types.canonical(t)).is_none()
    }

    /// Whether variable or template parameter object `d` declares or
    /// refers to an immediate object ([expr.const.const]/2-3, P4101R1):
    /// its constant value has a consteval-only constituent. Cached once
    /// the variable's evaluation has a verdict. Always false without
    /// `-freflection`.
    pub(crate) fn is_immediate_var(&mut self, d: DeclId) -> bool {
        if !self.reflection {
            return false;
        }
        match self.ast.decls.get(d).kind() {
            DeclKind::Var => {}
            DeclKind::TemplateParamObject => return self.ast.is_immediate_var(d),
            _ => return false,
        }
        let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        if !self.may_hold_consteval_only(ty) {
            return false;
        }
        if let Some(&b) = self.immediate_vars.get(&d) {
            return b;
        }
        let b = self.ast.is_immediate_var(d);
        let settled = self.ast.redecls_of(d).any(|x| {
            self.ast.decls.var(x).is_some_and(|v| {
                matches!(v.eval.state(), VarEvalState::Value(_) | VarEvalState::NotConstant)
            })
        });
        if settled {
            self.immediate_vars.insert(d, b);
        }
        b
    }

    /// [`Sema::is_immediate_var`] of what a reference to `d` designates:
    /// a variable, a template parameter object, or the object a
    /// structured binding names a subobject of (the decomposed variable,
    /// or a tuple-like binding's own variable).
    fn is_immediate_ref_target(&mut self, d: DeclId) -> bool {
        match self.ast.decls.get(d).kind() {
            DeclKind::Var | DeclKind::TemplateParamObject => self.is_immediate_var(d),
            DeclKind::Binding => {
                self.ast.decls.binding_var(d).is_some_and(|h| self.is_immediate_var(h))
                    || self.ast.decls.binding_decomp(d).is_some_and(|v| self.is_immediate_var(v))
            }
            _ => false,
        }
    }

    /// The leaf `e` was built: recorded for the end of its
    /// full-expression unless it is in an immediate function context
    /// ([expr.const.imm]/2). One built by substitution follows its
    /// pattern ([`Sema::note_inst_consteval_only`]).
    pub(crate) fn note_consteval_only(&mut self, e: ExprId) {
        if self.reflection && self.subst_expr_depth == 0 && !self.immediate_exempt() {
            self.note_pending_ref(e);
        }
    }

    /// `r`, substituted from the template's expression `e`: recorded
    /// when `e` was a candidate leaf outside every immediate function
    /// context of the definition and `r`'s value is consteval-only (a
    /// template is checked in each instantiation, where the enclosing
    /// function may escalate).
    pub(crate) fn note_inst_consteval_only(&mut self, e: ExprId, r: ExprId) {
        if !self.consteval_only_pattern.is_empty()
            && self.consteval_only_pattern.contains(&e)
            && !self.immediate_exempt()
            && self.is_consteval_only_leaf(r)
            && self.leaf_is_immediate(r)
        {
            self.note_pending_ref(r);
        }
    }

    /// Whether leaf `r` is an immediate constant expression by its value
    /// ([expr.const.const]/5): a reflection; a reference to an immediate
    /// object; a substituted template argument or an immediate
    /// invocation whose value is consteval-only.
    pub(crate) fn leaf_is_immediate(&mut self, r: ExprId) -> bool {
        match self.ast.exprs.get(r).kind() {
            ExprKind::CXXReflectExpr => true,
            ExprKind::DeclRefExpr | ExprKind::MemberExpr => {
                self.is_immediate_ref_target(self.ast.exprs.decl(r))
            }
            ExprKind::SubstNonTypeTemplateParmExpr => {
                let x = self.ast.exprs.subst_replacement(r);
                if self.ast.exprs.get(x).kind() == ExprKind::DeclRefExpr {
                    return self.leaf_is_immediate(x);
                }
                self.try_fold(r).is_some_and(|c| self.ast.has_consteval_only_value(c))
            }
            ExprKind::ConstantExpr => self
                .ast
                .exprs
                .constant_value(r)
                .is_some_and(|c| self.ast.has_consteval_only_value(c)),
            ExprKind::CXXDefaultInitExpr => {
                let f = self.ast.exprs.default_init_field(r);
                self.default_leaf(f).is_some()
            }
            ExprKind::CXXDefaultArgExpr => {
                let p = self.ast.exprs.default_arg_param(r);
                self.default_leaf(p).is_some()
            }
            _ => false,
        }
    }

    /// The consteval-only leaf of `decl`'s default argument or default
    /// member initializer, if it has one: it is checked at each use, where
    /// the initializer becomes part of the using full-expression (or of
    /// the constructor body, [expr.const.imm]/4 Note 1), not where it is
    /// written. Its immediate invocations are checked by their results
    /// there ([`Sema::check_default_use`]); those in an unevaluated
    /// operand or a lambda body are not leaves.
    pub(crate) fn default_leaf(&mut self, decl: DeclId) -> Option<ExprId> {
        if !self.reflection {
            return None;
        }
        if let Some(&l) = self.default_leaves.get(&decl) {
            return l;
        }
        let root = self
            .ast
            .decls
            .param(decl)
            .and_then(|p| p.default_arg)
            .or_else(|| self.ast.decls.field(decl).and_then(|f| f.init))?;
        // A self-using default (ill-formed) ends here.
        self.default_leaves.insert(decl, None);
        let mut stack = vec![root];
        let mut found = None;
        let mut steps = 0usize;
        while let Some(x) = stack.pop() {
            steps += 1;
            if steps > 1_000_000 {
                break;
            }
            match self.ast.exprs.get(x).kind() {
                ExprKind::ConstantExpr
                | ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::ExpressionTraitExpr
                | ExprKind::RequiresExpr
                | ExprKind::ConceptSpecializationExpr
                | ExprKind::LambdaExpr
                | ExprKind::CXXTypeidExpr => {}
                ExprKind::CXXReflectExpr
                | ExprKind::DeclRefExpr
                | ExprKind::MemberExpr
                | ExprKind::SubstNonTypeTemplateParmExpr
                | ExprKind::CXXDefaultInitExpr
                | ExprKind::CXXDefaultArgExpr
                    if self.leaf_is_immediate(x) =>
                {
                    found = Some(x);
                    break;
                }
                _ => {
                    let kids = self.ast.exprs.children(x, &self.ast.lists);
                    stack.extend(kids.into_iter().rev());
                }
            }
        }
        self.default_leaves.insert(decl, found);
        found
    }

    /// A reference `e` to declaration `d`: a leaf when `d` declares or
    /// refers to an immediate object ([expr.const.const]/3, /5.1). In a
    /// template definition a reference whose value is not known yet (a
    /// `constexpr` variable of dependent type or initializer, a non-type
    /// template parameter) is a candidate, decided in each instantiation
    /// ([`Sema::note_inst_consteval_only`]). A reference to a variable
    /// not yet defined is remembered ([`Sema::note_early_odr_use`]).
    pub(crate) fn note_consteval_only_ref(&mut self, e: ExprId, d: DeclId) {
        if !self.reflection {
            return;
        }
        let kind = self.ast.decls.get(d).kind();
        let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        match kind {
            DeclKind::Var | DeclKind::Binding | DeclKind::TemplateParamObject => {}
            DeclKind::NonTypeTemplateParm => {
                if self.in_template() && self.may_hold_consteval_only(ty) {
                    self.note_consteval_only(e);
                }
                return;
            }
            _ => return,
        }
        if self.is_immediate_ref_target(d) || self.is_pending_constexpr_leaf(d, ty) {
            self.note_consteval_only(e);
        } else if kind == DeclKind::Var {
            self.note_early_odr_use(e, d, ty);
        }
    }

    pub(crate) fn is_constexpr_var(&self, v: DeclId) -> bool {
        self.ast.decls.var(v).is_some_and(|x| x.spec.is_constexpr())
    }

    /// Whether a reference to `d` of type `ty` in a template definition
    /// is a candidate leaf, decided in each instantiation and expansion
    /// ([`Sema::note_inst_consteval_only`]): a `constexpr` variable or a
    /// binding of a `constexpr` structured binding whose value is not
    /// known in the definition (a dependent type or initializer: the
    /// `constexpr auto e` of a `template for`, a `constexpr T x` of a
    /// function template, `constexpr info r = ^^T;`; WP-1330).
    fn is_pending_constexpr_leaf(&self, d: DeclId, ty: QualType) -> bool {
        if !self.in_template() || !self.may_hold_consteval_only(ty) {
            return false;
        }
        let v = match self.ast.decls.get(d).kind() {
            DeclKind::Var => d,
            DeclKind::Binding => match self.ast.decls.binding_decomp(d) {
                Some(v) => v,
                None => return false,
            },
            _ => return false,
        };
        let Some(var) = self.ast.decls.var(v) else { return false };
        var.spec.is_constexpr()
            && (self.is_dependent_type(ty)
                || var.init.is_some_and(|i| self.expr_is_value_dependent(i))
                || !matches!(var.eval.state(), VarEvalState::Value(_) | VarEvalState::NotConstant))
    }

    /// A potentially-evaluated reference `e` to variable `d` of type `ty`
    /// outside every immediate function context, before `d` has an
    /// initializer (`extern const S s;`): it odr-uses `d` unless `d` is
    /// usable in constant expressions, which it is not yet. Remembered,
    /// and reported if the definition makes `d` an immediate object: the
    /// diagnostic is required as the use is reachable from it
    /// ([expr.const.const]/3, /3.4-3.5).
    fn note_early_odr_use(&mut self, e: ExprId, d: DeclId, ty: QualType) {
        if self.in_template()
            || self.subst_expr_depth != 0
            || self.immediate_exempt()
            || !self.may_hold_consteval_only(ty)
            || !self.has_static_storage(d)
            || self
                .ast
                .redecls_of(d)
                .any(|x| self.ast.decls.var(x).is_some_and(|v| v.init.is_some()))
        {
            return;
        }
        let loc = self.begin_loc(e);
        let first = self.ast.first(d);
        self.early_odr_uses.entry(first).or_default().push(loc);
    }

    /// `constexpr` variable `v` was initialized: the references made to
    /// it before its definition ([`Sema::note_early_odr_use`]) are errors
    /// if it is an immediate object.
    pub(crate) fn report_early_odr_uses(&mut self, v: DeclId) {
        if self.early_odr_uses.is_empty() {
            return;
        }
        let first = self.ast.first(v);
        let Some(locs) = self.early_odr_uses.remove(&first) else { return };
        if !self.is_immediate_var(v) {
            return;
        }
        let def = self.ast.decls.get(v).loc();
        for loc in locs {
            let note = self.diag(DiagId::note_declared_at, def);
            let msg = self.diag(DiagId::sema_consteval_only_expr, loc).with_note(note);
            self.emit(msg);
        }
    }

    /// A member access `e` naming static member variable `m` (`h.sm`,
    /// `S::sm` through an object): a leaf, as a reference to a variable
    /// is ([`Sema::note_consteval_only_ref`]). A non-static data member
    /// is never one by itself: its object expression decides.
    pub(crate) fn note_consteval_only_member(&mut self, e: ExprId, m: DeclId, _ty: QualType) {
        if self.ast.decls.get(m).kind() == DeclKind::Var && self.is_immediate_var(m) {
            self.note_consteval_only(e);
        }
    }

    /// The expression `e` of a template, shared unchanged by an
    /// instantiation: its leaves recorded by the definition
    /// ([`Sema::note_inst_consteval_only`]) are recorded again, except in
    /// unevaluated operands, lambda bodies and immediate invocations (an
    /// immediate invocation that is a leaf itself is recorded).
    pub(crate) fn note_shared_consteval_only(&mut self, e: ExprId) {
        if self.consteval_only_pattern.is_empty() || self.immediate_exempt() {
            return;
        }
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            let kind = self.ast.exprs.get(x).kind();
            if self.consteval_only_pattern.contains(&x) && self.leaf_is_immediate(x) {
                self.note_pending_ref(x);
            }
            match kind {
                ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::ExpressionTraitExpr
                | ExprKind::RequiresExpr
                | ExprKind::ConceptSpecializationExpr
                | ExprKind::LambdaExpr
                | ExprKind::ConstantExpr
                | ExprKind::CXXTypeidExpr => continue,
                _ => {}
            }
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
    }

    /// Whether the immediate reference `r` recorded for a full-expression
    /// is a consteval-only leaf ([`Sema::note_consteval_only`]) rather
    /// than a reference to an immediate function.
    pub(crate) fn is_consteval_only_leaf(&self, r: ExprId) -> bool {
        match self.ast.exprs.get(r).kind() {
            ExprKind::DeclRefExpr | ExprKind::MemberExpr => !matches!(
                self.ast.decls.get(self.ast.exprs.decl(r)).kind(),
                DeclKind::Function | DeclKind::CXXMethod
            ),
            _ => true,
        }
    }

    /// `refs` (references outside every immediate invocation of
    /// full-expression `e`) without its consteval-only leaves, each checked
    /// by [`Self::check_consteval_only_leaf`] until one is reported.
    pub(crate) fn check_consteval_only_leaves(
        &mut self,
        refs: Vec<ExprId>,
        e: ExprId,
        var: Option<DeclId>,
        escalating: Option<DeclId>,
    ) -> Vec<ExprId> {
        self.check_consteval_only_leaves_reported(refs, e, var, escalating).0
    }

    /// [`Self::check_consteval_only_leaves`], and whether one was
    /// reported.
    pub(crate) fn check_consteval_only_leaves_reported(
        &mut self,
        refs: Vec<ExprId>,
        e: ExprId,
        var: Option<DeclId>,
        escalating: Option<DeclId>,
    ) -> (Vec<ExprId>, bool) {
        let mut reported = false;
        let mut rest = Vec::with_capacity(refs.len());
        for r in refs {
            if !self.is_consteval_only_leaf(r) {
                rest.push(r);
            } else if !reported {
                reported = self.check_consteval_only_leaf(r, e, var, escalating);
            }
        }
        (rest, reported)
    }

    /// Whether the full-expression ending is itself a manifestly
    /// constant-evaluated constant-expression (a template argument, a
    /// `static_assert` condition, an array bound, a `case` label, a
    /// `noexcept` operand, ... finished by
    /// [`Sema::act_on_constant_full_expr`]): its subexpressions are in an
    /// immediate function context ([expr.const.imm]/1.2), so its
    /// consteval-only leaves are allowed. While the expression is parsed
    /// the context is entered up front ([`Sema::immediate_exempt`]); when
    /// it is rebuilt by template instantiation it is known only here, so
    /// both spellings meet this one rule.
    pub(crate) fn in_constant_full_expr(&self) -> bool {
        self.constant_full_expr > 0
    }

    /// The consteval-only leaf `r` of full-expression `e` outside every
    /// immediate invocation: allowed in the initializer of a variable
    /// whose initialization is constant (it is manifestly
    /// constant-evaluated, [expr.const.defns]; a non-`constexpr` one is
    /// then checked by its value, [`Sema::check_consteval_only_var`]); in
    /// an immediate-escalating function `g`, it makes `g` immediate
    /// ([expr.const.imm]/4.2); otherwise an error ([expr.const.imm]/3).
    /// Returns whether it reported one (one per full-expression).
    pub(crate) fn check_consteval_only_leaf(
        &mut self,
        r: ExprId,
        e: ExprId,
        var: Option<DeclId>,
        escalating: Option<DeclId>,
    ) -> bool {
        if self.has_errors_in(e) || self.in_constant_full_expr() {
            return false;
        }
        if let Some(v) = var {
            if self.is_constexpr_var(v) {
                return false;
            }
            if self.checks_constant_init(v) && self.try_fold(e).is_some() {
                self.consteval_only_inits.insert(v);
                return false;
            }
        }
        // A template definition: checked in each instantiation.
        if self.in_template() {
            self.consteval_only_pattern.insert(r);
            return false;
        }
        if let Some(g) = escalating {
            self.escalate_consteval_only(g, r);
            return false;
        }
        // A default argument or member initializer has no first token of
        // its own: it is reported where it is used (clang
        // `CXXDefaultArgExpr::getUsedLocation`).
        let loc = match self.ast.exprs.get(r).kind() {
            ExprKind::CXXDefaultArgExpr | ExprKind::CXXDefaultInitExpr => {
                self.imm.default_use_locs.get(&r).copied().unwrap_or(self.ast.exprs.get(r).loc())
            }
            _ => self.begin_loc(r),
        };
        self.error(DiagId::sema_consteval_only_expr, loc);
        true
    }

    /// Variable `v`, declared: a non-`constexpr` one whose constant
    /// initializer held a consteval-only leaf is an error if its value is
    /// consteval-only, as its object is then an immediate object that is
    /// not a `constexpr` variable's ([expr.const.const]/3, P4101R1). A
    /// reference refers to such an object without being one: its value is
    /// recorded instead (it is usable in constant expressions), so each
    /// use of it is a leaf. Every other variable is left to its
    /// initializer's leaves (a run-time initializer reports them), so a
    /// variable that merely can hold a reflection (`std::meta::info x;`,
    /// one holding the null reflection) is not restricted.
    pub(crate) fn check_consteval_only_var(&mut self, v: DeclId) {
        if !self.reflection || self.consteval_only_inits.is_empty() {
            return;
        }
        if !self.consteval_only_inits.remove(&v) || self.in_template() || self.immediate > 0 {
            return;
        }
        let n = *self.ast.decls.get(v);
        if n.kind() != DeclKind::Var || n.has(DeclFlags::INVALID) || self.is_constexpr_var(v) {
            return;
        }
        let Some(init) = self.ast.decls.var(v).and_then(|x| x.init) else { return };
        if self.has_errors_in(init) {
            return;
        }
        let Some(c) = self.fold_constant_init(init, v) else { return };
        if !self.ast.has_consteval_only_value(c) {
            return;
        }
        let ty = n.ty().unwrap_or(QualType::ERROR);
        if self.ast.types.is_reference(ty) {
            if self.ast.decls.begin_eval(v).is_ok() {
                self.ast.decls.finish_eval(v, Some(c));
                self.immediate_vars.remove(&v);
            }
            return;
        }
        let name = self.unqualified_decl_name(v);
        let msg = self.diag(DiagId::sema_consteval_only_var, n.loc()).arg(name);
        self.emit(msg);
    }
}
