//! The trial-fold fast path (spec/oven.md "Trial folds", WP-656): a
//! silent outermost fold ([`Oven::try_fold`], [`Oven::fold_rvalue`]) of an
//! integer expression made only of literals, enumerators, `sizeof` and
//! `alignof` of a type, integral and boolean conversions and the integer
//! unary, binary and conditional operators is computed by a direct walk of
//! the expression, with no thunk compile and no VM run. The walk computes
//! exactly what the compiled thunk would (the same `u128` register values,
//! the same `crate::arith` operations, the same freeze), so its answer is
//! the full path's; anything outside the whitelist, and any operation that
//! fails (overflow, `/ 0`, a bad shift), declines, and the full path runs.
//! Both arms of `?:`, `&&` and `||` must walk, since the full path
//! compiles the arm it does not run. A variable read the walk knows
//! ([`Oven::leaf_read`], WP-663) is answered too: the value of a resolved
//! constant integer variable, or a definite failure for a parameter or
//! automatic variable the full path faults on (the fold answers `None`
//! without the full path), whose global entry and resolution the walk
//! makes as the full path would ([`Oven::leaf_effects`]); so is a `.`
//! member read of such a parameter or variable, and a scalar glvalue fold
//! (codegen's integral member accesses), as that read.
//! The initializer [`Oven::is_constant_initialized`] checks is walked
//! the same way ([`Oven::leaf_init`]).

use kiln_ast::consts::Scalar;
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind, TraitKind, UnaryOp};
use kiln_ast::types::TypeView;
use kiln_ast::{ConstValueId, DeclId, ExprId};
use kiln_source::Loc;

use crate::arith::{canon, int_bin};
use crate::compile::{GState, is_local_var, is_vla, scalar_bits};
use crate::kbc::Op;
use crate::mem::{LAX_READ, Origin, Ptr, flags};
use crate::ty::{TyKind, eval_int};
use crate::value::var_info;
use crate::{Oven, SemaBridge};
use kiln_ast::decl::{DeclKind, VarEvalState};

/// Expression depth beyond which the walk declines.
const MAX_DEPTH: u32 = 64;

/// A value and its type's kind (`Int` or `Bool`).
type Val = (u128, TyKind);

/// A definite failure: the full path's evaluation faults, at the read of
/// a variable whose global it resolves first (`Some`: unresolved as the
/// next outermost evaluation finds it) or finds resolved (`None`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Failed(Option<DeclId>);

/// A walked subexpression: its value, or a definite failure.
type Lv = Result<Val, Failed>;

/// The fast path's answer for a fold it takes: the value, or `None` for
/// a definite failure.
pub(crate) type Leaf = Option<ConstValueId>;

/// The width and signedness of an `Int` or `Bool` kind.
fn int_of(k: TyKind) -> (u32, bool) {
    match k {
        TyKind::Int { width, signed } => (u32::from(width), signed),
        _ => (1, false),
    }
}

impl Oven {
    /// The silent outermost fold's fast path: `Some` of `e`'s answer by
    /// [`Oven::leaf_fold`] (a value, or `None` for a definite failure),
    /// or `None` to run the full path. With the `leaf-check` feature
    /// every answer is checked against the full path's (a validation
    /// build for the corpus and the benches).
    pub(crate) fn leaf_first(&mut self, b: &mut dyn SemaBridge, e: ExprId) -> Option<Leaf> {
        let r = self.leaf_fold(b, e, None);
        #[cfg(feature = "leaf-check")]
        if let Some(v) = r {
            let full = self.eval_ctx(b, e, None, true, false, false).0.ok();
            let x = v.map(|v| b.consts().scalar(v).map(|s| format!("{s:?}")));
            let y = full.map(|f| b.consts().scalar(f).map(|s| format!("{s:?}")));
            assert_eq!(x, y, "leaf fold of {e:?} differs from the full path");
        }
        r
    }

