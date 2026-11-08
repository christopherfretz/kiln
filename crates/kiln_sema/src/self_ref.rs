//! `-Wuninitialized` / `-Wstatic-self-init` for a variable used in its own
//! initializer (clang `CheckSelfReference` and `SelfReferenceChecker`,
//! `SemaDecl.cpp`; spec/sema.md §6).
//!
//! clang leaves most local variables to its CFG-based uninitialized-values
//! analysis (kiln's is [`crate::uninit`], spec/sema.md §7.14). This walk
//! covers what clang's syntactic checker covers: variables without automatic storage
//! duration, and local variables of class or reference type.

use kiln_ast::decl::{DeclKind, TlsKind, VarStorage};
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind, TraitKind, UnaryOp};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Span;

use crate::Sema;
use crate::class::SpecialKind;

/// The walk's state (clang `SelfReferenceChecker`'s members).
struct SelfRef {
    /// The variable being initialized.
    var: DeclId,
    /// For a variable of class type, whether the class is POD (taking the
    /// address of one of its members is then not a use).
    record_pod: Option<bool>,
    reference: bool,
    /// Inside a (semantic) initializer list.
    init_list: bool,
    /// The index of the member each enclosing initializer list is at.
    index: Vec<usize>,
}

/// Recursion guard: initializers deeper than this are not walked.
const MAX_DEPTH: u32 = 512;

impl Sema<'_, '_> {
    /// clang `CheckSelfReference`, called on the converted initializer
    /// `init` of variable `v` ([basic.life]/7), once the initialization is
    /// performed. `direct_list` is the direct-list-initialization flag of
    /// `act_on_init`; a parenthesized initializer is direct too.
    pub(crate) fn check_self_reference(&mut self, v: DeclId, init: ExprId, direct_list: bool) {
        if self.kind_of(v) != DeclKind::Var || self.is_err_expr(init) {
            return;
        }
        let Some(spec) = self.ast.decls.var(v).map(|x| x.spec) else { return };
        // The initializer of a `constexpr` variable is a constant expression
        // anyway (clang `DiagIfReachable`).
        if spec.is_constexpr() {
            return;
        }
        let direct = direct_list || spec.init_style() == kiln_ast::decl::InitStyle::Call;
        let ty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
        let record = self.ast.types.as_record(ty).is_some();
        let reference = self.ast.types.is_reference(ty);
        // clang `AddInitializerToDecl`: other locals are left to the CFG
        // analysis.
        if self.has_local_storage(v) && !record && !reference {
            return;
        }
        let e = self.self_ref_strip_parens(init);
        // `T a = a;` for a non-class `T` is the idiom that silences
        // uninitialized-variable warnings.
        if !direct
            && !record
            && self.ast.exprs.get(e).kind() == ExprKind::ImplicitCastExpr
            && self.ast.exprs.cast_kind(e) == CastKind::LValueToRValue
        {
            let sub = self.ast.exprs.operand(e);
            if self.self_ref_is_var(sub, v) {
                return;
            }
        }
        let mut st = SelfRef {
            var: v,
            record_pod: record.then(|| self.is_pod_for_uninit_check(ty)),
            reference,
            init_list: false,
            index: Vec::new(),
        };
        self.self_ref_check(&mut st, e, 0);
    }

    /// clang `CheckExpr`: tracks the member an initializer list is at, as
    /// members are initialized in order.
    fn self_ref_check(&mut self, st: &mut SelfRef, e: ExprId, depth: u32) {
        if depth > MAX_DEPTH {
            return;
        }
        if self.ast.exprs.get(e).kind() != ExprKind::InitListExpr {
            self.self_ref_visit(st, e, depth + 1);
            return;
        }
        st.init_list = true;
        st.index.push(0);
        let inits: Vec<ExprId> =
            self.ast.lists.iter::<ExprId>(self.ast.exprs.init_list_inits(e)).collect();
        for c in inits {
            self.self_ref_check(st, c, depth + 1);
            if let Some(i) = st.index.last_mut() {
                *i += 1;
            }
        }
        st.index.pop();
    }

