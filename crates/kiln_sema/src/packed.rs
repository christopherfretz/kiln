//! `-Waddress-of-packed-member` (clang `RefersToMemberWithReducedAlignment`,
//! `DiscardMisalignedMemberAddress` and `DiagnoseMisalignedMembers`,
//! `SemaChecking.cpp`; spec/sema.md §5.14).
//!
//! `&m` of a member whose alignment a `packed` attribute reduced below its
//! type's records a candidate. A conversion of that address to an integer,
//! to a pointer to an incomplete type or to a pointer whose pointee needs
//! no more alignment than the member has discards it; the rest are
//! reported at the end of the full-expression.

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{ExprFlags, ExprKind, UnaryOp};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;

use crate::Sema;

/// A member whose address may be misaligned: the `MemberExpr`, the class
/// that reduced its alignment, the member that did, and that alignment
/// in bytes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Misaligned {
    member: ExprId,
    class: DeclId,
    field: DeclId,
    align: u64,
}

impl Sema<'_, '_> {
    fn is_packed(&self, d: DeclId) -> bool {
        self.find_attr(d, AttrKind::PACKED).is_some()
    }

    fn align_of(&self, ty: QualType) -> Option<u64> {
        crate::target::size_align(&self.ast, ty, &self.abi).map(|(_, a)| a)
    }

    /// The operand of `&` (clang `CheckAddressOfPackedMember`): records
    /// `e` if it names a member whose alignment a `packed` attribute
    /// reduced.
    pub(crate) fn check_address_of_packed_member(&mut self, e: ExprId) {
        let e = self.ignore_parens(e);
        if self.ast.exprs.get(e).kind() != ExprKind::MemberExpr {
            return;
        }
        if let Some(m) = self.misaligned_member(e) {
            self.misaligned.push(m);
        }
    }

    fn misaligned_member(&mut self, e: ExprId) -> Option<Misaligned> {
        // The fields of the chain `a.b.c`, innermost (`c`) first.
        let mut chain = Vec::new();
        let mut any_packed = false;
        let mut me = e;
        let (top, top_arrow) = loop {
            let n = *self.ast.exprs.get(me);
            let f = self.ast.exprs.decl(me);
            if self.kind_of(f) != DeclKind::Field || self.ast.decls.get(f).has(DeclFlags::INVALID) {
                return None;
            }
            let base = self.ast.exprs.operand(me);
            if self.ast.exprs.get(base).is_type_dependent() {
                return None;
            }
            let parent = self.ast.decls.get(f).ctx()?;
            any_packed |= self.is_packed(parent) || self.is_packed(f);
            chain.push(f);
            let arrow = n.has(ExprFlags::ARROW);
            let b = self.ignore_parens(base);
            if chain.len() > 64 || self.ast.exprs.get(b).kind() != ExprKind::MemberExpr {
                break (base, arrow);
            }
            me = b;
        };
        if !any_packed {
            return None;
        }
        // Only a named object or `this` is in scope of the diagnostic.
        let top = self.ignore_paren_imp_casts(top);
        let top_kind = self.ast.exprs.get(top).kind();
        if !matches!(top_kind, ExprKind::DeclRefExpr | ExprKind::CXXThisExpr) {
            return None;
        }
        let expected = self.align_of(self.ast.exprs.get(e).ty())?;
        if expected <= 1 {
            return None;
        }
        let outer = self.ast.decls.get(*chain.last()?).ctx()?;
        let outer = self.ast.definition(outer).unwrap_or(outer);
        let mut complete =
            u64::from(kiln_abi::ensure_layout(&mut self.ast, outer, &self.abi)?.align);
        // The object named may be more aligned than its class.
        if top_kind == ExprKind::DeclRefExpr && !top_arrow {
            let v = self.ast.exprs.decl(top);
            let vt = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
            if !self.ast.types.is_reference(vt)
                && let Some(a) = self.align_of(vt)
            {
                let attr = kiln_abi::layout::decl_max_align(&self.ast, v, &self.abi) / 8;
                complete = complete.max(a).max(attr);
            }
        }
        let offset: u64 = chain.iter().map(|&f| self.field_bit_offset(f) / 8).sum();
        if offset.is_multiple_of(expected) && complete >= expected {
            return None;
        }
        // The culprit: the innermost member that is packed or is in a
        // packed class.
        for &f in &chain {
            let parent = self.ast.decls.get(f).ctx()?;
            if self.is_packed(f) || self.is_packed(parent) {
                let def = self.ast.definition(parent).unwrap_or(parent);
                let ft = self.ast.decls.get(f).ty()?;
                let fa = self.align_of(ft)?;
                let pa = u64::from(kiln_abi::ensure_layout(&mut self.ast, def, &self.abi)?.align);
                return Some(Misaligned { member: e, class: def, field: f, align: fa.min(pa) });
            }
        }
        None
    }

    /// The end of a full-expression: drops the candidates whose address
    /// is converted to a type that tolerates the reduced alignment (clang
    /// `DiscardMisalignedMemberAddress`, called on casts and conversions)
    /// and reports the rest.
    pub(crate) fn diagnose_misaligned_members(&mut self, full: ExprId) {
        let mut stack = vec![full];
        let mut seen = 0usize;
        while let Some(x) = stack.pop() {
            seen += 1;
            if seen > 1_000_000 || self.misaligned.is_empty() {
                break;
            }
            let n = *self.ast.exprs.get(x);
            if n.kind().is_cast() {
                self.discard_misaligned_member_address(n.ty(), self.ast.exprs.operand(x));
            }
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        let pending = std::mem::take(&mut self.misaligned);
        for m in pending {
            let at = self.begin_loc(m.member);
            let msg = self
                .diag(DiagId::sema_address_of_packed_member, at)
                .arg(self.decl_arg(m.field))
                .arg(self.decl_arg(m.class));
            self.emit(msg);
        }
    }

    pub(crate) fn discard_misaligned_member_address(&mut self, t: QualType, e: ExprId) {
        let t = self.ast.types.canonical(t);
        let pointee = self.ast.types.pointee(t).filter(|_| self.ast.types.is_pointer(t));
        if pointee.is_none() && !self.ast.types.is_integral(t) && !self.ast.types.is_dependent(t) {
            return;
        }
        let e = self.ignore_parens(e);
        if self.ast.exprs.get(e).kind() != ExprKind::UnaryOperator
            || self.ast.exprs.unary_op(e) != UnaryOp::AddrOf
        {
            return;
        }
        let op = self.ignore_parens(self.ast.exprs.operand(e));
        let Some(i) = self.misaligned.iter().position(|m| m.member == op) else { return };
        let keep = match pointee {
            Some(p) => self.align_of(p).is_some_and(|a| a > self.misaligned[i].align),
            None => false,
        };
        if !keep {
            self.misaligned.remove(i);
        }
    }
}