    /// [`Oven::is_constant_initialized`]'s fast path (WP-663): `Some` of
    /// whether variable `var`'s initializer `init` is constant, where
    /// [`Oven::leaf_fold`] answers it, else `None` to run the full path.
    /// The full path compiles `init` as `var`'s initializer, which for a
    /// non-reference variable and a scalar prvalue `init` is the fold's
    /// thunk (`rv_into`, no extended temporary: none is in the walk's
    /// domain), and no walked node depends on the manifest context (an
    /// operation failing in one mode only, `Op::Cce`, comes only from
    /// `reinterpret_cast`). What the full path leaves behind is redone:
    /// provisional resolutions are forgotten (`begin_outermost`), the
    /// placeholder of a non-local `var` is placed and its global left
    /// unresolved (`eval_inner`), and the static peak is noted
    /// (`end_outermost`), so the globals, the static space and
    /// `static_bytes` are as the full path leaves them.
    pub(crate) fn leaf_init(
        &mut self,
        b: &mut dyn SemaBridge,
        var: DeclId,
        init: ExprId,
    ) -> Option<bool> {
        let ast = b.ast();
        if ast.decls.get(var).ty().is_none_or(|t| ast.types.is_reference(t)) {
            return None;
        }
        let r = self.leaf_fold(b, init, Some(var))?.is_some();
        #[cfg(feature = "leaf-check")]
        {
            let full = self.eval_ctx(b, init, Some(var), true, false, true).0.is_ok();
            assert_eq!(r, full, "leaf init of {var:?} differs from the full path");
        }
        Some(r)
    }

    /// [`Oven::try_fold`] by the full path only (test-only: the fast
    /// path's equivalence tests compare against it).
    #[doc(hidden)]
    pub fn try_fold_full(&mut self, b: &mut dyn SemaBridge, e: ExprId) -> Option<ConstValueId> {
        self.eval_ctx(b, e, None, true, false, false).0.ok()
    }

    /// [`Oven::fold_rvalue`] by the full path only (test-only).
    #[doc(hidden)]
    pub fn fold_rvalue_full(&mut self, b: &mut dyn SemaBridge, e: ExprId) -> Option<ConstValueId> {
        (self.run.lax, self.mem.lax) = (true, true);
        let r = self.eval_ctx(b, e, None, true, false, false).0.ok();
        (self.run.lax, self.mem.lax) = (false, false);
        r
    }

    /// [`Oven::try_fold`] by the fast path only: `None` where it declines,
    /// `Some(None)` for a definite failure (test-only).
    #[doc(hidden)]
    pub fn try_fold_leaf(
        &mut self,
        b: &mut dyn SemaBridge,
        e: ExprId,
    ) -> Option<Option<ConstValueId>> {
        self.leaf_fold(b, e, None)
    }

    /// `e`'s answer by the fast path (`Some(None)`: the full path
    /// certainly fails), if `e` is in its domain and no pending one-shot
    /// evaluation setting would make the full path differ (a running
    /// evaluation, a warning or overflow check, an address or `void`
    /// result, a source-location root); `None` to run the full path.
    pub(crate) fn leaf_fold(
        &mut self,
        b: &mut dyn SemaBridge,
        e: ExprId,
        object: Option<DeclId>,
    ) -> Option<Leaf> {
        if self.ctx_depth != 0
            || self.cev_warn_next
            || self.ub_check_next
            || self.potential_next.is_some()
            || self.plain_next
            || self.void_next
            || self.addr_next
            || self.srcloc_root.is_some()
        {
            return None;
        }
        self.types.refresh(b);
        self.mp_refresh(b);
        let mut fx = Vec::new();
        let n = *b.ast().exprs.get(e);
        let r = if n.value_category().is_glvalue() {
            // A scalar glvalue fold (`compile_thunk`: `lv`, `load`) is the
            // read an lvalue-to-rvalue conversion of it would be.
            if object.is_some() || !matches!(n.kind(), ExprKind::DeclRefExpr | ExprKind::MemberExpr)
            {
                return None;
            }
            let t = self.types.intern(b, n.ty());
            let tk = self.types.kind(t);
            if !matches!(tk, TyKind::Bool | TyKind::Int { width: ..=128, .. }) {
                return None;
            }
            self.leaf_read(b, e, tk, &mut fx)?
        } else {
            self.leaf_val(b, e, 0, &mut fx)?
        };
        self.leaf_effects(b, object, &fx, r.err());
        let Ok((v, _)) = r else { return Some(None) };
        let q = b.ast().exprs.get(e).ty();
        let ty = self.types.intern(b, q);
        Some(self.freeze_scalar(b, ty, v, Loc::INVALID).ok())
    }