    /// The evaluated subexpressions of `e` (clang `EvaluatedExprVisitor`
    /// with the checker's overrides).
    fn self_ref_visit(&mut self, st: &mut SelfRef, e: ExprId, depth: u32) {
        if depth > MAX_DEPTH {
            return;
        }
        match self.ast.exprs.get(e).kind() {
            ExprKind::ImplicitCastExpr
                if self.ast.exprs.cast_kind(e) == CastKind::LValueToRValue =>
            {
                self.self_ref_value(st, self.ast.exprs.operand(e), depth + 1);
            }
            // Every use of a reference is a use before it is bound.
            ExprKind::DeclRefExpr => {
                if st.reference {
                    self.self_ref_found(st, e);
                }
            }
            ExprKind::MemberExpr => self.self_ref_member(st, e, depth + 1),
            ExprKind::CXXOperatorCallExpr => {
                let callee = self.ast.exprs.callee(e);
                self.self_ref_visit(st, callee, depth + 1);
                let args: Vec<ExprId> =
                    self.ast.lists.iter::<ExprId>(self.ast.exprs.call_args(e)).collect();
                for a in args {
                    let a = self.ignore_paren_imp_casts(a);
                    self.self_ref_value(st, a, depth + 1);
                }
            }
            ExprKind::UnaryOperator => {
                let op = self.ast.exprs.unary_op(e);
                let sub = self.ast.exprs.operand(e);
                if op == UnaryOp::AddrOf
                    && let Some(pod) = st.record_pod
                    && self.ast.exprs.get(self.self_ref_strip_parens(sub)).kind()
                        == ExprKind::MemberExpr
                {
                    // The address of a member of a POD object is fine.
                    if !pod {
                        self.self_ref_value(st, sub, depth + 1);
                    }
                } else if op.is_inc_dec() {
                    self.self_ref_value(st, sub, depth + 1);
                } else {
                    self.self_ref_children(st, e, depth);
                }
            }
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr
                if self.special_kind_of(self.ast.exprs.constructor(e))
                    == Some(SpecialKind::CopyCtor) =>
            {
                let args: Vec<ExprId> =
                    self.ast.lists.iter::<ExprId>(self.ast.exprs.construct_args(e)).collect();
                let Some(&a) = args.first() else { return };
                let mut a = a;
                if self.ast.exprs.get(a).kind() == ExprKind::InitListExpr {
                    let inits: Vec<ExprId> =
                        self.ast.lists.iter::<ExprId>(self.ast.exprs.init_list_inits(a)).collect();
                    if let [x] = inits.as_slice() {
                        a = *x;
                    }
                }
                if self.ast.exprs.get(a).kind() == ExprKind::ImplicitCastExpr
                    && self.ast.exprs.cast_kind(a) == CastKind::NoOp
                {
                    a = self.ast.exprs.operand(a);
                }
                self.self_ref_value(st, a, depth + 1);
            }
            ExprKind::CallExpr if self.self_ref_is_std_move(e) => {
                let args: Vec<ExprId> =
                    self.ast.lists.iter::<ExprId>(self.ast.exprs.call_args(e)).collect();
                self.self_ref_value(st, args[0], depth + 1);
            }
            ExprKind::CompoundAssignOperator => {
                let kids = self.ast.exprs.children(e, &self.ast.lists);
                if let [lhs, rhs] = kids.as_slice() {
                    self.self_ref_value(st, *lhs, depth + 1);
                    self.self_ref_visit(st, *rhs, depth + 1);
                }
            }
            // The condition and the true arm share the common expression.
            ExprKind::BinaryConditionalOperator => {
                let [_, cond, _, f] =
                    self.ast.exprs.binary_conditional_operands(e, &self.ast.lists);
                self.self_ref_visit(st, cond, depth + 1);
                self.self_ref_visit(st, f, depth + 1);
            }
            // An opaque value is visited through its source; unevaluated
            // operands ([expr.context]/1) are not visited.
            ExprKind::OpaqueValueExpr | ExprKind::CXXNoexceptExpr => {}
            ExprKind::UnaryExprOrTypeTraitExpr
                if self.ast.exprs.trait_kind(e) != TraitKind::VecStep => {}
            ExprKind::CXXTypeidExpr if !self.ast.exprs.typeid_potentially_evaluated(e) => {}
            _ => self.self_ref_children(st, e, depth),
        }
    }

    fn self_ref_children(&mut self, st: &mut SelfRef, e: ExprId, depth: u32) {
        for c in self.ast.exprs.children(e, &self.ast.lists) {
            self.self_ref_visit(st, c, depth + 1);
        }
    }

