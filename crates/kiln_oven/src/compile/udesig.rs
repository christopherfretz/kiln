//! The union member designators of an address (WP-1494, WP-1508;
//! spec/oven.md "Union member designators", runtime `vm/udesig.rs`).
//!
//! The address of a glvalue formed by a member access chain (`u.b`,
//! `u.s.k`, `u.arr[i]`, `(&u.s)->k`, `p->u.b`, a derived-to-base
//! conversion of such a glvalue) designates one union member at each
//! union level of the chain. Where another member of that union holds the
//! glvalue's object or one of its scalars at the same place (so it, or a
//! subobject the pointer reaches later, could be that member's), the
//! address alone does not tell the members apart, and the compiler tags the
//! pointer with the (union type, member index) of that level
//! (`VoidCast` aux `kbc::UNION_DESIG | idx << 4`, `[dst, p, union type]`;
//! the VM extends the register's path). Levels that cannot be confused
//! emit nothing, so code without such unions compiles as before.

use kiln_ast::ExprId;
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{CastKind, ExprFlags, ExprKind, UnaryOp};
use kiln_ast::types::TypeView;

use super::{Compiler, is_reference_decl, skip_parens};
use crate::kbc::Op;
use crate::ty::{OTy, TyKind};

/// The most scalar subobjects of the object looked at, and the most
/// (offset, scalar) pairs tested per union level.
const MAX_LEAVES: usize = 256;
const MAX_PAIRS: usize = 1024;
/// The most member accesses of a chain looked at.
const MAX_CHAIN: usize = 64;
/// The most element positions an array step expands to; past it a union
/// level is tagged without looking (a tag is never wrong, only slower).
const MAX_OFFSETS: usize = 64;

/// One step of a member access chain, outermost access first.
#[derive(Clone, Copy)]
enum Step {
    /// Member `idx` of record `rec`.
    Field { rec: OTy, idx: usize },
    /// An element of an array of `n` elements of size `es`.
    Elem { es: u64, n: u64 },
    /// A base class subobject at `off`.
    Base { off: u64 },
}