    /// What the full path leaves behind for an answered fold of `e`,
    /// redone in its order: provisional resolutions are forgotten
    /// (`begin_outermost`); the placeholder of a non-local `object` is
    /// placed (`eval_inner`); the thunk compile creates the global entries
    /// `news` lacks, in compile order, which is the walk's (every operand,
    /// run or not, left to right: `?:` its condition and then both arms,
    /// as `Compiler::rv_into`, `logical` and `arith_spine` emit them); the
    /// run resolves the global of the variable whose read faults first
    /// (`Op::GlobalAddr`, `resolve_global`); the placeholder's global is
    /// left unresolved again (`eval_inner`); and the static peak is noted
    /// (`end_outermost`). So the globals, the static space and the
    /// `static_bytes` statistic are as the full path leaves them.
    fn leaf_effects(
        &mut self,
        b: &mut dyn SemaBridge,
        object: Option<DeclId>,
        news: &[DeclId],
        fail: Option<Failed>,
    ) {
        for g in self.provisional.drain(..) {
            self.globals[g as usize].state = GState::Unresolved;
        }
        let placed = object.and_then(|d| self.place_init_object(b, d));
        for &d in news {
            self.global_index(b.ast(), d);
        }
        if let Some(Failed(Some(d))) = fail {
            let g = self.global_index(b.ast(), d);
            if self.globals[g as usize].state == GState::Unresolved {
                self.resolve_global(b, g, 0);
            }
        }
        if let Some(g) = placed {
            self.globals[g as usize].state = GState::Unresolved;
        }
        self.stats.note_static_peak(self.mem.static_peak());
    }

