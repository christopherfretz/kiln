//! Comparisons with a null pointer constant that always have the same
//! result (clang `Sema::DiagnoseAlwaysNonNullPointer`, comparison forms;
//! spec/sema.md §5.27).
//!
//! `==` / `!=` with exactly one null pointer constant operand
//! ([conv.ptr]/1) whose other operand is, through implicit conversions:
//! `this` ([expr.prim.this]: never null), the address of a reference or
//! of a call returning one ([dcl.ref]/5: a reference is bound to an
//! object), or the address of a named object, a named array or a named
//! function ([basic.compound]/3: each is a non-null pointer). The last
//! three are `-Wtautological-pointer-compare`, the first two
//! `-Wtautological-undefined-compare`. A weak declaration may be null;
//! an operand written in a macro body is not diagnosed. The check runs
//! wherever the comparison is built (function bodies, default member
//! initializers, namespace-scope initializers) and again when a template
//! is instantiated.

use kiln_ast::attr::{AttrKind, AttrNode};
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{CastKind, ExprKind, UnaryOp, ValueCategory};
use kiln_ast::types::{BuiltinType, FuncProto};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId, FixIt};
use kiln_source::{Loc, Span};

use crate::{Sema, conv};

/// Where an always-non-null pointer is used.
#[derive(Clone, Copy)]
pub(crate) enum NullUse {
    /// Compared with null pointer constant `null` by `==` (`eq`) or `!=`.
    Compare { null: ExprId, eq: bool },
    /// Converted to `bool` in conversion context `cc`.
    Bool { cc: Loc },
}

/// What the non-null operand is (clang's `%select{address of|function|array}`).
#[derive(Clone, Copy)]
enum NonNull {
    AddressOf = 0,
    Function = 1,
    Array = 2,
}

impl Sema<'_, '_> {
    /// `l op r` with `op` `==` or `!=`, before the usual conversions.
    pub(crate) fn diagnose_always_non_null_compare(&mut self, eq: bool, l: ExprId, r: ExprId) {
        // An instantiation rebuilding the comparison shares the pattern's
        // converted null operand (clang rebuilds it from the literal).
        let (l, r) = (self.unconverted_null(l), self.unconverted_null(r));
        let (ln, rn) =
            (conv::is_null_pointer_constant(self, l), conv::is_null_pointer_constant(self, r));
        if ln == rn {
            return;
        }
        let (e, null) = if rn { (l, r) } else { (r, l) };
        // A clang 19 `TypoExpr` operand makes the comparison
        // type-dependent; the correction rebuilds it under a SFINAE trap,
        // which drops warnings (`val<int> == nullptr` corrected to `::val`).
        if self.holds_typo_expr(e) {
            return;
        }
        let Some((msg, rewarn)) = self.always_non_null(e, NullUse::Compare { null, eq }) else {
            return;
        };
        if rewarn && self.in_template() {
            let n = self.null_compare_rewarn.len();
            self.null_compare_rewarn.entry(e).or_insert((n, msg.clone()));
        }
        // An instantiation warns again (clang rebuilds the comparison).
        let msg = if self.inst.stack.is_empty() { msg } else { msg.allow_duplicate() };
        self.emit(msg);
    }

    /// Issues again, at an instantiation sharing them, the warnings the
    /// template definition gave for comparisons whose non-null operand
    /// is one of `operands` ([`Sema::mark_shared_expr_referenced`]'s
    /// walk), in the order the definition gave them.
    pub(crate) fn reissue_null_compares(&mut self, operands: &[ExprId]) {
        let mut hits: Vec<(usize, Diag)> =
            operands.iter().filter_map(|x| self.null_compare_rewarn.get(x).cloned()).collect();
        hits.sort_unstable_by_key(|h| h.0);
        for (_, d) in hits {
            self.emit(d.allow_duplicate());
        }
    }