    /// clang `HandleValue`: `e` is used as a value.
    fn self_ref_value(&mut self, st: &mut SelfRef, e: ExprId, depth: u32) {
        if depth > MAX_DEPTH {
            return;
        }
        let e = self.self_ref_strip_parens(e);
        match self.ast.exprs.get(e).kind() {
            ExprKind::DeclRefExpr => self.self_ref_found(st, e),
            ExprKind::ConditionalOperator => {
                let kids = self.ast.exprs.children(e, &self.ast.lists);
                let [t, f] = self.ast.exprs.cond_arms(e);
                if let Some(&c) = kids.first() {
                    self.self_ref_visit(st, c, depth + 1);
                }
                self.self_ref_value(st, t, depth + 1);
                self.self_ref_value(st, f, depth + 1);
            }
            ExprKind::BinaryConditionalOperator => {
                let [_, cond, _, f] =
                    self.ast.exprs.binary_conditional_operands(e, &self.ast.lists);
                self.self_ref_visit(st, cond, depth + 1);
                self.self_ref_value(st, f, depth + 1);
            }
            ExprKind::OpaqueValueExpr => {
                if let Some(s) = self.ast.exprs.opaque_source(e) {
                    self.self_ref_value(st, s, depth + 1);
                }
            }
            ExprKind::BinaryOperator if self.ast.exprs.binary_op(e) == BinaryOp::Comma => {
                let [l, r] = self.ast.exprs.operands(e);
                self.self_ref_visit(st, l, depth + 1);
                self.self_ref_value(st, r, depth + 1);
            }
            ExprKind::MemberExpr => {
                if st.init_list && self.self_ref_list_member(st, e, false) {
                    return;
                }
                let mut base = self.ignore_paren_imp_casts(e);
                while self.ast.exprs.get(base).kind() == ExprKind::MemberExpr {
                    // A static data member is not part of the object.
                    if self.kind_of(self.ast.exprs.decl(base)) != DeclKind::Field {
                        return;
                    }
                    base = self.ignore_paren_imp_casts(self.ast.exprs.operand(base));
                }
                if self.ast.exprs.get(base).kind() == ExprKind::DeclRefExpr {
                    self.self_ref_found(st, base);
                }
            }
            _ => self.self_ref_visit(st, e, depth + 1),
        }
    }

    /// clang `VisitMemberExpr`: a member access that is not a value use
    /// warns only when it calls a non-static member function of the
    /// variable (through non-static data members).
    fn self_ref_member(&mut self, st: &mut SelfRef, e: ExprId, depth: u32) {
        if st.init_list && self.self_ref_list_member(st, e, true) {
            return;
        }
        let ty = self.ast.exprs.get(e).ty();
        // An array or function can be used as a pointer.
        if self.ast.types.is_array(ty) || self.ast.types.is_function(ty) {
            return;
        }
        let m = self.ast.exprs.decl(e);
        let mut warn = matches!(
            self.kind_of(m),
            DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
        ) && !self.is_static_method(m);
        let mut base = self.ignore_paren_imp_casts(self.ast.exprs.operand(e));
        while self.ast.exprs.get(base).kind() == ExprKind::MemberExpr {
            if self.kind_of(self.ast.exprs.decl(base)) != DeclKind::Field {
                warn = false;
            }
            base = self.ignore_paren_imp_casts(self.ast.exprs.operand(base));
        }
        if self.ast.exprs.get(base).kind() == ExprKind::DeclRefExpr {
            if warn {
                self.self_ref_found(st, base);
            }
            return;
        }
        self.self_ref_visit(st, base, depth + 1);
    }

    /// clang `CheckInitListMemberExpr`: a member of the variable used in
    /// its initializer list is fine once initialized. True if `me` needs
    /// no further checking (`check_ref`: only a reference member is a use).
    fn self_ref_list_member(&mut self, st: &mut SelfRef, me: ExprId, check_ref: bool) -> bool {
        let mut fields = Vec::new();
        let mut reference_field = false;
        let mut base = me;
        while self.ast.exprs.get(base).kind() == ExprKind::MemberExpr {
            let f = self.ast.exprs.decl(base);
            if self.kind_of(f) != DeclKind::Field {
                return false;
            }
            let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            reference_field |= self.ast.types.is_reference(fty);
            fields.push(f);
            base = self.ignore_paren_imp_casts(self.ast.exprs.operand(base));
        }
        if !self.self_ref_is_var(base, st.var) {
            return false;
        }
        if check_ref && !reference_field {
            return true;
        }
        let used: Vec<usize> = fields.iter().rev().map(|&f| self.self_ref_field_index(f)).collect();
        for (u, o) in used.iter().zip(st.index.iter()) {
            if u < o {
                return true;
            }
            if u > o {
                break;
            }
        }
        self.self_ref_found(st, base);
        true
    }

