//! Casts to reference types ([expr.static.cast]/2-4, [expr.const.cast]/4,
//! [expr.reinterpret.cast]/11, [expr.cast]/4) and the static downcast of a
//! pointer to a class ([expr.static.cast]/11).
//!
//! A cast to a reference yields the referent type: an lvalue for an
//! lvalue reference (or a function), an xvalue for an rvalue reference.
//! The class forms are resolved statically: the same class is a `NoOp`,
//! an upcast a `DerivedToBase` cast and a downcast a `BaseToDerived` cast,
//! each with its base path (as clang's `CastOperation` builds them).

use kiln_ast::expr::{CastKind, ExprFlags, ValueCategory};
use kiln_ast::types::TypeView;
use kiln_ast::{ExprId, ListId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::CastStyle;
use crate::Sema;
use crate::conv;
use crate::conv::bases::{BaseSearch, ambiguous_paths_string, find_base, path_list};
use crate::init::Initializer;

impl Sema<'_, '_> {
    /// `style<ty>(e)` for a reference type `ty`. `loc` is the cast's
    /// start, `rparen` its `)`.
    pub(super) fn ref_cast(
        &mut self,
        style: CastStyle,
        ty: QualType,
        e: ExprId,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let lref = matches!(self.ast.types.canonical_view(ty), TypeView::LValueRef(_));
        let r = self.non_reference(ty);
        let vk = if lref || self.ast.types.is_function(r) {
            ValueCategory::LValue
        } else {
            ValueCategory::XValue
        };
        let n = *self.ast.exprs.get(e);
        let et = n.ty();
        let evk = n.value_category();
        let binds = evk == ValueCategory::LValue || (!lref && evk == ValueCategory::XValue);
        let keeps_cv =
            self.ast.types.effective_quals(r).contains(self.ast.types.effective_quals(et));
        let same = self.ast.types.same_unqualified(r, et);
        if binds {
            // [expr.static.cast]/4, [expr.const.cast]/4: the same type,
            // (for `const_cast` and the C-style cast) cv-qualification
            // removed.
            if same && (style.allows_const() || (keeps_cv && style.allows_static())) {
                return self.typed_cast(style, CastKind::NoOp, e, r, vk, None, loc, rparen);
            }
            // [expr.static.cast]/2 and /4: a base or derived class.
            if style.allows_static()
                && (keeps_cv || style.allows_const())
                && let Some(x) = self.class_ref_cast(style, e, et, r, vk, loc, rparen)
            {
                return x;
            }
            // [expr.reinterpret.cast]/11: an lvalue as another type.
            if style.allows_reinterpret() && (keeps_cv || style.allows_const()) && !same {
                return self.typed_cast(
                    style,
                    CastKind::LValueBitCast,
                    e,
                    r,
                    vk,
                    None,
                    loc,
                    rparen,
                );
            }
        }
        if style.allows_static() && style != CastStyle::Functional {
            // [expr.static.cast]/4: `T& t(e);` (a prvalue bound to an rvalue
            // or const reference is materialized).
            let x = self.perform_init(ty, Initializer::Paren(vec![e], loc, rparen), loc, false);
            if self.is_err_expr(x) {
                return x;
            }
            self.mark_explicit(x, e);
            return self.typed_cast(style, CastKind::NoOp, x, r, vk, None, loc, rparen);
        }
        self.bad_cast(style, e, ty, loc)
    }

    /// A glvalue `e` of class type `et` as the related class `r`:
    /// `DerivedToBase` or (not through a virtual base) `BaseToDerived`.
    #[allow(clippy::too_many_arguments, reason = "the cast's parts")]
    fn class_ref_cast(
        &mut self,
        style: CastStyle,
        e: ExprId,
        et: QualType,
        r: QualType,
        vk: ValueCategory,
        loc: Loc,
        rparen: Loc,
    ) -> Option<ExprId> {
        let ec = self.ast.types.as_record(et)?;
        let rc = self.ast.types.as_record(r)?;
        let (ck, path) = self.class_relation(et, ec, r, rc, loc)?;
        Some(self.typed_cast(style, ck, e, r, vk, path, loc, rparen))
    }

    /// The cast between related classes `from` (record `fc`) and `to`
    /// (record `tc`): an upcast, or a downcast not through a virtual base
    /// ([expr.static.cast]/2, /11). `None` when unrelated; an ambiguous
    /// base is diagnosed (`Some` with `CastKind::NoOp` never occurs: the
    /// caller treats the error as a failed cast).
    fn class_relation(
        &mut self,
        from: QualType,
        fc: kiln_ast::DeclId,
        to: QualType,
        tc: kiln_ast::DeclId,
        loc: Loc,
    ) -> Option<(CastKind, Option<ListId>)> {
        let (fu, tu) = (conv::strip_cv(self, from), conv::strip_cv(self, to));
        match find_base(self, fc, tc) {
            BaseSearch::Unique(p) => {
                self.check_base_access(fu, tu, &p, loc);
                return Some((CastKind::DerivedToBase, Some(path_list(self, &p))));
            }
            BaseSearch::Ambiguous(paths) => {
                let shown = ambiguous_paths_string(self, from, &paths);
                let msg = self
                    .diag(DiagId::sema_ambiguous_derived_to_base, loc)
                    .arg(self.ty_arg(fu))
                    .arg(self.ty_arg(tu))
                    .arg(shown);
                self.emit(msg);
                return None;
            }
            BaseSearch::NotDerived => {}
        }
        match find_base(self, tc, fc) {
            BaseSearch::Unique(p) if p.first_virtual.is_none() => {
                self.check_base_access(tu, fu, &p, loc);
                Some((CastKind::BaseToDerived, Some(path_list(self, &p))))
            }
            _ => None,
        }
    }

    /// `static_cast<D*>(b)` for a prvalue `x` of type "pointer to cv B"
    /// where `D` is derived from `B` ([expr.static.cast]/11).
    pub(super) fn static_downcast(
        &mut self,
        style: CastStyle,
        x: ExprId,
        to: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> Option<ExprId> {
        let xt = self.ety(x);
        let fp = self.ast.types.pointee(xt).filter(|_| self.ast.types.is_pointer(xt))?;
        let tp = self.ast.types.pointee(to).filter(|_| self.ast.types.is_pointer(to))?;
        let fc = self.ast.types.as_record(fp)?;
        let tc = self.ast.types.as_record(tp)?;
        if !style.allows_const()
            && !self.ast.types.effective_quals(tp).contains(self.ast.types.effective_quals(fp))
        {
            return None;
        }
        let BaseSearch::Unique(p) = find_base(self, tc, fc) else { return None };
        if p.first_virtual.is_some() {
            return None;
        }
        let (fu, tu) = (conv::strip_cv(self, fp), conv::strip_cv(self, tp));
        self.check_base_access(tu, fu, &p, loc);
        let path = Some(path_list(self, &p));
        let vk = ValueCategory::PRValue;
        Some(self.typed_cast(style, CastKind::BaseToDerived, x, to, vk, path, loc, rparen))
    }

    /// The node of an explicit cast `style` of kind `ck` yielding a `vk`
    /// of type `to`.
    #[allow(clippy::too_many_arguments, reason = "the cast's parts")]
    fn typed_cast(
        &mut self,
        style: CastStyle,
        ck: CastKind,
        sub: ExprId,
        to: QualType,
        vk: ValueCategory,
        path: Option<ListId>,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let flags = ExprFlags::NONE;
        if style == CastStyle::Functional {
            self.builder().functional_cast(ck, sub, to, vk, path, loc, rparen, flags)
        } else {
            self.builder().cast(style.kind(), ck, sub, to, vk, path, loc, flags)
        }
    }
}
