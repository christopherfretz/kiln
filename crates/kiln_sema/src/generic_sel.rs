//! C11 `_Generic` selection in C++, a clang extension (spec/sema.md
//! "`_Generic` selection"): clang's `Sema::CreateGenericSelectionExpr`.
//!
//! The controlling operand is unevaluated (C11 6.5.1.1p3) and, in the
//! expression form, undergoes the lvalue, array and function conversions
//! before it is matched; the type form (`_Generic(type-name, ...)`, clang's
//! C2y extension) is matched as written. In C++ two types are compatible
//! when they are the same type. The result is the chosen association
//! expression: its type, value category and value.

use kiln_ast::QualType;
use kiln_ast::expr::{ExprDeps, GenericAssoc, GenericControl};
use kiln_ast::ids::ExprId;
use kiln_ast::types::TypeFlags;
use kiln_diag::{DiagId, Severity};
use kiln_source::Loc;

use crate::Sema;
use crate::conv;

impl Sema<'_, '_> {
    /// `_Generic(ctrl, assocs...)` at `loc` (the keyword), closed at
    /// `rparen` (clang `ActOnGenericSelectionExpr` /
    /// `CreateGenericSelectionExpr`). A type- or value-dependent operand or
    /// association type leaves the selection result-dependent until
    /// substitution; otherwise the association whose type is the
    /// controlling type is selected, else `default`. Errors: an
    /// incomplete (clang 19; a C2y extension in clang 22), non-object or
    /// variably modified association type, two associations of the same
    /// type, more than one match, or no match and no `default`.
    pub fn act_on_generic_selection(
        &mut self,
        loc: Loc,
        ctrl: GenericControl,
        assocs: &[GenericAssoc],
        rparen: Loc,
    ) -> ExprId {
        let mut kids: Vec<ExprId> = assocs.iter().map(|a| a.expr).collect();
        if let GenericControl::Expr(c) = ctrl {
            kids.insert(0, c);
        }
        if kids.iter().any(|&k| self.is_err_expr(k))
            || assocs.iter().any(|a| a.ty.is_some_and(|t| self.is_error_type(t)))
            || matches!(ctrl, GenericControl::Type(t, _) if self.is_error_type(t))
        {
            return self.recover_err(&kids, loc);
        }
        // C11 6.5.1.1p3: the controlling expression is not evaluated; its
        // lvalue conversion is done in an unevaluated context.
        let ctrl = match ctrl {
            GenericControl::Expr(c) => {
                self.enter_unevaluated();
                let c = conv::decay(self, c);
                self.exit_unevaluated();
                GenericControl::Expr(c)
            }
            t @ GenericControl::Type(..) => t,
        };
        let (ctrl_ty, ctrl_loc) = match ctrl {
            GenericControl::Expr(c) => (self.ast.exprs.get(c).ty(), self.begin_loc(c)),
            GenericControl::Type(t, l) => (t, l),
        };
        if let GenericControl::Expr(c) = ctrl
            && self.is_err_expr(c)
        {
            return self.recover_err(&kids, loc);
        }
        let mut dependent = match ctrl {
            GenericControl::Expr(c) => self.ast.exprs.get(c).deps().contains(ExprDeps::TYPE),
            GenericControl::Type(t, _) => self.is_dependent_type(t),
        };
        let is_expr = matches!(ctrl, GenericControl::Expr(_));
        if let GenericControl::Expr(c) = ctrl
            && !dependent
        {
            self.check_unevaluated_side_effects(c);
        }
        let (type_error, dep_assoc) = self.check_assoc_types(assocs, is_expr);
        dependent |= dep_assoc;
        if type_error {
            return self.recover_err(&kids, loc);
        }
        if dependent {
            return self.builder().generic_selection(ctrl, assocs, None, loc, rparen);
        }
        let matches: Vec<usize> = assocs
            .iter()
            .enumerate()
            .filter(|(_, a)| a.ty.is_some_and(|t| self.ast.types.same_type(ctrl_ty, t)))
            .map(|(i, _)| i)
            .collect();
        if matches.len() > 1 {
            let mut d = self
                .diag(DiagId::sema_generic_sel_multi_match, ctrl_loc)
                .arg(self.ty_arg(ctrl_ty))
                .arg(matches.len());
            for &i in &matches {
                let t = assocs[i].ty.unwrap_or(QualType::ERROR);
                d = d.with_note(
                    self.diag(DiagId::note_compat_assoc, assocs[i].ty_loc).arg(self.ty_arg(t)),
                );
            }
            self.emit(d);
            return self.recover_err(&kids, loc);
        }
        let selected =
            matches.first().copied().or_else(|| assocs.iter().position(|a| a.ty.is_none()));
        let Some(selected) = selected else {
            let d =
                self.diag(DiagId::sema_generic_sel_no_match, ctrl_loc).arg(self.ty_arg(ctrl_ty));
            self.emit(d);
            return self.recover_err(&kids, loc);
        };
        self.builder().generic_selection(ctrl, assocs, Some(selected), loc, rparen)
    }

