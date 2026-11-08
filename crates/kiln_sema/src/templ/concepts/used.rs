//! The template parameters an atomic constraint's expression names
//! ([temp.constr.atomic]/1, spec/sema-templates.md §8.2): an atom's
//! parameter mapping covers only those, so `sizeof(T) > 1` in
//! `C<T, int>` and in `C<T, long>` is one atom (clang
//! `MarkUsedTemplateParameters` over the atom's expression).
//!
//! The walk names every expression, type, template-name and
//! nested-name-specifier kind (WP-776): splices walk their operand, a
//! type's expression operand is walked, and sugar walks what it wraps.
//! Only what it cannot see answers `None` and keeps the whole mapping
//! (two atoms then compare as before, never more loosely): a body of
//! statements (a lambda, a statement expression), a dependent class or
//! enumeration (its enclosing template's parameters are implicit), and
//! a parameter index past 64.

use kiln_ast::expr::{ExprDeps, ExprKind, OffsetofComponent, TypeidOperand};
use kiln_ast::names::{DeclName, DeclNameView};
use kiln_ast::reflect::Reflectee;
use kiln_ast::templates::Requirement;
use kiln_ast::types::{
    ArgView, ExprOperand, Nns, TemplateName, TypeFlags, TypeOfOperand, TypeView,
};
use kiln_ast::{ArgListId, DeclId, ExprId, NnsId, QualType, RawId, TemplateNameId};

use crate::Sema;

/// Deeper than this, the walk gives up (`None`).
const MAX_DEPTH: u32 = 256;

/// The parameters of depth `level` found so far, one bit per index.
struct Used {
    level: u32,
    mask: u64,
}

impl Used {
    fn mark(&mut self, depth: u32, index: u32) -> Option<()> {
        if depth == self.level {
            if index >= 64 {
                return None;
            }
            self.mask |= 1 << index;
        }
        Some(())
    }
}

impl Sema<'_, '_> {
    /// The parameters of depth `level` expression `e` names, one bit per
    /// index; `None` when the walk cannot tell (cached per expression).
    pub(crate) fn used_template_params(&mut self, e: ExprId, level: u16) -> Option<u64> {
        if let Some(&r) = self.inst.concepts.used_params.get(&(e, level)) {
            return r;
        }
        let mut u = Used { level: u32::from(level), mask: 0 };
        let r = self.used_in_expr(e, &mut u, 0).map(|()| u.mask);
        self.inst.concepts.used_params.insert((e, level), r);
        r
    }

