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
//! compiles the arm it does not run.

use kiln_ast::consts::Scalar;
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind, TraitKind, UnaryOp};
use kiln_ast::types::TypeView;
use kiln_ast::{ConstValueId, ExprId};
use kiln_source::Loc;

use crate::arith::{canon, int_bin};
use crate::compile::{is_vla, scalar_bits};
use crate::kbc::Op;
use crate::ty::{TyKind, eval_int};
use crate::{Oven, SemaBridge};

/// Expression depth beyond which the walk declines.
const MAX_DEPTH: u32 = 64;

/// A value and its type's kind (`Int` or `Bool`).
type Val = (u128, TyKind);

/// The width and signedness of an `Int` or `Bool` kind.
fn int_of(k: TyKind) -> (u32, bool) {
    match k {
        TyKind::Int { width, signed } => (u32::from(width), signed),
        _ => (1, false),
    }
}

impl Oven {
    /// The silent outermost fold's fast path: `e`'s value by
    /// [`Oven::leaf_fold`], or `None` to run the full path. With the
    /// `leaf-check` feature every answer is checked against the full
    /// path's (a validation build for the corpus and the benches).
    pub(crate) fn leaf_first(&mut self, b: &mut dyn SemaBridge, e: ExprId) -> Option<ConstValueId> {
        let r = self.leaf_fold(b, e);
        #[cfg(feature = "leaf-check")]
        if let Some(v) = r {
            let full = self.eval_ctx(b, e, None, true, false, false).0.ok();
            let x = b.consts().scalar(v).map(|s| format!("{s:?}"));
            let y = full.and_then(|f| b.consts().scalar(f)).map(|s| format!("{s:?}"));
            assert_eq!(x, y, "leaf fold of {e:?} differs from the full path");
        }
        r
    }

    /// [`Oven::try_fold`] by the full path only (test-only: the fast
    /// path's equivalence tests compare against it).
    #[doc(hidden)]
    pub fn try_fold_full(&mut self, b: &mut dyn SemaBridge, e: ExprId) -> Option<ConstValueId> {
        self.eval_ctx(b, e, None, true, false, false).0.ok()
    }

    /// [`Oven::try_fold`] by the fast path only: `None` where it declines
    /// (test-only).
    #[doc(hidden)]
    pub fn try_fold_leaf(&mut self, b: &mut dyn SemaBridge, e: ExprId) -> Option<ConstValueId> {
        self.leaf_fold(b, e)
    }

    /// `e`'s value by the fast path, if `e` is in its domain and no
    /// pending one-shot evaluation setting would make the full path differ
    /// (a running evaluation, a warning or overflow check, an address or
    /// `void` result, a source-location root); `None` to run the full
    /// path.
    pub(crate) fn leaf_fold(&mut self, b: &mut dyn SemaBridge, e: ExprId) -> Option<ConstValueId> {
        if self.ctx_depth != 0
            || self.cev_warn_next
            || self.ub_check_next
            || self.potential_next
            || self.plain_next
            || self.void_next
            || self.addr_next
            || self.srcloc_root.is_some()
        {
            return None;
        }
        self.types.refresh(b);
        let (v, ..) = self.leaf_val(b, e, 0)?;
        let q = b.ast().exprs.get(e).ty();
        let ty = self.types.intern(b, q);
        self.freeze_scalar(b, ty, v, Loc::INVALID).ok()
    }