    /// The value of prvalue `e` as the compiled thunk's register holds it:
    /// `Some(Ok)` a value, `Some(Err)` a definite failure (a read of a
    /// variable the full path faults on, [`Oven::leaf_read`]),
    /// `None` outside the domain. A strict operator fails if an operand
    /// does; `?:`, `&&` and `||` take the arm that runs, once the
    /// condition has a value. Any operand outside the domain, run or not,
    /// declines the whole expression (the full path compiles it).
    fn leaf_val(
        &mut self,
        b: &mut dyn SemaBridge,
        e: ExprId,
        depth: u32,
        fx: &mut Vec<DeclId>,
    ) -> Option<Lv> {
        if depth > MAX_DEPTH {
            return None;
        }
        let n = *b.ast().exprs.get(e);
        if n.value_category().is_glvalue() {
            return None;
        }
        let t = self.types.intern(b, n.ty());
        let tk = self.types.kind(t);
        // A `_BitInt` wider than 128 bits traps in the full path.
        if !matches!(tk, TyKind::Bool | TyKind::Int { width: ..=128, .. }) {
            return None;
        }
        let (w, s) = int_of(tk);
        let d = depth + 1;
        let v = match n.kind() {
            ExprKind::IntegerLiteral => canon(eval_int(b.ast(), e).unwrap_or(0), w, s),
            ExprKind::CharacterLiteral => canon(u128::from(b.ast().exprs.char_value(e)), w, s),
            ExprKind::CXXBoolLiteralExpr => u128::from(b.ast().exprs.bool_value(e)),
            ExprKind::CXXNoexceptExpr => u128::from(b.ast().exprs.noexcept_value(e)),
            ExprKind::TypeTraitExpr | ExprKind::ExpressionTraitExpr => {
                u128::from(b.ast().exprs.type_trait_value(e))
            }
            ExprKind::ParenExpr => {
                let x = b.ast().exprs.operand(e);
                return self.leaf_val(b, x, d, fx);
            }
            ExprKind::SubstNonTypeTemplateParmExpr => {
                let x = b.ast().exprs.subst_replacement(e);
                return self.leaf_val(b, x, d, fx);
            }
            ExprKind::ConstantExpr => {
                let ast = b.ast();
                let scalar = ast.exprs.constant_value(e).and_then(|c| ast.consts.scalar(c));
                let x = ast.exprs.operand(e);
                match scalar {
                    Some(sc @ (Scalar::Int { .. } | Scalar::Bool { .. })) => scalar_bits(&sc)?,
                    Some(_) => return None,
                    None => return self.leaf_val(b, x, d, fx),
                }
            }
            ExprKind::DeclRefExpr => {
                let ast = b.ast();
                let c = ast.decls.enumerator(ast.exprs.decl(e)).and_then(|x| x.value)?;
                canon(ast.consts.scalar(c).and_then(|x| scalar_bits(&x)).unwrap_or(0), w, s)
            }
            ExprKind::UnaryExprOrTypeTraitExpr => canon(self.leaf_trait(b, e)?, w, s),
            ExprKind::UnaryOperator => {
                let x = b.ast().exprs.operand(e);
                let op = b.ast().exprs.unary_op(e);
                if !matches!(op, UnaryOp::Plus | UnaryOp::Minus | UnaryOp::Not | UnaryOp::LNot) {
                    return None;
                }
                let (vx, _) = match self.leaf_val(b, x, d, fx)? {
                    Ok(v) => v,
                    Err(f) => return Some(Err(f)),
                };
                match op {
                    UnaryOp::Plus => vx,
                    UnaryOp::Minus => int_bin(Op::ISub, w, s, 0, vx, 0, false).ok()?,
                    UnaryOp::Not => canon(!vx, w, s),
                    _ => u128::from(vx == 0),
                }
            }
            ExprKind::BinaryOperator => match self.leaf_binary(b, e, w, s, d, fx)? {
                Ok(v) => v,
                Err(f) => return Some(Err(f)),
            },
            ExprKind::ConditionalOperator => {
                let c = self.leaf_val(b, ExprId::new(n.a()), d, fx)?;
                let [x, y] = b.ast().exprs.cond_arms(e);
                let (rx, ry) = (self.leaf_val(b, x, d, fx)?, self.leaf_val(b, y, d, fx)?);
                let (c, _) = match c {
                    Ok(c) => c,
                    Err(f) => return Some(Err(f)),
                };
                return Some(if c != 0 { rx } else { ry }.map(|(v, _)| (v, tk)));
            }
            k if k.is_cast() && k != ExprKind::CXXReinterpretCastExpr => {
                let ast = b.ast();
                let op = ast.exprs.operand(e);
                match ast.exprs.cast_kind(e) {
                    CastKind::NoOp => {
                        return Some(self.leaf_val(b, op, d, fx)?.map(|(v, _)| (v, tk)));
                    }
                    // A conversion to an enumeration checks its range.
                    CastKind::IntegralCast
                        if matches!(ast.types.canonical_view(n.ty()), TypeView::Enum(_)) =>
                    {
                        return None;
                    }
                    CastKind::IntegralCast | CastKind::IntegralToBoolean => {
                        // `Compiler::convert` between `Int`/`Bool` kinds.
                        let (v, fk) = match self.leaf_val(b, op, d, fx)? {
                            Ok(v) => v,
                            Err(f) => return Some(Err(f)),
                        };
                        if fk == tk {
                            v
                        } else if tk == TyKind::Bool {
                            u128::from(v != 0)
                        } else {
                            canon(v, w, s)
                        }
                    }
                    CastKind::LValueToRValue => return self.leaf_read(b, op, tk, fx),
                    _ => return None,
                }
            }
            _ => return None,
        };
        Some(Ok((v, tk)))
    }