    fn used_decl(&self, d: DeclId, u: &mut Used) -> Option<()> {
        match self.ast.template_parm(d) {
            Some(i) => u.mark(u32::from(i.depth), u32::from(i.index)),
            None => Some(()),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn used_in_expr(&self, e: ExprId, u: &mut Used, depth: u32) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        let n = *self.ast.exprs.get(e);
        if !n.deps().intersects(ExprDeps::TYPE_VALUE_INST)
            && !n.deps().contains(ExprDeps::UNEXPANDED_PACK)
        {
            return Some(());
        }
        let d = depth + 1;
        let ex = &self.ast.exprs;
        // Every kind is listed: what a node holds besides its children,
        // its type and its written template arguments and qualifier (all
        // walked below) is walked here.
        match n.kind() {
            ExprKind::IntegerLiteral
            | ExprKind::FloatingLiteral
            | ExprKind::CharacterLiteral
            | ExprKind::StringLiteral
            | ExprKind::CXXBoolLiteralExpr
            | ExprKind::CXXNullPtrLiteralExpr
            | ExprKind::UserDefinedLiteral
            | ExprKind::ImaginaryLiteral
            | ExprKind::EmbedExpr
            | ExprKind::GNUNullExpr
            | ExprKind::MemberExpr
            | ExprKind::CXXThisExpr
            | ExprKind::PredefinedExpr
            | ExprKind::SourceLocExpr
            | ExprKind::UnresolvedMemberExpr
            | ExprKind::SubstNonTypeTemplateParmExpr
            | ExprKind::UnaryOperator
            | ExprKind::BinaryOperator
            | ExprKind::CompoundAssignOperator
            | ExprKind::ConditionalOperator
            | ExprKind::BinaryConditionalOperator
            | ExprKind::ArraySubscriptExpr
            | ExprKind::ParenExpr
            | ExprKind::CallExpr
            | ExprKind::CXXMemberCallExpr
            | ExprKind::CXXOperatorCallExpr
            | ExprKind::CXXRewrittenBinaryOperator
            | ExprKind::ParenListExpr
            | ExprKind::InitListExpr
            | ExprKind::ImplicitValueInitExpr
            | ExprKind::DesignatedInitExpr
            | ExprKind::CXXConstructExpr
            | ExprKind::CXXTemporaryObjectExpr
            | ExprKind::CXXInheritedCtorInitExpr
            | ExprKind::CXXScalarValueInitExpr
            | ExprKind::CXXParenListInitExpr
            | ExprKind::ArrayInitLoopExpr
            | ExprKind::ArrayInitIndexExpr
            | ExprKind::CXXStdInitializerListExpr
            | ExprKind::CompoundLiteralExpr
            | ExprKind::NoInitExpr
            | ExprKind::DesignatedInitUpdateExpr
            | ExprKind::MaterializeTemporaryExpr
            | ExprKind::CXXBindTemporaryExpr
            | ExprKind::ExprWithCleanups
            | ExprKind::CXXDeleteExpr
            | ExprKind::CXXThrowExpr
            | ExprKind::CXXNoexceptExpr
            | ExprKind::ExpressionTraitExpr
            | ExprKind::PackExpansionExpr
            | ExprKind::CXXFoldExpr
            | ExprKind::CXXUnresolvedConstructExpr
            | ExprKind::CoawaitExpr
            | ExprKind::DependentCoawaitExpr
            | ExprKind::CoyieldExpr
            | ExprKind::ConstantExpr
            | ExprKind::AddrLabelExpr
            | ExprKind::ChooseExpr
            | ExprKind::AtomicExpr
            | ExprKind::ShuffleVectorExpr
            | ExprKind::ConvertVectorExpr
            | ExprKind::ExtVectorElementExpr
            | ExprKind::RecoveryExpr
            | ExprKind::PseudoObjectExpr
            // The default is the parameter's or member's, already
            // substituted: what it names is not this expression's.
            | ExprKind::CXXDefaultArgExpr
            | ExprKind::CXXDefaultInitExpr => {}
            ExprKind::ImplicitCastExpr
            | ExprKind::CStyleCastExpr
            | ExprKind::CXXFunctionalCastExpr
            | ExprKind::CXXStaticCastExpr
            | ExprKind::CXXConstCastExpr
            | ExprKind::CXXReinterpretCastExpr
            | ExprKind::CXXDynamicCastExpr
            | ExprKind::BuiltinBitCastExpr => {
                if let Some(t) = ex.cast_written_type(e) {
                    self.used_in_type(t, u, d)?;
                }
            }
            ExprKind::DeclRefExpr => self.used_decl(ex.decl(e), u)?,
            ExprKind::SizeOfPackExpr => self.used_decl(DeclId::from_raw(n.a())?, u)?,
            ExprKind::UnresolvedLookupExpr => {
                let l = kiln_ast::ids::ListId::new(n.b());
                for p in self.ast.lists.iter::<DeclId>(l) {
                    self.used_decl(p, u)?;
                }
                if let Some(p) = self.var_ttp_ref(e) {
                    self.used_decl(p, u)?;
                }
            }
            ExprKind::DependentScopeDeclRefExpr => {
                if let Some(q) = NnsId::from_raw(n.a()) {
                    self.used_in_nns(q, u, d)?;
                }
                self.used_in_name(self.ast.dependent_scope_ref_name(e), u, d)?;
            }
            ExprKind::CXXDependentScopeMemberExpr => {
                self.used_in_name(self.ast.dependent_member_name(e), u, d)?;
            }
            ExprKind::SubstNonTypeTemplateParmPackExpr => {
                self.used_in_args(ex.subst_pack_args(e), u, d)?;
            }
            ExprKind::FunctionParmPackExpr => {
                for p in self.ast.lists.iter::<DeclId>(ex.parm_pack_params(e)) {
                    if let Some(t) = self.ast.decls.get(p).ty() {
                        self.used_in_type(t, u, d)?;
                    }
                }
            }
            ExprKind::OpaqueValueExpr => {
                if let Some(s) = ex.opaque_source(e) {
                    self.used_in_expr(s, u, d)?;
                }
            }
            ExprKind::PackIndexingExpr => {
                for k in [n.a(), n.b()].into_iter().filter_map(ExprId::from_raw) {
                    self.used_in_expr(k, u, d)?;
                }
            }
            ExprKind::CXXPseudoDestructorExpr => {
                self.used_in_type(ex.pseudo_destructor(e).2, u, d)?;
            }
            ExprKind::CXXNewExpr => self.used_in_type(ex.new_extra(e).allocated_ty, u, d)?,
            ExprKind::CXXTypeidExpr => {
                if let TypeidOperand::Type(t) = ex.typeid_operand(e) {
                    self.used_in_type(t, u, d)?;
                }
            }
            ExprKind::UnaryExprOrTypeTraitExpr => {
                if let Some(t) = ex.trait_type_arg(e) {
                    self.used_in_type(t, u, d)?;
                }
            }
            ExprKind::TypeTraitExpr => {
                for t in self.ast.lists.iter::<QualType>(ex.type_trait_args(e)) {
                    self.used_in_type(t, u, d)?;
                }
            }
            ExprKind::ArrayTypeTraitExpr => {
                self.used_in_type(ex.array_trait_type(e), u, d)?;
                if let Some(dim) = ex.array_trait(e).dim {
                    self.used_in_expr(dim, u, d)?;
                }
            }
            ExprKind::OffsetOfExpr => {
                if let Some((t, comps)) = ex.offsetof_pattern(e) {
                    self.used_in_type(t, u, d)?;
                    for c in comps {
                        if let OffsetofComponent::Index { index, .. } = *c {
                            self.used_in_expr(index, u, d)?;
                        }
                    }
                }
            }
            ExprKind::VAArgExpr => self.used_in_type(ex.va_arg(e).1, u, d)?,
            ExprKind::CXXReflectExpr => match ex.reflect_operand(e) {
                Reflectee::Type(t) => self.used_in_type(t, u, d)?,
                Reflectee::Decl(p) => self.used_decl(p, u)?,
                Reflectee::Template(t) => self.used_in_template_name(t, u, d)?,
                // A dependent qualified name is the child; the others
                // are values naming no parameter.
                Reflectee::Null
                | Reflectee::Namespace(_)
                | Reflectee::NamespaceAlias(_)
                | Reflectee::Value(_)
                | Reflectee::Object(_)
                | Reflectee::BaseSpec(..)
                | Reflectee::DataMemberSpec(_)
                | Reflectee::Annotation(_)
                | Reflectee::FnParam(..) => {}
            },
            ExprKind::CXXSpliceExpr => {
                if let Some(l) = ex.splice_template_args(e) {
                    self.used_in_args(l, u, d)?;
                }
            }
            ExprKind::ConceptSpecializationExpr => {
                self.used_decl(DeclId::from_raw(n.a())?, u)?;
                self.used_in_args(ArgListId::from_raw(n.b())?, u, d)?;
            }
            ExprKind::RequiresExpr => {
                let rq = self.ast.requires_of(e)?;
                for p in self.ast.lists.iter::<DeclId>(rq.params) {
                    if let Some(t) = self.ast.decls.get(p).ty() {
                        self.used_in_type(t, u, d)?;
                    }
                }
                for r in self.ast.templates.requirements(rq) {
                    match *r {
                        Requirement::Simple(x) | Requirement::Nested(x) => {
                            self.used_in_expr(x, u, d)?;
                        }
                        Requirement::Compound { expr, constraint, .. } => {
                            self.used_in_expr(expr, u, d)?;
                            if let Some(c) = constraint {
                                self.used_in_expr(c, u, d)?;
                            }
                        }
                        Requirement::Type(t, _) => self.used_in_type(t, u, d)?,
                    }
                }
                return Some(());
            }
            // A body of statements (a lambda's, a statement expression's)
            // and an expansion statement's element are not walked: the
            // atom keeps the whole mapping.
            ExprKind::LambdaExpr | ExprKind::StmtExpr | ExprKind::CXXExpansionSelectExpr => {
                return None;
            }
        }
        if let Some(&l) = self.ast.templates.expr_targs.get(e) {
            self.used_in_args(l, u, d)?;
        }
        if let Some(&q) = self.ast.templates.expr_nns.get(e) {
            self.used_in_nns(q, u, d)?;
        }
        for k in ex.children(e, &self.ast.lists) {
            self.used_in_expr(k, u, d)?;
        }
        // A written type the node keeps as its own (`T()`, `T{}`), or a
        // declaration's type.
        self.used_in_type(n.ty(), u, d)
    }

    /// The types a member or qualified name holds: `operator T`, `~T`.
    fn used_in_name(&self, name: DeclName, u: &mut Used, depth: u32) -> Option<()> {
        match self.ast.names.view(name) {
            DeclNameView::Conversion(t) => self.used_in_type(t, u, depth + 1),
            DeclNameView::Constructor(t) | DeclNameView::Destructor(t) => {
                self.used_in_type(QualType::unqual(t), u, depth + 1)
            }
            DeclNameView::Empty
            | DeclNameView::Identifier(_)
            | DeclNameView::Operator(_)
            | DeclNameView::LiteralOperator(_)
            | DeclNameView::DeductionGuide(_) => Some(()),
        }
    }

    fn used_in_args(&self, l: ArgListId, u: &mut Used, depth: u32) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        for &a in self.ast.types.args(l) {
            match a.view() {
                ArgView::Null | ArgView::Value { .. } => {}
                ArgView::Type(q) => self.used_in_type(q, u, depth + 1)?,
                ArgView::Expr { expr, .. } => self.used_in_expr(expr, u, depth + 1)?,
                ArgView::Profile(p) => {
                    self.used_in_expr(self.ast.types.profile_expr(p)?, u, depth + 1)?;
                }
                ArgView::Template(t) | ArgView::TemplateExpansion { name: t, .. } => {
                    self.used_in_template_name(t, u, depth + 1)?;
                }
                ArgView::Pack(l) => self.used_in_args(l, u, depth + 1)?,
            }
        }
        Some(())
    }

