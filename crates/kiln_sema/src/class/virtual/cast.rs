//! `dynamic_cast` ([expr.dynamic.cast]; clang `CastOperation::CheckDynamicCast`).
//!
//! Sema checks the operand and target and resolves the static forms:
//! a cast to the same class is a `NoOp`, and an upcast is a `DerivedToBase`
//! cast with its path ([expr.dynamic.cast]/5). Every other cast is a
//! `Dynamic` cast, which is checked at run time. Its source must be a
//! polymorphic class ([expr.dynamic.cast]/6).

use kiln_ast::decl::DefinitionBits;
use kiln_ast::expr::{CastKind, ValueCategory};
use kiln_ast::types::TypeView;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv;
use crate::conv::bases::{BaseSearch, ambiguous_paths_string, find_base, path_list, same_class};

/// The target of a `dynamic_cast`: a pointer or an lvalue/rvalue reference.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Pointer,
    LValueRef,
    RValueRef,
}

impl Sema<'_, '_> {
    /// `dynamic_cast<ty>(e)` ([expr.dynamic.cast]). `loc` is the keyword
    /// and `rparen` the closing `)`. Every diagnostic is reported at `loc`,
    /// as clang does.
    #[allow(clippy::too_many_lines, reason = "one step per paragraph of [expr.dynamic.cast]")]
    pub(crate) fn act_on_dynamic_cast(
        &mut self,
        ty: QualType,
        e: ExprId,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        if self.is_err_expr(e) || self.is_error_type(ty) {
            return self.dynamic_cast_recover(ty, e, loc);
        }
        // [expr.dynamic.cast]/1: T is a pointer or a reference to a complete
        // class type, or "pointer to cv void".
        let (target, dest_pointee) = match self.ast.types.canonical_view(ty) {
            TypeView::Pointer(p) => (Target::Pointer, p),
            TypeView::LValueRef(p) => (Target::LValueRef, p),
            TypeView::RValueRef(p) => (Target::RValueRef, p),
            _ => {
                let msg = self.diag(DiagId::sema_dynamic_cast_bad_target, loc).arg(self.ty_arg(ty));
                self.emit(msg);
                return self.dynamic_cast_recover(ty, e, loc);
            }
        };
        let dest_record = self.ast.types.as_record(dest_pointee);
        if dest_record.is_some() {
            if !self.require_complete(dest_pointee, loc) {
                return self.dynamic_cast_incomplete(dest_pointee, ty, e, loc);
            }
        } else if !self.ast.types.is_void(dest_pointee) {
            return self.dynamic_cast_not_class(dest_pointee, ty, e, loc);
        }
        // [expr.dynamic.cast]/2: for a pointer T, v is a prvalue of pointer
        // to complete class type. For an lvalue reference, v is an lvalue of
        // complete class type. For an rvalue reference, v is any expression
        // of complete class type.
        let mut bad = false;
        let (src, src_pointee) = if target == Target::Pointer {
            let x = conv::decay(self, e);
            self.mark_explicit(x, e);
            let xt = self.ety(x);
            let TypeView::Pointer(p) = self.ast.types.canonical_view(xt) else {
                let msg = self
                    .diag(DiagId::sema_dynamic_cast_not_ptr, loc)
                    .arg(self.ty_arg(xt))
                    .arg(self.ty_arg(ty));
                self.emit(msg);
                return self.dynamic_cast_recover(ty, e, loc);
            };
            (x, p)
        } else {
            let n = *self.ast.exprs.get(e);
            let mut x = e;
            if target == Target::LValueRef && n.value_category() != ValueCategory::LValue {
                let msg =
                    self.diag(DiagId::sema_dynamic_cast_rvalue_to_ref, loc).arg(self.ty_arg(ty));
                self.emit(msg);
                bad = true;
            } else if target == Target::RValueRef
                && n.value_category() == ValueCategory::PRValue
                && self.ast.types.as_record(n.ty()).is_some()
            {
                x = self.materialize_temporary(e, n.ty(), false);
            }
            (x, n.ty())
        };
        let Some(src_record) = self.ast.types.as_record(src_pointee) else {
            return self.dynamic_cast_not_class(src_pointee, ty, e, loc);
        };
        if !self.require_complete(src_pointee, loc) {
            return self.dynamic_cast_incomplete(src_pointee, ty, e, loc);
        }
        // [expr.dynamic.cast]/1: no casting away constness.
        let (dq, sq) = (
            self.ast.types.effective_quals(dest_pointee),
            self.ast.types.effective_quals(src_pointee),
        );
        if !dq.contains(sq) {
            let from = self.ety(src);
            let msg = self
                .diag(DiagId::sema_bad_cast_qualifiers_away, loc)
                .arg("3")
                .arg(self.ty_arg(from))
                .arg(self.ty_arg(ty));
            self.emit(msg);
            return self.dynamic_cast_recover(ty, e, loc);
        }
        let (rty, vk) = match target {
            Target::Pointer => (conv::strip_cv(self, ty), ValueCategory::PRValue),
            Target::LValueRef => (self.non_reference(ty), ValueCategory::LValue),
            Target::RValueRef => (self.non_reference(ty), ValueCategory::XValue),
        };
        // [expr.dynamic.cast]/3: the same class, up to cv-qualification.
        if let Some(d) = dest_record
            && same_class(self, d, src_record)
        {
            if bad {
                return self.dynamic_cast_recover(ty, e, loc);
            }
            return self.builder().dynamic_cast(CastKind::NoOp, src, rty, vk, None, loc, rparen);
        }
        // [expr.dynamic.cast]/5: upcasts are resolved statically.
        if let Some(d) = dest_record {
            match find_base(self, src_record, d) {
                BaseSearch::Unique(p) => {
                    let (dt, bt) =
                        (conv::strip_cv(self, src_pointee), conv::strip_cv(self, dest_pointee));
                    if !self.check_base_access(dt, bt, &p, loc) || bad {
                        return self.dynamic_cast_recover(ty, e, loc);
                    }
                    let list = path_list(self, &p);
                    return self.builder().dynamic_cast(
                        CastKind::DerivedToBase,
                        src,
                        rty,
                        vk,
                        Some(list),
                        loc,
                        rparen,
                    );
                }
                BaseSearch::Ambiguous(paths) => {
                    let shown = ambiguous_paths_string(self, src_pointee, &paths);
                    let msg = self
                        .diag(DiagId::sema_ambiguous_derived_to_base, loc)
                        .arg(self.ty_arg(conv::strip_cv(self, src_pointee)))
                        .arg(self.ty_arg(conv::strip_cv(self, dest_pointee)))
                        .arg(shown);
                    self.emit(msg);
                    return self.dynamic_cast_recover(ty, e, loc);
                }
                BaseSearch::NotDerived => {}
            }
        }
        // [expr.dynamic.cast]/6: otherwise v is a pointer to or a glvalue of
        // a polymorphic type.
        let polymorphic = self
            .ast
            .definition(src_record)
            .and_then(|def| self.ast.decls.record(def))
            .is_some_and(|r| r.defn.contains(DefinitionBits::POLYMORPHIC));
        if !polymorphic {
            let msg = self
                .diag(DiagId::sema_dynamic_cast_not_polymorphic, loc)
                .arg(self.ty_arg(conv::strip_cv(self, src_pointee)));
            self.emit(msg);
            bad = true;
        }
        // A cast to `void*` needs no RTTI (clang `err_no_dynamic_cast_with_fno_rtti`).
        if !self.rtti && dest_record.is_some() {
            self.error(DiagId::sema_dynamic_cast_no_rtti, loc);
            bad = true;
        }
        if bad {
            return self.dynamic_cast_recover(ty, e, loc);
        }
        self.builder().dynamic_cast(CastKind::Dynamic, src, rty, vk, None, loc, rparen)
    }

