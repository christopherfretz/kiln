//! The reflection operator `^^` for every operand kind (M6-03, WP-472;
//! P2996R13 [expr.reflect]; spec/reflection-repr.md §2).
//!
//! Each operand maps to a [`Reflectee`] keyed so that equal entities
//! intern to equal ids (`kiln_ast::reflect`): declarations, namespaces
//! and namespace aliases on their first declaration, templates on the
//! canonical name of theirs, types through `Ast::reflection_type`. A
//! declaration of a dependent context reflects the pattern until
//! substitution remaps it (`templ/subst/exprs.rs`).

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::names::DeclName;
use kiln_ast::reflect::Reflectee;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::lookup::LookupKind;
use crate::templ::actions::TemplateIdRef;
use crate::using::Qualifier;

impl Sema<'_, '_> {
    /// `^^type-id` ([expr.reflect]/6): the type as written, a top-level
    /// alias kept.
    pub fn act_on_reflect_type(&mut self, ty: QualType, loc: Loc, end: Loc) -> ExprId {
        let info = self.meta_info_type();
        if ty == QualType::ERROR {
            return self.recover(info, &[], loc);
        }
        self.make_reflect_expr(Reflectee::Type(ty), loc, end)
    }

    /// `^^::` ([expr.reflect]/4): the global namespace.
    pub fn act_on_reflect_global(&mut self, loc: Loc, end: Loc) -> ExprId {
        let tu = self.ast.tu;
        self.make_reflect_expr(Reflectee::Namespace(tu), loc, end)
    }

    /// `^^name` / `^^nns name` ([expr.reflect]/5, /7): a namespace,
    /// namespace alias, template, concept, or the entity an
    /// id-expression names. An overload set is ill-formed (/7: the
    /// id-expression must name a unique function).
    pub fn act_on_reflect_name(
        &mut self,
        qual: Option<Qualifier>,
        name: Symbol,
        name_loc: Loc,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        self.act_on_reflect_dn(qual, DeclName::ident(name), name_loc, loc, end)
    }

    /// [`Sema::act_on_reflect_name`] for any declaration name: also
    /// `^^S::operator=`, `^^operator+`, `^^S::operator int` (WP-519; the
    /// implicit special members the name can find are declared first).
    pub fn act_on_reflect_dn(
        &mut self,
        qual: Option<Qualifier>,
        name: DeclName,
        name_loc: Loc,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        if let Some(q) = qual
            && self.is_dependent_ctx(q.ctx)
        {
            return self.act_on_reflect_dependent(
                (q.nns, q.loc),
                name,
                None,
                None,
                name_loc,
                loc,
                end,
            );
        }
        let info = self.meta_info_type();
        match qual {
            Some(q) => self.declare_implicit_named(q.ctx, name),
            None => self.declare_implicit_named_in_scope(name),
        }
        let lookup = |s: &Self, k| match qual {
            Some(q) => s.lookup_qualified_dn(q.ctx, name, k),
            None => s.lookup_name_dn(name, k),
        };
        let mut found = lookup(self, LookupKind::Ordinary);
        if found.decls.is_empty() && name.as_ident().is_some() {
            found = lookup(self, LookupKind::Nested);
        }
        let mut decls: Vec<DeclId> =
            found.decls.iter().map(|&d| self.ast.strip_shadow(d)).collect();
        decls.dedup_by_key(|d| self.entity_decl(*d)); // same_entity: raw ok
        match decls.as_slice() {
            [] => {
                let text = self.decl_name_str(name);
                let d = match qual {
                    Some(q) => self
                        .diag(DiagId::sema_no_member, name_loc)
                        .arg(text)
                        .arg(self.qualifier_desc(q.ctx)),
                    None => self.diag(DiagId::sema_undeclared_var_use, name_loc).arg(text),
                };
                self.emit(d);
                self.recover(info, &[], loc)
            }
            &[d] if qual.is_none() && self.reflect_local_across_lambda(d, name_loc) => {
                self.recover(info, &[], loc)
            }
            &[d] => {
                // The operand names a member: it is access-checked like
                // any other use of the name ([class.access]/4; GCC 16.1
                // rejects `^^C::priv` outside C). Splices are not
                // ([expr.prim.splice]), so this is the only check.
                if !self.check_found_access(&found, name_loc) {
                    return self.recover(info, &[], loc);
                }
                self.reflect_entity(d, name_loc, loc, end)
            }
            _ => self.reflect_overload_set(name_loc, loc),
        }
    }