    fn used_in_template_name(&self, t: TemplateNameId, u: &mut Used, depth: u32) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        let d = depth + 1;
        match self.ast.types.template_name_view(t) {
            TemplateName::Template(p) => self.used_decl(p, u),
            TemplateName::TemplateTemplateParm { depth: pd, index, .. }
            | TemplateName::CanonTemplateTemplateParm { depth: pd, index, .. } => u.mark(pd, index),
            TemplateName::Qualified { nns, underlying, .. } => {
                self.used_in_nns(nns, u, d)?;
                self.used_in_template_name(underlying, u, d)
            }
            TemplateName::DependentIdentifier { nns, .. }
            | TemplateName::DependentOperator { nns, .. } => self.used_in_nns(nns, u, d),
            TemplateName::SubstTemplateTemplateParm { replacement: x, .. }
            | TemplateName::Using { underlying: x, .. } => self.used_in_template_name(x, u, d),
            TemplateName::SubstTemplateTemplateParmPack { args, .. } => {
                self.used_in_args(args, u, d)
            }
            TemplateName::Overloaded(_) => Some(()),
            TemplateName::Splice { expr, .. } => self.used_in_expr(expr, u, d),
            TemplateName::SpliceProfile(p) => {
                self.used_in_expr(self.ast.types.profile_expr(p)?, u, d)
            }
        }
    }

    fn used_in_nns(&self, n: NnsId, u: &mut Used, depth: u32) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        match self.ast.types.nns_view(n) {
            Nns::Global | Nns::Namespace { .. } | Nns::NamespaceAlias { .. } => Some(()),
            Nns::Type { prefix, ty, .. } => {
                if let Some(p) = prefix {
                    self.used_in_nns(p, u, depth + 1)?;
                }
                self.used_in_type(ty, u, depth + 1)
            }
            Nns::Identifier { prefix, .. } => match prefix {
                Some(p) => self.used_in_nns(p, u, depth + 1),
                None => Some(()),
            },
            // `[: r :]::` names what its operand names.
            Nns::Splice(e) => self.used_in_expr(e, u, depth + 1),
        }
    }

    /// An expression operand of a type (a sugar node's expression or the
    /// canonical node's profile).
    fn used_in_operand(&self, o: ExprOperand, u: &mut Used, depth: u32) -> Option<()> {
        let e = match o {
            ExprOperand::Expr(e) => e,
            ExprOperand::Profile(p) => self.ast.types.profile_expr(p)?,
        };
        self.used_in_expr(e, u, depth)
    }

    #[allow(clippy::too_many_lines)]
    fn used_in_type(&self, q: QualType, u: &mut Used, depth: u32) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        let t = &self.ast.types;
        if !t.flags(q.ty()).intersects(TypeFlags::DEPENDENT_INSTANTIATION) {
            return Some(());
        }
        let d = depth + 1;
        match t.view(q.ty()) {
            TypeView::Builtin(_) | TypeView::BitInt { .. } => Some(()),
            // A dependent class or enumeration (a member of a class
            // template) depends on its enclosing template's parameters,
            // which the type does not name; an unresolved using-declaration
            // likewise. The atom keeps the whole mapping.
            TypeView::Invalid
            | TypeView::Record(_)
            | TypeView::Enum(_)
            | TypeView::UnresolvedUsing(_) => None,
            TypeView::TemplateTypeParm(p) => self.used_decl(p, u),
            TypeView::CanonTemplateTypeParm { depth: pd, index, .. } => u.mark(pd, index),
            TypeView::Pointer(x)
            | TypeView::LValueRef(x)
            | TypeView::RValueRef(x)
            | TypeView::IncompleteArray(x)
            | TypeView::Paren(x)
            | TypeView::Complex(x)
            | TypeView::Atomic(x)
            | TypeView::ExtQual { base: x, .. }
            | TypeView::Attributed { modified: x, .. }
            | TypeView::Vector { elem: x, .. }
            | TypeView::PackExpansion { pattern: x, .. }
            | TypeView::SubstTemplateTypeParm { replacement: x, .. }
            | TypeView::UnaryTransform { base: x, .. }
            | TypeView::InjectedClassName { injected: x, .. }
            | TypeView::ConstantArray(kiln_ast::types::ConstArray { elem: x, .. }) => {
                self.used_in_type(x, u, d)
            }
            TypeView::QualifiedArray { base, .. } => {
                self.used_in_type(QualType::unqual(base), u, d)
            }
            TypeView::Elaborated { nns, named, .. } => {
                if let Some(n) = nns {
                    self.used_in_nns(n, u, d)?;
                }
                self.used_in_type(named, u, d)
            }
            TypeView::Typedef { .. } | TypeView::Using { .. } => {
                self.used_in_type(t.canonical(q), u, d)
            }
            TypeView::SubstTemplateTypeParmPack { pack, .. } => self.used_in_args(pack, u, d),
            TypeView::MemberPointer { pointee, class } => {
                self.used_in_type(QualType::unqual(class), u, d)?;
                self.used_in_type(pointee, u, d)
            }
            TypeView::VariableArray { elem, size } => {
                self.used_in_type(elem, u, d)?;
                self.used_in_expr(size, u, d)
            }
            TypeView::DependentSizedArray { elem, size }
            | TypeView::DependentVector { elem, size, .. }
            | TypeView::PackIndexing { pattern: elem, index: size, .. } => {
                self.used_in_type(elem, u, d)?;
                self.used_in_operand(size, u, d)
            }
            TypeView::DependentBitInt { width: p, .. } | TypeView::DependentDecltype(p) => {
                self.used_in_operand(ExprOperand::Profile(p), u, d)
            }
            TypeView::Decltype { expr, .. } => self.used_in_expr(expr, u, d),
            TypeView::TypeOf { operand, .. } => match operand {
                TypeOfOperand::Expr(e) => self.used_in_expr(e, u, d),
                TypeOfOperand::Type(x) => self.used_in_type(x, u, d),
                TypeOfOperand::Profile(p) => self.used_in_operand(ExprOperand::Profile(p), u, d),
            },
            TypeView::Function(p) => {
                self.used_in_type(p.ret(), u, d)?;
                for &x in t.params(&p) {
                    self.used_in_type(x, u, d)?;
                }
                Some(())
            }
            TypeView::DependentName { nns, .. } => self.used_in_nns(nns, u, d),
            TypeView::TemplateSpecialization { name, args, .. } => {
                self.used_in_template_name(name, u, d)?;
                self.used_in_args(args, u, d)
            }
            TypeView::DependentTemplateSpecialization { nns, args, .. } => {
                self.used_in_nns(nns, u, d)?;
                self.used_in_args(args, u, d)
            }
            TypeView::Auto { deduced, concept, args, .. } => {
                if let Some(x) = deduced {
                    return self.used_in_type(x, u, d);
                }
                if let Some(c) = concept {
                    self.used_decl(c, u)?;
                }
                match args {
                    Some(l) => self.used_in_args(l, u, d),
                    None => Some(()),
                }
            }
            TypeView::DeducedTemplateSpecialization { name, deduced } => {
                self.used_in_template_name(name, u, d)?;
                match deduced {
                    Some(x) => self.used_in_type(x, u, d),
                    None => Some(()),
                }
            }
            TypeView::Splice { operand, designated } => {
                self.used_in_operand(operand, u, d)?;
                match designated {
                    Some(x) => self.used_in_type(x, u, d),
                    None => Some(()),
                }
            }
        }
    }
}
