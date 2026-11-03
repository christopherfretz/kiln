//! Consteval-only types (M6-03, WP-472; P2996R13 [basic.types.general]
//! and [expr.const]; spec/reflection-repr.md §6).
//!
//! A consteval-only type is `std::meta::info` or a type built from one.
//! Its objects exist only during constant evaluation: a variable of such
//! a type is `constexpr` (or lives where every evaluation is constant),
//! a function of such a type is `consteval`, and an expression of such a
//! type appears only in an immediate function context. An expression
//! is tracked through its consteval-only leaves (a `^^` operand, a named
//! variable): they ride the immediate-reference list of the enclosing
//! full-expression ([`Sema::note_consteval_only`]) and are classified
//! with the immediate invocations in `consteval.rs`. A `std::meta::info`
//! value therefore never reaches code generation.

use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind};
use kiln_ast::expr::ExprKind;
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// Whether `t` is consteval-only ([basic.types.general], P2996R13):
    /// `std::meta::info`; a pointer, reference or array of one; a
    /// function type with a consteval-only return or parameter type; a
    /// member pointer with a consteval-only member type; a class with a
    /// consteval-only base or non-static data member. Always false
    /// without `-freflection`.
    pub(crate) fn is_consteval_only_type(&mut self, t: QualType) -> bool {
        if !self.reflection {
            return false;
        }
        let mut t = t;
        // Bounded: a type nests finitely; classes recurse below.
        loop {
            match self.ast.types.canonical_view(t) {
                TypeView::Builtin(_) => return self.is_meta_info(t),
                TypeView::Pointer(p)
                | TypeView::LValueRef(p)
                | TypeView::RValueRef(p)
                | TypeView::IncompleteArray(p)
                | TypeView::MemberPointer { pointee: p, .. } => t = p,
                TypeView::ConstantArray(_) | TypeView::VariableArray { .. } => {
                    let Some(e) = self.ast.types.element(t) else { return false };
                    t = e;
                }
                TypeView::Function(p) => {
                    let mut parts = vec![p.ret()];
                    parts.extend_from_slice(self.ast.types.params(&p));
                    return parts.into_iter().any(|q| self.is_consteval_only_type(q));
                }
                TypeView::Record(r) => return self.is_consteval_only_record(r),
                _ => return false,
            }
        }
    }

    /// [`Sema::is_consteval_only_type`] of class `r`, cached once the
    /// class is complete. A class reached again through its own members
    /// (`struct N { N* next; }`) counts as not consteval-only there: the
    /// answer is then decided by its other members.
    fn is_consteval_only_record(&mut self, r: DeclId) -> bool {
        let Some(def) = self.ast.definition(r) else { return false };
        if let Some(&b) = self.consteval_only_records.get(&def) {
            return b;
        }
        let complete = self
            .ast
            .decls
            .record(def)
            .is_some_and(|x| x.defn.contains(kiln_ast::decl::DefinitionBits::COMPLETE));
        self.consteval_only_records.insert(def, false);
        let bases: Vec<QualType> =
            self.ast.decls.bases(def, &self.ast.lists).map(|b| b.ty).collect();
        let fields: Vec<QualType> = self
            .ast
            .members(def)
            .filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field)
            .filter_map(|m| self.ast.decls.get(m).ty())
            .collect();
        let b = bases.into_iter().chain(fields).any(|q| self.is_consteval_only_type(q));
        if complete {
            self.consteval_only_records.insert(def, b);
        } else {
            self.consteval_only_records.remove(&def);
        }
        b
    }

    /// The expression `e` of consteval-only type (a `^^` operand, a
    /// reference to a consteval-only variable) was built: recorded for
    /// the end of its full-expression unless it is in an immediate
    /// function context ([expr.const], P2996R13). One built by
    /// substitution follows its pattern ([`Sema::note_inst_consteval_only`]).
    pub(crate) fn note_consteval_only(&mut self, e: ExprId) {
        if self.reflection && self.subst_expr_depth == 0 && !self.immediate_exempt() {
            self.note_pending_ref(e);
        }
    }

    /// `r`, substituted from the template's expression `e`: recorded
    /// when `e` was a consteval-only leaf outside every immediate
    /// function context of the definition (a template is checked in
    /// each instantiation, where the enclosing function may escalate).
    pub(crate) fn note_inst_consteval_only(&mut self, e: ExprId, r: ExprId) {
        if !self.consteval_only_pattern.is_empty()
            && self.consteval_only_pattern.contains(&e)
            && !self.immediate_exempt()
            && self.is_consteval_only_leaf(r)
        {
            self.note_pending_ref(r);
        }
    }

    /// A reference `e` to variable `d` ([`Sema::note_consteval_only`]
    /// when `d`'s type is consteval-only).
    pub(crate) fn note_consteval_only_ref(&mut self, e: ExprId, d: DeclId) {
        if !self.reflection
            || !matches!(
                self.ast.decls.get(d).kind(),
                DeclKind::Var | DeclKind::ParmVar | DeclKind::Binding
            )
        {
            return;
        }
        let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        if self.is_consteval_only_type(ty) {
            self.note_consteval_only(e);
        }
    }

    /// The expression `e` of a template, shared unchanged by an
    /// instantiation: its leaves recorded by the definition
    /// ([`Sema::note_inst_consteval_only`]) are recorded again, except in
    /// unevaluated operands, lambda bodies and immediate invocations.
    pub(crate) fn note_shared_consteval_only(&mut self, e: ExprId) {
        if self.consteval_only_pattern.is_empty() || self.immediate_exempt() {
            return;
        }
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            match self.ast.exprs.get(x).kind() {
                ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::ExpressionTraitExpr
                | ExprKind::RequiresExpr
                | ExprKind::ConceptSpecializationExpr
                | ExprKind::LambdaExpr
                | ExprKind::ConstantExpr
                | ExprKind::CXXTypeidExpr => continue,
                _ if self.consteval_only_pattern.contains(&x) => self.note_pending_ref(x),
                _ => {}
            }
            stack.extend(self.ast.exprs.children(x, &self.ast.lists));
        }
    }

    /// Whether the immediate reference `r` recorded for a full-expression
    /// is a consteval-only leaf ([`Sema::note_consteval_only`]) rather
    /// than a reference to an immediate function.
    pub(crate) fn is_consteval_only_leaf(&self, r: ExprId) -> bool {
        match self.ast.exprs.get(r).kind() {
            ExprKind::CXXReflectExpr => true,
            ExprKind::DeclRefExpr => !matches!(
                self.ast.decls.get(self.ast.exprs.decl(r)).kind(),
                DeclKind::Function | DeclKind::CXXMethod
            ),
            _ => false,
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
        let mut reported = false;
        let mut rest = Vec::with_capacity(refs.len());
        for r in refs {
            if !self.is_consteval_only_leaf(r) {
                rest.push(r);
            } else if !reported {
                reported = self.check_consteval_only_leaf(r, e, var, escalating);
            }
        }
        rest
    }

    /// The consteval-only leaf `r` of full-expression `e` outside every
    /// immediate invocation: allowed in the initializer of a variable
    /// whose initialization is constant ([expr.const]: it is manifestly
    /// constant-evaluated); in an immediate-escalating function `g`, it
    /// makes `g` immediate; otherwise an error. Returns whether it
    /// reported one (one per full-expression).
    pub(crate) fn check_consteval_only_leaf(
        &mut self,
        r: ExprId,
        e: ExprId,
        var: Option<DeclId>,
        escalating: Option<DeclId>,
    ) -> bool {
        if self.has_errors_in(e) {
            return false;
        }
        if let Some(v) = var {
            let constexpr = self.ast.decls.var(v).is_some_and(|x| x.spec.is_constexpr());
            if constexpr || (self.checks_constant_init(v) && self.try_fold(e).is_some()) {
                return false;
            }
        }
        // A template definition: checked in each instantiation.
        if self.in_template() {
            self.consteval_only_pattern.insert(r);
            return false;
        }
        if let Some(g) = escalating {
            self.ast.decls.mark_function(g, kiln_ast::decl::FnSpec::new().with_escalated(true));
            return false;
        }
        // The variable itself is reported ([`Sema::check_consteval_only_var`]).
        if let Some(v) = var
            && self.ast.decls.get(v).kind() == DeclKind::Var
        {
            let ty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
            if self.is_consteval_only_type(ty) {
                return false;
            }
        }
        let loc = self.begin_loc(r);
        self.error(DiagId::sema_consteval_only_expr, loc);
        true
    }

    /// Variable `v`, declared: one of consteval-only type is `constexpr`
    /// unless it is in an immediate function context or an immediate-
    /// escalating function ([basic.types.general], P2996R13: every
    /// object of consteval-only type is a constexpr variable's, a
    /// template parameter object, or one whose lifetime is within a
    /// constant evaluation). A template's variable is checked when
    /// instantiated.
    pub(crate) fn check_consteval_only_var(&mut self, v: DeclId) {
        if !self.reflection || self.in_template() || self.immediate > 0 {
            return;
        }
        let n = *self.ast.decls.get(v);
        if n.kind() != DeclKind::Var || n.has(DeclFlags::INVALID) {
            return;
        }
        // Constexpr, or an initializer already in error.
        if self
            .ast
            .decls
            .var(v)
            .is_some_and(|x| x.spec.is_constexpr() || x.init.is_some_and(|i| self.has_errors_in(i)))
        {
            return;
        }
        if let Some(f) = self.func.as_ref().map(|s| s.decl)
            && (self.is_immediate_fn(f) || self.is_immediate_escalating(f))
        {
            return;
        }
        let ty = n.ty().unwrap_or(QualType::ERROR);
        if self.is_consteval_only_type(ty) {
            let name = self.unqualified_decl_name(v);
            let msg = self.diag(DiagId::sema_consteval_only_var, n.loc()).arg(name);
            self.emit(msg);
        }
    }

    /// Function `f`, declared at `loc`: one of consteval-only type is
    /// `consteval` ([basic.types.general], P2996R13), unless it is
    /// immediate-escalating (a lambda's call operator, a defaulted
    /// member, a templated `constexpr` function: it becomes immediate
    /// through its body).
    pub(crate) fn check_consteval_only_fn(&mut self, f: DeclId, loc: Loc) {
        if !self.reflection {
            return;
        }
        let n = *self.ast.decls.get(f);
        let Some(x) = self.ast.decls.function(f) else { return };
        let spec = x.spec;
        if n.has(DeclFlags::INVALID) || self.is_immediate_fn(f) || self.is_immediate_escalating(f) {
            return;
        }
        // A templated constexpr function is immediate-escalating; the
        // other members of an instantiated class were checked in the
        // pattern (follow-up: a type that became consteval-only).
        if (self.in_template() && spec.constexpr_kind() == ConstexprKind::Constexpr)
            || self.inst.stack.in_instantiation_record()
        {
            return;
        }
        let ty = n.ty().unwrap_or(QualType::ERROR);
        if self.is_consteval_only_type(ty) {
            self.error(DiagId::sema_consteval_only_fn, loc);
        }
    }
}