    /// `e` without its null pointer conversion.
    fn unconverted_null(&self, e: ExprId) -> ExprId {
        let n = self.ast.exprs.get(e);
        if n.kind() == ExprKind::ImplicitCastExpr
            && matches!(
                self.ast.exprs.cast_kind(e),
                CastKind::NullToPointer | CastKind::NullToMemberPointer
            )
        {
            return self.ast.exprs.operand(e);
        }
        e
    }

    pub(crate) fn ignore_imp_casts(&self, mut e: ExprId) -> ExprId {
        for _ in 0..256 {
            match self.ast.exprs.get(e).kind() {
                ExprKind::ImplicitCastExpr | ExprKind::ConstantExpr => {
                    e = self.ast.exprs.operand(e);
                }
                _ => break,
            }
        }
        e
    }

    /// clang `getExprLoc`: the member name of a member access, the start
    /// of anything else this check reports.
    fn non_null_loc(&self, e: ExprId) -> Loc {
        match self.ast.exprs.get(e).kind() {
            ExprKind::MemberExpr | ExprKind::UnaryOperator | ExprKind::CXXThisExpr => self.eloc(e),
            ExprKind::CXXMemberCallExpr => self.non_null_loc(self.ast.exprs.callee(e)),
            _ => self.begin_loc(e),
        }
    }

    /// clang `DiagnoseAlwaysNonNullPointer` for expression `e` used as
    /// `u` says: the warning, and whether an instantiation warns again
    /// (not for the lambda form, which clang skips while instantiating).
    /// For a comparison `e` is the operand as written; for a conversion
    /// to `bool` its implicit casts and parentheses are stripped. Every
    /// form underlines `e` and the context (the null pointer constant,
    /// or the token of the conversion context).
    #[allow(clippy::too_many_lines, reason = "one arm per clang form, in clang's order")]
    pub(crate) fn always_non_null(&self, e: ExprId, u: NullUse) -> Option<(Diag, bool)> {
        let (at, from) = match u {
            NullUse::Compare { null, .. } => (self.eloc(e), self.begin_loc(null)),
            NullUse::Bool { cc } => (self.expr_loc(e), cc),
        };
        if at.is_macro() && (self.in_any_macro_body(at) || self.in_any_macro_body(from)) {
            return None;
        }
        let ctx = match u {
            NullUse::Compare { null, .. } => self.expr_range(null),
            NullUse::Bool { cc } => Span::new(cc, self.token_end(cc).unwrap_or(cc)),
        };
        // clang's `%select{true|false}` on `IsEqual`.
        let result = match u {
            NullUse::Compare { eq, .. } => Some(u32::from(eq)),
            NullUse::Bool { .. } => None,
        };
        let with_result = |d: Diag| match result {
            Some(r) => d.arg(r),
            None => d,
        };
        let mut e = self.ignore_imp_casts(e);
        if self.ast.exprs.get(e).kind() == ExprKind::CXXThisExpr {
            let id = if result.is_some() {
                DiagId::sema_this_null_compare
            } else {
                DiagId::sema_this_bool_conversion
            };
            let msg = with_result(self.diag(id, self.eloc(e)));
            return Some((msg.range(self.expr_range(e)).range(ctx), true));
        }
        let p = self.ignore_parens(e);
        let address_of = self.ast.exprs.get(p).kind() == ExprKind::UnaryOperator;
        if address_of {
            if self.ast.exprs.unary_op(p) != UnaryOp::AddrOf {
                return None;
            }
            e = self.ast.exprs.operand(p);
            if let Some(msg) = self.null_compare_reference(e, result) {
                return Some((msg.range(ctx), true));
            }
        }
        // A call to a `returns_nonnull` function.
        let call = self.ignore_paren_imp_casts(e);
        if matches!(
            self.ast.exprs.get(call).kind(),
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr
        ) && let Some(a) =
            self.callee_decl(call).and_then(|f| self.find_attr(f, AttrKind::RETURNS_NONNULL))
        {
            let node = *self.ast.attrs.get(a);
            return Some((self.nonnull_warning(e, u, node, false, ctx), true));
        }
        if let Some((loc, lbracket)) = self.lambda_pointer_conversion(e, u) {
            let id = DiagId::sema_impcast_lambda_pointer_to_bool;
            let lb = Span::new(lbracket, self.token_end(lbracket).unwrap_or(lbracket));
            let msg = self.diag(id, loc).range(lb).range(ctx);
            return Some((msg, false));
        }
        let n = self.ast.exprs.get(e);
        let d = match n.kind() {
            ExprKind::DeclRefExpr | ExprKind::MemberExpr => self.ast.exprs.decl(e),
            _ => return None,
        };
        if self.ast.is_weak_decl(d) {
            return None;
        }
        if self.ast.decls.get(d).kind() == DeclKind::ParmVar
            && !self.nonnull_param_modified(d)
            && let Some((node, _)) = self.nonnull_param_attr(d)
        {
            return Some((self.nonnull_warning(e, u, node, true, ctx), true));
        }
        let t = self.ast.decls.get(d).ty()?;
        let (function, array) = (self.ast.types.is_function(t), self.ast.types.is_array(t));
        let what = match (address_of, function, array) {
            (true, false, _) => NonNull::AddressOf,
            (false, true, _) => NonNull::Function,
            (false, false, true) => NonNull::Array,
            (true, true, _) | (false, false, false) => return None,
        };
        let at = self.non_null_loc(e);
        let text = self.expr_text_as_written(e);
        let id = if result.is_some() {
            DiagId::sema_null_pointer_compare
        } else {
            DiagId::sema_impcast_pointer_to_bool
        };
        let msg = with_result(self.diag(id, at).arg(what as u32).arg(text))
            .range(self.expr_range(e))
            .range(ctx);
        if !matches!(what, NonNull::Function) {
            return Some((msg, true));
        }
        let begin = self.begin_loc(e);
        let mut msg = msg.with_note(
            self.diag(DiagId::note_function_warning_silence, at).fixit(FixIt::insert(begin, "&")),
        );
        let call = match u {
            NullUse::Compare { null, .. } => self.suggest_call(d, null),
            NullUse::Bool { .. } => self.zero_arg_call_returns_bool(d),
        };
        if call {
            let last = self.expr_end_loc(e);
            let end = self.token_end(last).unwrap_or(last);
            msg = msg.with_note(
                self.diag(DiagId::note_function_to_function_call, at)
                    .fixit(FixIt::insert(end, "()")),
            );
        }
        Some((msg, true))
    }

