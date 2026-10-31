//! Lowering of pointers to members ([expr.unary.op]/4, [conv.mem],
//! [expr.mptr.oper]; the value model is [`crate::memptr`]): `&C::m` is an
//! interned constant, the member pointer conversions are `Op::MpCast`
//! steps along the cast's base path, and `obj.*mp` / `p->*mp` an
//! `Op::MpAddr` (a call through a pointer to member function adds
//! `Op::MpFn` for the callee).

use kiln_ast::expr::{BinaryOp, CastKind, ExprKind};
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType};

use super::class::Callee;
use super::{Access, Compiler, Place, skip_parens};
use crate::kbc::{Op, ak};

impl Compiler<'_> {
    /// `&C::m` (`x` the `DeclRefExpr`) into `dst`.
    pub(super) fn mp_const(&mut self, e: ExprId, x: ExprId, dst: u32) {
        let x = skip_parens(self.ast(), x);
        let d = self.ast().exprs.decl(x);
        match self.o.mp_intern(&mut *self.b, d, false, &[]) {
            Some(v) => self.konst(dst, v),
            None => self.unsupported(e),
        }
    }

    /// The record of member pointer type `q`'s class.
    fn mp_class(&self, q: QualType) -> Option<DeclId> {
        let ast = self.ast();
        match ast.types.canonical_view(q) {
            TypeView::MemberPointer { class, .. } => ast.types.as_record(QualType::unqual(class)),
            _ => None,
        }
    }

    /// A `BaseToDerivedMemberPointer` / `DerivedToBaseMemberPointer`
    /// cast `e` into `dst` (clang's `MemberPointerExprEvaluator`).
    pub(super) fn mp_conv(&mut self, e: ExprId, dst: u32, ck: CastKind) {
        let op = self.ast().exprs.operand(e);
        self.rv_into(op, dst);
        let ast = self.ast();
        let bases: Vec<Option<DeclId>> = ast
            .exprs
            .cast_path(e)
            .map(|l| {
                ast.lists
                    .get(l)
                    .iter()
                    .map(|&i| ast.types.as_record(ast.decls.base_spec(i).ty))
                    .collect()
            })
            .unwrap_or_default();
        if bases.is_empty() {
            return;
        }
        let steps: Vec<Option<DeclId>> = if ck == CastKind::BaseToDerivedMemberPointer {
            // The path is in derived-to-base order and names each arc's
            // base: walk it backwards, staggered by one, then end at the
            // result's class.
            let mut v: Vec<_> = bases[..bases.len() - 1].iter().rev().copied().collect();
            v.push(self.mp_class(self.node(e).ty()));
            v
        } else {
            bases
        };
        let to_base = u32::from(ck == CastKind::DerivedToBaseMemberPointer);
        for s in steps {
            let Some(c) = s else {
                self.unsupported(e);
                return;
            };
            self.emit_at(Op::MpCast, to_base, &[dst, c.index()], e);
        }
    }

    /// The object operand of `obj.*mp` / `p->*mp`: its address.
    fn mp_object(&mut self, obj: ExprId, arrow: bool, acc: Access) -> u32 {
        if arrow {
            return self.rv(obj);
        }
        if self.node(obj).value_category().is_glvalue() {
            let sub = match acc {
                Access::Write => Access::Write,
                Access::Addr => Access::Addr,
                Access::Read | Access::Rmw => Access::Read,
            };
            return self.lv_mem(obj, sub);
        }
        let oty = self.ety(obj);
        self.materialize(obj, oty)
    }

    /// `obj.*mp` / `p->*mp` (`e`) as a glvalue.
    pub(super) fn mp_access(&mut self, e: ExprId, acc: Access) -> Place {
        let ast = self.ast();
        let arrow = ast.exprs.binary_op(e) == BinaryOp::PtrMemI;
        let [a, b] = ast.exprs.operands(e);
        let obj = self.mp_object(a, arrow, acc);
        let mp = self.rv(b);
        let r = self.reg();
        // clang reports a bad member pointer at the operand.
        let loc = self.node(b).loc();
        self.emit_site(Op::MpAddr, 0, &[r, obj, mp], b, loc);
        // A union member is checked where it is used.
        let check = match (acc, self.rmw_loc) {
            (Access::Read, _) => Some((ak::READ, self.begin(e))),
            (Access::Write | Access::Rmw, Some(l)) => Some((ak::ASSIGN, l)),
            (Access::Write | Access::Rmw, None) => Some((ak::ASSIGN, self.node(e).loc())),
            (Access::Addr, _) => None,
        };
        if let Some((k, at)) = check {
            self.emit_site(Op::MpActive, k, &[r, mp], e, at);
        }
        Place::Mem(r)
    }

    /// A call whose callee is `(obj.*mp)` / `(p->*mp)` (`bin`).
    pub(super) fn mp_call(&mut self, e: ExprId, bin: ExprId, sret: Option<u32>) -> u32 {
        let rty = self.ety(e);
        let ast = self.ast();
        let args: Vec<ExprId> = ast.lists.iter::<ExprId>(ast.exprs.call_args(e)).collect();
        let arrow = ast.exprs.binary_op(bin) == BinaryOp::PtrMemI;
        let [a, b] = ast.exprs.operands(bin);
        let mq = ast.exprs.get(b).ty();
        let refs: Vec<bool> = match ast.types.canonical_view(mq) {
            TypeView::MemberPointer { pointee, .. } => ast
                .types
                .proto(pointee)
                .map(|p| ast.types.params(p).iter().map(|&t| ast.types.is_reference(t)).collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let loc = self.begin(e);
        let obj = self.mp_object(a, arrow, Access::Addr);
        let mp = self.rv(b);
        let this = self.reg();
        let at = self.node(b).loc();
        self.emit_site(Op::MpAddr, 0, &[this, obj, mp], b, at);
        let f = self.reg();
        self.emit_site(Op::MpFn, 0, &[f, mp, this], b, at);
        self.call_frame(e, Callee::Ind(f), rty, sret, Some(this), &args, &refs, Some(loc))
    }

    /// Whether `callee` (parentheses skipped) is a `.*` / `->*`.
    pub(super) fn is_mp_callee(&self, callee: ExprId) -> Option<ExprId> {
        let ast = self.ast();
        let c = skip_parens(ast, callee);
        (ast.exprs.get(c).kind() == ExprKind::BinaryOperator && ast.exprs.binary_op(c).is_ptr_mem())
            .then_some(c)
    }
}
