//! The template parameters an atomic constraint's expression names
//! ([temp.constr.atomic]/1, spec/sema-templates.md §8.2): an atom's
//! parameter mapping covers only those, so `sizeof(T) > 1` in
//! `C<T, int>` and in `C<T, long>` is one atom (clang
//! `MarkUsedTemplateParameters` over the atom's expression).
//!
//! The walk is a whitelist: a node it does not understand answers
//! `None` and the atom keeps the whole mapping (two atoms then compare
//! as before, never more loosely).

use kiln_ast::expr::{ExprDeps, ExprKind};
use kiln_ast::templates::Requirement;
use kiln_ast::types::{ArgView, ExprOperand, Nns, TemplateName, TypeFlags, TypeView};
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
        match n.kind() {
            ExprKind::IntegerLiteral
            | ExprKind::FloatingLiteral
            | ExprKind::CharacterLiteral
            | ExprKind::StringLiteral
            | ExprKind::CXXBoolLiteralExpr
            | ExprKind::CXXNullPtrLiteralExpr
            | ExprKind::UnaryOperator
            | ExprKind::BinaryOperator
            | ExprKind::ConditionalOperator
            | ExprKind::ArraySubscriptExpr
            | ExprKind::ParenExpr
            | ExprKind::CallExpr
            | ExprKind::CXXOperatorCallExpr
            | ExprKind::ParenListExpr
            | ExprKind::InitListExpr
            | ExprKind::CXXTemporaryObjectExpr
            | ExprKind::CXXScalarValueInitExpr
            | ExprKind::CXXUnresolvedConstructExpr
            | ExprKind::CXXNoexceptExpr
            | ExprKind::CXXFoldExpr
            | ExprKind::PackExpansionExpr
            | ExprKind::CXXDependentScopeMemberExpr
            | ExprKind::DependentScopeDeclRefExpr => {}
            k if k.is_cast() => {
                if let Some(t) = self.ast.exprs.cast_written_type(e) {
                    self.used_in_type(t, u, d)?;
                }
            }
            ExprKind::DeclRefExpr => self.used_decl(self.ast.exprs.decl(e), u)?,
            ExprKind::SizeOfPackExpr => self.used_decl(DeclId::from_raw(n.a())?, u)?,
            ExprKind::UnresolvedLookupExpr => {
                let l = kiln_ast::ids::ListId::new(n.b());
                for p in self.ast.lists.iter::<DeclId>(l) {
                    self.used_decl(p, u)?;
                }
            }
            ExprKind::ConceptSpecializationExpr => {
                self.used_decl(DeclId::from_raw(n.a())?, u)?;
                self.used_in_args(ArgListId::from_raw(n.b())?, u, d)?;
            }
            ExprKind::UnaryExprOrTypeTraitExpr => {
                if let Some(t) = self.ast.exprs.trait_type_arg(e) {
                    self.used_in_type(t, u, d)?;
                }
            }
            ExprKind::TypeTraitExpr => {
                for t in self.ast.lists.iter::<QualType>(self.ast.exprs.type_trait_args(e)) {
                    self.used_in_type(t, u, d)?;
                }
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
            _ => return None,
        }
        if let Some(&l) = self.ast.templates.expr_targs.get(e) {
            self.used_in_args(l, u, d)?;
        }
        if let Some(&q) = self.ast.templates.expr_nns.get(e) {
            self.used_in_nns(q, u, d)?;
        }
        for k in self.ast.exprs.children(e, &self.ast.lists) {
            self.used_in_expr(k, u, d)?;
        }
        // A written type the node keeps as its own (`T()`, `T{}`), or a
        // declaration's type.
        self.used_in_type(n.ty(), u, d)
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
        match self.ast.types.template_name_view(t) {
            TemplateName::Template(d) => self.used_decl(d, u),
            TemplateName::TemplateTemplateParm { depth: pd, index, .. }
            | TemplateName::CanonTemplateTemplateParm { depth: pd, index, .. } => u.mark(pd, index),
            TemplateName::Qualified { nns, underlying, .. } => {
                self.used_in_nns(nns, u, depth + 1)?;
                self.used_in_template_name(underlying, u, depth + 1)
            }
            TemplateName::DependentIdentifier { nns, .. }
            | TemplateName::DependentOperator { nns, .. } => self.used_in_nns(nns, u, depth + 1),
            #[allow(unreachable_patterns)]
            _ => None,
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
            #[allow(unreachable_patterns)]
            _ => None,
        }
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
            TypeView::Builtin(_) => Some(()),
            TypeView::TemplateTypeParm(p) => self.used_decl(p, u),
            TypeView::CanonTemplateTypeParm { depth: pd, index, .. } => u.mark(pd, index),
            TypeView::Pointer(x)
            | TypeView::LValueRef(x)
            | TypeView::RValueRef(x)
            | TypeView::IncompleteArray(x)
            | TypeView::Paren(x)
            | TypeView::Complex(x)
            | TypeView::Atomic(x)
            | TypeView::Elaborated { named: x, .. }
            | TypeView::ExtQual { base: x, .. }
            | TypeView::Attributed { modified: x, .. }
            | TypeView::Vector { elem: x, .. }
            | TypeView::PackExpansion { pattern: x, .. }
            | TypeView::ConstantArray(kiln_ast::types::ConstArray { elem: x, .. }) => {
                self.used_in_type(x, u, d)
            }
            TypeView::Typedef { .. } | TypeView::Using { .. } => {
                self.used_in_type(t.canonical(q), u, d)
            }
            TypeView::MemberPointer { pointee, class } => {
                self.used_in_type(QualType::unqual(class), u, d)?;
                self.used_in_type(pointee, u, d)
            }
            TypeView::DependentSizedArray { elem, size } => {
                self.used_in_type(elem, u, d)?;
                let size = match size {
                    ExprOperand::Expr(e) => e,
                    ExprOperand::Profile(p) => t.profile_expr(p)?,
                };
                self.used_in_expr(size, u, d)
            }
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
            TypeView::Decltype { expr, .. } => self.used_in_expr(expr, u, d),
            _ => None,
        }
    }
}