    /// `r`, the instantiated `^^T::name` (no `typename`): a name that was
    /// assumed not to name a type ([temp.res.general]/4: `^^T::x` is not
    /// a type-only context) must not name one at instantiation, nor a
    /// class or alias template unless `template` said so (`^^T::template
    /// m`). Clang's missing-`typename` error, at the name; GCC 16.1
    /// rejects the same operands.
    pub(crate) fn check_reflect_dependent_kind(
        &mut self,
        r: ExprId,
        ctx: DeclId,
        name: DeclName,
        template_kw: bool,
        name_loc: Loc,
        loc: Loc,
    ) -> ExprId {
        if self.ast.exprs.get(r).kind() != ExprKind::CXXReflectExpr {
            return r;
        }
        let names_type = match self.ast.exprs.reflect_operand(r) {
            Reflectee::Type(_) => true,
            Reflectee::Template(n) => {
                !template_kw
                    && self.ast.types.template_decl(n).is_some_and(|d| {
                        matches!(
                            self.kind_of(d),
                            DeclKind::ClassTemplate | DeclKind::TypeAliasTemplate
                        )
                    })
            }
            _ => false,
        };
        if !names_type {
            return r;
        }
        let text = format!("{}::{}", self.qualified_decl_name(ctx), self.decl_name_str(name));
        let d = self.diag(DiagId::parse_missing_typename, name_loc).arg(text);
        self.emit(d);
        let info = self.meta_info_type();
        self.recover(info, &[], loc)
    }

    /// `^^nns ~C` / `^^~C` ([expr.reflect]/7, [class.dtor]/1): the
    /// destructor of class `C` (its implicit one declared first). The
    /// type-name is looked up in the qualifier's class, then here, as for
    /// a destructor name in a member access ([basic.lookup.qual.general]/4);
    /// it must name the qualifier's class.
    pub fn act_on_reflect_destructor(
        &mut self,
        qual: Option<Qualifier>,
        name: Symbol,
        name_loc: Loc,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        let in_class = qual.filter(|q| self.kind_of(q.ctx) == DeclKind::CXXRecord).and_then(|q| {
            let found = self.lookup_in_class(q.ctx, name, LookupKind::Ordinary);
            self.type_named_by(&found)
        });
        let named = in_class.or_else(|| {
            let found = self.lookup_name(name, LookupKind::Ordinary);
            self.type_named_by(&found)
        });
        let named = self.injected_named(named);
        // `^^T::~T`: the destructor name of the type named here, looked
        // up in `T`'s class at instantiation.
        if let Some(q) = qual
            && self.is_dependent_ctx(q.ctx)
            && let Some((ty, _)) = named
        {
            let canon = self.ast.types.canonical(ty).ty();
            let dn = self.ast.names.destructor(canon);
            return self.act_on_reflect_dependent(
                (q.nns, q.loc),
                dn,
                None,
                None,
                name_loc,
                loc,
                end,
            );
        }
        let Some((ty, _)) = named else {
            let text = crate::decl::sym_str(self, name);
            let d = self.diag(DiagId::sema_undeclared_var_use, name_loc).arg(text);
            self.emit(d);
            let info = self.meta_info_type();
            return self.recover(info, &[], loc);
        };
        let Some(c) = self.ast.types.as_record(ty) else {
            return self.reflect_bad_operand(name_loc, loc);
        };
        if let Some(q) = qual
            && !self.same_entity(q.ctx, c)
        {
            return self.reflect_bad_operand(name_loc, loc);
        }
        self.reflect_destructor_of(c, name_loc, loc, end)
    }

    /// The reflection of class `c`'s destructor, an implicit one
    /// declared first; `^^` of a class with none (incomplete, or a
    /// dependent pattern without a user-declared one) is ill-formed.
    pub(crate) fn reflect_destructor_of(
        &mut self,
        c: DeclId,
        name_loc: Loc,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        let def = self.ast.definition(c).unwrap_or(c);
        self.ensure_implicit(def, crate::class::SpecialKind::Dtor);
        match self.ast.destructor(def) {
            Some(d) => self.reflect_entity(d, name_loc, loc, end),
            None => self.reflect_bad_operand(name_loc, loc),
        }
    }

