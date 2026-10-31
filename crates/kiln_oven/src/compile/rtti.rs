//! Lowering of `dynamic_cast` ([expr.dynamic.cast]) and `typeid`
//! ([expr.typeid]) to `Op::DynCast` / `Op::TypeId` (WP-077; spec/oven.md
//! "Dynamic types"). Both read the dynamic-type word of a polymorphic
//! operand; the runtime checks are clang 19's `HandleDynamicCast` and
//! `VisitCXXTypeidExpr`.

use kiln_ast::expr::TypeidOperand;
use kiln_ast::{ExprId, QualType};

use super::{Access, Compiler};
use crate::kbc::Op;

impl Compiler<'_> {
    /// `dynamic_cast` `e` (cast kind `Dynamic`) into `dst`: of a pointer
    /// (`glvalue` false) or of a glvalue, whose address `dst` receives.
    pub(super) fn dynamic_cast(&mut self, e: ExprId, dst: u32, glvalue: bool) {
        let ast = self.ast();
        let op = ast.exprs.operand(e);
        let (oq, rq) = (ast.exprs.get(op).ty(), ast.exprs.get(e).ty());
        let (sq, tq) = if glvalue {
            (oq, rq)
        } else {
            (ast.types.pointee(oq).unwrap_or(oq), ast.types.pointee(rq).unwrap_or(rq))
        };
        let target = if ast.types.is_void(tq) { u32::MAX } else { self.ty(tq).0 };
        let stat = self.ty(sq);
        let p = if glvalue { self.lv_mem(op, Access::Addr) } else { self.rv(op) };
        self.emit_at(Op::DynCast, u32::from(glvalue), &[dst, p, stat.0, target], e);
    }

    /// `typeid` expression `e`: the address of its `std::type_info`
    /// object.
    pub(super) fn typeid(&mut self, e: ExprId) -> u32 {
        let ast = self.ast();
        let (operand, dynamic) =
            (ast.exprs.typeid_operand(e), ast.exprs.typeid_potentially_evaluated(e));
        let ti = self.ety(e);
        let dst = self.reg();
        match operand {
            TypeidOperand::Expr(x) if dynamic => {
                let cls = self.ety(x);
                let p = self.lv_mem(x, Access::Addr);
                self.emit_at(Op::TypeId, 1, &[dst, p, ti.0, cls.0], e);
            }
            op => {
                let q = match op {
                    TypeidOperand::Type(t) => t,
                    TypeidOperand::Expr(x) => self.ast().exprs.get(x).ty(),
                };
                let key = self.typeid_key(q);
                self.emit_at(Op::TypeId, 0, &[dst, key.to_raw(), ti.0, 0], e);
            }
        }
        dst
    }

    /// The type `typeid` names for `q`: canonical, without a reference
    /// or top-level cv-qualifiers ([expr.typeid]/5).
    fn typeid_key(&self, q: QualType) -> QualType {
        let types = &self.ast().types;
        let q = if types.is_reference(q) { types.pointee(q).unwrap_or(q) } else { q };
        types.canonical(q).without_quals()
    }
}
