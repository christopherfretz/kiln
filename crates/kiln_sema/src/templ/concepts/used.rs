//! The template parameters an atomic constraint's expression names
//! ([temp.constr.atomic]/1, spec/sema-templates.md §8.2): an atom's
//! parameter mapping covers only those, so `sizeof(T) > 1` in
//! `C<T, int>` and in `C<T, long>` is one atom (clang
//! `MarkUsedTemplateParameters` over the atom's expression).
//!
//! The walk names every expression, statement, type, template-name and
//! nested-name-specifier kind (WP-776, WP-821): splices walk their
//! operand, a type's expression operand is walked, and sugar walks what
//! it wraps. A lambda walks its captures, its call operator's template
//! head, parameters, type and requires-clauses, and its body; a
//! statement expression walks its statements, and a closure type or a
//! local class or enumeration declared in a walked body is accounted for
//! there.
//! Only what it cannot see answers `None` and keeps the whole mapping
//! (two atoms then compare as before, never more loosely): a dependent
//! member class or enumeration (its enclosing template's parameters are
//! implicit), an unresolved using-declaration, a declaration kind a body
//! cannot be seen through (a friend, a using-declaration), and a
//! parameter index past 64.

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{ExprDeps, ExprKind, OffsetofComponent, TypeidOperand};

use kiln_ast::names::{DeclName, DeclNameView};
use kiln_ast::reflect::Reflectee;
use kiln_ast::stmt::StmtKind;
use kiln_ast::templates::Requirement;
use kiln_ast::types::{
    ArgView, ExcSpec, ExprOperand, Nns, TemplateArg, TemplateName, TypeFlags, TypeOfOperand,
    TypeView,
};
use kiln_ast::{
    ArgListId, DeclId, ExprId, LambdaId, NnsId, QualType, RawId, StmtId, TemplateNameId,
    TemplateParamListId,
};

use crate::Sema;

/// Deeper than this, the walk gives up (`None`).
const MAX_DEPTH: u32 = 256;

