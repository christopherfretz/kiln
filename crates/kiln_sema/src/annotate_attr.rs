//! clang's `annotate` and `unavailable` attribute arguments (WP-954;
//! spec/sema.md §4.5).
//!
//! `annotate("str", args...)` (clang `handleAnnotateAttr`): at least one
//! argument, the first a string literal; the others are constant
//! expressions, evaluated by the Oven once none is value-dependent (clang
//! `ConstantFoldAttrArgs`), else at instantiation. `unavailable("msg")`
//! (clang `handleAttrWithMessage`): at most one argument, a string
//! literal.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::expr::BinaryOp;
use kiln_ast::ids::RawId;
use kiln_ast::{AttrId, ExprId, ListId, QualType};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use crate::Sema;
use crate::decl::AttrArg;
use crate::templ::pack::PackOperand;

impl Sema<'_, '_> {
    /// `unavailable` / `unavailable("message")`: `a` is the message.
    pub(crate) fn unavailable_attribute(
        &mut self,
        syntax: AttrSyntax,
        shown: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        if args.len() > 1 {
            let msg = self
                .diag(DiagId::sema_attribute_too_many_arguments_one, loc)
                .arg(format!("'{shown}'"));
            self.emit(msg);
            return None;
        }
        let mut a = 0;
        if let Some(arg) = args.first() {
            let e = match arg {
                AttrArg::Expr(e) if self.string_lit_of(*e).is_some() => *e,
                _ => {
                    let at = match arg {
                        AttrArg::Expr(e) => self.ast.exprs.get(*e).loc(),
                        _ => loc,
                    };
                    // clang names it without its scope here.
                    let msg =
                        self.diag(DiagId::parse_attribute_requires_string, at).arg("unavailable");
                    self.emit(msg);
                    return None;
                }
            };
            a = e.raw();
        }
        let node = AttrNode { kind: AttrKind::UNAVAILABLE, syntax, flags: 0, loc, a, b: 0 };
        Some(self.ast.attrs.push(node))
    }

    /// `annotate("str", args...)`: `a` is the string, `b` the list of the
    /// other arguments (0: none).
    pub(crate) fn annotate_attribute(
        &mut self,
        syntax: AttrSyntax,
        shown: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let Some(first) = args.first() else {
            let msg = self
                .diag(DiagId::sema_attribute_too_few_arguments_one, loc)
                .arg(format!("'{shown}'"));
            self.emit(msg);
            return None;
        };
        let s = match first {
            AttrArg::Expr(e) if self.string_lit_of(*e).is_some() => *e,
            _ => {
                let at = match first {
                    AttrArg::Expr(e) => self.ast.exprs.get(*e).loc(),
                    _ => loc,
                };
                let msg = self.diag(DiagId::parse_attribute_requires_string, at).arg("annotate");
                self.emit(msg);
                return None;
            }
        };
        let mut rest = Vec::with_capacity(args.len() - 1);
        for arg in &args[1..] {
            let AttrArg::Expr(e) = *arg else {
                self.error(DiagId::parse_expected_expression, loc);
                return None;
            };
            // [temp.variadic]/12: a pack is named only in an expansion
            // (`args...`, clang `AcceptsExprPack`).
            let at = self.begin_loc(e);
            if self.diagnose_unexpanded_pack(PackOperand::Expr(e), "expression", at) {
                return None;
            }
            rest.push(e);
        }
        let rest = self.check_annotate_args(shown, &rest)?;
        let b = if rest.is_empty() { 0 } else { self.ast.lists.push(&rest).raw() };
        let node = AttrNode { kind: AttrKind::ANNOTATE, syntax, flags: 0, loc, a: s.raw(), b };
        Some(self.ast.attrs.push(node))
    }

    /// clang `ConstantFoldAttrArgs`: unless one of `args` is
    /// value-dependent (checked at instantiation), each is converted as
    /// a function argument would be (array/function decay,
    /// lvalue-to-rvalue) and is a constant expression, else `{shown}
    /// attribute requires parameter N to be a constant expression` with
    /// the evaluation's notes, and None (the attribute is dropped). Each
    /// converted argument is wrapped in a `ConstantExpr` holding its
    /// value, which codegen lowers into the `.args` struct (WP-1113).
    pub(crate) fn check_annotate_args(
        &mut self,
        shown: &str,
        args: &[ExprId],
    ) -> Option<Vec<ExprId>> {
        if args.iter().any(|&e| self.expr_is_value_dependent(e) || self.has_errors_in(e)) {
            let bad = args.iter().any(|&e| self.has_errors_in(e));
            return if bad { None } else { Some(args.to_vec()) };
        }
        let mut out = Vec::with_capacity(args.len());
        for (i, &e) in args.iter().enumerate() {
            let e = crate::conv::decay(self, e);
            if self.is_err_expr(e) {
                return None;
            }
            let at = self.begin_loc(e);
            // The Oven freezes a value: a `void` argument is evaluated as
            // `e, true` (as for a consteval block).
            let probe = if self.ast.types.is_void(self.ast.exprs.get(e).ty()) {
                let t = self.builder().bool_literal(QualType::BOOL, true, at);
                self.builder().binary(BinaryOp::Comma, e, t, QualType::BOOL, at)
            } else {
                e
            };
            let v = match self.eval_const(probe, EvalContext::new(EvalKind::Other, at)) {
                Ok(v) => v,
                Err(f) => {
                    let msg = self
                        .diag(DiagId::sema_attribute_param_not_constant_expr, at)
                        .arg(format!("'{shown}'"))
                        .arg((i + 1).to_string());
                    let msg = Self::with_failure_notes(msg, f);
                    self.emit(msg);
                    return None;
                }
            };
            // A `void` argument has no value to emit (clang drops it from
            // the `.args` struct too); keep it unwrapped.
            let e = if probe == e { self.builder().constant_expr(e, Some(v)) } else { e };
            out.push(e);
        }
        Some(out)
    }

    /// The other arguments of `annotate` attribute `n` (empty: none).
    pub(crate) fn annotate_args(&self, n: &AttrNode) -> Vec<ExprId> {
        match ListId::from_raw(n.b) {
            Some(l) if n.b != 0 && n.kind == AttrKind::ANNOTATE => {
                self.ast.lists.iter::<ExprId>(l).collect()
            }
            _ => Vec::new(),
        }
    }
}
