//! Sema's side of constant evaluation (spec/sema.md §8, spec/oven.md
//! "Integration"): the [`SemaBridge`] the Oven calls back through, and the
//! `Sema` entry points every evaluation site uses.
//!
//! One [`Oven`] lives in each `Sema` (one per translation unit), so
//! compiled bytecode, resolved globals and the memoization cache are
//! shared by every evaluation of the TU. Manifestly constant-evaluated
//! contexts call [`Sema::eval_const`] with the [`EvalContext`] of the
//! construct (recorded in the Oven's statistics and the
//! `-fconstexpr-report`); warnings and other speculative folds call
//! [`Sema::try_fold`], which is silent and unrecorded.

use kiln_ast::consts::{ConstStore, PrintOptions, Scalar};
use kiln_ast::types::RecordLayout;
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, QualType, StmtId};
use kiln_diag::Diag;
use kiln_oven::{
    Budget, EvalContext, EvalFailure, MetaCtx, MetaError, MetaOp, MetaOut, Oven, SemaBridge,
    Subobject, SubobjectId,
};
use kiln_source::{Loc, Span};

use crate::Sema;

/// The bridge over a `Sema` (its `Oven` taken out) for the duration of
/// one evaluation.
struct Bridge<'s, 'a, 'sm> {
    sema: &'s mut Sema<'a, 'sm>,
}

impl SemaBridge for Bridge<'_, '_, '_> {
    fn ast(&self) -> &Ast {
        &self.sema.ast
    }

    fn sources(&self) -> Option<&kiln_source::SourceManager> {
        Some(self.sema.diags.sm())
    }

    fn cxx_year(&self) -> u32 {
        use kiln_lex::Std;
        match self.sema.std {
            Std::Cxx98 => 1998,
            Std::Cxx03 => 2003,
            Std::Cxx11 => 2011,
            Std::Cxx14 => 2014,
            Std::Cxx17 => 2017,
            Std::Cxx20 => 2020,
            Std::Cxx23 => 2023,
            Std::Cxx26 => 2026,
        }
    }

    fn set_temporary_value(&mut self, t: DeclId, v: ConstValueId) {
        // Write-once: a re-evaluation of the same initializer (a fold,
        // then the check) computes the same value.
        let _ = self.sema.ast.decls.set_lifetime_extended_value(t, v);
    }

    fn lvalue_ref_to(&mut self, t: QualType) -> Option<QualType> {
        Some(self.sema.ast.types.lvalue_ref_to(t))
    }

    fn ensure_body(&mut self, f: DeclId, oven: &mut Oven) -> Option<StmtId> {
        // A body exists once its definition has been parsed or
        // instantiated (a later definition is picked up by the next
        // evaluation). Implicit and defaulted members are defined here on
        // demand from WP-067b.
        if let Some(b) = self.sema.defined_body(f) {
            return Some(b);
        }
        if let Some(b) = self.sema.define_virtual_dtor(f) {
            return Some(b);
        }
        if self.sema.is_defaulted_cmp(f) {
            // A defaulted comparison used only in a context the use did
            // not define it in (WP-082; spec/sema-class.md §8.3).
            let loc = self.sema.ast.decls.get(f).loc();
            self.sema.define_defaulted_comparison(f, loc);
            return self.sema.defined_body(f);
        }
        self.sema.instantiate_for_oven(f, oven)
    }

    fn is_nothrow(&mut self, f: DeclId, oven: &mut Oven) -> bool {
        if let Some(n) = self.sema.known_nothrow(f) {
            return n;
        }
        let at = self.sema.inst.eval_loc.unwrap_or_else(|| self.sema.ast.decls.get(f).loc());
        self.sema.lend_oven(oven, |s| s.resolve_nothrow(f, at))
    }

    fn ensure_layout(&mut self, record: DeclId) -> Option<&RecordLayout> {
        let s = &mut *self.sema;
        kiln_abi::ensure_layout(&mut s.ast, record, &s.abi)
    }

    fn opaque_size_align(&mut self, ty: QualType) -> (u64, u32) {
        let s = &*self.sema;
        crate::target::size_align(&s.ast, ty, &s.abi)
            .map_or((0, 1), |(size, align)| (size, u32::try_from(align).unwrap_or(1)))
    }

    // clang `getDeclAlign(D, /*ForAlignof=*/false)` (spec/abi.md §3.2).
    fn decl_align(&mut self, d: DeclId) -> u64 {
        let s = &*self.sema;
        kiln_abi::layout::decl_align(&s.ast, d, &s.abi, false)
    }

    // clang `getDeclAlign(D, /*ForAlignof=*/true)`.
    fn alignof_decl(&mut self, d: DeclId) -> u64 {
        let s = &*self.sema;
        kiln_abi::layout::decl_align(&s.ast, d, &s.abi, true)
    }

    // Final overriders and subobject graphs are computed on demand and
    // cached in `Ast::virtuals` (WP-078a; spec/sema-class.md §3.5).
    fn final_overrider(
        &mut self,
        most_derived: DeclId,
        at: SubobjectId,
        method: DeclId,
    ) -> Option<(DeclId, SubobjectId)> {
        let ast = &mut self.sema.ast;
        let method = ast.first(method);
        crate::class::ensure_virtuals(ast, most_derived)?.final_overrider(at, method)
    }

    fn subobjects(&mut self, record: DeclId) -> &[Subobject] {
        crate::class::ensure_virtuals(&mut self.sema.ast, record).map_or(&[], |v| &v.subobjects)
    }

    fn consts(&mut self) -> &mut ConstStore {
        &mut self.sema.ast.consts
    }

    fn diag(&mut self, d: Diag) {
        d.emit(self.sema.diags);
    }

    // Reflection: sema's halves are in `meta/` (spec/reflection.md §3).
    fn meta_intrinsic(&self, f: DeclId) -> Option<MetaOp> {
        self.sema.meta_intrinsic_of(f)
    }

    fn meta(
        &mut self,
        op: MetaOp,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<(), MetaError> {
        self.sema.meta(op, args, out, ctx, oven)
    }

    fn meta_shape(
        &mut self,
        op: MetaOp,
        param: Option<usize>,
        q: QualType,
    ) -> Result<Option<kiln_oven::Shape>, String> {
        self.sema.meta_shape(op, param, q)
    }

    fn meta_exception(&mut self) -> Option<kiln_oven::ExcCtor> {
        self.sema.meta_exception_ctor()
    }

    fn meta_throw(
        &mut self,
        op: MetaOp,
        callee: DeclId,
        what: &str,
        loc: Loc,
    ) -> Option<kiln_oven::MetaThrow> {
        self.sema.meta_throw(op, callee, what, loc)
    }
}

