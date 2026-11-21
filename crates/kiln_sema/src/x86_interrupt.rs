//! x86-64 GNU `interrupt` (spec/sema.md §4.5; clang
//! `SemaX86::handleAnyInterruptAttr`, `AnyX86InterruptAttr`; WP-1745).
//!
//! On a function with a prototype that is not an instance member
//! function or a static overloaded operator (else clang's "only applies
//! to non-K&R-style functions" warning drops it), the signature must be
//! `void (T *)` or `void (T *, unsigned long)`: a `void` return, one or
//! two parameters, a pointer first and a 64-bit unsigned integer second.
//! A valid attribute also makes the function `used` (an implicit
//! attribute, as clang adds one). A direct call of an interrupt handler
//! is an error, and a call from one (or from a
//! `no_caller_saved_registers` function) of a function without
//! `no_caller_saved_registers` warns (`-Wexcessive-regsave`), as the
//! target keeps x87/MMX/SSE state kiln would have to save. Codegen
//! gives the definition `x86_intrcc` and a `byval` first parameter.

use kiln_ast::attr::{AttrKind, AttrNode};
use kiln_ast::expr::{ExprKind, UnaryOp};
use kiln_ast::names::OverloadedOperator;
use kiln_ast::{AttrId, DeclId, ExprId};
use kiln_diag::DiagId;

use crate::Sema;
use crate::declspec::{DeclSpec, Declarator};

impl Sema<'_, '_> {
    /// clang `handleAnyInterruptAttr`'s signature checks for function
    /// `f` declared by `ds` / `d`: each `interrupt` attribute of a
    /// function whose signature fails is diagnosed and marked (`a` = 1)
    /// for [`Sema::check_x86_interrupt_subject`] to drop.
    pub(crate) fn check_x86_interrupt_signature(
        &mut self,
        f: DeclId,
        ds: &DeclSpec,
        d: &Declarator,
    ) {
        let ids: Vec<AttrId> = ds
            .attrs
            .iter()
            .chain(&d.attrs)
            .copied()
            .filter(|&a| self.ast.attrs.get(a).kind == AttrKind::X86_INTERRUPT)
            .collect();
        if ids.is_empty() || !self.x86_interrupt_subject_ok(f) {
            return;
        }
        let Some(ty) = self.ast.decls.get(f).ty() else { return };
        let Some(proto) = self.ast.types.proto(ty) else { return };
        let ret = proto.ret();
        let params = self.ast.types.params(proto).to_vec();
        let infos = d.function_params().map(<[_]>::to_vec).unwrap_or_default();
        let begin = |i: usize| infos.get(i).map(|p| p.range.0);
        let at = if !self.ast.types.is_void(ret) {
            Some((0, ds.type_loc.unwrap_or(ds.loc)))
        } else if params.is_empty() || params.len() > 2 {
            Some((1, ds.loc))
        } else if !self.ast.types.is_pointer(params[0]) {
            Some((2, begin(0).unwrap_or(ds.loc)))
        } else if params.len() == 2
            && !(self.is_unsigned_integer_type(params[1])
                && crate::conv::rank::int_width(self, params[1]) == Some(64))
        {
            Some((3, begin(1).unwrap_or(ds.loc)))
        } else {
            None
        };
        let Some((which, loc)) = at else { return };
        for a in ids {
            let msg = self
                .diag(DiagId::sema_anyx86_interrupt_attribute, loc)
                .arg(which)
                .arg("'unsigned long'");
            self.emit(msg);
            self.ast.attrs.set_args(a, 1, 0);
        }
    }

    /// The subject clang accepts: a function that is not an instance
    /// member function or a static overloaded operator (`new`, `delete`,
    /// `()`, `[]`).
    fn x86_interrupt_subject_ok(&self, f: DeclId) -> bool {
        if !self.kind_of(f).is_function() || self.is_instance_member(f) {
            return false;
        }
        !matches!(
            self.ast.decls.get(f).name().as_operator(),
            Some(
                OverloadedOperator::New
                    | OverloadedOperator::ArrayNew
                    | OverloadedOperator::Delete
                    | OverloadedOperator::ArrayDelete
                    | OverloadedOperator::Call
                    | OverloadedOperator::Subscript
            )
        )
    }

    /// The subject check of `interrupt` attribute `node` on `d` (clang's
    /// `warn_attribute_wrong_decl_type` with `ExpectedFunctionWithProtoType`):
    /// false drops it, as does a failed signature check.
    pub(crate) fn check_x86_interrupt_subject(&mut self, d: DeclId, node: AttrNode) -> bool {
        if !self.x86_interrupt_subject_ok(d) {
            let shown = self.attr_shown(&node, "interrupt");
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
                .arg(format!("'{shown}'"))
                .arg("non-K&R-style functions");
            self.emit(msg);
            return false;
        }
        node.a == 0
    }