    /// `^^nns name` with a dependent `nns` (`^^T::x`, `^^T::template
    /// m<U>`, `^^T::operator=`, `^^T::~T`; [temp.dep.constexpr]): a
    /// value-dependent reflection of the `DependentScopeDeclRefExpr`,
    /// resolved by substitution (`templ/subst/exprs.rs`) to whatever the
    /// qualified-id names then.
    #[allow(clippy::too_many_arguments, reason = "the parts of the qualified-id")]
    pub fn act_on_reflect_dependent(
        &mut self,
        (nns, begin): (kiln_ast::ids::NnsId, Loc),
        name: DeclName,
        targs: Option<kiln_ast::ArgListId>,
        template_kw: Option<Loc>,
        name_loc: Loc,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        let r = self.act_dependent_scope_ref(nns, name, targs, begin, name_loc);
        self.set_template_kw(r, template_kw);
        let info = self.meta_info_type();
        let e = self.builder().dependent_reflect_expr(r, info, loc, end);
        self.note_consteval_only(e);
        e
    }

    /// `^^template-id` ([expr.reflect]/6-7): a class or alias template
    /// specialization is a type, a variable template specialization a
    /// variable, a function template-id the unique specialization it
    /// selects without a target ([over.over]).
    pub fn act_on_reflect_template_id(
        &mut self,
        r: TemplateIdRef,
        name_loc: Loc,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        let info = self.meta_info_type();
        let e = match r {
            TemplateIdRef::Type(t) => return self.act_on_reflect_type(t, loc, end),
            TemplateIdRef::Value(e) => e,
            // The one specialization the template-id names without a
            // target ([over.over]/3, [temp.arg.explicit]/4), found as
            // `&f<A>` finds it but not referenced: a member function
            // template's needs no object.
            TemplateIdRef::Overload(e) => match self.single_template_spec(e) {
                Some((d, _)) if !self.expr_has_error(e) => {
                    return self.reflect_entity(d, name_loc, loc, end);
                }
                _ => e,
            },
        };
        if self.expr_has_error(e) {
            return self.recover(info, &[], loc);
        }
        match self.ast.exprs.get(e).kind() {
            ExprKind::DeclRefExpr => {
                let d = self.ast.exprs.decl(e);
                self.reflect_entity(d, name_loc, loc, end)
            }
            ExprKind::UnresolvedLookupExpr if self.expr_is_value_dependent(e) => {
                self.unsupported("reflection of a dependent template-id", name_loc);
                self.recover(info, &[], loc)
            }
            ExprKind::UnresolvedLookupExpr => self.reflect_overload_set(name_loc, loc),
            _ => self.reflect_bad_operand(name_loc, loc),
        }
    }

    /// The reflection of the entity declaration `d` names (after lookup;
    /// also substitution's remapped declaration).
    pub(crate) fn reflect_entity(
        &mut self,
        d: DeclId,
        name_loc: Loc,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        match self.kind_of(d) {
            // A type found by name (an injected-class-name hidden from the
            // type-id parse, say): its type.
            DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::CXXRecord | DeclKind::Enum => {
                match self.ast.decls.get(d).ty() {
                    Some(t) => self.act_on_reflect_type(t, loc, end),
                    None => self.reflect_bad_operand(name_loc, loc),
                }
            }
            // [expr.reflect]/7: an id-expression naming a non-type template
            // parameter is ill-formed.
            DeclKind::NonTypeTemplateParm => {
                let text = self.decl_name(d);
                let e = self.diag(DiagId::sema_reflect_nttp, name_loc).arg(text);
                self.emit(e);
                let info = self.meta_info_type();
                self.recover(info, &[], loc)
            }
            DeclKind::CXXConstructor => self.reflect_overload_set(name_loc, loc),
            _ => match self.reflectee_of_decl(d) {
                Some(r) => self.make_reflect_expr(r, loc, end),
                None => self.reflect_bad_operand(name_loc, loc),
            },
        }
    }

