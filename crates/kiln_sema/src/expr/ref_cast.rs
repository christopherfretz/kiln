//! Casts to reference types ([expr.static.cast]/2-4, [expr.const.cast]/4,
//! [expr.reinterpret.cast]/11, [expr.cast]/4) and the static downcast of a
//! pointer to a class ([expr.static.cast]/11) and the static upcast of a
//! pointer to member ([expr.static.cast]/12).
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
        // [expr.reinterpret.cast]/11: any glvalue (clang accepts an xvalue
        // as an lvalue reference too).
        let reinterprets = style.allows_reinterpret() && evk != ValueCategory::PRValue;
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
            // [expr.const.cast]/4: an lvalue of `T1` as `T2&` when a
            // `T1*` const_casts to `T2*`, i.e. similar types with
            // cv-qualification changed at any level (bslstl_vector's
            // `const_cast<const iterator&>(position)`, `J *const &` from
            // `const J *`).
            if style.allows_const() && conv::similar_same_levels(self, r, et) {
                return self.typed_cast(style, CastKind::NoOp, e, r, vk, None, loc, rparen);
            }
            // [expr.static.cast]/2 and /4: a base or derived class.
            if style.allows_static()
                && (keeps_cv || style.allows_const())
                && let Some(x) = self.class_ref_cast(style, e, et, ty, vk, loc, rparen)
            {
                return x;
            }
        }
        // [expr.cast]/4: a C-style or functional cast is a `static_cast`
        // before it is a `reinterpret_cast`, so `(const int&)c` for a
        // `char c` binds the reference to a temporary `int` ([expr.static.cast]/4,
        // `const int& t(c);`) rather than reinterpreting `c` (clang
        // `TryStaticImplicitCast`).
        let by_init = matches!(style, CastStyle::CStyle | CastStyle::Functional)
            && !(binds && same)
            && self.ref_init_viable(ty, e);
        if (binds || reinterprets) && !by_init && style.allows_reinterpret() {
            if style == CastStyle::Reinterpret {
                return self.reinterpret_ref(e, et, ty, r, vk, loc, rparen);
            }
            // [expr.reinterpret.cast]/11 after [expr.const.cast]: a glvalue
            // as another type (or an xvalue as an lvalue), cv-qualifiers
            // cast away as by a `const_cast`.
            if keeps_cv || style.allows_const() {
                let ck = if same { CastKind::NoOp } else { CastKind::LValueBitCast };
                return self.typed_cast(style, ck, e, r, vk, None, loc, rparen);
            }
        }
        // [expr.const.cast]/3: a class prvalue as an xvalue of the same
        // class (cv-qualification changed) by `const_cast<T&&>` (or the
        // C-style cast's const_cast step), through temporary
        // materialization (clang `TryConstCast`'s
        // `NeedToMaterializeTemporary`).
        if !lref
            && evk == ValueCategory::PRValue
            && style.allows_const()
            && same
            && self.ast.types.as_record(et).is_some()
        {
            let x = self.materialize_temporary(e, et, false);
            return self.typed_cast(style, CastKind::NoOp, x, r, vk, None, loc, rparen);
        }
        // clang `err_bad_cxx_cast_rvalue`: `const_cast` binds a reference
        // only to an lvalue (or, for `T&&`, an xvalue or a class prvalue),
        // `reinterpret_cast` only to a glvalue.
        let rvalue = match style {
            CastStyle::Const => {
                !binds
                    && (lref
                        || (evk == ValueCategory::PRValue
                            && self.ast.types.as_record(et).is_none()))
            }
            CastStyle::Reinterpret => evk == ValueCategory::PRValue,
            _ => false,
        };
        if rvalue {
            let msg = self
                .diag(DiagId::sema_bad_cxx_cast_rvalue, loc)
                .arg(style.select())
                .arg(self.ty_arg(ty));
            self.emit(msg);
            return self.recover(ty, &[e], loc);
        }
        if style.allows_static() && (style != CastStyle::Functional || by_init) {
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

    /// `reinterpret_cast<ty>(e)` for the glvalue `e` of type `et` and the
    /// reference type `ty` to `r` (yielding a `vk`): [expr.reinterpret.cast]/11,
    /// `*reinterpret_cast<r*>(&e)`, so any `et` whose pointer casts to
    /// `r*` ([expr.reinterpret.cast]/7), its own type and a more
    /// cv-qualified one included (the reference workload's source file:
    /// `reinterpret_cast<const T&>(u)` for a `const U& u` with `T` = `U`),
    /// but not one that casts away constness ([expr.reinterpret.cast]/2,
    /// [expr.const.cast]/8, at any level), nor a bit-field (clang
    /// `TryReinterpretCast`).
    #[allow(clippy::too_many_arguments, reason = "the cast's parts")]
    fn reinterpret_ref(
        &mut self,
        e: ExprId,
        et: QualType,
        ty: QualType,
        r: QualType,
        vk: ValueCategory,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let style = CastStyle::Reinterpret;
        if conv::source_bit_field(self, e).is_some() {
            let msg = self
                .diag(DiagId::sema_bad_cxx_cast_bitfield, loc)
                .arg(style.select())
                .arg(self.ty_arg(ty));
            self.emit(msg);
            return self.recover(ty, &[e], loc);
        }
        let from = self.ast.types.pointer_to(et);
        let to = self.ast.types.pointer_to(r);
        if matches!(
            conv::casts_away_constness(self, from, to),
            conv::CastAway::Similar | conv::CastAway::SimilarKind
        ) {
            let msg = self
                .diag(DiagId::sema_bad_cast_qualifiers_away, loc)
                .arg(style.select())
                .arg(self.ty_arg(et))
                .arg(self.ty_arg(ty));
            self.emit(msg);
            return self.recover(ty, &[e], loc);
        }
        // clang: `CK_NoOp` when the two pointer types are the same,
        // cv-qualification included.
        let ck =
            if self.ast.types.same_type(et, r) { CastKind::NoOp } else { CastKind::LValueBitCast };
        self.typed_cast(style, ck, e, r, vk, None, loc, rparen)
    }

    /// Whether `T& t(e);` (direct-initialization of the reference type
    /// `ty`) has an implicit conversion sequence ([dcl.init.ref]/5,
    /// [over.match.ref]/1.1: explicit conversion functions count).
    fn ref_init_viable(&mut self, ty: QualType, e: ExprId) -> bool {
        let a = self.arg_info(e);
        !self
            .try_implicit_conversion(&a, ty, crate::overload::ics::IcsOptions::EXPLICIT_REF_CONV)
            .is_bad()
    }

    /// A glvalue `e` of class type `et` as the referent `r` of reference
    /// type `ty`: `DerivedToBase` or (not through a virtual base)
    /// `BaseToDerived`; a downcast through a virtual base is diagnosed
    /// and recovered.
    #[allow(clippy::too_many_arguments, reason = "the cast's parts")]
    fn class_ref_cast(
        &mut self,
        style: CastStyle,
        e: ExprId,
        et: QualType,
        ty: QualType,
        vk: ValueCategory,
        loc: Loc,
        rparen: Loc,
    ) -> Option<ExprId> {
        let r = self.ast.types.pointee(ty)?;
        let ec = self.ast.types.as_record(et)?;
        let rc = self.ast.types.as_record(r)?;
        match self.class_relation(style, et, ec, r, rc, loc) {
            Ok(Some((ck, path))) => Some(self.typed_cast(style, ck, e, r, vk, path, loc, rparen)),
            Ok(None) => None,
            Err(v) => {
                self.static_downcast_via_virtual(et, ty, v, loc);
                Some(self.recover(ty, &[e], loc))
            }
        }
    }

    /// clang `err_static_downcast_via_virtual` ([expr.static.cast]/2,
    /// /11): a downcast from `from` to `to` through virtual base `v`.
    fn static_downcast_via_virtual(
        &mut self,
        from: QualType,
        to: QualType,
        v: kiln_ast::DeclId,
        loc: Loc,
    ) {
        let vt = QualType::unqual(self.ast.types.record_type(v));
        let msg = self
            .diag(DiagId::sema_static_downcast_via_virtual, loc)
            .arg(self.ty_arg(from))
            .arg(self.ty_arg(to))
            .arg(self.ty_arg(vt));
        self.emit(msg);
    }

    /// The cast between related classes `from` (record `fc`) and `to`
    /// (record `tc`): an upcast, or a downcast not through a virtual base
    /// ([expr.static.cast]/2, /11). `Ok(None)` when unrelated; an
    /// ambiguous base is diagnosed (the caller treats the error as a
    /// failed cast); `Err(v)` a downcast through virtual base `v`.
    fn class_relation(
        &mut self,
        style: CastStyle,
        from: QualType,
        fc: kiln_ast::DeclId,
        to: QualType,
        tc: kiln_ast::DeclId,
        loc: Loc,
    ) -> Result<Option<(CastKind, Option<ListId>)>, kiln_ast::DeclId> {
        let (fu, tu) = (conv::strip_cv(self, from), conv::strip_cv(self, to));
        // The relation is only known for complete classes: a class
        // template specialization is instantiated first ([temp.inst]/2;
        // clang `isCompleteType` in `TryStaticDowncast`).
        let _ = self.require_complete_silent(fu, loc);
        let _ = self.require_complete_silent(tu, loc);
        match find_base(self, fc, tc) {
            BaseSearch::Unique(p) => {
                if !style.ignores_base_access() {
                    self.check_base_access(fu, tu, &p, loc);
                }
                return Ok(Some((CastKind::DerivedToBase, Some(path_list(self, &p)))));
            }
            BaseSearch::Ambiguous(paths) => {
                let shown = ambiguous_paths_string(self, from, &paths);
                let msg = self
                    .diag(DiagId::sema_ambiguous_derived_to_base, loc)
                    .arg(self.ty_arg(fu))
                    .arg(self.ty_arg(tu))
                    .arg(shown);
                self.emit(msg);
                return Ok(None);
            }
            BaseSearch::NotDerived => {}
        }
        match find_base(self, tc, fc) {
            BaseSearch::Unique(p) => {
                if let Some(v) = p.first_virtual {
                    return Err(v);
                }
                if !style.ignores_base_access() {
                    self.check_base_access(tu, fu, &p, loc);
                }
                Ok(Some((CastKind::BaseToDerived, Some(path_list(self, &p)))))
            }
            _ => Ok(None),
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
        let (fu, tu) = (conv::strip_cv(self, fp), conv::strip_cv(self, tp));
        // clang `TryStaticDowncast`: the derived class must be complete,
        // instantiating a class template specialization ([temp.inst]/2).
        if !self.require_complete_silent(tu, loc) {
            return None;
        }
        let BaseSearch::Unique(p) = find_base(self, tc, fc) else { return None };
        if let Some(v) = p.first_virtual {
            self.static_downcast_via_virtual(xt, to, v, loc);
            return Some(self.recover(to, &[x], loc));
        }
        if !style.ignores_base_access() {
            self.check_base_access(tu, fu, &p, loc);
        }
        let path = Some(path_list(self, &p));
        let vk = ValueCategory::PRValue;
        Some(self.typed_cast(style, CastKind::BaseToDerived, x, to, vk, path, loc, rparen))
    }

    /// `static_cast<T B::*>(m)` for a prvalue `x` of type "pointer to
    /// member of `D` of type cv1 `T`" where `D` is derived from `B`
    /// ([expr.static.cast]/12): a `DerivedToBaseMemberPointer` cast along
    /// the base path, its value checked when used ([expr.mptr.oper]/4).
    pub(super) fn static_mp_upcast(
        &mut self,
        style: CastStyle,
        x: ExprId,
        to: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> Option<ExprId> {
        let xt = self.ety(x);
        let parts = |s: &Self, q| match s.ast.types.canonical_view(q) {
            TypeView::MemberPointer { pointee, class } => Some((pointee, class)),
            _ => None,
        };
        let (fp, fcls) = parts(self, xt)?;
        let (tp, tcls) = parts(self, to)?;
        if !self.ast.types.same_unqualified(fp, tp) {
            return None;
        }
        if !style.allows_const()
            && !self.ast.types.effective_quals(tp).contains(self.ast.types.effective_quals(fp))
        {
            return None;
        }
        let d = self.ast.types.as_record(QualType::unqual(fcls))?;
        let b = self.ast.types.as_record(QualType::unqual(tcls))?;
        let BaseSearch::Unique(p) = find_base(self, d, b) else { return None };
        if let Some(v) = p.first_virtual {
            // [expr.static.cast]/12: no virtual base on the path (clang
            // `TryStaticMemberPointerUpcast`, a hard failure), so no
            // member-pointer conversion with a virtual step reaches codegen.
            let vt = QualType::unqual(self.ast.types.record_type(v));
            let msg = self
                .diag(DiagId::sema_member_pointer_conv_via_virtual, loc)
                .arg(self.ty_arg(QualType::unqual(fcls)))
                .arg(self.ty_arg(QualType::unqual(tcls)))
                .arg(self.ty_arg(vt));
            self.emit(msg);
            return Some(self.recover(to, &[x], loc));
        }
        let path = Some(path_list(self, &p));
        let vk = ValueCategory::PRValue;
        let ck = CastKind::DerivedToBaseMemberPointer;
        Some(self.typed_cast(style, ck, x, to, vk, path, loc, rparen))
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