    /// The checks on every non-dependent association type (clang
    /// `CreateGenericSelectionExpr`'s loop): `(error, dependent)`, where
    /// `error` is a diagnostic that invalidates the selection and
    /// `dependent` is set by a dependent association type.
    fn check_assoc_types(&mut self, assocs: &[GenericAssoc], is_expr: bool) -> (bool, bool) {
        let (mut type_error, mut dependent) = (false, false);
        for (i, a) in assocs.iter().enumerate() {
            let Some(t) = a.ty else { continue };
            if self.is_dependent_type(t) {
                dependent = true;
                continue;
            }
            if let Some(d) = self.assoc_type_check(t, a.ty_loc, is_expr) {
                // clang: the unreachable-association warning never makes
                // the selection invalid, even as an error (-Werror).
                let fatal = d.id().is_some_and(|id| {
                    id != DiagId::sema_unreachable_association
                        && self.diags.severity_at(id, a.ty_loc) >= Severity::Error
                });
                if fatal {
                    type_error = true;
                }
                self.emit(d);
            }
            // C11 6.5.1.1p2: no two associations name compatible types.
            for b in &assocs[i + 1..] {
                let Some(u) = b.ty else { continue };
                if self.is_dependent_type(u) || !self.ast.types.same_type(t, u) {
                    continue;
                }
                let note = self.diag(DiagId::note_compat_assoc, a.ty_loc).arg(self.ty_arg(t));
                let d = self
                    .diag(DiagId::sema_assoc_compatible_types, b.ty_loc)
                    .arg(self.ty_arg(u))
                    .arg(self.ty_arg(t))
                    .with_note(note);
                self.emit(d);
                type_error = true;
            }
        }
        (type_error, dependent)
    }

    /// The diagnostic for a non-dependent association type `t` at `at`, if
    /// any (clang `CreateGenericSelectionExpr`). The incomplete and
    /// non-object checks apply to the expression form only (the type form
    /// relaxes them); a type the converted controlling expression can never
    /// have (an array, or a qualified non-class type) draws
    /// `-Wunreachable-code-generic-assoc`.
    fn assoc_type_check(&mut self, t: QualType, at: Loc, is_expr: bool) -> Option<kiln_diag::Diag> {
        let types = &self.ast.types;
        let (fun, rf) = (types.is_function(t), types.is_reference(t));
        let id = if is_expr && !fun && !rf && !self.is_complete(t) {
            if self.clang_compat < 22 {
                DiagId::sema_assoc_type_incomplete
            } else {
                DiagId::sema_assoc_type_incomplete_c2y
            }
        } else if is_expr && (fun || rf || types.is_void(t)) {
            DiagId::sema_assoc_type_nonobject
        } else if types.qflags(t).contains(TypeFlags::VARIABLY_MODIFIED) {
            DiagId::sema_assoc_type_variably_modified
        } else if is_expr {
            // [conv.lval]/1: a class prvalue keeps its cv-qualifiers.
            let reason = if types.is_array(t) {
                0
            } else if !types.canonical(t).quals().is_empty() && types.as_record(t).is_none() {
                1
            } else {
                return None;
            };
            let d =
                self.diag(DiagId::sema_unreachable_association, at).arg(self.ty_arg(t)).arg(reason);
            return Some(d);
        } else {
            return None;
        };
        Some(self.diag(id, at).arg(self.ty_arg(t)))
    }
}