    /// The full path's read of glvalue `op` (an lvalue-to-rvalue
    /// conversion to `tk`), where the walk knows it: `op` names (through
    /// parentheses) a variable whose `GlobalAddr`, `Load` either reads a
    /// known value or certainly faults, or a member of one
    /// ([`Oven::leaf_member_base`], WP-663 B4: `GlobalAddr`, `Field`s,
    /// `Load`), which is only ever a definite failure. The global is taken as the next
    /// outermost evaluation finds it: a provisional resolution is
    /// forgotten first (`begin_outermost`).
    ///
    /// - `Some(Err)`, a definite failure: a function parameter or an
    ///   automatic variable (outside its frame: the walk runs only at the
    ///   outermost level), neither usable in constant expressions nor
    ///   laxly readable (`value::var_info`). If its global is resolved,
    ///   to a live external object that no lax read may read, the `Load`
    ///   faults (`MemFault::External`) in a strict and in a lax fold
    ///   alike. If it is unresolved (or has no entry yet), and the
    ///   variable is not a reference, `resolve_global` resolves it to an
    ///   external object (`resolve_var_in`: neither usable nor lax; or
    ///   the provisional object it made before), whose `Load` faults the
    ///   same way: the failure names the variable, whose resolution
    ///   [`Oven::leaf_effects`] redoes if this read is the one that runs
    ///   first. A variable with no global entry is pushed on `fx`, in the
    ///   order the thunk compile would create the entries.
    /// - `Some(Ok)`, the value: a variable usable in constant
    ///   expressions, not a reference, not `volatile`, of `tk`'s kind,
    ///   whose initializer's value is known (`VarEvalState::Value`) and
    ///   whose global is resolved to a constant, initialized object
    ///   holding it (not external, not under construction, not lax-only):
    ///   the `Load` reads that value in both modes.
    /// - `None` otherwise: the full path runs.
    fn leaf_read(
        &mut self,
        b: &mut dyn SemaBridge,
        op: ExprId,
        tk: TyKind,
        fx: &mut Vec<DeclId>,
    ) -> Option<Lv> {
        let mut x = op;
        while b.ast().exprs.get(x).kind() == ExprKind::ParenExpr {
            x = b.ast().exprs.operand(x);
        }
        let member = b.ast().exprs.get(x).kind() == ExprKind::MemberExpr;
        if member {
            x = self.leaf_member_base(b, x)?;
        }
        let ast = b.ast();
        if ast.exprs.get(x).kind() != ExprKind::DeclRefExpr {
            return None;
        }
        let d = ast.exprs.decl(x);
        if !matches!(ast.decls.get(d).kind(), DeclKind::Var | DeclKind::ParmVar) {
            return None;
        }
        let info = var_info(ast, d);
        let entry = self.global_map.get(&ast.first(d)).copied();
        let state = match entry.map(|g| (g, self.globals[g as usize].state)) {
            None => GState::Unresolved,
            Some((g, GState::Resolved(_))) if self.provisional.contains(&g) => GState::Unresolved,
            Some((_, st)) => st,
        };
        if info.usable {
            if member {
                return None;
            }
            let VarEvalState::Value(cv) = info.state else { return None };
            let GState::Resolved(p) = state else { return None };
            let c = ast.types.canonical(info.ty);
            if ast.types.is_reference(c) || ast.types.effective_quals(info.ty).is_volatile() {
                return None;
            }
            let bits = match ast.consts.scalar(cv)? {
                sc @ (Scalar::Int { .. } | Scalar::Bool { .. }) => scalar_bits(&sc)?,
                _ => return None,
            };
            let t = self.types.intern(b, info.ty);
            if self.types.kind(t) != tk {
                return None;
            }
            let a = self.mem.alloc_of(Ptr::decode(p)).ok()?;
            let bad = flags::EXTERNAL | flags::RESOLVING | flags::TRACKED;
            let ok = a.flags & flags::CONST != 0
                && a.flags & bad == 0
                && a.frame != LAX_READ
                && a.ty == t
                && matches!(a.origin, Origin::Global(_));
            let (w, s) = int_of(tk);
            return ok.then_some(Ok((canon(bits, w, s), tk)));
        }
        if info.lax || !is_local_var(ast, d) {
            return None;
        }
        match state {
            GState::Resolved(p) => {
                let a = self.mem.alloc_of(Ptr::decode(p)).ok()?;
                let fails = a.flags & flags::EXTERNAL != 0
                    && matches!(a.origin, Origin::External(_))
                    && a.frame != LAX_READ;
                fails.then_some(Err(Failed(None)))
            }
            GState::Unresolved => {
                // A reference's object may be bound to an unknown one
                // (`bind_unknown`), which the read reaches.
                if ast.decls.get(d).ty().is_none_or(|q| ast.types.is_reference(q)) {
                    return None;
                }
                if entry.is_none() {
                    fx.push(d);
                }
                Some(Err(Failed(Some(d))))
            }
        }
    }