    /// The value of prvalue `e` as the compiled thunk's register holds it.
    fn leaf_val(&mut self, b: &mut dyn SemaBridge, e: ExprId, depth: u32) -> Option<Val> {
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
                self.leaf_val(b, x, d)?.0
            }
            ExprKind::SubstNonTypeTemplateParmExpr => {
                let x = b.ast().exprs.subst_replacement(e);
                self.leaf_val(b, x, d)?.0
            }
            ExprKind::ConstantExpr => {
                let ast = b.ast();
                let scalar = ast.exprs.constant_value(e).and_then(|c| ast.consts.scalar(c));
                let x = ast.exprs.operand(e);
                match scalar {
                    Some(sc @ (Scalar::Int { .. } | Scalar::Bool { .. })) => scalar_bits(&sc)?,
                    Some(_) => return None,
                    None => self.leaf_val(b, x, d)?.0,
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
                match b.ast().exprs.unary_op(e) {
                    UnaryOp::Plus => self.leaf_val(b, x, d)?.0,
                    UnaryOp::Minus => {
                        int_bin(Op::ISub, w, s, 0, self.leaf_val(b, x, d)?.0, 0, false).ok()?
                    }
                    UnaryOp::Not => canon(!self.leaf_val(b, x, d)?.0, w, s),
                    UnaryOp::LNot => u128::from(self.leaf_val(b, x, d)?.0 == 0),
                    _ => return None,
                }
            }
            ExprKind::BinaryOperator => self.leaf_binary(b, e, w, s, d)?,
            ExprKind::ConditionalOperator => {
                let c = self.leaf_val(b, ExprId::new(n.a()), d)?.0;
                let [x, y] = b.ast().exprs.cond_arms(e);
                let (vx, vy) = (self.leaf_val(b, x, d)?.0, self.leaf_val(b, y, d)?.0);
                if c != 0 { vx } else { vy }
            }
            k if k.is_cast() && k != ExprKind::CXXReinterpretCastExpr => {
                let ast = b.ast();
                let op = ast.exprs.operand(e);
                match ast.exprs.cast_kind(e) {
                    CastKind::NoOp => self.leaf_val(b, op, d)?.0,
                    // A conversion to an enumeration checks its range.
                    CastKind::IntegralCast
                        if matches!(ast.types.canonical_view(n.ty()), TypeView::Enum(_)) =>
                    {
                        return None;
                    }
                    CastKind::IntegralCast | CastKind::IntegralToBoolean => {
                        // `Compiler::convert` between `Int`/`Bool` kinds.
                        let (v, fk) = self.leaf_val(b, op, d)?;
                        if fk == tk {
                            v
                        } else if tk == TyKind::Bool {
                            u128::from(v != 0)
                        } else {
                            canon(v, w, s)
                        }
                    }
                    _ => return None,
                }
            }
            _ => return None,
        };
        Some((v, tk))
    }

    /// A builtin binary operator of integer type `(w, s)`.
    fn leaf_binary(
        &mut self,
        b: &mut dyn SemaBridge,
        e: ExprId,
        w: u32,
        s: bool,
        d: u32,
    ) -> Option<u128> {
        let op = b.ast().exprs.binary_op(e);
        if op.is_compound_assignment() || op.is_ptr_mem() {
            return None;
        }
        let [x, y] = b.ast().exprs.operands(e);
        let iop = match op {
            BinaryOp::Comma | BinaryOp::Assign | BinaryOp::Cmp => return None,
            BinaryOp::LAnd | BinaryOp::LOr => {
                let (vx, vy) = (self.leaf_val(b, x, d)?.0, self.leaf_val(b, y, d)?.0);
                let lor = op == BinaryOp::LOr;
                return Some(if (vx != 0) == lor { vx } else { vy });
            }
            BinaryOp::EQ
            | BinaryOp::NE
            | BinaryOp::LT
            | BinaryOp::GT
            | BinaryOp::LE
            | BinaryOp::GE => {
                let (vx, xk) = self.leaf_val(b, x, d)?;
                let vy = self.leaf_val(b, y, d)?.0;
                let o = if int_of(xk).1 { (vx as i128).cmp(&(vy as i128)) } else { vx.cmp(&vy) };
                let r = match op {
                    BinaryOp::EQ => o.is_eq(),
                    BinaryOp::NE => o.is_ne(),
                    BinaryOp::LT => o.is_lt(),
                    BinaryOp::GT => o.is_gt(),
                    BinaryOp::LE => o.is_le(),
                    _ => o.is_ge(),
                };
                return Some(u128::from(r));
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
        let vx = self.leaf_val(b, x, d)?.0;
        let (vy, yk) = self.leaf_val(b, y, d)?;
        let (cw, cs) = if matches!(iop, Op::IShl | Op::IShr) { int_of(yk) } else { (0, false) };
        int_bin(iop, w, s, vx, vy, cw, cs).ok()
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
