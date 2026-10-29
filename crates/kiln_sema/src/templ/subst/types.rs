//! Substitution into types (spec/sema-templates.md §5.3): a type without
//! instantiation-dependent or unexpanded-pack flags is its own result;
//! otherwise it is rebuilt through the type context's constructors (or
//! the declarator checks, so `T*` with `T = int&` is diagnosed as when
//! written).

use kiln_ast::decl::DeclKind;
use kiln_ast::types::{ArgView, ExcSpec, ExprOperand, Nns, TypeView};
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::{SubstFail, SubstResult, Substituter};
use crate::Sema;
use crate::declspec::{Chunk, Declarator, DeclaratorContext};
use crate::lookup::LookupKind;
use crate::templ::actions::TemplateIdRef;
use crate::templ::args::ArgLookup;

impl Substituter<'_, '_, '_> {
    /// Substitutes into type `q` (its qualifiers kept, collapsing as the
    /// constructors do).
    pub fn ty(&mut self, q: QualType, loc: Loc) -> SubstResult<QualType> {
        let flags = self.sema.ast.types.qflags(q);
        if !Sema::dependent_flags(flags) {
            return Ok(q);
        }
        let inner = self.ty_node(q.without_quals(), loc)?;
        if self.sema.is_error_type(inner) {
            return Err(SubstFail);
        }
        Ok(self.sema.ast.types.qualify(inner, q.quals()))
    }

    /// One declarator operator applied to `base` with the declarator's
    /// checks.
    fn chunk(&mut self, base: QualType, chunk: Chunk, loc: Loc) -> SubstResult<QualType> {
        let mut d = Declarator::new(DeclaratorContext::TypeName, loc);
        d.name = self.declarator_name;
        d.chunks.push(chunk);
        let t = self.sema.type_for_declarator_at(base, loc, &d);
        if self.sema.is_error_type(t) { Err(SubstFail) } else { Ok(t) }
    }

    /// The type a type declaration found by lookup names.
    pub(crate) fn decl_type(&mut self, d: DeclId) -> QualType {
        match self.sema.kind_of(d) {
            DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::TemplateTypeParm => {
                self.sema.ast.decls.get(d).ty().unwrap_or(QualType::ERROR)
            }
            _ => self.sema.tag_type(d),
        }
    }

    #[allow(clippy::too_many_lines, reason = "one arm per type kind, as clang's TreeTransform")]
    fn ty_node(&mut self, q: QualType, loc: Loc) -> SubstResult<QualType> {
        let view = self.sema.ast.types.view(q.ty());
        match view {
            TypeView::TemplateTypeParm(d) => {
                let Some(info) = self.sema.ast.template_parm(d).copied() else {
                    return Err(SubstFail);
                };
                self.type_parm(q, Some(d), info.depth, info.index, loc)
            }
            TypeView::CanonTemplateTypeParm { depth, index, .. } => {
                self.type_parm(q, None, depth as u16, index as u16, loc)
            }
            TypeView::SubstTemplateTypeParm { replacement, associated, index, pack_index } => {
                let r = self.ty(replacement, loc)?;
                Ok(self.sema.ast.types.subst_template_type_parm(r, associated, index, pack_index))
            }
            TypeView::Pointer(p) => {
                let p = self.ty(p, loc)?;
                self.chunk(p, Chunk::Pointer { quals: kiln_ast::types::Quals::NONE, loc }, loc)
            }
            TypeView::LValueRef(p) => {
                let p = self.ty(p, loc)?;
                self.chunk(p, Chunk::LRef { loc }, loc)
            }
            TypeView::RValueRef(p) => {
                let p = self.ty(p, loc)?;
                self.chunk(p, Chunk::RRef { loc }, loc)
            }
            TypeView::MemberPointer { pointee, class } => {
                let p = self.ty(pointee, loc)?;
                let c = self.ty(QualType::unqual(class), loc)?;
                let t = &self.sema.ast.types;
                if t.as_record(c).is_none() && !self.sema.is_dependent_type(c) {
                    let d = self
                        .sema
                        .diag(DiagId::sema_mempointer_in_nonclass_type, loc)
                        .arg(self.sema.ty_arg(c));
                    self.sema.emit(d);
                    return Err(SubstFail);
                }
                Ok(self.sema.ast.types.member_pointer(p, c.ty()))
            }
            TypeView::ConstantArray(a) => {
                let e = self.ty(a.elem, loc)?;
                match a.size_expr {
                    Some(s) => self.chunk(e, Chunk::Array { size: Some(s), loc }, loc),
                    None => Ok(self.sema.ast.types.constant_array(e, a.size, None)),
                }
            }
            TypeView::IncompleteArray(elem) => {
                let e = self.ty(elem, loc)?;
                self.chunk(e, Chunk::Array { size: None, loc }, loc)
            }
            TypeView::DependentSizedArray { elem, size: ExprOperand::Expr(s) } => {
                let e = self.ty(elem, loc)?;
                let s = self.expr(s)?;
                self.chunk(e, Chunk::Array { size: Some(s), loc }, loc)
            }
            TypeView::Function(p) => {
                let ret = self.ty(p.ret(), loc)?;
                let params = self.sema.ast.types.params(&p).to_vec();
                let mut out = Vec::with_capacity(params.len());
                for x in params {
                    out.push(self.ty(x, loc)?);
                }
                let exc = match self.sema.ast.types.exc_spec(&p) {
                    ExcSpec::NoexceptExpr {
                        expr,
                        value: kiln_ast::types::NoexceptValue::Dependent(_),
                    } => {
                        let e = self.expr(expr)?;
                        self.sema.act_on_noexcept_spec(e, loc, loc)
                    }
                    ExcSpec::Dynamic(_) | ExcSpec::DependentNoexcept(_) => {
                        return self.unimplemented("dependent exception specification", loc);
                    }
                    other => other,
                };
                Ok(self.sema.ast.types.function(ret, &out, p.info(), exc))
            }
            TypeView::Typedef { decl, underlying } => {
                // A member typedef of the pattern: the instantiated
                // member's typedef (sugar kept); a local one: the
                // instantiated local.
                let ctx_is_record = self
                    .sema
                    .ast
                    .decls
                    .get(decl)
                    .ctx()
                    .is_some_and(|c| self.sema.kind_of(c) == DeclKind::CXXRecord);
                if ctx_is_record || self.locals.contains_key(&decl) {
                    let d = self.find_instantiated_decl(decl, loc)?;
                    if d != decl {
                        // Naming it in the instantiation is a use.
                        self.sema.mark_type_referenced(d);
                        return Ok(self.decl_type(d));
                    }
                }
                let u = self.ty(underlying, loc)?;
                Ok(self.sema.ast.types.typedef_type(decl, u).into())
            }
            TypeView::Using { underlying, .. } => self.ty(underlying, loc),
            TypeView::Elaborated { keyword, nns, named } => {
                let nns = match nns {
                    Some(n) => Some(self.nns(n, loc)?),
                    None => None,
                };
                let named = self.ty(named, loc)?;
                Ok(self.sema.ast.types.elaborated(keyword, nns, named))
            }
            TypeView::TemplateSpecialization { name, args, underlying, alias } => {
                let _ = (underlying, alias);
                let name = self.template_name(name, loc)?;
                let args = self.args(args, loc)?;
                let v = self.sema.ast.types.args(args).to_vec();
                match self.sema.act_template_id(name, &v, loc, loc) {
                    Ok(TemplateIdRef::Type(t)) => Ok(t),
                    Ok(_) | Err(()) => Err(SubstFail),
                }
            }
            TypeView::InjectedClassName { pattern, injected } => {
                if let Some(&(_, i)) = self.pairs.iter().rev().find(|p| p.0 == pattern) {
                    return Ok(self.sema.tag_type(i));
                }
                self.ty(injected, loc)
            }
            TypeView::Record(d) | TypeView::Enum(d) => {
                let i = self.find_instantiated_decl(d, loc)?;
                Ok(self.sema.tag_type(i))
            }
            TypeView::DependentName { keyword, nns, name } => {
                let n = self.nns(nns, loc)?;
                if let Nns::Type { ty, .. } = self.sema.ast.types.nns_view(n)
                    && self.sema.is_dependent_type(ty)
                {
                    // Substituted with dependent arguments (an alias
                    // template named in a template): still dependent.
                    return Ok(self.sema.ast.types.dependent_name(keyword, n, name));
                }
                let ctx = self.nns_context(n, loc)?;
                let found = self.sema.lookup_qualified(ctx, name, LookupKind::Ordinary);
                let Some(&d) = found.decls.first() else {
                    let d = self
                        .sema
                        .diag(DiagId::sema_typename_nested_not_found, loc)
                        .arg(crate::decl::sym_str(self.sema, name))
                        .arg(self.ctx_arg(ctx));
                    self.sema.emit(d);
                    return Err(SubstFail);
                };
                if !matches!(
                    self.sema.kind_of(d),
                    DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::CXXRecord | DeclKind::Enum
                ) {
                    let at = self.sema.ast.decls.get(d).loc();
                    let n = crate::decl::sym_str(self.sema, name);
                    let note = self.sema.diag(DiagId::note_typename_member_refers_here, at).arg(n);
                    let d = self
                        .sema
                        .diag(DiagId::sema_typename_nested_not_type, loc)
                        .arg(crate::decl::sym_str(self.sema, name))
                        .arg(self.ctx_arg(ctx))
                        .with_note(note);
                    self.sema.emit(d);
                    return Err(SubstFail);
                }
                self.sema.mark_type_referenced(d);
                let t = self.decl_type(d);
                Ok(self.sema.ast.types.elaborated(kiln_ast::types::TagKeyword::None, Some(n), t))
            }
            // spec/sema-templates.md §9.2.
            TypeView::UnaryTransform { kind, base, .. } => {
                let b = self.ty(base, loc)?;
                self.sema.build_unary_transform(kind, b, loc).ok_or(SubstFail)
            }
            TypeView::Decltype { expr, .. } => {
                self.sema.enter_decltype();
                let e = self.expr(expr);
                self.sema.exit_decltype(e.ok());
                let e = e?;
                self.sema.act_on_decltype(e, loc, loc).ok_or(SubstFail)
            }
            TypeView::Auto { keyword, deduced, concept, args, pack } => match deduced {
                // Deduced from a type-dependent initializer
                // (`Sema::dependent_auto`): deduced again from the
                // substituted one.
                Some(d) if d == QualType::DEPENDENT => {
                    Ok(self.sema.ast.types.auto_type(keyword, None, concept, args, pack))
                }
                Some(d) => {
                    let d = self.ty(d, loc)?;
                    Ok(self.sema.ast.types.auto_type(keyword, Some(d), concept, args, pack))
                }
                None => Ok(q),
            },
            TypeView::Attributed { kind, modified, equivalent } => {
                let m = self.ty(modified, loc)?;
                let e = self.ty(equivalent, loc)?;
                Ok(self.sema.ast.types.attributed(kind, m, e))
            }
            TypeView::Complex(e) => {
                let e = self.ty(e, loc)?;
                Ok(self.sema.ast.types.complex(e))
            }
            TypeView::DependentTemplateSpecialization { keyword, nns, name, args } => {
                let n = self.nns(nns, loc)?;
                let args = self.args(args, loc)?;
                if let Nns::Type { ty, .. } = self.sema.ast.types.nns_view(n)
                    && self.sema.is_dependent_type(ty)
                {
                    return Ok(self
                        .sema
                        .ast
                        .types
                        .dependent_template_specialization(keyword, n, name, args));
                }
                // `T::template X<args>` ([temp.names]/5): the member
                // template of the substituted class.
                let d = self.member_template_named(n, name, loc)?;
                let underlying = self.sema.template_name_for(d);
                let tn =
                    self.sema.ast.types.template_name(kiln_ast::types::TemplateName::Qualified {
                        nns: n,
                        template_kw: true,
                        underlying,
                    });
                let v = self.sema.ast.types.args(args).to_vec();
                let Ok(TemplateIdRef::Type(t)) = self.sema.act_template_id(tn, &v, loc, loc) else {
                    return Err(SubstFail);
                };
                let named = match self.sema.ast.types.view(t.ty()) {
                    TypeView::Elaborated { nns: None, named, .. } => named,
                    _ => t,
                };
                Ok(self.sema.ast.types.elaborated(keyword, Some(n), named))
            }
            TypeView::PackExpansion { .. } | TypeView::PackIndexing { .. } => {
                self.unimplemented("pack expansion", loc)
            }
            _ => self.unimplemented("substitution into this type", loc),
        }
    }

    /// Template type parameter `(depth, index)` (§5.3): its argument as
    /// `SubstTemplateTypeParmType` sugar, or itself when retained.
    fn type_parm(
        &mut self,
        q: QualType,
        decl: Option<DeclId>,
        depth: u16,
        index: u16,
        loc: Loc,
    ) -> SubstResult<QualType> {
        match self.lookup(depth, index) {
            ArgLookup::Arg(a) => {
                let a = self.element(a, loc)?;
                let ArgView::Type(t) = a.view() else { return Err(SubstFail) };
                let Some(assoc) = self.assoc(depth) else { return Ok(t) };
                Ok(self.sema.ast.types.subst_template_type_parm(
                    t,
                    assoc,
                    u32::from(index),
                    self.pack_index,
                ))
            }
            ArgLookup::Retain(d) if d == depth => Ok(q),
            // A parameter of a member template redeclared one level
            // shallower ([temp.mem]; §4.6): its new declaration.
            ArgLookup::Retain(d) => {
                if let Some(p) = decl
                    && let Ok(np) = self.find_instantiated_decl(p, loc)
                    && np != p
                    && let Some(t) = self.sema.ast.decls.get(np).ty()
                {
                    return Ok(QualType::new(t.ty(), q.quals_bits() | t.quals_bits()));
                }
                let pack =
                    matches!(
                        self.sema.ast.types.view(q.ty()),
                        TypeView::CanonTemplateTypeParm { pack: true, .. }
                    ) || decl.and_then(|p| self.sema.ast.template_parm(p)).is_some_and(|i| i.pack);
                let t = self.sema.ast.types.canon_template_type_parm(
                    u32::from(d),
                    u32::from(index),
                    pack,
                );
                Ok(QualType::new(t, q.quals_bits()))
            }
            ArgLookup::Missing => Err(SubstFail),
        }
    }

    /// The member template `name` of the context `n` names after
    /// substitution (`T::template X`, [temp.names]/5); diagnosed when
    /// there is none or it is not a template.
    pub(crate) fn member_template_named(
        &mut self,
        n: kiln_ast::ids::NnsId,
        name: kiln_source::symbol::Symbol,
        loc: Loc,
    ) -> SubstResult<DeclId> {
        let ctx = self.nns_context(n, loc)?;
        let found = self.sema.lookup_qualified(ctx, name, LookupKind::Ordinary);
        let Some(&d) = found.decls.first() else {
            let d = self
                .sema
                .diag(DiagId::sema_no_member, loc)
                .arg(crate::decl::sym_str(self.sema, name))
                .arg(self.ctx_arg(ctx));
            self.sema.emit(d);
            return Err(SubstFail);
        };
        if let Some(t) = self.sema.template_of(d) {
            return Ok(t);
        }
        let at = self.sema.ast.decls.get(d).loc();
        let note = self.sema.diag(DiagId::note_declared_non_template, at);
        let d = self
            .sema
            .diag(DiagId::sema_template_kw_refers_to_non_template, loc)
            .arg(crate::decl::sym_str(self.sema, name))
            .with_note(note);
        self.sema.emit(d);
        Err(SubstFail)
    }

    /// A context for diagnostics: `'X<int>'` or `namespace 'n'`.
    pub(crate) fn ctx_arg(&self, ctx: DeclId) -> String {
        match self.sema.kind_of(ctx) {
            DeclKind::CXXRecord | DeclKind::Enum => self.sema.ty_arg(self.sema.tag_type(ctx)),
            _ => format!("'{}'", self.sema.decl_name(ctx)),
        }
    }
}