impl Compiler<'_> {
    /// Tags address register `q` of glvalue `e` (a member access, an
    /// element access or a derived-to-base conversion whose address is
    /// taken) with the union members its chain designates where another
    /// member holds the object or one of its scalars; returns the register holding
    /// the result.
    pub(super) fn tag_union_chain(&mut self, e: ExprId, q: u32) -> u32 {
        let steps = self.chain_steps(e);
        if !steps.iter().any(|s| {
            matches!(s, Step::Field { rec, .. }
                if matches!(self.o.types.kind(*rec), TyKind::Record { union: true, .. }))
        }) {
            return q;
        }
        let fty = self.ety(e);
        if self.o.types.holds_no_data(fty) {
            return q;
        }
        let mut leaves = Vec::new();
        let mut wide = !self.scalar_leaves(fty, 0, &mut leaves);
        let mut q = q;
        // The offsets of `e`'s object in the object of the current step.
        let mut offs: Vec<u64> = vec![0];
        for s in steps {
            match s {
                Step::Field { rec, idx } => {
                    let fs = self.o.types.fields(rec);
                    let Some(f) = fs.get(idx).copied() else { return q };
                    for o in &mut offs {
                        *o += f.offset;
                    }
                    if !matches!(self.o.types.kind(rec), TyKind::Record { union: true, .. }) {
                        continue;
                    }
                    // Another member holding a scalar of the object (or
                    // the object itself) at its place: a subobject the
                    // pointer reaches later (`p->k` of `&u.s`) may be one
                    // that member holds too.
                    let t = &self.o.types;
                    let wide = wide || offs.len() * leaves.len() > MAX_PAIRS;
                    let shared = fs.iter().enumerate().any(|(j, g)| {
                        j != idx
                            && g.width.is_none()
                            && !t.holds_no_data(g.ty)
                            && (wide
                                || offs.iter().any(|&r| {
                                    (r >= g.offset && t.has_subobject(g.ty, r - g.offset, fty))
                                        || leaves.iter().any(|&(d, lt)| {
                                            r + d >= g.offset
                                                && t.has_subobject(g.ty, r + d - g.offset, lt)
                                        })
                                }))
                    });
                    if shared {
                        let r = self.reg();
                        let aux = crate::kbc::UNION_DESIG | (idx as u32) << 4;
                        self.emit_at(Op::VoidCast, aux, &[r, q, rec.0], e);
                        q = r;
                    }
                }
                Step::Elem { es, n } => {
                    if wide || offs.len().saturating_mul(n as usize) > MAX_OFFSETS {
                        wide = true;
                        continue;
                    }
                    offs = offs.iter().flat_map(|&o| (0..n).map(move |k| o + k * es)).collect();
                }
                Step::Base { off } => {
                    for o in &mut offs {
                        *o += off;
                    }
                }
            }
        }
        q
    }

    /// Pushes the scalar subobjects (offset, type) of an object of type
    /// `t` at `base`, bit-fields and empty classes aside; false past
    /// [`MAX_LEAVES`].
    fn scalar_leaves(&self, t: OTy, base: u64, out: &mut Vec<(u64, OTy)>) -> bool {
        let types = &self.o.types;
        match types.kind(t) {
            TyKind::Array { elem, n } => {
                let es = types.size(elem);
                (0..n).all(|i| {
                    out.len() <= MAX_LEAVES && self.scalar_leaves(elem, base + i * es, out)
                })
            }
            TyKind::Record { .. } => types
                .fields(t)
                .iter()
                .filter(|f| f.width.is_none())
                .all(|f| self.scalar_leaves(f.ty, base + f.offset, out)),
            _ => {
                out.push((base, t));
                out.len() <= MAX_LEAVES
            }
        }
    }

    /// The steps of the member access chain of glvalue `e`, outermost
    /// first, up to its first operand that is not a member, element or
    /// base of a glvalue (a variable, a call, a pointer dereference other
    /// than `(&x)->m`).
    fn chain_steps(&mut self, e: ExprId) -> Vec<Step> {
        let mut steps = Vec::new();
        let mut x = e;
        while steps.len() < MAX_CHAIN {
            let ast = self.ast();
            x = skip_parens(ast, x);
            let n = *ast.exprs.get(x);
            match n.kind() {
                ExprKind::MemberExpr => {
                    let fd = ast.exprs.decl(x);
                    if ast.decls.get(fd).kind() != DeclKind::Field || is_reference_decl(ast, fd) {
                        break;
                    }
                    let base = ExprId::new(n.a());
                    let arrow = n.has(ExprFlags::ARROW);
                    let bq = ast.exprs.get(base).ty();
                    let bt = self.ty(bq);
                    let rec = match (arrow, self.o.types.kind(bt)) {
                        (true, TyKind::Ptr(p)) => p,
                        (true, _) => break,
                        (false, _) => bt,
                    };
                    let Some(idx) = self.o.types.field_index(rec, fd) else { break };
                    steps.push(Step::Field { rec, idx });
                    if !arrow {
                        x = base;
                        continue;
                    }
                    // `(&x)->m` continues in `x`.
                    let ast = self.ast();
                    let mut b = skip_parens(ast, base);
                    while ast.exprs.get(b).kind() == ExprKind::ImplicitCastExpr
                        && ast.exprs.cast_kind(b) == CastKind::NoOp
                    {
                        b = skip_parens(ast, ast.exprs.operand(b));
                    }
                    if ast.exprs.get(b).kind() != ExprKind::UnaryOperator
                        || ast.exprs.unary_op(b) != UnaryOp::AddrOf
                    {
                        break;
                    }
                    x = ast.exprs.operand(b);
                }
                ExprKind::ArraySubscriptExpr => {
                    let [a, b] = ast.exprs.operands(x);
                    let a_ptr = matches!(
                        ast.types.canonical_view(ast.exprs.get(a).ty()),
                        TypeView::Pointer(_)
                    );
                    let pe = skip_parens(ast, if a_ptr { a } else { b });
                    if ast.exprs.get(pe).kind() != ExprKind::ImplicitCastExpr
                        || ast.exprs.cast_kind(pe) != CastKind::ArrayToPointerDecay
                    {
                        break;
                    }
                    let inner = ast.exprs.operand(pe);
                    let at = self.ety(inner);
                    let TyKind::Array { elem, n } = self.o.types.kind(at) else { break };
                    steps.push(Step::Elem { es: self.o.types.size(elem), n });
                    x = inner;
                }
                ExprKind::ImplicitCastExpr | ExprKind::CXXStaticCastExpr
                    if n.value_category().is_glvalue() =>
                {
                    match ast.exprs.cast_kind(x) {
                        CastKind::NoOp => x = ast.exprs.operand(x),
                        CastKind::DerivedToBase | CastKind::UncheckedDerivedToBase => {
                            let op = ast.exprs.operand(x);
                            let d = self.ety(op);
                            let (off, _, rest) = self.path_offset(x, d);
                            if !rest.is_empty() {
                                break; // a virtual base: not at a fixed offset
                            }
                            steps.push(Step::Base { off });
                            x = op;
                        }
                        _ => break,
                    }
                }
                _ => break,
            }
        }
        steps
    }
}