    /// The object variable of `.` member access `e` (through parentheses
    /// and nested `.` accesses) whose read the full path compiles as
    /// `GlobalAddr` of the variable, `Field` offsets and a `Load`
    /// (`Compiler::member`): each member a non-static, non-reference,
    /// non-bit-field data member of a non-union class the Oven lays out.
    /// `None` for anything else (`->`, a static data member, a union,
    /// a base that is not a variable).
    fn leaf_member_base(&mut self, b: &mut dyn SemaBridge, mut e: ExprId) -> Option<ExprId> {
        for _ in 0..MAX_DEPTH {
            let ast = b.ast();
            match ast.exprs.get(e).kind() {
                ExprKind::ParenExpr => e = ast.exprs.operand(e),
                ExprKind::MemberExpr => {
                    let n = *ast.exprs.get(e);
                    let fd = ast.exprs.decl(e);
                    if n.has(kiln_ast::expr::ExprFlags::ARROW)
                        || ast.decls.get(fd).kind() != DeclKind::Field
                        || ast.decls.get(fd).ty().is_none_or(|q| ast.types.is_reference(q))
                    {
                        return None;
                    }
                    let base = ExprId::new(n.a());
                    let bq = ast.exprs.get(base).ty();
                    let rt = self.types.intern(b, bq);
                    let TyKind::Record { union: false, .. } = self.types.kind(rt) else {
                        return None;
                    };
                    let idx = self.types.field_index(rt, fd)?;
                    if self.types.fields(rt)[idx].width.is_some() {
                        return None;
                    }
                    e = base;
                }
                ExprKind::DeclRefExpr => return Some(e),
                _ => return None,
            }
        }
        None
    }

