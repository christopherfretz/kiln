//! Non-type template parameters of class and reference type
//! ([temp.param]/6-7, [temp.arg.nontype]; spec/sema-templates.md §6.10):
//! the structural-type check of a parameter's type, and the template
//! parameter objects ([temp.param]/8) class-type arguments designate.

use kiln_ast::consts::{PtrBase, Scalar};
use kiln_ast::decl::{Access, DeclFlags, DeclKind, DefinitionBits};
use kiln_ast::expr::{ExprFlags, NonOdrUse, UnaryOp};
use kiln_ast::types::{Quals, TemplateArg, TypeView};
use kiln_ast::{ConstValueId, DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_oven::EvalContext;
use kiln_source::Loc;

use crate::Sema;
use crate::init::Initializer;

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
        if self.is_error_type(ty) || self.is_dependent_type(ty) {
            return true;
        }
        let t = &self.ast.types;
        if matches!(t.canonical_view(ty), TypeView::RValueRef(_)) {
            let d = self.diag(DiagId::sema_nttp_rvalue_ref, loc).arg(self.ty_arg(ty));
            self.emit(d);
            return false;
        }
        let Some(r) = t.as_record(ty) else { return true };
        let Some(def) = self.ast.definition(r) else { return true };
        let bits = self.record_bits(def);
        if !bits.contains(DefinitionBits::COMPLETE) {
            return true;
        }
        if !bits.contains(DefinitionBits::LITERAL) {
            let mut d = self.diag(DiagId::sema_nttp_non_literal, loc).arg(self.ty_arg(ty));
            if let Some(n) = self.non_literal_note(ty) {
                d = d.with_note(n);
            }
            self.emit(d);
            return false;
        }
        if self.is_structural(ty) {
            return true;
        }
        let mut d = self.diag(DiagId::sema_nttp_not_structural, loc).arg(self.ty_arg(ty));
        for n in self.not_structural_notes(ty) {
            d = d.with_note(n);
        }
        self.emit(d);
        false
    }

    fn record_bits(&self, def: DeclId) -> DefinitionBits {
        self.ast.decls.record(def).map(|x| x.defn).unwrap_or_default()
    }

    /// Whether `ty` is a structural type ([temp.param]/7), as clang's
    /// `Type::isStructuralType`, recomputed for classes so a member of
    /// rvalue reference type counts (the `STRUCTURAL` bit misses it).
    fn is_structural(&self, ty: QualType) -> bool {
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
    fn not_structural_notes(&self, ty: QualType) -> Vec<Diag> {
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
                } else if self.ast.decls.field(f).is_some_and(|x| x.is_mutable()) {
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

    /// Why class type `ty` is not literal ([basic.types.general]/10;
    /// clang `RequireLiteralType`'s notes), as far as kiln tracks it.
    fn non_literal_note(&self, ty: QualType) -> Option<Diag> {
        let def = self.ast.types.as_record(ty).and_then(|r| self.ast.definition(r))?;
        let bits = self.record_bits(def);
        let shown = self.ty_arg(ty);
        if !bits.contains(DefinitionBits::AGGREGATE)
            && !bits.contains(DefinitionBits::HAS_CONSTEXPR_CTOR)
            && !bits.contains(DefinitionBits::LAMBDA)
        {
            let loc = self.ast.decls.get(def).loc();
            return Some(self.diag(DiagId::note_non_literal_no_constexpr_ctors, loc).arg(shown));
        }
        for b in self.ast.decls.bases(def, &self.ast.lists) {
            if !self.is_literal_type(b.ty) {
                return Some(
                    self.diag(DiagId::note_non_literal_base_class, b.loc)
                        .arg(&shown)
                        .arg(self.ty_arg(b.ty)),
                );
            }
        }
        for f in self.fields_in(def) {
            let n = self.ast.decls.get(f);
            let q = n.ty()?;
            if !self.is_literal_type(q) {
                return Some(
                    self.diag(DiagId::note_non_literal_field, n.loc())
                        .arg(&shown)
                        .arg(self.decl_arg(f))
                        .arg(self.ty_arg(q)),
                );
            }
        }
        let dtor = self
            .ast
            .members(def)
            .find(|&m| self.ast.decls.get(m).kind() == DeclKind::CXXDestructor)?;
        let constexpr = self
            .ast
            .decls
            .function(dtor)
            .is_some_and(|f| f.spec.constexpr_kind() != kiln_ast::decl::ConstexprKind::None);
        (!constexpr).then(|| {
            self.diag(DiagId::note_non_literal_non_constexpr_dtor, self.ast.decls.get(dtor).loc())
                .arg(shown)
        })
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
        let c = self.act_on_full_expr(c);
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

    /// The expression a non-type template argument value of class,
    /// pointer or null type stands for when substituted (clang's
    /// `BuildExpressionFromNonTypeTemplateArgument`): an lvalue naming
    /// the template parameter object, `&x` / `&f` / `f`, or a null
    /// pointer converted to `ty`. `None` for values handled elsewhere
    /// (integers) or not yet supported.
    pub(crate) fn nttp_value_expr(
        &mut self,
        ty: QualType,
        v: ConstValueId,
        loc: Loc,
    ) -> Option<ExprId> {
        if self.ast.types.as_record(ty).is_some() {
            let d = self.ast.templates.param_object(v)?;
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
            Scalar::Null { .. } => {
                let n = self.builder().nullptr_literal(QualType::NULLPTR, loc);
                if self.is_nullptr_type(ty) {
                    return Some(n);
                }
                crate::conv::implicit_convert(self, n, ty, crate::conv::ConvCtx::Initialization)
                    .ok()
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
                let a = self.act_on_unary(UnaryOp::AddrOf, r, loc);
                let at = self.ety(a);
                if self.ast.types.canonical(at) == self.ast.types.canonical(ty) {
                    Some(a)
                } else {
                    crate::conv::implicit_convert(self, a, ty, crate::conv::ConvCtx::Initialization)
                        .ok()
                }
            }
            _ => None,
        }
    }
}
