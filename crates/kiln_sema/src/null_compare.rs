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

use kiln_ast::attr::AttrKind;
use kiln_ast::expr::{ExprKind, UnaryOp};
use kiln_ast::types::{BuiltinType, FuncProto};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId, FixIt};
use kiln_source::Loc;

use crate::{Sema, conv};

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
        let Some(msg) = self.null_compare_non_null(e, null, eq) else { return };
        let msg = msg.range(self.expr_range(null));
        if self.in_template() {
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

    fn ignore_imp_casts(&self, mut e: ExprId) -> ExprId {
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

    fn null_compare_non_null(&self, e: ExprId, null: ExprId, eq: bool) -> Option<Diag> {
        let at = self.eloc(e);
        if at.is_macro()
            && (self.in_any_macro_body(at) || self.in_any_macro_body(self.begin_loc(null)))
        {
            return None;
        }
        // clang's `%select{true|false}` on `IsEqual`.
        let result = u32::from(eq);
        let mut e = self.ignore_imp_casts(e);
        if self.ast.exprs.get(e).kind() == ExprKind::CXXThisExpr {
            let msg = self.diag(DiagId::sema_this_null_compare, self.eloc(e));
            return Some(msg.arg(result).range(self.expr_range(e)));
        }
        let p = self.ignore_parens(e);
        let address_of = self.ast.exprs.get(p).kind() == ExprKind::UnaryOperator;
        if address_of {
            if self.ast.exprs.unary_op(p) != UnaryOp::AddrOf {
                return None;
            }
            e = self.ast.exprs.operand(p);
            if let Some(msg) = self.null_compare_reference(e, result) {
                return Some(msg);
            }
        }
        let n = self.ast.exprs.get(e);
        let d = match n.kind() {
            ExprKind::DeclRefExpr | ExprKind::MemberExpr => self.ast.exprs.decl(e),
            _ => return None,
        };
        if self.find_attr(d, AttrKind::WEAK).is_some() {
            return None;
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
        let msg = self
            .diag(DiagId::sema_null_pointer_compare, at)
            .arg(what as u32)
            .arg(text)
            .arg(result)
            .range(self.expr_range(e));
        if !matches!(what, NonNull::Function) {
            return Some(msg);
        }
        let begin = self.begin_loc(e);
        let mut msg = msg.with_note(
            self.diag(DiagId::note_function_warning_silence, at).fixit(FixIt::insert(begin, "&")),
        );
        if self.suggest_call(d, null) {
            let end = self.token_end(self.eloc(e)).unwrap_or(at);
            msg = msg.with_note(
                self.diag(DiagId::note_function_to_function_call, at)
                    .fixit(FixIt::insert(end, "()")),
            );
        }
        Some(msg)
    }

    /// clang `CheckForReference`: `&r` of a reference variable or member
    /// `r`, or of a call returning a reference (with a note at the
    /// function).
    fn null_compare_reference(&self, operand: ExprId, result: u32) -> Option<Diag> {
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
                let f = self.callee_decl(e)?;
                let ret = self.declared_return_type(f);
                if !ret.is_some_and(|r| self.ast.types.is_reference(r)) {
                    return None;
                }
                Some(f)
            }
            _ => return None,
        };
        let mut msg = self
            .diag(DiagId::sema_address_of_reference_null_compare, self.non_null_loc(e))
            .arg(result)
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