    /// A builtin binary operator of integer type `(w, s)`.
    fn leaf_binary(
        &mut self,
        b: &mut dyn SemaBridge,
        e: ExprId,
        w: u32,
        s: bool,
        d: u32,
        fx: &mut Vec<DeclId>,
    ) -> Option<Result<u128, Failed>> {
        let op = b.ast().exprs.binary_op(e);
        if op.is_compound_assignment() || op.is_ptr_mem() {
            return None;
        }
        let [x, y] = b.ast().exprs.operands(e);
        let iop = match op {
            BinaryOp::Comma | BinaryOp::Assign | BinaryOp::Cmp => return None,
            BinaryOp::LAnd | BinaryOp::LOr => {
                let (rx, ry) = (self.leaf_val(b, x, d, fx)?, self.leaf_val(b, y, d, fx)?);
                let (vx, _) = match rx {
                    Ok(v) => v,
                    Err(f) => return Some(Err(f)),
                };
                let lor = op == BinaryOp::LOr;
                return Some(if (vx != 0) == lor { Ok(vx) } else { ry.map(|(v, _)| v) });
            }
            BinaryOp::EQ
            | BinaryOp::NE
            | BinaryOp::LT
            | BinaryOp::GT
            | BinaryOp::LE
            | BinaryOp::GE => {
                let (rx, ry) = (self.leaf_val(b, x, d, fx)?, self.leaf_val(b, y, d, fx)?);
                let ((vx, xk), (vy, _)) = match (rx, ry) {
                    (Ok(x), Ok(y)) => (x, y),
                    (Err(f), _) | (_, Err(f)) => return Some(Err(f)),
                };
                let o = if int_of(xk).1 { (vx as i128).cmp(&(vy as i128)) } else { vx.cmp(&vy) };
                let r = match op {
                    BinaryOp::EQ => o.is_eq(),
                    BinaryOp::NE => o.is_ne(),
                    BinaryOp::LT => o.is_lt(),
                    BinaryOp::GT => o.is_gt(),
                    BinaryOp::LE => o.is_le(),
                    _ => o.is_ge(),
                };
                return Some(Ok(u128::from(r)));
            }
            BinaryOp::Mul => Op::IMul,
            BinaryOp::Div => Op::IDiv,
            BinaryOp::Rem => Op::IRem,
            BinaryOp::Add => Op::IAdd,
            BinaryOp::Sub => Op::ISub,
            BinaryOp::Shl => Op::IShl,
            BinaryOp::Shr => Op::IShr,
            BinaryOp::And => Op::IAnd,
            BinaryOp::Xor => Op::IXor,
            BinaryOp::Or => Op::IOr,
            _ => return None,
        };
        let (rx, ry) = (self.leaf_val(b, x, d, fx)?, self.leaf_val(b, y, d, fx)?);
        // The first failure in run order (left to right) is the full
        // path's.
        let ((vx, _), (vy, yk)) = match (rx, ry) {
            (Ok(x), Ok(y)) => (x, y),
            (Err(f), _) | (_, Err(f)) => return Some(Err(f)),
        };
        let (cw, cs) = if matches!(iop, Op::IShl | Op::IShr) { int_of(yk) } else { (0, false) };
        Some(Ok(int_bin(iop, w, s, vx, vy, cw, cs).ok()?))
    }

    /// `sizeof` of any type but a variable length array, `alignof` of a
    /// type no typedef in whose sugar carries attributes; `None` for the
    /// rest (`alignof` of an expression, `__datasizeof`, `vec_step`).
    fn leaf_trait(&mut self, b: &mut dyn SemaBridge, e: ExprId) -> Option<u128> {
        let ast = b.ast();
        let kind = ast.exprs.trait_kind(e);
        let q = match ast.exprs.trait_type_arg(e) {
            Some(q) => q,
            None if kind == TraitKind::SizeOf => {
                ast.exprs.get(ExprId::new(ast.exprs.get(e).a())).ty()
            }
            None => return None,
        };
        let q = match ast.types.canonical_view(q) {
            TypeView::LValueRef(r) | TypeView::RValueRef(r) => r,
            _ => q,
        };
        match kind {
            TraitKind::SizeOf if !is_vla(ast, q) => {
                let t = self.types.intern(b, q);
                Some(u128::from(self.types.size(t)))
            }
            TraitKind::AlignOf | TraitKind::PreferredAlignOf if !typedef_attributed(ast, q) => {
                let t = self.types.intern(b, q);
                Some(u128::from(self.types.info(t).align))
            }
            _ => None,
        }
    }
}

/// Whether a typedef in `q`'s sugar chain has attributes
/// (`Compiler::typedef_align`, which then asks the ABI).
fn typedef_attributed(ast: &kiln_ast::Ast, q: kiln_ast::QualType) -> bool {
    let mut x = q;
    loop {
        if let TypeView::Typedef { decl, .. } = ast.types.view(x.ty())
            && ast.decls.get(decl).has(kiln_ast::decl::DeclFlags::HAS_ATTRS)
        {
            return true;
        }
        let d = ast.types.desugar_once(x);
        if d.ty() == x.ty() {
            return false;
        }
        x = d;
    }
}