    /// clang's implicit `used` of a valid `interrupt` attribute: added
    /// to `attrs` (the checked attributes of one declaration) when one
    /// is there.
    pub(crate) fn add_x86_interrupt_used(&mut self, attrs: &mut Vec<AttrId>) {
        let Some(node) = attrs
            .iter()
            .map(|&a| *self.ast.attrs.get(a))
            .find(|n| n.kind == AttrKind::X86_INTERRUPT)
        else {
            return;
        };
        if attrs.iter().any(|&a| self.ast.attrs.get(a).kind == AttrKind::USED) {
            return;
        }
        let used = AttrNode { kind: AttrKind::USED, flags: AttrNode::IMPLICIT, a: 0, b: 0, ..node };
        attrs.push(self.ast.attrs.push(used));
    }

    /// Whether function `f` (or a previous declaration) is an x86
    /// interrupt handler.
    pub(crate) fn is_x86_interrupt_fn(&self, f: DeclId) -> bool {
        self.ast.attrs.has_kind(AttrKind::X86_INTERRUPT)
            && self.ast.inherited_attr(f, AttrKind::X86_INTERRUPT).is_some()
    }

    /// clang `BuildResolvedCallExpr`, for a call of `callee`, which names
    /// function `fdecl` ([`Sema::x86_call_fdecl`]): naming an interrupt handler is an error at the
    /// callee (false: the call is invalid); a call from an interrupt
    /// handler or a `no_caller_saved_registers` function of a callee
    /// without `no_caller_saved_registers` warns (with a note at a named
    /// callee).
    pub(crate) fn check_x86_interrupt_call(&mut self, callee: ExprId) -> bool {
        let fdecl = self.x86_call_fdecl(callee);
        if let Some(f) = fdecl
            && self.is_x86_interrupt_fn(f)
        {
            let msg = self.diag(DiagId::sema_anyx86_interrupt_called, self.expr_loc(callee));
            self.emit(msg);
            return false;
        }
        // clang's `Caller`: the current context, when a function.
        let from = self.ctx;
        if !self.kind_of(from).is_function() {
            return true;
        }
        let from_isr = self.is_x86_interrupt_fn(from);
        if !from_isr && !self.fn_has_no_csr(from) {
            return true;
        }
        // clang `hasFeature("sse") || ("x87") || ("mmx")`: the x86-64
        // base has all three unless a `-target-feature` takes them away.
        let off = |n: &str| {
            self.target_features.iter().rev().find_map(|e| match e.split_at_checked(1) {
                Some((sign, f)) if f == n => Some(sign == "-"),
                _ => None,
            }) == Some(true)
        };
        if off("sse") && off("x87") && off("mmx") {
            return true;
        }
        if fdecl.is_some_and(|f| self.fn_has_no_csr(f)) {
            return true;
        }
        let msg = self
            .diag(DiagId::sema_anyx86_excessive_regsave, self.expr_loc(callee))
            .arg(u32::from(!from_isr));
        let msg = match fdecl {
            Some(f) => {
                let name = format!("'{}'", self.decl_name(f));
                let note = self
                    .diag(DiagId::note_entity_declared_here, self.ast.decls.get(f).loc())
                    .arg(name);
                msg.with_note(note)
            }
            None => msg,
        };
        self.emit(msg);
        true
    }

    /// clang `BuildCallExpr`'s `NDecl` as a function: the function a
    /// callee names once parentheses and a `&` are removed, through a
    /// `DeclRefExpr` or a `MemberExpr`.
    fn x86_call_fdecl(&self, callee: ExprId) -> Option<DeclId> {
        let mut x = self.ignore_parens(callee);
        if self.ast.exprs.get(x).kind() == ExprKind::UnaryOperator
            && self.ast.exprs.unary_op(x) == UnaryOp::AddrOf
        {
            x = self.ignore_parens(self.ast.exprs.operand(x));
        }
        match self.ast.exprs.get(x).kind() {
            ExprKind::DeclRefExpr | ExprKind::MemberExpr => Some(self.ast.exprs.decl(x)),
            _ => None,
        }
        .filter(|&d| self.ast.decls.function(d).is_some())
    }

    /// Whether function `f`'s type has `no_caller_saved_registers`.
    fn fn_has_no_csr(&self, f: DeclId) -> bool {
        self.ast
            .decls
            .get(f)
            .ty()
            .and_then(|t| self.ast.types.proto(t))
            .is_some_and(|p| p.info().no_caller_saved_regs())
    }
}
