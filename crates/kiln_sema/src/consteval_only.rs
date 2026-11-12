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

use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind, Defaulted};
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
            && !self.inst_lost_consteval_only(e, r)
        {
            self.note_pending_ref(r);
        }
    }

    /// Whether pattern leaf `e`, a reference to a variable of dependent
    /// type ([`Sema::is_dependent_constexpr_leaf`]), was substituted to
    /// `r` whose variable is not of consteval-only type.
    fn inst_lost_consteval_only(&mut self, e: ExprId, r: ExprId) -> bool {
        if self.ast.exprs.get(e).kind() != ExprKind::DeclRefExpr
            || self.ast.exprs.get(r).kind() != ExprKind::DeclRefExpr
        {
            return false;
        }
        let pd = self.ast.exprs.decl(e);
        let pty = self.ast.decls.get(pd).ty().unwrap_or(QualType::ERROR);
        if !self.is_dependent_type(pty) {
            return false;
        }
        !self.is_consteval_only_var(self.ast.exprs.decl(r))
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
        if self.is_consteval_only_var(d) || self.is_dependent_constexpr_leaf(d, ty) {
            self.note_consteval_only(e);
        }
    }

    /// Whether a reference to variable `d` is a consteval-only leaf: `d`
    /// is of consteval-only type, or is a structured binding of a
    /// `constexpr` decomposition of consteval-only type (`n` of
    /// `constexpr auto [r, n] = e;`, as of a `template for` destructuring
    /// an aggregate with a `std::meta::info` member: the binding names a
    /// subobject of a consteval-only object, P2996R13 [expr.const]/25.3;
    /// "a glvalue that refers to an immediate object", P4101R1
    /// [expr.const.const]/5.1; GCC 16.1 rejects it too; WP-1330).
    fn is_consteval_only_var(&mut self, d: DeclId) -> bool {
        let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        if self.is_consteval_only_type(ty) {
            return true;
        }
        if self.ast.decls.get(d).kind() != DeclKind::Binding {
            return false;
        }
        let Some(v) = self.ast.decls.binding_decomp(d) else { return false };
        let vty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
        self.is_constexpr_var(v) && self.is_consteval_only_type(vty)
    }

    fn is_constexpr_var(&self, v: DeclId) -> bool {
        self.ast.decls.var(v).is_some_and(|x| x.spec.is_constexpr())
    }

    /// Whether a reference to `d` of type `ty` in a template definition
    /// is a candidate consteval-only leaf, decided in each instantiation
    /// and expansion ([`Sema::note_inst_consteval_only`]): a `constexpr`
    /// variable or a structured binding whose type is dependent (the
    /// `constexpr auto e` of a `template for`, a `constexpr T x` of a
    /// function template; WP-1330). A non-constexpr variable or a
    /// parameter of consteval-only type is reported as a declaration
    /// ([`Sema::check_consteval_only_var`], `check_consteval_only_inst`).
    fn is_dependent_constexpr_leaf(&self, d: DeclId, ty: QualType) -> bool {
        self.in_template()
            && self.is_dependent_type(ty)
            && match self.ast.decls.get(d).kind() {
                DeclKind::Var => self.is_constexpr_var(d),
                DeclKind::Binding => {
                    self.ast.decls.binding_decomp(d).is_some_and(|v| self.is_constexpr_var(v))
                }
                _ => false,
            }
    }

    /// A member access `e` naming data member or static member variable
    /// `m` with type `ty` (`this->r`, `h.sm`): a consteval-only leaf,
    /// as a reference to a variable is ([`Sema::note_consteval_only_ref`]).
    pub(crate) fn note_consteval_only_member(&mut self, e: ExprId, m: DeclId, ty: QualType) {
        if matches!(
            self.ast.decls.get(m).kind(),
            DeclKind::Field | DeclKind::IndirectField | DeclKind::Var
        ) && self.is_consteval_only_type(ty)
        {
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
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
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
            ExprKind::MemberExpr => matches!(
                self.ast.decls.get(self.ast.exprs.decl(r)).kind(),
                DeclKind::Field | DeclKind::IndirectField | DeclKind::Var
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
            self.escalate_consteval_only(g, r);
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
        // A member of a templated class is checked per instantiated
        // definition, when its class is known (`check_consteval_only_inst`;
        // GCC 16.1 checks no templated member through its class).
        if self.is_consteval_only_type(ty)
            || (!self.in_template() && self.has_consteval_only_object_param(f))
        {
            self.error(DiagId::sema_consteval_only_fn, loc);
        }
    }

    /// Instantiated function `f` of pattern `pattern`, at `loc`: the
    /// rule of `check_consteval_only_fn` for a type that became
    /// consteval-only by substitution (`template <class T> void h(T)`
    /// as `h<info>`, a member of `W<info>` with a `T` member), checked when
    /// the definition is instantiated, as GCC 16.1 does. A pattern whose
    /// own type was already consteval-only was reported at its
    /// declaration; a `constexpr` or defaulted pattern is
    /// immediate-escalating.
    pub(crate) fn check_consteval_only_inst(&mut self, f: DeclId, pattern: DeclId, loc: Loc) {
        if !self.reflection
            || self.ast.decls.get(f).has(DeclFlags::INVALID)
            || self.is_immediate_fn(f)
            || self.is_immediate_escalating(f)
        {
            return;
        }
        let Some(p) = self.ast.decls.function(pattern) else { return };
        if p.spec.constexpr_kind() != ConstexprKind::None || p.spec.defaulted() != Defaulted::No {
            return;
        }
        // Standard libraries that predate CWG3115 (libc++ 19, libstdc++
        // 14) define non-constexpr helpers (`vector::__throw_length_error`,
        // `ranges::__access::__begin`) that `vector<info>` instantiates; a
        // pattern in a system header is not diagnosed (kiln extension).
        let ploc = self.ast.decls.get(pattern).loc();
        if self.diags.sm().is_in_system_header(ploc) {
            return;
        }
        let ty = |s: &Self, d: DeclId| s.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        let (fty, pty) = (ty(self, f), ty(self, pattern));
        if (self.is_consteval_only_type(fty) || self.has_consteval_only_object_param(f))
            && !self.is_consteval_only_type(pty)
        {
            self.error(DiagId::sema_consteval_only_fn, loc);
        }
    }

    /// Whether implicit object member function `f` (a constructor and a
    /// destructor included) is of a consteval-only class: GCC 16.1 counts
    /// the implicit object parameter in the function's type (CWG3115), so
    /// the class's members declared so far decide, as for its other
    /// parameters.
    fn has_consteval_only_object_param(&mut self, f: DeclId) -> bool {
        if !self.is_implicit_object_member(f) {
            return false;
        }
        let Some(c) = self.ast.decls.get(f).ctx() else { return false };
        self.kind_of(c) == DeclKind::CXXRecord && self.is_consteval_only_record(c)
    }
}
