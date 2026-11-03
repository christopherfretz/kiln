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
        let info = self.meta_info_type();
        let lookup = |s: &Self, k| match qual {
            Some(q) => s.lookup_qualified(q.ctx, name, k),
            None => s.lookup_name(name, k),
        };
        let mut found = lookup(self, LookupKind::Ordinary);
        if found.decls.is_empty() {
            found = lookup(self, LookupKind::Nested);
        }
        let mut decls: Vec<DeclId> =
            found.decls.iter().map(|&d| self.ast.strip_shadow(d)).collect();
        decls.dedup_by_key(|d| self.ast.first(*d)); // same_entity: raw ok
        match decls.as_slice() {
            [] => {
                let text = crate::decl::sym_str(self, name);
                let d = self.diag(DiagId::sema_undeclared_var_use, name_loc).arg(text);
                self.emit(d);
                self.recover(info, &[], loc)
            }
            &[d] => self.reflect_entity(d, name_loc, loc, end),
            _ => self.reflect_overload_set(name_loc, loc),
        }
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
            TemplateIdRef::Overload(e) => self.resolve_single_template_id(e),
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
        let info = self.meta_info_type();
        let first = self.ast.first(d);
        let r = match self.kind_of(d) {
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
            // A type found by name (an injected-class-name hidden from the
            // type-id parse, say): its type.
            DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::CXXRecord | DeclKind::Enum => {
                match self.ast.decls.get(d).ty() {
                    Some(t) => return self.act_on_reflect_type(t, loc, end),
                    None => return self.reflect_bad_operand(name_loc, loc),
                }
            }
            // [expr.reflect]/7: an id-expression naming a non-type template
            // parameter is ill-formed.
            DeclKind::NonTypeTemplateParm => {
                let text = self.decl_name(d);
                let e = self.diag(DiagId::sema_reflect_nttp, name_loc).arg(text);
                self.emit(e);
                return self.recover(info, &[], loc);
            }
            DeclKind::CXXConstructor => return self.reflect_overload_set(name_loc, loc),
            _ => return self.reflect_bad_operand(name_loc, loc),
        };
        self.make_reflect_expr(r, loc, end)
    }

    /// The `CXXReflectExpr` of `r`, an expression of consteval-only type
    /// ([`Sema::note_consteval_only`]).
    pub(crate) fn make_reflect_expr(&mut self, r: Reflectee, loc: Loc, end: Loc) -> ExprId {
        let info = self.meta_info_type();
        let e = self.builder().reflect_expr(r, info, loc, end);
        self.note_consteval_only(e);
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
