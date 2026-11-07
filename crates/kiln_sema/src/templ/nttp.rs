//! Non-type template parameters of class and reference type
//! ([temp.param]/6-7, [temp.arg.nontype]; spec/sema-templates.md §6.10):
//! the structural-type check of a parameter's type, and the template
//! parameter objects ([temp.param]/8) class-type arguments designate.

use kiln_ast::consts::{PtrBase, Scalar};
use kiln_ast::decl::{Access, DeclFlags, DeclKind, DefinitionBits, VarStorage};
use kiln_ast::expr::{
    CastKind, CharKind, ExprFlags, ExprKind, NonOdrUse, Radix, UnaryOp, ValueCategory,
};
use kiln_ast::types::{BuiltinType, Quals, TemplateArg, TypeView};
use kiln_ast::{ConstValueId, DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_oven::{EvalContext, EvalFailure, FailKind, Note};
use kiln_source::Loc;

use crate::Sema;
use crate::init::Initializer;
use crate::overload::IcsOptions;

/// Recursion bound for the explanation walk (a class cannot contain
/// itself, but a broken one may refer to itself through errors).
const MAX_DEPTH: usize = 256;

impl Sema<'_, '_> {
    /// Checks the type of a non-type template parameter at `loc` (clang
    /// `CheckNonTypeTemplateParameterType`): it must be a structural
    /// type ([temp.param]/6), so a class must be literal with only
    /// public, non-mutable bases and members of structural type
    /// ([temp.param]/7). Dependent and placeholder types are checked
    /// when substituted or deduced. False after a diagnostic.
    pub(crate) fn check_nttp_type(&mut self, ty: QualType, loc: Loc) -> bool {
        self.check_nttp_type_noting(ty, loc, None)
    }

    /// [`Self::check_nttp_type`] whose diagnostic ends with "template
    /// parameter is declared here" at `param` (clang
    /// `NoteTemplateParameterLocation` after checking a deduced type).
    pub(crate) fn check_nttp_type_noting(
        &mut self,
        ty: QualType,
        loc: Loc,
        param: Option<Loc>,
    ) -> bool {
        let here = |d: Diag| match param {
            Some(l) => d.note(DiagId::note_template_param_here, kiln_source::Span::empty(l)),
            None => d,
        };
        if self.is_error_type(ty) || self.is_dependent_type(ty) {
            return true;
        }
        let t = &self.ast.types;
        if matches!(t.canonical_view(ty), TypeView::RValueRef(_)) {
            let d = self.diag(DiagId::sema_nttp_rvalue_ref, loc).arg(self.ty_arg(ty));
            self.emit(here(d));
            return false;
        }
        let Some(r) = t.as_record(ty) else { return true };
        // clang `RequireStructuralType`: the type must be complete
        // ([temp.param]/6 via [basic.types.general]/5), in every language
        // mode, also when substituted.
        if !self.require_complete(ty, loc) {
            let d = self.diag(DiagId::sema_nttp_incomplete, loc).arg(self.ty_arg(ty));
            let d = self.incomplete_note(d, ty);
            self.emit(here(d));
            return false;
        }
        let Some(def) = self.ast.definition(r) else { return true };
        let bits = self.record_bits(def);
        if !bits.contains(DefinitionBits::COMPLETE) {
            return true;
        }
        if !bits.contains(DefinitionBits::LITERAL) {
            let d = self.diag(DiagId::sema_nttp_non_literal, loc).arg(self.ty_arg(ty));
            let d = self.with_non_literal_notes(d, ty);
            self.emit(here(d));
            return false;
        }
        if self.is_structural(ty) {
            return true;
        }
        let mut d = self.diag(DiagId::sema_nttp_not_structural, loc).arg(self.ty_arg(ty));
        for n in self.not_structural_notes(ty) {
            d = d.with_note(n);
        }
        self.emit(here(d));
        false
    }

    pub(crate) fn record_bits(&self, def: DeclId) -> DefinitionBits {
        self.ast.decls.record(def).map(|x| x.defn).unwrap_or_default()
    }

    /// Whether `ty` is a structural type ([temp.param]/7), as clang's
    /// `Type::isStructuralType`, recomputed for classes so a member of
    /// rvalue reference type counts (the `STRUCTURAL` bit misses it).
    pub(crate) fn is_structural(&self, ty: QualType) -> bool {
        self.is_structural_at(ty, 0)
    }

    fn is_structural_at(&self, ty: QualType, depth: usize) -> bool {
        let q = self.base_element(ty);
        let t = &self.ast.types;
        let Some(def) = t.as_record(q).and_then(|r| self.ast.definition(r)) else {
            return !matches!(t.canonical_view(q), TypeView::RValueRef(_));
        };
        let b = self.record_bits(def);
        if !b.contains(DefinitionBits::LITERAL) || !b.contains(DefinitionBits::STRUCTURAL) {
            return false;
        }
        if depth >= MAX_DEPTH {
            return true;
        }
        self.fields_in(def)
            .into_iter()
            .all(|f| self.ast.decls.get(f).ty().is_none_or(|q| self.is_structural_at(q, depth + 1)))
            && self
                .ast
                .decls
                .bases(def, &self.ast.lists)
                .all(|b| self.is_structural_at(b.ty, depth + 1))
    }

    fn fields_in(&self, def: DeclId) -> Vec<DeclId> {
        self.ast.members(def).filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field).collect()
    }

    /// clang's drill-down into why class type `ty` is not structural
    /// (`RequireStructuralType`): a local reason (a non-public or mutable
    /// member, an rvalue reference member, a non-public base) ends the
    /// walk, else the first non-structural subobject is named and the
    /// walk continues into its type.
    pub(crate) fn not_structural_notes(&self, ty: QualType) -> Vec<Diag> {
        let mut notes = Vec::new();
        let mut ty = ty;
        for _ in 0..MAX_DEPTH {
            let t = &self.ast.types;
            let Some(def) = t.as_record(ty).and_then(|r| self.ast.definition(r)) else { break };
            let shown = self.ty_arg(ty);
            let fields = self.fields_in(def);
            for &f in &fields {
                let n = *self.ast.decls.get(f);
                let note = if n.misc().access() != Access::Public {
                    Some(
                        self.diag(DiagId::note_not_structural_non_public, n.loc())
                            .arg(&shown)
                            .arg(0u8),
                    )
                } else if self.ast.decls.field(f).is_some_and(kiln_ast::decl::FieldData::is_mutable)
                {
                    Some(self.diag(DiagId::note_not_structural_mutable_field, n.loc()).arg(&shown))
                } else if n.ty().is_some_and(|q| {
                    matches!(self.ast.types.canonical_view(q), TypeView::RValueRef(_))
                }) {
                    Some(
                        self.diag(DiagId::note_not_structural_rvalue_ref_field, n.loc())
                            .arg(&shown),
                    )
                } else {
                    None
                };
                if let Some(note) = note {
                    notes.push(note);
                    return notes;
                }
            }
            let bases: Vec<_> = self.ast.decls.bases(def, &self.ast.lists).copied().collect();
            for b in &bases {
                if self.base_access(def, b.access) != Access::Public {
                    notes.push(
                        self.diag(DiagId::note_not_structural_non_public, b.loc)
                            .arg(&shown)
                            .arg(1u8),
                    );
                    return notes;
                }
            }
            let sub = fields
                .iter()
                .find_map(|&f| {
                    let n = self.ast.decls.get(f);
                    let q = self.base_element(n.ty()?);
                    (!self.is_structural(q)).then_some((n.loc(), q, 0u8))
                })
                .or_else(|| {
                    bases
                        .iter()
                        .find_map(|b| (!self.is_structural(b.ty)).then_some((b.loc, b.ty, 1u8)))
                });
            let Some((at, q, kind)) = sub else { break };
            notes.push(
                self.diag(DiagId::note_not_structural_subobject, at)
                    .arg(&shown)
                    .arg(kind)
                    .arg(self.ty_arg(q)),
            );
            ty = q;
        }
        notes
    }

    /// A template argument `e` for a non-type parameter of class type
    /// `pty` ([temp.arg.nontype]/2): a `const T` copy-initialized from
    /// `e` as a converted constant expression, evaluated to a frozen
    /// aggregate value whose template parameter object is created here
    /// ([temp.param]/8: one object per type and value).
    pub(crate) fn convert_class_nttp_arg(
        &mut self,
        pty: QualType,
        e: ExprId,
        at: Loc,
    ) -> Result<TemplateArg, ()> {
        if !self.check_nttp_type(pty, at) {
            return Err(());
        }
        let canon = self.ast.types.canonical(pty);
        let canon = crate::conv::strip_cv(self, canon);
        let cty = self.ast.types.qualify(canon, Quals::CONST);
        let c = self.perform_init(cty, Initializer::Expr(e), at, false);
        if self.is_err_expr(c) || self.has_errors_in(c) {
            return Err(());
        }
        let c = self.act_on_constant_full_expr(c);
        match self.eval_const(c, EvalContext::converted(at)) {
            Ok(v) => {
                self.builder().template_param_object(cty, v);
                Ok(TemplateArg::value(canon, v))
            }
            Err(f) => {
                let d = self.diag(DiagId::sema_expr_not_cce, at).arg("non-type template argument");
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                Err(())
            }
        }
    }

    /// Clang 22 (`CheckConvertedConstantExpression` called from
    /// `CheckTemplateArgument`) follows a conversion error in a non-type
    /// template argument with "template parameter is declared here" at
    /// the parameter `ploc`; clang 19 does not.
    pub(crate) fn note_nttp_param(&self, d: Diag, ploc: Option<Loc>) -> Diag {
        match ploc {
            Some(l) if self.clang_compat >= 22 => {
                d.note(DiagId::note_template_param_here, kiln_source::Span::empty(l))
            }
            _ => d,
        }
    }

    /// Converts argument `e` of a reference-typed non-type template
    /// parameter `pty` ([temp.arg.nontype]/2): the reference is bound as
    /// in copy-initialization and the result must be a constant
    /// expression designating an object or function with static storage;
    /// the argument value is that address (a pointer value), keyed at the
    /// canonical reference type.
    pub(crate) fn convert_ref_nttp_arg(
        &mut self,
        pty: QualType,
        e: ExprId,
        at: Loc,
        ploc: Option<Loc>,
    ) -> Result<TemplateArg, ()> {
        if !self.check_nttp_type(pty, at) {
            return Err(());
        }
        let canon = self.ast.types.canonical(pty);
        // clang `CheckConvertedConstantExpression`: the conversion must
        // exist and must not bind the reference to a temporary.
        let info = self.arg_info(e);
        let ety = self.ety(e);
        if self.try_implicit_conversion(&info, pty, IcsOptions::NONE).is_bad() {
            let d = self
                .diag(DiagId::sema_cce_not_implicitly_convertible, at)
                .arg(self.ty_arg(ety))
                .arg(self.ty_arg(pty));
            let d = self.note_nttp_param(d, ploc);
            self.emit(d);
            return Err(());
        }
        let c = self.perform_init(pty, Initializer::Expr(e), at, false);
        if self.is_err_expr(c) || self.has_errors_in(c) {
            return Err(());
        }
        let mut inner = c;
        for _ in 0..MAX_DEPTH {
            match self.ast.exprs.get(inner).kind() {
                ExprKind::ImplicitCastExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::ConstantExpr => {
                    inner = self.ast.exprs.operand(inner);
                }
                _ => break,
            }
        }
        if self.ast.exprs.get(inner).kind() == ExprKind::MaterializeTemporaryExpr {
            let d = self
                .diag(DiagId::sema_cce_ref_binds_temporary, at)
                .arg(self.ty_arg(ety))
                .arg(self.ty_arg(pty));
            let d = self.note_nttp_param(d, ploc);
            self.emit(d);
            return Err(());
        }
        // The value is the referent's address: evaluate the built-in `&`
        // of the bound glvalue (never an overloaded `operator&`).
        let rt = self.ety(c);
        let pt = self.ast.types.pointer_to(rt);
        let loc = self.ast.exprs.get(c).loc();
        let a = self.builder().unary(UnaryOp::AddrOf, c, pt, loc, ExprFlags::NONE);
        let a = self.act_on_constant_full_expr(a);
        let r = self.eval_const(a, EvalContext::converted(at));
        // The Oven reports the address of an automatic variable as a
        // pointer: here it is the reference's referent.
        let r = r.map_err(|mut f| {
            if let Some(n) = f.notes.first_mut()
                && n.msg.starts_with("pointer to '")
            {
                n.msg.replace_range(.."pointer".len(), "reference");
            }
            f
        });
        let r = r.and_then(|v| {
            // [temp.arg.nontype]/3: the referent has static storage
            // duration (an automatic variable's address is not constant).
            match self.ast.consts.scalar(v) {
                Some(Scalar::Pointer { base: PtrBase::Decl(d), .. }) if self.is_automatic(d) => {
                    let n = format!(
                        "reference to '{}' is not a constant expression",
                        self.decl_name(d)
                    );
                    let notes = vec![
                        Note { loc: at, msg: n },
                        Note { loc: self.ast.decls.get(d).loc(), msg: "declared here".into() },
                    ];
                    Err(EvalFailure { kind: FailKind::NotConstant, notes })
                }
                // Re-keyed at the reference type: printed without `&`
                // (`T<gi>`), as clang.
                Some(Scalar::Pointer { base, offset, designated, .. }) => Ok(self
                    .ast
                    .consts
                    .push_scalar(Scalar::Pointer { ty: canon, base, offset, designated })
                    .unwrap_or(v)),
                _ => Ok(v),
            }
        });
        match r {
            Ok(v) => Ok(TemplateArg::value(canon, v)),
            Err(f) => {
                let d = self.diag(DiagId::sema_expr_not_cce, at).arg("non-type template argument");
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                Err(())
            }
        }
    }

    /// A `void *` argument pointing to the first element of an array
    /// variable is the same argument as one pointing to the array (clang's
    /// `CheckTemplateArgument` ignores the array-to-pointer decay and
    /// keys both by the declaration): its designated type is the
    /// variable's ([temp.type]/2.4, [temp.arg.nontype]).
    pub(crate) fn undecayed_void_ptr_arg(&mut self, v: ConstValueId) -> ConstValueId {
        let Some(Scalar::Pointer { ty, base: PtrBase::Decl(d), offset: 0, designated: Some(t) }) =
            self.ast.consts.scalar(v)
        else {
            return v;
        };
        let types = &self.ast.types;
        let Some(whole) = self.ast.decls.get(d).ty().map(|q| types.canonical(q)) else {
            return v;
        };
        let unq = |q: QualType| types.canonical(q).without_quals();
        match types.element(whole) {
            Some(e) if types.is_array(whole) && (unq(e) == unq(t) || unq(whole) == unq(t)) => {
                let s = Scalar::Pointer {
                    ty,
                    base: PtrBase::Decl(d),
                    offset: 0,
                    designated: Some(whole),
                };
                self.ast.consts.push_scalar(s).unwrap_or(v)
            }
            _ => v,
        }
    }

    /// Whether `d` is a variable or parameter with automatic storage
    /// duration.
    fn is_automatic(&self, d: DeclId) -> bool {
        let n = self.ast.decls.get(d);
        match n.kind() {
            DeclKind::ParmVar => true,
            DeclKind::Var => {
                self.ast.decls.var(d).is_some_and(|v| {
                    matches!(v.spec.storage(), VarStorage::None | VarStorage::Register)
                }) && n.ctx().is_some_and(|c| self.ast.decls.get(c).kind().is_function())
            }
            _ => false,
        }
    }

    /// The expression a non-type template argument value stands for
    /// when substituted (clang's
    /// `BuildExpressionFromNonTypeTemplateArgument`): a literal of an
    /// integral or enumeration value, else [`Self::nttp_value_expr`].
    /// `None` for values not yet supported.
    pub(crate) fn nttp_arg_expr(
        &mut self,
        ty: QualType,
        value: ConstValueId,
        loc: Loc,
    ) -> Option<ExprId> {
        // A wide `_BitInt` (any value), or an integer outside `i128` (an
        // `unsigned __int128` above `i128::MAX`): a literal of the
        // argument's exact bits.
        let wide = matches!(
            self.ast.types.canonical_view(ty),
            TypeView::BitInt { width, .. } if width > 128
        );
        let int = matches!(self.ast.consts.scalar(value), Some(Scalar::Int { .. }));
        if int && self.ast.types.as_enum(ty).is_none() && (wide || self.const_int(value).is_none())
        {
            return Some(self.ast.builder().wide_int_literal(ty, value, Radix::Decimal, loc));
        }
        let Some(v) = self.const_int(value) else {
            // Class, pointer and null values (§6.10).
            return self.nttp_value_expr(ty, value, loc);
        };
        let t = &self.ast.types;
        if let Some(en) = t.as_enum(ty) {
            let it =
                self.ast.decls.enumeration(en).and_then(|d| d.int_type).unwrap_or(QualType::INT);
            let lit = self.nttp_int_literal(it, v, loc);
            return Some(self.ast.builder().cast(
                ExprKind::CStyleCastExpr,
                CastKind::IntegralCast,
                lit,
                ty,
                ValueCategory::PRValue,
                None,
                loc,
                ExprFlags::NONE,
            ));
        }
        let kind = match t.builtin(ty) {
            Some(BuiltinType::Bool) => {
                return Some(self.ast.builder().bool_literal(ty, v != 0, loc));
            }
            Some(BuiltinType::Char | BuiltinType::SChar | BuiltinType::UChar) => {
                Some(CharKind::Ascii)
            }
            Some(BuiltinType::WChar) => Some(CharKind::Wide),
            Some(BuiltinType::Char8) => Some(CharKind::Utf8),
            Some(BuiltinType::Char16) => Some(CharKind::Utf16),
            Some(BuiltinType::Char32) => Some(CharKind::Utf32),
            Some(_) => None,
            None if matches!(t.canonical_view(ty), TypeView::BitInt { .. }) => None,
            None => return None,
        };
        if let Some(kind) = kind {
            let u = u32::try_from(v & 0xffff_ffff).unwrap_or(0);
            return Some(self.ast.builder().char_literal(ty, kind, u, loc));
        }
        Some(self.nttp_int_literal(ty, v, loc))
    }

    /// The integer literal of `v` at type `ty`.
    pub(crate) fn nttp_int_literal(&mut self, ty: QualType, v: i128, loc: Loc) -> ExprId {
        if let Ok(u) = u64::try_from(v) {
            self.ast.builder().int_literal(ty, u, Radix::Decimal, loc)
        } else {
            let c = self.int_const(ty, v);
            self.ast.builder().wide_int_literal(ty, c, Radix::Decimal, loc)
        }
    }

    /// The expression a non-type template argument value of class,
    /// pointer or null type stands for when substituted (clang's
    /// `BuildExpressionFromNonTypeTemplateArgument`): an lvalue naming
    /// the template parameter object, `&x` / `&f` / `f`, or a null
    /// pointer converted to `ty`. `None` for values handled elsewhere
    /// (integers) or not yet supported.
    #[allow(clippy::too_many_lines, reason = "one arm per `Scalar` kind of the value")]
    pub(crate) fn nttp_value_expr(
        &mut self,
        ty: QualType,
        v: ConstValueId,
        loc: Loc,
    ) -> Option<ExprId> {
        if self.ast.types.as_record(ty).is_some() {
            // The template parameter object holding `v` ([temp.param]/8),
            // made here for a value that did not come from a template
            // argument (a spliced value reflection, WP-550).
            let d = if let Some(d) = self.ast.templates.param_object(v) {
                d
            } else {
                let canon = self.ast.types.canonical(ty);
                let canon = crate::conv::strip_cv(self, canon);
                let cty = self.ast.types.qualify(canon, Quals::CONST);
                self.builder().template_param_object(cty, v)
            };
            let q = self.ast.decls.get(d).ty()?;
            self.ast.decls.set_flags(d, DeclFlags::REFERENCED);
            return Some(self.builder().decl_ref(
                d,
                q,
                loc,
                None,
                NonOdrUse::None,
                ExprFlags::NONE,
            ));
        }
        match self.ast.consts.scalar(v)? {
            // A `std::meta::info` argument (-freflection spike): the
            // reflect-expression designating the same entity.
            Scalar::Reflection { id, .. } => {
                let r = self.ast.reflections.get(id);
                Some(self.builder().reflect_expr(r, ty, loc, loc))
            }
            // A floating-point argument ([temp.arg.nontype]/1): the
            // literal of its value (clang `FloatingLiteral::Create`).
            Scalar::Float { .. } => Some(self.builder().float_literal(ty, v, loc)),
            Scalar::Null { .. } => {
                let n = self.builder().nullptr_literal(QualType::NULLPTR, loc);
                if self.is_nullptr_type(ty) {
                    return Some(n);
                }
                crate::conv::implicit_convert(self, n, ty, crate::conv::ConvCtx::Initialization)
                    .ok()
            }
            // A pointer into an object with static storage and no
            // declaration (a `define_static_array` element, P3491 §3.5.1):
            // the constant itself, which no expression of the program
            // spells.
            Scalar::Pointer { base: PtrBase::Const(_), .. } => {
                let o = self.builder().opaque_value_of(ty, ValueCategory::PRValue, loc);
                Some(self.builder().constant_expr(o, Some(v)))
            }
            Scalar::Pointer { ty: pty, base, offset, .. }
                if self.is_subobject_ptr(pty, base, offset) =>
            {
                self.subobject_arg_expr(ty, pty, base, offset, loc)
            }
            Scalar::Pointer { base, offset: 0, .. } => {
                let (PtrBase::Decl(d) | PtrBase::Function(d)) = base else { return None };
                let r = if self.kind_of(d) == DeclKind::TemplateParamObject {
                    let q = self.ast.decls.get(d).ty()?;
                    self.builder().decl_ref(d, q, loc, None, NonOdrUse::None, ExprFlags::NONE)
                } else {
                    self.build_decl_ref(d, loc)
                };
                let t = &self.ast.types;
                if t.is_reference(ty) {
                    return Some(r);
                }
                let rt = self.ety(r);
                // A pointer to the array itself (`int (*)[2]`, the argument
                // `&arr`) takes its address below.
                let to_array =
                    self.ast.types.pointee(ty).is_some_and(|p| self.ast.types.is_array(p));
                if self.ast.types.is_array(rt) && !to_array {
                    // `arr` as a pointer argument: the array decays ([conv.array]).
                    return crate::conv::implicit_convert(
                        self,
                        r,
                        ty,
                        crate::conv::ConvCtx::Initialization,
                    )
                    .ok();
                }
                let a = self.act_on_unary(UnaryOp::AddrOf, r, loc);
                let at = self.ety(a);
                if self.ast.types.canonical(at) == self.ast.types.canonical(ty) {
                    Some(a)
                } else {
                    crate::conv::implicit_convert(self, a, ty, crate::conv::ConvCtx::Initialization)
                        .ok()
                }
            }
            // A non-null pointer to member: `&C::m` of the member's own
            // class, converted to the parameter's type along the value's
            // base/derived path, by a `static_cast` towards a base (clang
            // `BuildExpressionFromDeclTemplateArgument`).
            Scalar::MemberPtr { member, derived, .. } => {
                let class = self.declaring_class(member)?;
                let declared = self.ast.decls.get(member).ty()?;
                let r = self.builder().decl_ref(
                    member,
                    declared,
                    loc,
                    None,
                    NonOdrUse::None,
                    ExprFlags::NONE,
                );
                let class_ty = self.tag_type(class).ty();
                let mp = self.ast.types.member_pointer(declared, class_ty);
                let a =
                    self.builder().unary(UnaryOp::AddrOf, r, mp, loc, ExprFlags::CANNOT_OVERFLOW);
                if self.ast.types.canonical(mp) == self.ast.types.canonical(ty) {
                    Some(a)
                } else if derived {
                    let c = self.act_on_cast(crate::expr::CastStyle::Static, ty, a, loc, loc);
                    (!self.is_err_expr(c)).then_some(c)
                } else {
                    crate::conv::implicit_convert(self, a, ty, crate::conv::ConvCtx::Initialization)
                        .ok()
                }
            }
            _ => None,
        }
    }

    /// A braced-init-list template argument (P2308R1, [temp.arg.general]
    /// /1): kept as written, nested lists and designators included, until
    /// [`Sema::braced_template_arg`] gives it back to initialize the
    /// parameter.
    pub fn act_on_braced_template_arg(&mut self, l: &crate::init::InitList) -> ExprId {
        if self.list_has_expr_error(l) {
            let e = self.recover_err(&Self::list_exprs(l), l.lbrace);
            return self.mark_expr_error(e);
        }
        self.dependent_init_list(l)
    }

    /// The braced-init-list behind template argument `e`, if it is one
    /// (a `void` `InitListExpr`, as written or substituted).
    pub(crate) fn braced_template_arg(&self, e: ExprId) -> Option<crate::init::InitList> {
        use crate::init::{Designator, InitItem, InitValue};
        use kiln_ast::expr::Designator as AstDesignator;
        let ex = &self.ast.exprs;
        let n = ex.get(e);
        if n.kind() != ExprKind::InitListExpr || n.ty() != QualType::VOID {
            return None;
        }
        let kids: Vec<ExprId> = self.ast.lists.iter::<ExprId>(ex.init_list_inits(e)).collect();
        let items = kids
            .into_iter()
            .map(|x| {
                let (designator, x) = if ex.get(x).kind() == ExprKind::DesignatedInitExpr {
                    let ds = ex.designators(x).iter().map(|d| match *d {
                        AstDesignator::Field { name, loc } => Designator::Field(name, loc),
                        AstDesignator::Index { index, loc } => Designator::Index(index, loc),
                        AstDesignator::Range { first, last, loc, ellipsis } => {
                            Designator::Range(first, last, loc, ellipsis)
                        }
                    });
                    (ds.collect(), ex.designated_init(x))
                } else {
                    (Vec::new(), x)
                };
                let value = self.braced_template_arg(x).map_or(InitValue::Expr(x), InitValue::List);
                InitItem { designator, value }
            })
            .collect();
        Some(crate::init::InitList {
            items,
            lbrace: n.loc(),
            rbrace: ex.end_loc(e).unwrap_or(n.loc()),
        })
    }
}