    /// The `nonnull` warning for `e`, a call to a `returns_nonnull`
    /// function or (`param`) a `nonnull` parameter, with the note at
    /// attribute `node`.
    fn nonnull_warning(
        &self,
        e: ExprId,
        u: NullUse,
        node: AttrNode,
        param: bool,
        ctx: Span,
    ) -> Diag {
        let text = self.expr_text_as_written(e);
        let at = self.non_null_loc(e);
        let msg = match u {
            NullUse::Compare { eq, .. } => self
                .diag(DiagId::sema_nonnull_expr_compare, at)
                .arg(u32::from(param))
                .arg(text)
                .arg(u32::from(eq)),
            NullUse::Bool { .. } => {
                self.diag(DiagId::sema_cast_nonnull_to_bool, at).arg(u32::from(param)).arg(text)
            }
        };
        msg.range(self.expr_range(e))
            .range(ctx)
            .with_note(self.diag(DiagId::note_declared_nonnull, node.loc).arg(u32::from(param)))
    }

    /// A lambda converted to a function pointer (clang: outside an
    /// instantiation, a `CXXMemberCallExpr` on a closure type): the
    /// warning location and the lambda's `[`. A comparison sees the
    /// closure-typed operand before the conversion.
    fn lambda_pointer_conversion(&self, e: ExprId, u: NullUse) -> Option<(Loc, Loc)> {
        if !self.inst.stack.is_empty() {
            return None;
        }
        let (at, record) = match self.ast.exprs.get(e).kind() {
            ExprKind::CXXMemberCallExpr => {
                let f = self.callee_decl(e)?;
                let object = self.ast.exprs.children(self.ast.exprs.callee(e), &self.ast.lists);
                let at = object.first().map_or_else(|| self.begin_loc(e), |&o| self.begin_loc(o));
                (at, self.ast.decls.get(f).ctx()?)
            }
            _ if matches!(u, NullUse::Compare { .. }) => {
                let t = self.ast.types.canonical(self.ety(e));
                (self.begin_loc(e), self.ast.types.as_record(t)?)
            }
            _ => return None,
        };
        let lambda = self.ast.closure_lambda(record)?;
        Some((at, self.ast.exprs.lambdas.get(lambda).lbracket))
    }

