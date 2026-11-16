//! The traits that query special members, layout and register passing
//! (spec/sema-templates.md §9.1; WP-166): the GCC `__has_nothrow_*`
//! family, `__is_trivially_equality_comparable`, `__can_pass_in_regs`
//! (and `__is_trivially_relocatable`, which uses it; at
//! `-fclang-compat=22` `Sema::act_type_trait` warns it deprecated, as
//! clang 22 does),
//! `__is_bitwise_cloneable`, `__is_layout_compatible` and
//! `__is_pointer_interconvertible_base_of`.
//! Each follows clang 19 (`EvaluateUnaryTypeTrait`,
//! `EvaluateBinaryTypeTrait` and the `ASTContext` / `Sema` helpers they
//! call), including its extension behaviour.

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{DeclFlags, DeclKind, Defaulted, DefinitionBits};
use kiln_ast::expr::{BinaryOp, ExprKind, TypeTrait, ValueCategory};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::{Quals, TypeView};
use kiln_ast::{AttrId, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::class::SpecialKind;
use crate::templ::stack::FrameKind;

/// The recursion bound of the subobject walks.
const MAX_DEPTH: usize = 256;

impl Sema<'_, '_> {
    /// The `__has_nothrow_*` traits (clang 19 `UTT_HasNothrow*`, the GCC
    /// type-traits documentation): true when the trivial member applies,
    /// else when every declared member of the kind is non-throwing (and,
    /// for constructors, takes no argument beyond the copied one; default
    /// arguments are assumed to throw).
    pub(super) fn has_nothrow(&mut self, t: TypeTrait, q: QualType, loc: Loc) -> bool {
        use SpecialKind as K;
        let tys = &self.ast.types;
        match t {
            TypeTrait::HasNothrowAssign => {
                let e = self.base_element(q);
                if (!tys.is_reference(e) && tys.effective_quals(e).is_const())
                    || tys.is_reference(q)
                {
                    return false;
                }
                if self.is_pod_type(q) {
                    return true;
                }
                let Some(r) = self.defined_record(q) else { return false };
                self.all_trivial(r, K::CopyAssign)
                    || self.nothrow_members(r, K::CopyAssign, None, loc)
            }
            TypeTrait::HasNothrowMoveAssign => {
                if self.is_pod_type(q) {
                    return true;
                }
                let Some(r) = self.element_record(q) else { return false };
                self.all_trivial(r, K::MoveAssign)
                    || self.nothrow_members(r, K::MoveAssign, None, loc)
            }
            TypeTrait::HasNothrowCopy => {
                if self.is_pod_type(q) || tys.is_reference(q) {
                    return true;
                }
                let Some(r) = self.defined_record(q) else { return false };
                self.all_trivial(r, K::CopyCtor)
                    || self.nothrow_members(r, K::CopyCtor, Some(1), loc)
            }
            TypeTrait::HasNothrowConstructor => {
                if self.is_pod_type(q) {
                    return true;
                }
                let Some(r) = self.element_record(q) else { return false };
                self.all_trivial(r, K::DefaultCtor)
                    || self.nothrow_members(r, K::DefaultCtor, Some(0), loc)
            }
            _ => false,
        }
    }

    /// clang's `HasNoThrowOperator` and its constructor loops: whether
    /// class `r` has a member of kind `k` (implicit ones declared first;
    /// templates and using-declarations skipped) and each such member is
    /// non-throwing with at most `max_params` parameters.
    fn nothrow_members(
        &mut self,
        r: DeclId,
        k: SpecialKind,
        max_params: Option<usize>,
        loc: Loc,
    ) -> bool {
        let d = self.class_def(r);
        self.ensure_implicit(d, k);
        let cands: Vec<DeclId> = if k.is_ctor() {
            self.ast.constructors(d).iter().collect()
        } else {
            self.ast.lookup_local(d, DeclName::operator(OverloadedOperator::Equal)).iter().collect()
        };
        let mut found = false;
        for m in cands {
            if !self.is_member_of_kind(m, k) {
                continue;
            }
            found = true;
            self.resolve_exception_spec(m, loc);
            if !self.fn_is_nothrow(m) || max_params.is_some_and(|n| self.param_decls(m).len() > n) {
                return false;
            }
        }
        found
    }

    /// clang's `isDefaultConstructor` / `isCopyConstructor` /
    /// `isCopyOrMoveConstructor` / `is{Copy,Move}AssignmentOperator`: a
    /// constructor whose first parameter has a default argument can be
    /// both a default and a copy (or move) constructor.
    fn is_member_of_kind(&self, m: DeclId, k: SpecialKind) -> bool {
        match self.special_kind_of(m) {
            Some(x) if x == k => true,
            Some(SpecialKind::DefaultCtor)
                if matches!(k, SpecialKind::CopyCtor | SpecialKind::MoveCtor) =>
            {
                let params = self.param_decls(m);
                let Some(&p) = params.first() else { return false };
                let class = self.ast.decls.get(m).ctx();
                let ty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                let of_class = |t: QualType| class.is_some_and(|c| self.is_class_type_of(t, c));
                params[1..].iter().all(|&x| self.param_has_default(x))
                    && match self.ast.types.canonical_view(ty) {
                        TypeView::LValueRef(t) => k == SpecialKind::CopyCtor && of_class(t),
                        TypeView::RValueRef(t) => k == SpecialKind::MoveCtor && of_class(t),
                        _ => false,
                    }
            }
            _ => false,
        }
    }

    /// Whether function `f`'s (resolved) exception specification is
    /// non-throwing.
    fn fn_is_nothrow(&self, f: DeclId) -> bool {
        let first = self.ast.first(f);
        let ty = self.ast.decls.get(first).ty().unwrap_or(QualType::ERROR);
        let t = &self.ast.types;
        t.proto(ty).is_some_and(|p| t.exc_spec(p).is_nothrow())
    }

    /// `__is_trivially_equality_comparable(T)` (clang 19
    /// `isTriviallyEqualityComparableType`): a complete non-enum,
    /// non-array type whose classes compare with non-deleted defaulted
    /// `operator==` throughout, and whose value representation is its
    /// object representation.
    pub(super) fn trivially_equality_comparable(&mut self, q: QualType, loc: Loc) -> bool {
        let c = self.ast.types.canonical(q);
        if !self.is_complete(c)
            || self.ast.types.is_void(c)
            || self.is_dependent_type(c)
            || matches!(self.ast.types.canonical_view(c), TypeView::Enum(_))
            || self.ast.types.is_array(c)
        {
            return false;
        }
        if let Some(r) = self.ast.types.as_record(c)
            && !self.defaulted_equality(r, loc, 0)
        {
            return false;
        }
        self.unique_reps(c, false)
    }

    /// clang 19 `HasNonDeletedDefaultedEqualityComparison`: `obj == obj`
    /// for a `const` lvalue of class `r`, resolved from the translation
    /// unit in an unevaluated SFINAE context, calls a defaulted
    /// `operator==` taking the class (by value only when it is trivially
    /// copyable), and so do the bases and the class-type members; no
    /// member is a reference or an enumeration.
    fn defaulted_equality(&mut self, r: DeclId, loc: Loc, depth: usize) -> bool {
        let d = self.class_def(r);
        if depth > MAX_DEPTH || self.is_union(d) {
            return false;
        }
        if self.bits_of(d).contains(DefinitionBits::LAMBDA) {
            return self.closure_default_constructible(d);
        }
        let cty = self.class_type(d);
        let class = self.ast.types.canonical(cty).without_quals();
        let operand_ty = self.ast.types.qualify(class, Quals::CONST);
        let tu = self.ast.tu;
        let saved = std::mem::replace(&mut self.ctx, tu);
        let found = self.in_sfinae(FrameKind::ConstraintSubstitution, tu, None, loc, |s| {
            s.enter_unevaluated();
            let l = s.builder().opaque_value_of(operand_ty, ValueCategory::LValue, loc);
            let rr = s.builder().opaque_value_of(operand_ty, ValueCategory::LValue, loc);
            let e = s.binary_operator(BinaryOp::EQ, l, rr, None, loc);
            s.exit_unevaluated();
            if s.is_err_expr(e) { None } else { s.operator_call_callee(e) }
        });
        self.ctx = saved;
        let Some((Some(callee), Ok(()))) = found else { return false };
        if self.fn_spec(callee).defaulted() == Defaulted::No {
            return false;
        }
        let Some(&p) = self.param_decls(callee).first() else { return false };
        let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        let by_ref = self.ast.types.is_reference(pty);
        if !by_ref && !self.bits_of(d).contains(DefinitionBits::TRIVIALLY_COPYABLE) {
            return false;
        }
        let pointee = match self.ast.types.canonical_view(pty) {
            TypeView::LValueRef(t) | TypeView::RValueRef(t) => t,
            _ => pty,
        };
        if self.ast.types.canonical(pointee).without_quals() != class {
            return false;
        }
        let bases: Vec<QualType> = self.ast.decls.bases(d, &self.ast.lists).map(|b| b.ty).collect();
        for b in bases {
            if let Some(br) = self.ast.types.as_record(b)
                && !self.defaulted_equality(br, loc, depth + 1)
            {
                return false;
            }
        }
        for f in self.fields_of(d) {
            let ft = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            let ft = if self.ast.types.is_array(ft) { self.base_element(ft) } else { ft };
            if self.ast.types.is_reference(ft)
                || matches!(self.ast.types.canonical_view(ft), TypeView::Enum(_))
            {
                return false;
            }
            if let Some(fr) = self.ast.types.as_record(ft)
                && !self.defaulted_equality(fr, loc, depth + 1)
            {
                return false;
            }
        }
        true
    }

    /// The function an overloaded-operator call `e` calls directly
    /// (clang `CXXOperatorCallExpr::getDirectCallee`).
    fn operator_call_callee(&self, e: ExprId) -> Option<DeclId> {
        let exprs = &self.ast.exprs;
        if exprs.get(e).kind() != ExprKind::CXXOperatorCallExpr {
            return None;
        }
        let mut c = self.ignore_parens(exprs.callee(e));
        while exprs.get(c).kind().is_cast() {
            c = self.ignore_parens(exprs.operand(c));
        }
        matches!(exprs.get(c).kind(), ExprKind::DeclRefExpr | ExprKind::MemberExpr)
            .then(|| exprs.decl(c))
    }

    /// `__can_pass_in_regs(T)` (clang 19 `UTT_CanPassInRegs`): the operand
    /// must be an unqualified class type ("argument 'T' is not an
    /// unqualified class type" at the keyword, and the trait is false).
    pub(super) fn can_pass_in_regs_trait(&mut self, q: QualType, loc: Loc) -> bool {
        let tys = &self.ast.types;
        if !tys.is_reference(q)
            && q.quals().is_empty()
            && tys.canonical(q).quals().is_empty()
            && let Some(r) = tys.as_record(q)
        {
            return self.can_pass_in_registers(r);
        }
        let d = self.diag(DiagId::sema_builtin_pass_in_regs_non_class, loc).arg(self.ty_arg(q));
        self.emit(d);
        false
    }

    /// clang's `canPassInRegisters` (SemaDeclCXX.cpp, the Itanium rule of
    /// [class.temporary]/3): each copy constructor, move constructor and
    /// destructor of class `r` is trivial for the purposes of calls
    /// (trivial, or made so by `[[clang::trivial_abi]]`) or deleted, and
    /// at least one copy or move constructor is not deleted. The implicit
    /// members not declared yet are judged from the class bits.
    pub(crate) fn can_pass_in_registers(&self, r: DeclId) -> bool {
        use SpecialKind as K;
        let d = self.class_def(r);
        let flags = self.ast.decls.get(d).flags();
        if flags.contains(DeclFlags::DEPENDENT_CTX) || flags.contains(DeclFlags::INVALID) {
            return false;
        }
        let bits = self.bits_of(d);
        let mut has_copy_or_move = false;
        let for_call = |k: K| {
            bits.contains(k.trivial_bit())
                || crate::trivial_abi::for_call_bit(k).is_some_and(|b| bits.contains(b))
        };
        for k in [K::CopyCtor, K::MoveCtor] {
            if self.needs_implicit(d, k) && !self.known_deleted(d, k) {
                if !for_call(k) {
                    return false;
                }
                has_copy_or_move = true;
            }
        }
        if self.needs_implicit(d, K::Dtor) && !self.known_deleted(d, K::Dtor) && !for_call(K::Dtor)
        {
            return false;
        }
        for m in self.ast.members(d) {
            let kind = self.ast.decls.get(m).kind();
            // clang skips a deleted member and one that is not eligible
            // ([special]/6) or not the selected destructor
            // (`isIneligibleOrNotSelected`).
            if !matches!(kind, DeclKind::CXXConstructor | DeclKind::CXXDestructor)
                || self.is_deleted_fn(m)
                || self.fn_spec(m).not_selected()
            {
                continue;
            }
            let k = if kind == DeclKind::CXXConstructor {
                let k = if self.is_member_of_kind(m, K::CopyCtor) {
                    K::CopyCtor
                } else if self.is_member_of_kind(m, K::MoveCtor) {
                    K::MoveCtor
                } else {
                    continue;
                };
                has_copy_or_move = true;
                k
            } else if self.special_kind_of(m) == Some(K::Dtor) {
                K::Dtor
            } else {
                continue;
            };
            if !self.fn_spec(m).is_trivial() && !self.member_trivial_for_call(d, m, k) {
                return false;
            }
        }
        has_copy_or_move
    }

    /// `__is_bitwise_cloneable(T)` (clang 19 `isBitwiseCloneableType`):
    /// an array of such elements, any complete non-class non-`void` type,
    /// or a complete class whose fields all are.
    pub(super) fn is_bitwise_cloneable(&self, q: QualType, depth: usize) -> bool {
        let c = self.ast.types.canonical(q);
        if depth > MAX_DEPTH || self.is_dependent_type(c) {
            return false;
        }
        if self.ast.types.is_array(c) {
            return self.is_bitwise_cloneable(self.base_element(c), depth + 1);
        }
        if self.ast.types.is_void(c) {
            return false;
        }
        // Every non-class type (references and functions included) is
        // bitwise cloneable; a class needs every field to be (clang 19
        // `QualType::isBitwiseCloneableType`; polymorphism is allowed).
        let Some(r) = self.ast.types.as_record(c) else { return true };
        if self.ast.definition(r).is_none() {
            return false;
        }
        let d = self.class_def(r);
        self.fields_of(d)
            .into_iter()
            .map(|f| self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR))
            .all(|t| self.is_bitwise_cloneable(t, depth + 1))
    }

    /// `__is_layout_compatible(T, U)` ([basic.types.general]/11, clang 19
    /// `BTT_IsLayoutCompatible`): each operand other than `void` or an
    /// array of unknown bound must be complete ("incomplete type 'X'
    /// where a complete type is required" where it is written). The
    /// rule is `kiln_abi::layout::compat` (WP-1472), shared with
    /// `__builtin_is_corresponding_member`.
    pub(super) fn eval_layout_compatible(
        &mut self,
        a: (QualType, Loc),
        b: (QualType, Loc),
    ) -> bool {
        for (q, at) in [a, b] {
            let tys = &self.ast.types;
            // A function or reference type is complete (clang
            // `isIncompleteType`).
            if tys.is_void(q)
                || tys.is_function(q)
                || tys.is_reference(q)
                || matches!(tys.canonical_view(q), TypeView::IncompleteArray(_))
                || self.trait_complete(q, at)
            {
                continue;
            }
            self.trait_incomplete(DiagId::sema_incomplete_type_required, at, q);
        }
        for q in [a.0, b.0] {
            self.ensure_record_layout(q);
        }
        kiln_abi::layout::compat::layout_compatible(&self.ast, &self.abi, a.0, b.0)
    }

    /// Lays out (and caches) the complete class of `q`, if any, with the
    /// classes it contains, so that `kiln_abi::layout::compat` reads
    /// cached layouts.
    pub(crate) fn ensure_record_layout(&mut self, q: QualType) {
        let mut q = q;
        while let Some(e) = self.array_element_type(q) {
            q = e;
        }
        if let Some(r) = self.ast.types.as_record(q) {
            let _ = kiln_abi::ensure_layout(&mut self.ast, r, &self.abi);
        }
    }

    fn array_element_type(&self, q: QualType) -> Option<QualType> {
        match self.ast.types.canonical_view(q) {
            TypeView::ConstantArray(a) => Some(a.elem),
            TypeView::IncompleteArray(e) => Some(e),
            _ => None,
        }
    }

    /// `[[no_unique_address]]` written on the field `f`.
    pub(crate) fn has_nua(&self, f: DeclId) -> bool {
        self.ast.decls.get(f).has(DeclFlags::HAS_ATTRS)
            && self
                .ast
                .lists
                .iter::<AttrId>(self.ast.attrs.of_decl(f))
                .any(|a| self.ast.attrs.get(a).kind == AttrKind::NO_UNIQUE_ADDRESS)
    }

    /// `__is_pointer_interconvertible_base_of(Base, Derived)`
    /// ([meta.rel]/3, clang 19 `IsPointerInterconvertibleBaseOf`): the
    /// same class, or a standard-layout `Derived` with base `Base`
    /// (accessibility and ambiguity ignored). Two different classes need
    /// `Derived` complete ("incomplete type ... where a complete type is
    /// required" at `Derived`).
    pub(super) fn eval_pointer_interconvertible_base(
        &mut self,
        a: (QualType, Loc),
        b: (QualType, Loc),
    ) -> bool {
        let class = |s: &Self, q: QualType| s.ast.types.as_record(q).filter(|&r| !s.is_union(r));
        let base = class(self, a.0);
        let derived = class(self, b.0);
        let ca = self.ast.types.canonical(a.0).without_quals();
        let cb = self.ast.types.canonical(b.0).without_quals();
        let (Some(base), Some(derived)) = (base, derived) else { return false };
        if self.ast.types.same_type(ca, cb) {
            return true;
        }
        if !self.trait_complete(b.0, b.1) {
            self.trait_incomplete(DiagId::sema_incomplete_type_required, b.1, b.0);
            return false;
        }
        crate::conv::bases::is_derived_from(self, derived, base)
            && self.bits_of(derived).contains(DefinitionBits::STANDARD_LAYOUT)
    }
}