impl<'a, 'sm> Sema<'a, 'sm> {
    /// Runs `f` with a bridge over `self`, for an Oven call a reflection
    /// intrinsic makes on the running Oven (WP-479's freezes).
    pub(crate) fn bridge<R>(&mut self, f: impl FnOnce(&mut dyn SemaBridge) -> R) -> R {
        f(&mut Bridge { sema: self })
    }

    /// Runs `f` with the running Oven (`oven`, from a bridge callback)
    /// lent back to sema, so the work can evaluate (nested, R-OVEN-1).
    pub(crate) fn lend_oven<R>(&mut self, oven: &mut Oven, f: impl FnOnce(&mut Self) -> R) -> R {
        debug_assert!(self.oven.is_none(), "lend_oven outside an evaluation");
        let mut lent = self.spare_oven.take().unwrap_or_else(|| Box::new(Oven::new(oven.budget())));
        std::mem::swap(oven, &mut *lent);
        self.oven = Some(lent);
        let r = f(self);
        if let Some(mut o) = self.oven.take() {
            std::mem::swap(oven, &mut *o);
            self.spare_oven = Some(o);
        }
        r
    }

    /// Runs `f` with the Oven taken out of `self` and a bridge over the
    /// rest (spec/sema-templates.md §4.11, R-OVEN-1). `eval_loc` is the
    /// evaluation's location if it is manifestly constant-evaluated (the
    /// point of instantiation of what `ensure_body` instantiates), `None`
    /// for a speculative fold, which never instantiates.
    fn with_oven<R>(
        &mut self,
        eval_loc: Option<Loc>,
        f: impl FnOnce(&mut Oven, &mut Bridge<'_, 'a, 'sm>) -> R,
    ) -> R {
        let Some(mut oven) = self.oven.take() else {
            unreachable!("sema evaluated a constant while its Oven was out (outside ensure_body)")
        };
        // The note wording follows sema's compatibility level (a field
        // set after the Oven is built).
        oven.set_clang_compat(self.clang_compat);
        // As is the language mode, for codegen's folds (WP-407).
        let year = Bridge { sema: self }.cxx_year();
        oven.set_cxx_year(year);
        let saved = std::mem::replace(&mut self.inst.eval_loc, eval_loc);
        let r = f(&mut oven, &mut Bridge { sema: self });
        self.inst.eval_loc = saved;
        self.oven = Some(oven);
        r
    }

    /// The body of `f`'s definition, if it has one.
    pub(crate) fn defined_body(&self, f: DeclId) -> Option<StmtId> {
        let d = self.ast.definition(f)?;
        self.ast.decls.function(d)?.body
    }

    /// An implicitly defined virtual destructor reached only through a
    /// virtual call (WP-077): sema defines a destructor when it is
    /// odr-used by name, clang also when the class's vtable is used
    /// (`MarkVirtualMembersReferenced`); the Oven's dispatch is that use.
    fn define_virtual_dtor(&mut self, f: DeclId) -> Option<StmtId> {
        let first = self.ast.first(f);
        if self.special_kind_of(first) != Some(crate::class::SpecialKind::Dtor)
            || !crate::class::is_virtual(&self.ast, first)
        {
            return None;
        }
        let spec = self.ast.decls.function(first)?.spec;
        if spec.defaulted() == kiln_ast::decl::Defaulted::No || spec.is_deleted() {
            return None;
        }
        let loc = self.ast.decls.get(first).loc();
        self.define_implicit_special(first, crate::class::SpecialKind::Dtor, loc);
        self.defined_body(f)
    }

    /// `ensure_body` of a function sema has not defined
    /// (spec/sema-templates.md §4.11): a member of a class template
    /// specialization that no expression marked (one reached through a
    /// virtual call or a pointer) is instantiated now, the running
    /// evaluation's location its point of instantiation, with the Oven
    /// lent back to sema so the instantiation can evaluate (nested,
    /// R-OVEN-1). A constexpr specialization already marked but not
    /// instantiable then (its pattern was defined later, so it waits in
    /// the end-of-TU queue) is instantiated now if its pattern is defined
    /// by the point of this evaluation, as GCC does ([temp.inst]/8: it is
    /// needed for constant evaluation; WP-484 — clang 19/22 leave it
    /// undefined). A pattern still undefined here keeps "undefined
    /// function 'f' cannot be used in a constant expression"; a queued
    /// non-constexpr function stays queued; nothing is instantiated
    /// during a speculative fold.
    fn instantiate_for_oven(&mut self, f: DeclId, oven: &mut Oven) -> Option<StmtId> {
        let poi = self.inst.eval_loc?;
        let pattern = self.ast.instantiated_from(f)?;
        if self.inst.fatal {
            return None;
        }
        if self.inst.queued.contains(&f) {
            let constexpr =
                self.ast.decls.function(f).is_some_and(|x| {
                    x.spec.constexpr_kind() != kiln_ast::decl::ConstexprKind::None
                });
            if !constexpr || self.defined_body(pattern).is_none() {
                return None;
            }
        }
        self.lend_oven(oven, |s| s.instantiate_function_definition(f, poi));
        self.defined_body(f)
    }

    /// Evaluates the constant expression `e` in a manifestly
    /// constant-evaluated context described by `ctx` (recorded in the
    /// statistics; diagnostics the evaluation itself reports, such as
    /// `-Wenum-constexpr-conversion`, are emitted).
    ///
    /// # Errors
    /// If `e` is not a constant expression or exceeds a limit; the
    /// failure's notes explain why ([`Sema::with_failure_notes`]).
    pub fn eval_const(&mut self, e: ExprId, ctx: EvalContext) -> Result<ConstValueId, EvalFailure> {
        if !self.eval_stack_ok(e) {
            return Err(EvalFailure::silent(kiln_oven::FailKind::NotConstant));
        }
        self.flush_member_refs();
        if self.special.has_pending() {
            self.flush_pending_deleted();
        }
        if let Some(v) = self.carried_lookup(e, &ctx) {
            return Ok(v);
        }
        let probe = self.carried_probe(e, &ctx);
        let r = self.with_oven(Some(ctx.loc), |oven, b| oven.evaluate(b, e, ctx));
        if let Some(p) = probe {
            self.carried_record(&p, e, &r);
        }
        r
    }

    /// The characters of a `static_assert` message: its `size` and
    /// `data` calls evaluated together ([`Oven::evaluate_chars`]): the
    /// characters read, and the evaluation's failure.
    pub fn eval_chars(
        &mut self,
        size: ExprId,
        data: ExprId,
        ctx: EvalContext,
    ) -> (Vec<u8>, Result<(), EvalFailure>) {
        if !self.eval_stack_ok(size) {
            return (Vec::new(), Err(EvalFailure::silent(kiln_oven::FailKind::NotConstant)));
        }
        self.flush_member_refs();
        if self.special.has_pending() {
            self.flush_pending_deleted();
        }
        self.with_oven(Some(ctx.loc), |oven, b| oven.evaluate_chars(b, size, data, ctx))
    }

    /// The value of `e` if it is a constant expression. Silent (no
    /// diagnostics, no notes) and not recorded in the statistics: for
    /// warnings and other speculative folds.
    #[cfg_attr(feature = "op-stats", track_caller)]
    pub fn try_fold(&mut self, e: ExprId) -> Option<ConstValueId> {
        #[cfg(feature = "op-stats")]
        let _caller =
            kiln_oven::trial_stats::CallerGuard::new("try_fold", std::panic::Location::caller());
        if let Some(v) = self.early_immediate_value(e) {
            return Some(v);
        }
        if !self.eval_stack_ok(e) {
            return None;
        }
        self.with_oven(None, |oven, b| oven.try_fold(b, e))
    }

    /// The value of the initializer `e` of variable `var` as a constant
    /// initializer ([basic.start.static]/2; [`Oven::fold_constant_init`]):
    /// for a reference, the pointer to the object it binds. Silent.
    pub(crate) fn fold_constant_init(&mut self, e: ExprId, var: DeclId) -> Option<ConstValueId> {
        if !self.eval_stack_ok(e) {
            return None;
        }
        self.with_oven(None, |oven, b| oven.fold_constant_init(b, e, var))
    }

    /// [`Sema::try_fold`]'s two paths for `e` (`lax`: those of the lax
    /// fold, [`Oven::fold_rvalue`]), the Oven's trial-fold fast path
    /// (`None` where it declines, `Some(None)` for a definite failure) and
    /// its full path, as `(fast, full)`: the fast path's equivalence tests
    /// (`tests/trial_fold.rs`; test-only).
    #[doc(hidden)]
    pub fn trial_fold_paths(
        &mut self,
        e: ExprId,
        lax: bool,
    ) -> (Option<Option<ConstValueId>>, Option<ConstValueId>) {
        (self.trial_fold_fast(e), self.trial_fold_full(e, lax))
    }

    /// The fast path of [`Self::trial_fold_paths`] only (test-only).
    #[doc(hidden)]
    pub fn trial_fold_fast(&mut self, e: ExprId) -> Option<Option<ConstValueId>> {
        self.with_oven(None, |oven, b| oven.try_fold_leaf(b, e))
    }

    /// The full path of [`Self::trial_fold_paths`] only (test-only).
    #[doc(hidden)]
    pub fn trial_fold_full(&mut self, e: ExprId, lax: bool) -> Option<ConstValueId> {
        self.with_oven(None, |oven, b| {
            if lax { oven.fold_rvalue_full(b, e) } else { oven.try_fold_full(b, e) }
        })
    }

    /// [`Self::is_constant_initialized`] by the Oven's full path only
    /// (`tests/trial_fold.rs`; test-only).
    #[doc(hidden)]
    pub fn is_constant_initialized_full(&mut self, var: DeclId) -> bool {
        self.with_oven(None, |oven, b| oven.is_constant_initialized_full(b, var))
    }

    /// `cond` (`None`: `true`) evaluated with each parameter of `binds`
    /// initialized from its argument ([`Oven::evaluate_with_params`]),
    /// silently; `loc` is the point of instantiation of what it needs.
    pub(crate) fn eval_with_params(
        &mut self,
        cond: Option<ExprId>,
        binds: &[(DeclId, ExprId)],
        at: ExprId,
        loc: Loc,
    ) -> Option<ConstValueId> {
        if !self.eval_stack_ok(at) {
            return None;
        }
        self.with_oven(Some(loc), |oven, b| oven.evaluate_with_params(b, cond, binds, at))
    }

    /// [`Sema::try_fold`], laxly ([`Oven::fold_rvalue`], clang
    /// `EvaluateAsInt`): a `const` non-volatile variable with an
    /// initializer reads as its value even when it is not usable in
    /// constant expressions.
    #[cfg_attr(feature = "op-stats", track_caller)]
    pub fn fold_rvalue_int(&mut self, e: ExprId) -> Option<i128> {
        #[cfg(feature = "op-stats")]
        let _caller = kiln_oven::trial_stats::CallerGuard::new(
            "fold_rvalue_int",
            std::panic::Location::caller(),
        );
        let v = self.fold_rvalue(e)?;
        self.const_int(v)
    }

    /// [`Sema::try_fold`], laxly ([`Oven::fold_rvalue`], clang
    /// `EvaluateAsRValue`), of an expression of any type.
    #[cfg_attr(feature = "op-stats", track_caller)]
    pub fn fold_rvalue(&mut self, e: ExprId) -> Option<ConstValueId> {
        #[cfg(feature = "op-stats")]
        let _caller =
            kiln_oven::trial_stats::CallerGuard::new("fold_rvalue", std::panic::Location::caller());
        if let Some(v) = self.early_immediate_value(e) {
            return Some(v);
        }
        if !self.eval_stack_ok(e) {
            return None;
        }
        self.with_oven(None, |oven, b| oven.fold_rvalue(b, e))
    }

    /// [`Oven::fold_for_overflow`] of `e` (clang `EvaluateForOverflow`):
    /// its value, if it folds, and the signed integer operations that
    /// overflowed on the way, which yielded their wrapped values.
    #[cfg_attr(feature = "op-stats", track_caller)]
    pub(crate) fn fold_for_overflow(
        &mut self,
        e: ExprId,
    ) -> (Option<ConstValueId>, Vec<kiln_oven::Overflow>) {
        #[cfg(feature = "op-stats")]
        let _caller = kiln_oven::trial_stats::CallerGuard::new(
            "fold_for_overflow",
            std::panic::Location::caller(),
        );
        if let Some(v) = self.early_immediate_value(e) {
            return (Some(v), Vec::new());
        }
        if !self.eval_stack_ok(e) {
            return (None, Vec::new());
        }
        self.with_oven(None, |oven, b| oven.fold_for_overflow(b, e))
    }

    /// [`Oven::evaluate_call`] of `f` without arguments at `at`: the
    /// failure, with its notes, if the call is not a constant expression.
    pub(crate) fn evaluate_call(&mut self, f: DeclId, at: Loc) -> Result<(), EvalFailure> {
        if (self.stack_exhausted() || self.stack_low())
            && !self.stack_ok(crate::stack_guard::StackPhase::ConstantEvaluation, at)
        {
            return Ok(());
        }
        self.with_oven(Some(at), |oven, b| oven.evaluate_call(b, f, at))
    }

    /// [`Oven::evaluate_potential`] of `e` (a never-constant check of a
    /// `constexpr` function body), reported at `at`.
    pub(crate) fn eval_potential(
        &mut self,
        e: ExprId,
        at: Loc,
    ) -> Result<ConstValueId, EvalFailure> {
        if !self.eval_stack_ok(e) {
            return Err(EvalFailure::silent(kiln_oven::FailKind::NotConstant));
        }
        self.flush_member_refs();
        if self.special.has_pending() {
            self.flush_pending_deleted();
        }
        self.with_oven(Some(at), |oven, b| oven.evaluate_potential(b, e))
    }

    /// The stack guard's check before entering the Oven (P4 (c)): the
    /// evaluation may instantiate, which re-enters sema.
    #[inline]
    fn eval_stack_ok(&self, e: ExprId) -> bool {
        if !self.stack_exhausted() && !self.stack_low() {
            return true;
        }
        let loc = self.ast.exprs.get(e).loc();
        self.stack_ok(crate::stack_guard::StackPhase::ConstantEvaluation, loc)
    }

    /// [`Sema::eval_const`] of an integral (or `bool`) expression.
    ///
    /// # Errors
    /// As [`Sema::eval_const`]; a value that is not an integer is a
    /// failure without notes.
    pub fn eval_int(&mut self, e: ExprId, ctx: EvalContext) -> Result<i128, EvalFailure> {
        let v = self.eval_const(e, ctx)?;
        self.const_int(v).ok_or_else(|| EvalFailure::silent(kiln_oven::FailKind::NotConstant))
    }

    /// [`Sema::try_fold`] of an integral (or `bool`) expression.
    #[cfg_attr(feature = "op-stats", track_caller)]
    pub fn fold_int(&mut self, e: ExprId) -> Option<i128> {
        #[cfg(feature = "op-stats")]
        let _caller =
            kiln_oven::trial_stats::CallerGuard::new("fold_int", std::panic::Location::caller());
        let v = self.try_fold(e)?;
        self.const_int(v)
    }

    /// Whether `var`'s initializer is a constant initializer
    /// ([basic.start.static]/2). Silent and not recorded.
    #[cfg_attr(feature = "op-stats", track_caller)]
    pub fn is_constant_initialized(&mut self, var: DeclId) -> bool {
        #[cfg(feature = "op-stats")]
        let _caller = kiln_oven::trial_stats::CallerGuard::new(
            "is_constant_initialized",
            std::panic::Location::caller(),
        );
        if self.stack_exhausted() || self.stack_low() {
            let loc = self.ast.decls.get(var).loc();
            let _ = self.stack_ok(crate::stack_guard::StackPhase::ConstantEvaluation, loc);
            return false;
        }
        self.with_oven(None, |oven, b| oven.is_constant_initialized(b, var))
    }

    /// clang's `-Wconstant-evaluated` for the initializer `e` of variable
    /// `v` that is neither `constexpr` nor `constinit` (those report it
    /// when checked): `CheckCompleteVariableDeclaration` evaluates the
    /// initializer of every variable with static or thread storage, or
    /// that might be usable in constant expressions (a `const`
    /// non-volatile integral or enumeration variable, a reference), as a
    /// constant initializer, so `__builtin_is_constant_evaluated()` or
    /// `std::is_constant_evaluated()` called directly by it is always
    /// true there, whether or not the initialization succeeds
    /// ([`kiln_oven::Oven::warn_constant_evaluated`]; spec/sema.md
    /// "Constant initialization"). Only an initializer that names one
    /// of them is evaluated.
    pub(crate) fn warn_constant_evaluated_init(&mut self, v: DeclId, e: ExprId) {
        if !self.constant_init_candidate(v)
            || !self.calls_is_constant_evaluated(e)
            || !self.constant_init_evaluable(v, e)
        {
            return;
        }
        self.with_oven(None, |oven, b| oven.warn_constant_evaluated(b, e, v));
    }

    /// The contract assertions of the initializer `e` of variable `v`
    /// that is neither `constexpr` nor `constinit` (those are checked as
    /// constant expressions): one whose initialization is
    /// constant-initialized is manifestly constant-evaluated
    /// ([expr.const.init]/4.1, [basic.contract.eval]/8;
    /// [`kiln_oven::Oven::check_contract_init`], spec/contracts.md §6.4).
    /// Only once the translation unit has contract assertions under a
    /// checking semantic.
    pub(crate) fn check_contract_init(&mut self, v: DeclId, e: ExprId) {
        if !self.has_contracts
            || !self.ast.contract_semantic.checks()
            || !self.constant_init_candidate(v)
            || !self.constant_init_evaluable(v, e)
        {
            return;
        }
        self.with_oven(None, |oven, b| oven.check_contract_init(b, e, v));
    }

    /// Whether `v` is a variable neither `constexpr` nor `constinit`
    /// whose initializer clang's `CheckCompleteVariableDeclaration`
    /// evaluates as a constant initializer: one with static or thread
    /// storage, or that might be usable in constant expressions (a
    /// `const` non-volatile integral or enumeration variable, a
    /// reference).
    fn constant_init_candidate(&self, v: DeclId) -> bool {
        let n = *self.ast.decls.get(v);
        let Some(data) = self.ast.decls.var(v) else { return false };
        if n.kind() != kiln_ast::decl::DeclKind::Var
            || n.has(kiln_ast::decl::DeclFlags::INVALID)
            || data.spec.is_constexpr()
            || data.spec.is_constinit()
        {
            return false;
        }
        let ty = n.ty().unwrap_or(QualType::ERROR);
        self.has_static_storage(v)
            || data.spec.tls() != kiln_ast::decl::TlsKind::None
            || self.ast.types.is_reference(ty)
            || {
                let q = self.ast.types.effective_quals(ty);
                q.is_const() && !q.is_volatile() && self.is_integral_or_enum(ty)
            }
    }

    /// Whether the initializer `e` of `v` can be evaluated at all.
    fn constant_init_evaluable(&mut self, v: DeclId, e: ExprId) -> bool {
        let ty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
        !self.is_dependent_type(ty)
            && !self.expr_is_value_dependent(e)
            && !self.has_errors_in(e)
            && self.eval_stack_ok(e)
    }

    /// Whether `e` calls `__builtin_is_constant_evaluated` or
    /// `std::is_constant_evaluated` directly (a bounded walk; a lambda's
    /// or a callee's body is not under `e`).
    fn calls_is_constant_evaluated(&self, e: ExprId) -> bool {
        use kiln_ast::expr::ExprKind;
        let mut stack = vec![e];
        let mut budget = 1usize << 16;
        while let Some(x) = stack.pop() {
            if budget == 0 {
                return false;
            }
            budget -= 1;
            if self.ast.exprs.get(x).kind() == ExprKind::CallExpr
                && let Some(f) = self.callee_decl(x)
                && (self.ast.decls.builtin_id(f)
                    == Some(kiln_ast::decl::builtin::IS_CONSTANT_EVALUATED)
                    || self.is_std_is_constant_evaluated(f))
            {
                return true;
            }
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        false
    }

    /// Whether function `f` is `std::is_constant_evaluated`.
    fn is_std_is_constant_evaluated(&self, f: DeclId) -> bool {
        self.ast
            .decls
            .get(f)
            .name()
            .as_ident()
            .is_some_and(|s| self.ast.symbols().as_str(s) == "is_constant_evaluated")
            && self.is_in_std_namespace(f)
    }

    /// The integer value of the scalar constant `v` (`bool` as 0/1), if
    /// it is one.
    pub fn const_int(&self, v: ConstValueId) -> Option<i128> {
        match self.ast.consts.scalar(v)? {
            Scalar::Int { bits, signed: true, .. } => bits.to_i128(),
            Scalar::Int { bits, .. } => bits.to_u128().and_then(|u| i128::try_from(u).ok()),
            Scalar::Bool { value, .. } => Some(i128::from(value)),
            _ => None,
        }
    }

    /// The truth value of the scalar constant `v` ([conv.bool]); `None`
    /// for an aggregate.
    pub fn const_truth(&self, v: ConstValueId) -> Option<bool> {
        match self.ast.consts.scalar(v)? {
            Scalar::Bool { value, .. } => Some(value),
            Scalar::Int { bits, .. } => Some(bits.to_u128() != Some(0)),
            Scalar::Float { kind, bits, .. } => Some(bits & !(1u128 << (kind.bits() - 1)) != 0),
            Scalar::Null { .. } => Some(false),
            _ => Some(true),
        }
    }

    /// `v` printed for a note ("expression evaluates to '…'").
    pub(crate) fn const_display(&self, v: ConstValueId) -> String {
        self.ast.consts.display(v, &self.ast.strings, &self.ast, &PrintOptions::DEFAULT)
    }

    /// The location of the diagnostic reporting the failed evaluation
    /// `f`, normally `loc`. As clang (`VerifyIntegerConstantExpression`,
    /// `CheckCompleteVariableDeclaration`, `CheckConvertedConstantExpression`),
    /// a lone "subexpression not valid" note adds nothing but a location:
    /// it is dropped and the diagnostic points where it did.
    pub(crate) fn fold_lone_invalid(f: &mut EvalFailure, loc: Loc) -> Loc {
        match &f.notes[..] {
            [n] if n.msg == "subexpression not valid in a constant expression" => {
                let at = n.loc;
                f.notes.clear();
                at
            }
            _ => loc,
        }
    }

    /// The failed integral constant expression `c` (evaluation failure
    /// `f`, diagnosed at `loc`) where clang's
    /// `VerifyIntegerConstantExpression` allows folding (an enumerator
    /// without a fixed underlying type, a bit-field width, an array
    /// designator): when `c` folds laxly ([`Sema::fold_rvalue_int`]) its
    /// value, with `expression is not an integral constant expression;
    /// folding it to a constant is a GNU extension`
    /// [-Wgnu-folding-constant, off unless -pedantic] and the notes; else
    /// `None` after the `expression is not an integral constant
    /// expression` error.
    pub(crate) fn fold_not_ice(&mut self, c: ExprId, mut f: EvalFailure, loc: Loc) -> Option<i128> {
        let at = Self::fold_lone_invalid(&mut f, loc);
        let folded = self.fold_rvalue_int(c);
        let id = if folded.is_some() {
            kiln_diag::DiagId::sema_expr_not_ice_folded
        } else {
            kiln_diag::DiagId::sema_expr_not_ice
        };
        let d = Self::with_failure_notes(self.diag(id, at), f);
        self.emit(d);
        folded
    }

    /// `d` with the notes explaining the failed evaluation `f` (the
    /// reason, the call stack and supporting notes, in clang's order).
    pub(crate) fn with_failure_notes(d: Diag, f: EvalFailure) -> Diag {
        f.notes.into_iter().fold(d, |d, n| {
            if n.is_error() {
                d.error_note(n.msg, Span::empty(n.loc))
            } else {
                d.note(n.msg, Span::empty(n.loc))
            }
        })
    }

    /// The constant evaluator of this translation unit.
    pub fn oven(&self) -> &Oven {
        let Some(o) = &self.oven else { unreachable!("the Oven is out during an evaluation") };
        o
    }

    /// Replaces the evaluator with one using `budget` and showing at most
    /// `backtrace_limit` call frames (0: all) in a failure's notes
    /// (`-fconstexpr-steps=` and friends). Call before parsing.
    pub fn set_constexpr_limits(&mut self, budget: Budget, backtrace_limit: usize) {
        let mut oven = Box::new(Oven::new(budget));
        oven.set_backtrace_limit(backtrace_limit);
        self.oven = Some(oven);
    }

    /// Records the constant-evaluation profile at `level`
    /// (`-fconstexpr-stats`, `-fconstexpr-profile`; spec/oven.md
    /// "Profile counters"). Call before parsing.
    pub fn set_constexpr_profile(&mut self, level: crate::ProfileLevel) {
        if let Some(o) = &mut self.oven {
            o.set_profile(level);
        }
    }

    /// The AST and the evaluator, at the end of the translation unit (the
    /// evaluator's [`Oven::stats`] are its `-fconstexpr-report=`).
    pub fn into_parts(self) -> (Ast, Oven) {
        let Some(mut oven) = self.oven else {
            unreachable!("the Oven is out during an evaluation")
        };
        // The splice fast path's count goes to the Oven's report: the
        // evaluations it saved (spec/reflection.md §5).
        oven.note_splice_fast(self.splice_fast);
        (self.ast, *oven)
    }
}