    /// clang `FieldDecl::getFieldIndex`.
    fn self_ref_field_index(&self, f: DeclId) -> usize {
        let Some(class) = self.ast.decls.get(f).ctx() else { return 0 };
        let def = self.ast.definition(class).unwrap_or(class);
        self.ast
            .decls
            .members(def)
            .filter(|&m| self.kind_of(m) == DeclKind::Field)
            .position(|m| m == f)
            .unwrap_or(0)
    }

    /// clang `HandleDeclRefExpr`: `dre` names the variable.
    fn self_ref_found(&mut self, st: &SelfRef, dre: ExprId) {
        if !self.self_ref_is_var(dre, st.var) {
            return;
        }
        let d = self.ast.exprs.decl(dre);
        let v = st.var;
        let id = if st.reference {
            DiagId::sema_uninit_self_reference_in_reference_init
        } else if self.self_ref_is_static_local(v) {
            DiagId::sema_static_self_reference_in_init
        } else if self.ast.decls.get(v).ctx().is_some_and(|c| {
            matches!(self.kind_of(c), DeclKind::TranslationUnit | DeclKind::Namespace)
        }) || self
            .ast
            .decls
            .get(d)
            .ty()
            .is_some_and(|t| self.ast.types.as_record(t).is_some())
        {
            DiagId::sema_uninit_self_reference_in_init
        } else {
            // Other locals are left to the CFG analysis.
            return;
        };
        // clang: `<< OrigDecl->getLocation() << DRE->getSourceRange()`.
        let at = self.begin_loc(dre);
        let name = self.ast.decls.get(v).loc();
        let name = Span::new(name, self.token_end(name).unwrap_or(name));
        let msg =
            self.diag(id, at).arg(self.decl_arg(d)).range(name).range(self.self_ref_dre_range(dre));
        self.diag_runtime_behavior(msg);
    }

    /// The source range of a variable `DeclRefExpr`: from its first token
    /// (a qualifier, if any) to the end of the name token. A stored end
    /// location is not trusted here: a member-access base can carry the
    /// enclosing member name's end.
    pub(crate) fn self_ref_dre_range(&self, dre: ExprId) -> Span {
        let name = self.ast.exprs.get(dre).loc();
        Span::new(self.begin_loc(dre), self.token_end(name).unwrap_or(name))
    }

    /// clang `VarDecl::isStaticLocal`.
    fn self_ref_is_static_local(&self, v: DeclId) -> bool {
        let Some(spec) = self.ast.decls.var(v).map(|x| x.spec) else { return false };
        let local =
            self.ast.decls.get(v).ctx().is_some_and(|c| self.ast.decls.get(c).kind().is_function());
        local
            && (spec.storage() == VarStorage::Static
                || (spec.storage() == VarStorage::None && spec.tls() != TlsKind::None))
    }

    /// Whether `e` is a `DeclRefExpr` naming variable `v`.
    fn self_ref_is_var(&self, e: ExprId, v: DeclId) -> bool {
        self.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr
            && self.ast.first(self.ast.exprs.decl(e)) == self.ast.first(v)
    }

    /// clang `CallExpr::isCallToStdMove`: a one-argument call of
    /// `std::move`.
    pub(crate) fn self_ref_is_std_move(&self, call: ExprId) -> bool {
        if self.ast.lists.iter::<ExprId>(self.ast.exprs.call_args(call)).count() != 1 {
            return false;
        }
        let callee = self.ignore_paren_imp_casts(self.ast.exprs.callee(call));
        if self.ast.exprs.get(callee).kind() != ExprKind::DeclRefExpr {
            return false;
        }
        let f = self.ast.exprs.decl(callee);
        if self.kind_of(f) != DeclKind::Function
            || self.ast.decls.get(f).name().as_ident() != Some(self.intern("move"))
        {
            return false;
        }
        let mut c = self.ast.decls.get(f).ctx();
        while let Some(ns) = c {
            if self.kind_of(ns) != DeclKind::Namespace {
                return false;
            }
            let parent = self.ast.decls.get(ns).ctx();
            if self.ast.decls.get(ns).name().as_ident() == Some(self.intern("std"))
                && parent.is_some_and(|p| self.kind_of(p) == DeclKind::TranslationUnit)
            {
                return true;
            }
            if !self
                .ast
                .decls
                .namespace(self.ast.first(ns))
                .is_some_and(kiln_ast::decl::NamespaceData::is_inline)
            {
                return false;
            }
            c = parent;
        }
        false
    }

    fn self_ref_strip_parens(&self, mut e: ExprId) -> ExprId {
        while self.ast.exprs.get(e).kind() == ExprKind::ParenExpr {
            e = self.ast.exprs.operand(e);
        }
        e
    }
}