    /// clang `CheckForReference`: `&r` of a reference variable or member
    /// `r`, or of a call returning a reference (with a note at the
    /// function). `result` is the comparison's (`None`: a conversion to
    /// `bool`).
    fn null_compare_reference(&self, operand: ExprId, result: Option<u32>) -> Option<Diag> {
        let e = self.ignore_paren_imp_casts(operand);
        let callee = match self.ast.exprs.get(e).kind() {
            ExprKind::DeclRefExpr | ExprKind::MemberExpr => {
                let d = self.ast.exprs.decl(e);
                if !self.ast.decls.get(d).ty().is_some_and(|t| self.ast.types.is_reference(t)) {
                    return None;
                }
                None
            }
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr => {
                // A glvalue call returns a reference; the note names a
                // direct callee (`getDirectCallee`: a function, not a
                // pointer to one).
                if self.ast.exprs.get(e).value_category() == ValueCategory::PRValue {
                    return None;
                }
                self.callee_decl(e).filter(|&f| {
                    self.ast
                        .decls
                        .get(f)
                        .ty()
                        .is_some_and(|t| self.ast.types.is_function(self.ast.types.canonical(t)))
                })
            }
            _ => return None,
        };
        let at = self.non_null_loc(e);
        let mut msg = match result {
            Some(r) => self.diag(DiagId::sema_address_of_reference_null_compare, at).arg(r),
            None => self.diag(DiagId::sema_address_of_reference_bool_conversion, at),
        }
        .range(self.expr_range(operand));
        if let Some(f) = callee {
            let fl = self.ast.decls.get(f).loc();
            msg = msg.with_note(
                self.diag(DiagId::note_reference_is_return_value, fl)
                    .arg(format!("'{}'", self.decl_name(f))),
            );
        }
        Some(msg)
    }

    /// clang `tryExprAsCall` for the "suffix with parentheses" note:
    /// function `f` is callable without arguments and returns a pointer,
    /// or an integer when the null pointer constant is a literal `0`.
    fn suggest_call(&self, f: DeclId, null: ExprId) -> bool {
        let Some(data) = self.ast.decls.function(f) else { return false };
        let callable = self
            .ast
            .lists
            .iter::<DeclId>(data.params)
            .all(|p| self.ast.decls.param(p).is_some_and(|x| x.default_arg.is_some()));
        let ret = self.declared_return_type(f);
        let Some(ret) = ret.filter(|_| callable) else { return false };
        if self.ast.types.is_pointer(ret) {
            return true;
        }
        let zero_literal = self.ast.types.builtin(self.ety(null)) != Some(BuiltinType::NullPtr)
            && self.ast.exprs.get(self.ignore_parens(null)).kind() != ExprKind::GNUNullExpr;
        zero_literal && self.ast.types.is_integral(ret)
    }

    /// The return type of function `f`'s declared type.
    fn declared_return_type(&self, f: DeclId) -> Option<QualType> {
        let t = self.ast.decls.get(f).ty()?;
        self.ast.types.proto_of(self.ast.types.canonical(t).ty()).map(FuncProto::ret)
    }
}