    /// `'T' is an incomplete type`, with the forward-declaration note.
    fn dynamic_cast_incomplete(
        &mut self,
        t: QualType,
        ty: QualType,
        e: ExprId,
        loc: Loc,
    ) -> ExprId {
        let msg = self.diag(DiagId::sema_dynamic_cast_incomplete, loc).arg(self.ty_arg(t));
        let unqual = conv::strip_cv(self, t);
        self.emit(self.incomplete_note(msg, unqual));
        self.dynamic_cast_recover(ty, e, loc)
    }

    /// `'T' is not a class type`, for the unqualified `t`.
    fn dynamic_cast_not_class(&mut self, t: QualType, ty: QualType, e: ExprId, loc: Loc) -> ExprId {
        let unqual = conv::strip_cv(self, t);
        let msg = self.diag(DiagId::sema_dynamic_cast_not_class, loc).arg(self.ty_arg(unqual));
        self.emit(msg);
        self.dynamic_cast_recover(ty, e, loc)
    }

    /// Recovery for a failed cast: the target type for a pointer, as
    /// clang's caller sees no expression at all for a reference.
    fn dynamic_cast_recover(&mut self, ty: QualType, e: ExprId, loc: Loc) -> ExprId {
        if self.ast.types.is_reference(ty) {
            return self.recover_err(&[e], loc);
        }
        self.recover(ty, &[e], loc)
    }
}