/// The parameters of depth `level` found so far, one bit per index.
struct Used {
    level: u32,
    mask: u64,
    /// The lambdas walked (each once: its closure type may recur).
    lambdas: Vec<LambdaId>,
    /// The local classes and enumerations whose declarations were walked.
    locals: Vec<DeclId>,
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
        let mut u =
            Used { level: u32::from(level), mask: 0, lambdas: Vec::new(), locals: Vec::new() };
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
            // The closure type is the lambda's own: walking the lambda
            // covers it.
            ExprKind::LambdaExpr => return self.used_in_lambda(ex.lambda_id(e), u, d),
            ExprKind::StmtExpr => self.used_in_stmt(ex.stmt_expr_body(e), u, d)?,
            // An expansion statement's element names its synthetic
            // template level (deeper than any concept's), whose range the
            // statement walks ([stmt.expand]).
            ExprKind::CXXExpansionSelectExpr => {
                if n.a() == u.level {
                    return None;
                }
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
            self.used_in_arg(a, u, depth)?;
        }
        Some(())
    }

    fn used_in_arg(&self, a: TemplateArg, u: &mut Used, depth: u32) -> Option<()> {
        match a.view() {
            ArgView::Null | ArgView::Value { .. } => Some(()),
            ArgView::Type(q) => self.used_in_type(q, u, depth + 1),
            ArgView::Expr { expr, .. } => self.used_in_expr(expr, u, depth + 1),
            ArgView::Profile(p) => self.used_in_expr(self.ast.types.profile_expr(p)?, u, depth + 1),
            ArgView::Template(t) | ArgView::TemplateExpansion { name: t, .. } => {
                self.used_in_template_name(t, u, depth + 1)
            }
            ArgView::Pack(l) => self.used_in_args(l, u, depth + 1),
        }
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
            TypeView::Record(r) | TypeView::Enum(r) => self.used_in_tag(r, u, d),
            // An unresolved using-declaration depends on its enclosing
            // template's parameters, which the type does not name.
            TypeView::Invalid | TypeView::UnresolvedUsing(_) => None,
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
                match t.exc_spec(&p) {
                    ExcSpec::NoexceptExpr { expr, .. } => self.used_in_expr(expr, u, d),
                    ExcSpec::DependentNoexcept(p) => {
                        self.used_in_operand(ExprOperand::Profile(p), u, d)
                    }
                    ExcSpec::Dynamic(l) => {
                        for &x in t.type_list_types(l) {
                            self.used_in_type(x, u, d)?;
                        }
                        Some(())
                    }
                    ExcSpec::None
                    | ExcSpec::Nothrow
                    | ExcSpec::NothrowAttr
                    | ExcSpec::DynamicNone
                    | ExcSpec::BasicNoexcept => Some(()),
                    // Computed from the declaration's members, not written.
                    ExcSpec::Deferred(_) => Some(()),
                }
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

    /// A dependent class or enumeration type `r`: a closure type walks
    /// its lambda; a local enumeration or class whose declaration a
    /// walked body holds was accounted for there; a class template
    /// specialization at namespace scope walks its arguments. Anything
    /// else (a member of a class template, a local class declared
    /// outside the walk) depends on parameters the type does not name:
    /// `None`.
    fn used_in_tag(&self, r: DeclId, u: &mut Used, depth: u32) -> Option<()> {
        let first = self.ast.first(r);
        if u.locals.contains(&first) {
            return Some(());
        }
        if let Some(l) = self.ast.closure_lambda(r) {
            return self.used_in_lambda(l, u, depth);
        }
        let (_, s) = self.ast.spec_info(r)?;
        let ctx = self.ast.decls.get(r).ctx()?;
        if !matches!(
            self.ast.decls.get(ctx).kind(),
            DeclKind::Namespace | DeclKind::TranslationUnit
        ) {
            return None;
        }
        self.used_in_args(s.args, u, depth + 1)
    }

    /// A lambda ([expr.prim.lambda]): its captures (an init-capture's
    /// variable, a simple capture's initializer), its call operator's
    /// template head, parameters, type and trailing requires-clause,
    /// then its body (clang's `RecursiveASTVisitor::TraverseLambdaExpr`).
    fn used_in_lambda(&self, l: LambdaId, u: &mut Used, depth: u32) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        if u.lambdas.contains(&l) {
            return Some(());
        }
        u.lambdas.push(l);
        let d = depth + 1;
        let data = self.ast.exprs.lambdas.get(l);
        if let Some(inits) = data.inits {
            for e in self.ast.lists.iter::<ExprId>(inits) {
                self.used_in_expr(e, u, d)?;
            }
        }
        for c in &data.captures {
            let Some(v) = c.var else { continue };
            if c.init {
                self.used_in_local_decl(v, u, d)?;
            } else if let Some(t) = self.ast.decls.get(v).ty() {
                self.used_in_type(t, u, d)?;
            }
        }
        if let Some(op) = data.call_op {
            if let Some(t) = self.ast.described_template(op) {
                let params = self.ast.template_info(t)?.params;
                self.used_in_template_params(params, u, d)?;
            }
            if let Some(f) = self.ast.decls.function(op) {
                for p in self.ast.lists.iter::<DeclId>(f.params) {
                    self.used_in_local_decl(p, u, d)?;
                }
            }
            if let Some(&r) = self.ast.templates.trailing_requires.get(op) {
                self.used_in_expr(r, u, d)?;
            }
        }
        if let Some(b) = data.body {
            self.used_in_stmt(b, u, d)?;
        }
        // The type last: a deduced return type may name a local class
        // of the body.
        if let Some(t) = data.call_op.and_then(|op| self.ast.decls.get(op).ty()) {
            self.used_in_type(t, u, d)?;
        }
        Some(())
    }

    /// A template head: each parameter's type, immediately-declared
    /// constraint, default and own parameters, and the requires-clause.
    fn used_in_template_params(
        &self,
        l: TemplateParamListId,
        u: &mut Used,
        depth: u32,
    ) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        let d = depth + 1;
        let list = *self.ast.templates.param_list(l);
        for p in self.ast.lists.iter::<DeclId>(list.params) {
            let info = *self.ast.template_parm(p)?;
            if let Some(c) = info.constraint {
                self.used_in_expr(c, u, d)?;
            }
            if let Some(nested) = info.params {
                self.used_in_template_params(nested, u, d)?;
            }
            if self.ast.decls.get(p).kind() == DeclKind::NonTypeTemplateParm {
                if let Some(t) = self.ast.decls.get(p).ty() {
                    self.used_in_type(t, u, d)?;
                }
            }
            if let Some(&a) = self.ast.templates.defaults.get(p) {
                self.used_in_arg(a, u, d)?;
            }
        }
        match list.requires {
            Some(r) => self.used_in_expr(r, u, d),
            None => Some(()),
        }
    }

    /// A declaration in a walked body (or a lambda's parameter or
    /// init-capture): a variable's or parameter's type and initializer, a
    /// typedef's type, a static assertion, a local enumeration's
    /// enumerators, a local class's bases and members (a member
    /// function's type, parameters, requires-clause, ctor-initializers
    /// and body). A kind not listed answers `None`.
    fn used_in_local_decl(&self, v: DeclId, u: &mut Used, depth: u32) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        let d = depth + 1;
        let decls = &self.ast.decls;
        match decls.get(v).kind() {
            DeclKind::Var | DeclKind::Binding | DeclKind::ParmVar => {
                if let Some(t) = decls.get(v).ty() {
                    self.used_in_type(t, u, d)?;
                }
                let init = match decls.get(v).kind() {
                    DeclKind::Var => decls.var(v).and_then(|x| x.init),
                    DeclKind::ParmVar => decls.param(v).and_then(|x| x.default_arg),
                    _ => decls.binding_expr(v),
                };
                match init {
                    Some(e) => self.used_in_expr(e, u, d),
                    None => Some(()),
                }
            }
            DeclKind::Typedef | DeclKind::TypeAlias => {
                self.used_in_type(decls.typedef(v)?.underlying, u, d)
            }
            DeclKind::StaticAssert | DeclKind::ConstevalBlock => {
                let x = *decls.static_assert(v)?;
                self.used_in_expr(x.cond, u, d)?;
                match x.message {
                    Some(m) => self.used_in_expr(m, u, d),
                    None => Some(()),
                }
            }
            DeclKind::Enum => {
                if let Some(t) = decls.enumeration(v)?.int_type {
                    self.used_in_type(t, u, d)?;
                }
                for m in self.ast.members(v) {
                    if let Some(e) = decls.enumerator(m).and_then(|x| x.init) {
                        self.used_in_expr(e, u, d)?;
                    }
                }
                u.locals.push(self.ast.first(v));
                Some(())
            }
            // A closure type is walked with its lambda-expression.
            DeclKind::CXXRecord if self.ast.closure_lambda(v).is_some() => Some(()),
            // A local class: its bases and members (it cannot have member
            // templates, [temp.mem]/2).
            DeclKind::CXXRecord => {
                u.locals.push(self.ast.first(v));
                for b in decls.bases(v, &self.ast.lists) {
                    self.used_in_type(b.ty, u, d)?;
                }
                for m in self.ast.members(v) {
                    self.used_in_local_decl(m, u, d)?;
                }
                Some(())
            }
            DeclKind::Field => {
                if let Some(t) = decls.get(v).ty() {
                    self.used_in_type(t, u, d)?;
                }
                let f = *decls.field(v)?;
                for e in [f.bit_width, f.init].into_iter().flatten() {
                    self.used_in_expr(e, u, d)?;
                }
                Some(())
            }
            DeclKind::Function
            | DeclKind::CXXMethod
            | DeclKind::CXXConstructor
            | DeclKind::CXXDestructor
            | DeclKind::CXXConversion => {
                if let Some(t) = decls.get(v).ty() {
                    self.used_in_type(t, u, d)?;
                }
                let f = *decls.function(v)?;
                for p in self.ast.lists.iter::<DeclId>(f.params) {
                    self.used_in_local_decl(p, u, d)?;
                }
                if let Some(&r) = self.ast.templates.trailing_requires.get(v) {
                    self.used_in_expr(r, u, d)?;
                }
                for i in self.ast.ctor_inits(v) {
                    self.used_in_expr(i.init, u, d)?;
                }
                match f.body {
                    Some(b) => self.used_in_stmt(b, u, d),
                    None => Some(()),
                }
            }
            DeclKind::Empty
            | DeclKind::AccessSpec
            | DeclKind::IndirectField
            | DeclKind::UsingDirective
            | DeclKind::NamespaceAlias => Some(()),
            _ => None,
        }
    }

    /// The statements of a lambda's body or a statement expression
    /// (every kind is listed; a dependent coroutine's synthesized parts
    /// come from its function's type, walked with the lambda).
    #[allow(clippy::too_many_lines)]
    fn used_in_stmt(&self, s: StmtId, u: &mut Used, depth: u32) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        let d = depth + 1;
        let st = &self.ast.stmts;
        let lists = &self.ast.lists;
        let stmt = |x: Option<StmtId>, u: &mut Used| match x {
            Some(x) => self.used_in_stmt(x, u, d),
            None => Some(()),
        };
        let expr = |x: Option<ExprId>, u: &mut Used| match x {
            Some(x) => self.used_in_expr(x, u, d),
            None => Some(()),
        };
        let var = |x: Option<DeclId>, u: &mut Used| match x {
            Some(x) => self.used_in_local_decl(x, u, d),
            None => Some(()),
        };
        match st.get(s).kind() {
            StmtKind::NullStmt
            | StmtKind::BreakStmt
            | StmtKind::ContinueStmt
            | StmtKind::GotoStmt => Some(()),
            StmtKind::CompoundStmt => {
                for c in lists.iter::<StmtId>(st.compound_body(s)) {
                    self.used_in_stmt(c, u, d)?;
                }
                Some(())
            }
            StmtKind::DeclStmt => {
                for v in lists.iter::<DeclId>(st.decl_stmt_decls(s)) {
                    self.used_in_local_decl(v, u, d)?;
                }
                Some(())
            }
            StmtKind::ExprStmt => self.used_in_expr(st.expr_stmt_expr(s), u, d),
            StmtKind::IfStmt => {
                let i = *st.if_data(s);
                stmt(i.init, u)?;
                var(i.cond_var, u)?;
                expr(i.cond, u)?;
                stmt(i.then, u)?;
                stmt(i.els, u)
            }
            StmtKind::SwitchStmt => {
                let w = *st.switch_data(s);
                stmt(w.init, u)?;
                var(w.cond_var, u)?;
                expr(Some(w.cond), u)?;
                stmt(Some(w.body), u)
            }
            StmtKind::CaseStmt => {
                let c = *st.case_data(s);
                expr(Some(c.lhs), u)?;
                expr(c.rhs, u)?;
                stmt(Some(c.sub), u)
            }
            StmtKind::DefaultStmt => stmt(Some(st.default_sub(s)), u),
            StmtKind::LabelStmt => stmt(Some(st.sub_stmt(s)), u),
            StmtKind::AttributedStmt => {
                // An assumption and a loop hint's value are expressions.
                for a in lists.iter::<kiln_ast::AttrId>(st.stmt_attrs(s)) {
                    let node = self.ast.attrs.get(a);
                    if matches!(node.kind, AttrKind::ASSUME | AttrKind::LOOP_HINT) {
                        expr(ExprId::from_raw(node.a), u)?;
                    }
                }
                stmt(Some(st.sub_stmt(s)), u)
            }
            StmtKind::WhileStmt | StmtKind::ForStmt | StmtKind::DoStmt => {
                let l = *st.loop_data(s);
                stmt(l.init, u)?;
                var(l.cond_var, u)?;
                expr(l.cond, u)?;
                expr(l.inc, u)?;
                stmt(l.body, u)
            }
            StmtKind::CXXForRangeStmt => {
                let r = *st.range_for_data(s);
                stmt(r.init, u)?;
                var(Some(r.range), u)?;
                var(r.begin, u)?;
                var(r.end, u)?;
                expr(r.cond, u)?;
                expr(r.inc, u)?;
                var(Some(r.loop_var), u)?;
                stmt(Some(r.body), u)
            }
            StmtKind::CXXExpansionStmt => {
                let x = *st.expansion_data(s);
                stmt(x.init, u)?;
                if st.expansion_expanded(s) {
                    for c in
                        lists.iter::<StmtId>(x.prelude).chain(lists.iter::<StmtId>(x.expansions))
                    {
                        self.used_in_stmt(c, u, d)?;
                    }
                    return Some(());
                }
                for e in lists.iter::<ExprId>(x.exprs) {
                    self.used_in_expr(e, u, d)?;
                }
                expr(x.range_init, u)?;
                var(Some(x.decl), u)?;
                stmt(Some(x.body), u)
            }
            StmtKind::ReturnStmt => expr(st.return_value(s), u),
            StmtKind::IndirectGotoStmt => expr(Some(st.indirect_goto_target(s)), u),
            StmtKind::ContractAssertStmt => {
                var(st.contract_result_name(s), u)?;
                expr(Some(st.contract_cond(s)), u)
            }
            StmtKind::CoreturnStmt => {
                expr(st.coreturn_operand(s), u)?;
                expr(st.coreturn_promise_call(s), u)
            }
            StmtKind::CoroutineBodyStmt => stmt(Some(st.coroutine_body(s).body), u),
            StmtKind::GCCAsmStmt => {
                for op in st.asm_operands(s) {
                    self.used_in_expr(op.expr, u, d)?;
                }
                Some(())
            }
            StmtKind::CXXTryStmt => {
                stmt(Some(st.try_block(s)), u)?;
                for h in lists.iter::<StmtId>(st.try_handlers(s)) {
                    self.used_in_stmt(h, u, d)?;
                }
                Some(())
            }
            StmtKind::CXXCatchStmt => {
                var(st.catch_var(s), u)?;
                stmt(Some(st.catch_block(s)), u)
            }
        }
    }
}