    /// The reflectee of the entity declaration `d` declares, keyed as
    /// `^^` keys it (spec/reflection-repr.md §1): a namespace, alias,
    /// template, value entity, or a type (its declared type, a typedef's
    /// alias kept). `None` for anything else (a constructor, a template
    /// parameter, a declaration with no entity).
    pub(crate) fn reflectee_of_decl(&mut self, d: DeclId) -> Option<Reflectee> {
        // One reflection per entity (`Sema::same_entity`).
        let first = self.entity_decl(d);
        Some(match self.kind_of(d) {
            DeclKind::Namespace | DeclKind::TranslationUnit => Reflectee::Namespace(first),
            DeclKind::NamespaceAlias => Reflectee::NamespaceAlias(first),
            DeclKind::ClassTemplate
            | DeclKind::FunctionTemplate
            | DeclKind::VarTemplate
            | DeclKind::TypeAliasTemplate
            | DeclKind::Concept
            | DeclKind::TemplateTemplateParm => {
                let n = self.template_name_for(first);
                Reflectee::Template(self.ast.types.canonical_template_name(n))
            }
            DeclKind::Var
            | DeclKind::ParmVar
            | DeclKind::Binding
            | DeclKind::VarTemplateSpecialization
            | DeclKind::Field
            | DeclKind::IndirectField
            | DeclKind::EnumConstant
            | DeclKind::Function
            | DeclKind::CXXMethod
            | DeclKind::CXXConversion
            | DeclKind::CXXDestructor => Reflectee::Decl(first),
            DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::CXXRecord | DeclKind::Enum => {
                let t = self.ast.decls.get(d).ty().filter(|&t| t != QualType::ERROR)?;
                Reflectee::Type(self.ast.reflection_type(t))
            }
            _ => return None,
        })
    }

    /// The `CXXReflectExpr` of `r`, an expression of consteval-only type
    /// ([`Sema::note_consteval_only`]); past the reflection table's limit,
    /// the fatal `sema_too_many_reflections` and a recovery expression
    /// ([`Sema::build_reflect_expr`]).
    pub(crate) fn make_reflect_expr(&mut self, r: Reflectee, loc: Loc, end: Loc) -> ExprId {
        let info = self.meta_info_type();
        let e = self.build_reflect_expr(r, info, loc, end);
        if self.ast.exprs.get(e).kind() == ExprKind::CXXReflectExpr {
            self.note_consteval_only(e);
        }
        e
    }

    /// [expr.reflect]/7: an id-expression that names an overload set does
    /// not designate an entity.
    pub fn reflect_overload_set(&mut self, name_loc: Loc, loc: Loc) -> ExprId {
        self.error(DiagId::sema_reflect_overload_set, name_loc);
        let info = self.meta_info_type();
        self.recover(info, &[], loc)
    }

    /// [expr.reflect]/1: the operand is none of the reflectable forms.
    pub fn reflect_bad_operand(&mut self, at: Loc, loc: Loc) -> ExprId {
        self.error(DiagId::sema_reflect_operand, at);
        let info = self.meta_info_type();
        self.recover(info, &[], loc)
    }

    /// The namespace or class `name ::` names in `ctx` (the current scope
    /// when `None`), for the parser's bare-template-name lookahead: plain
    /// namespaces, namespace aliases and classes only.
    pub fn lookup_nested_ctx(&self, name: Symbol, ctx: Option<DeclId>) -> Option<DeclId> {
        let found = match ctx {
            Some(c) => self.lookup_qualified(c, name, LookupKind::Nested),
            None => self.lookup_name(name, LookupKind::Nested),
        };
        let &[d] = found.decls.as_slice() else { return None };
        let d = self.ast.resolve_alias(self.ast.strip_shadow(d));
        matches!(self.kind_of(d), DeclKind::Namespace | DeclKind::CXXRecord).then_some(d)
    }

    /// True if `name` in `ctx` (the current scope when `None`) finds a
    /// template itself — not an injected-class-name, which is a type
    /// when no `<` follows ([temp.local]/1) — for the parser's
    /// bare-template-name lookahead.
    pub fn names_template(&self, name: Symbol, ctx: Option<DeclId>) -> bool {
        let found = match ctx {
            Some(c) => self.lookup_qualified(c, name, LookupKind::Ordinary),
            None => self.lookup_name(name, LookupKind::Ordinary),
        };
        found.decls.iter().any(|&d| {
            matches!(
                self.kind_of(self.ast.strip_shadow(d)),
                DeclKind::ClassTemplate
                    | DeclKind::FunctionTemplate
                    | DeclKind::VarTemplate
                    | DeclKind::TypeAliasTemplate
                    | DeclKind::Concept
                    | DeclKind::TemplateTemplateParm
            )
        })
    }

    /// True if `c` is a class whose name is `name` (`C::C` names its
    /// constructors, [class.qual]/2).
    pub fn is_class_named(&self, c: DeclId, name: Symbol) -> bool {
        self.kind_of(c) == DeclKind::CXXRecord
            && self.ast.decls.get(c).name().as_ident() == Some(name)
    }
}
