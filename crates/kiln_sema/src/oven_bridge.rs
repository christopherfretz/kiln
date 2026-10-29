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
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, StmtId};
use kiln_diag::Diag;
use kiln_oven::{Budget, EvalContext, EvalFailure, Oven, SemaBridge, Subobject, SubobjectId};
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

    fn ensure_body(&mut self, f: DeclId, oven: &mut Oven) -> Option<StmtId> {
        // A body exists once its definition has been parsed or
        // instantiated (a later definition is picked up by the next
        // evaluation). Implicit and defaulted members are defined here on
        // demand from WP-067b.
        if let Some(b) = self.sema.defined_body(f) {
            return Some(b);
        }
        self.sema.instantiate_for_oven(f, oven)
    }

    fn ensure_layout(&mut self, record: DeclId) -> &RecordLayout {
        let s = &mut *self.sema;
        kiln_abi::ensure_layout(&mut s.ast, record, &s.abi)
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
}

impl<'a, 'sm> Sema<'a, 'sm> {
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
        let saved = std::mem::replace(&mut self.inst.eval_loc, eval_loc);
        let r = f(&mut oven, &mut Bridge { sema: self });
        self.inst.eval_loc = saved;
        self.oven = Some(oven);
        r
    }

    /// The body of `f`'s definition, if it has one.
    fn defined_body(&self, f: DeclId) -> Option<StmtId> {
        let d = self.ast.definition(f)?;
        self.ast.decls.function(d)?.body
    }

    /// `ensure_body` of a function sema has not defined
    /// (spec/sema-templates.md §4.11): a member of a class template
    /// specialization that no expression marked (one reached through a
    /// virtual call or a pointer) is instantiated now, the running
    /// evaluation's location its point of instantiation, with the Oven
    /// lent back to sema so the instantiation can evaluate (nested,
    /// R-OVEN-1). A specialization already marked but not instantiable
    /// then (its pattern was defined later) stays undefined, as clang 19
    /// never instantiates from the evaluator ("undefined function 'f'
    /// cannot be used in a constant expression"); so does everything
    /// during a speculative fold.
    fn instantiate_for_oven(&mut self, f: DeclId, oven: &mut Oven) -> Option<StmtId> {
        let poi = self.inst.eval_loc?;
        if self.inst.fatal
            || self.ast.instantiated_from(f).is_none()
            || self.inst.queued.contains(&f)
        {
            return None;
        }
        debug_assert!(self.oven.is_none(), "ensure_body outside an evaluation");
        let lent = std::mem::replace(oven, Oven::new(oven.budget()));
        self.oven = Some(lent);
        self.instantiate_function_definition(f, poi);
        if let Some(o) = self.oven.take() {
            *oven = o;
        }
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
        self.flush_member_refs();
        if !self.special.pending_deleted.is_empty() {
            self.flush_pending_deleted();
        }
        self.with_oven(Some(ctx.loc), |oven, b| oven.evaluate(b, e, ctx))
    }

    /// The value of `e` if it is a constant expression. Silent (no
    /// diagnostics, no notes) and not recorded in the statistics: for
    /// warnings and other speculative folds.
    pub fn try_fold(&mut self, e: ExprId) -> Option<ConstValueId> {
        self.with_oven(None, |oven, b| oven.try_fold(b, e))
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
    pub fn fold_int(&mut self, e: ExprId) -> Option<i128> {
        let v = self.try_fold(e)?;
        self.const_int(v)
    }

    /// Whether `var`'s initializer is a constant initializer
    /// ([basic.start.static]/2). Silent and not recorded.
    pub fn is_constant_initialized(&mut self, var: DeclId) -> bool {
        self.with_oven(None, |oven, b| oven.is_constant_initialized(b, var))
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

    /// `d` with the notes explaining the failed evaluation `f` (the
    /// reason, the call stack and supporting notes, in clang's order).
    pub(crate) fn with_failure_notes(d: Diag, f: EvalFailure) -> Diag {
        f.notes.into_iter().fold(d, |d, n| d.note(n.msg, Span::empty(n.loc)))
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
        let mut oven = Oven::new(budget);
        oven.set_backtrace_limit(backtrace_limit);
        self.oven = Some(oven);
    }

    /// The AST and the evaluator, at the end of the translation unit (the
    /// evaluator's [`Oven::stats`] are its `-fconstexpr-report=`).
    pub fn into_parts(self) -> (Ast, Oven) {
        let Some(oven) = self.oven else { unreachable!("the Oven is out during an evaluation") };
        (self.ast, oven)
    }
}
