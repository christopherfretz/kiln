//! Redeclarations of a name as a different kind of entity involving
//! templates, typedefs and namespaces ([basic.scope.scope]/4,
//! [temp.pre]/7, [dcl.type.elab]/2): clang's "redefinition of 'X' as
//! different kind of symbol" between a class template and any other
//! declaration of its scope, "definition of type 'X' conflicts with
//! typedef of the same name", and "template 'X' cannot be referenced
//! with the 'struct' specifier" (spec/sema-templates.md §3.13).

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclKind, FriendKind};
use kiln_ast::names::DeclName;
use kiln_ast::types::TagKeyword;
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::decl::sym_str;
use crate::lookup::LookupKind;

/// What a new declaration whose name may conflict is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NewKind {
    /// A class template (a primary template's declaration or definition).
    ClassTemplate,
    /// A non-template class or enumeration declaration or definition.
    Tag,
    /// A non-template variable.
    Var,
    /// A non-template function.
    Function,
}

impl Sema<'_, '_> {
    /// The declaration of the current scope a new declaration `new` of
    /// `name` conflicts with, by kind (clang's IDNS-filtered redeclaration
    /// lookup): a class template conflicts with anything but a class
    /// template; a tag with a class, variable or alias template, a
    /// concept or a namespace; a variable with any template or concept; a
    /// function with any template but a function template (overloads).
    /// Friends not yet declared in the scope are not found.
    pub(crate) fn template_kind_conflict(&self, name: Symbol, new: NewKind) -> Option<DeclId> {
        self.lookup_current_scope(name).into_iter().rev().find(|&d| {
            if self.ast.decls.get(d).misc().friend() == FriendKind::Undeclared {
                return false;
            }
            let k = self.kind_of(d);
            let templ = matches!(
                k,
                DeclKind::ClassTemplate
                    | DeclKind::VarTemplate
                    | DeclKind::TypeAliasTemplate
                    | DeclKind::Concept
            );
            match new {
                NewKind::ClassTemplate => {
                    k != DeclKind::ClassTemplate && self.ast.described_template(d).is_none()
                }
                NewKind::Tag => {
                    templ || matches!(k, DeclKind::Namespace | DeclKind::NamespaceAlias)
                }
                NewKind::Var => templ || k == DeclKind::FunctionTemplate,
                NewKind::Function => templ,
            }
        })
    }

    /// Reports a declaration of `name` at `loc` conflicting by kind with
    /// one of its scope ([`Sema::template_kind_conflict`]). True when
    /// diagnosed.
    pub(crate) fn diagnose_template_kind_conflict(
        &mut self,
        name: Symbol,
        new: NewKind,
        loc: Loc,
    ) -> bool {
        let Some(p) = self.template_kind_conflict(name, new) else { return false };
        let msg = self.diag(DiagId::sema_redefinition_different_kind, loc).arg(sym_str(self, name));
        let msg = msg.with_note(self.diag(DiagId::note_previous_definition, self.conflict_loc(p)));
        self.emit(msg);
        true
    }

    /// A non-template class or enumeration declared or defined as `name`
    /// where its scope has a typedef of that name that names no tag
    /// `name` of the typedef's scope ([dcl.typedef]/6: clang's
    /// `err_tag_definition_of_typedef`). True when diagnosed.
    pub(crate) fn diagnose_tag_typedef_conflict(&mut self, name: Symbol, loc: Loc) -> bool {
        let found = self.lookup_current_scope(name).into_iter().rev().find(|&d| {
            matches!(self.kind_of(d), DeclKind::Typedef | DeclKind::TypeAlias)
                && self.ast.decls.get(d).misc().friend() != FriendKind::Undeclared
        });
        let Some(td) = found else { return false };
        let underlying = self.ast.decls.typedef(td).map(|t| t.underlying);
        let tag = underlying.and_then(|u| {
            let t = self.ast.types.canonical(u);
            self.ast.types.as_record(t).or_else(|| self.ast.types.as_enum(t))
        });
        if let Some(tag) = tag
            && self.ast.decls.get(tag).name() == DeclName::ident(name)
            && self.ast.decls.get(tag).ctx().map(|c| self.ast.redecl_context(c))
                == self.ast.decls.get(td).ctx().map(|c| self.ast.redecl_context(c))
        {
            return false;
        }
        let alias = u32::from(self.kind_of(td) == DeclKind::TypeAlias);
        let quoted = format!("'{}'", sym_str(self, name));
        let note =
            self.diag(DiagId::note_entity_declared_here, self.ast.decls.get(td).loc()).arg(quoted);
        let msg = self
            .diag(DiagId::sema_tag_definition_of_typedef, loc)
            .arg(sym_str(self, name))
            .arg(alias)
            .with_note(note);
        self.emit(msg);
        true
    }

    /// An unqualified elaborated-type-specifier `keyword name` whose tag
    /// lookup found `tag` in an outer scope while a nearer scope declares
    /// `name` as a typedef, type alias or class template
    /// ([basic.lookup.elab]/1 ignores only non-type names, so that one is
    /// found; [dcl.type.elab]/2 then makes the reference ill-formed).
    /// clang `LookupTagName` sees type names too, then
    /// `err_tag_reference_non_tag`. A typedef of the same name in the
    /// same scope naming a tag (`typedef struct A A;`) is looked through,
    /// as clang's `ActOnTag` does. True when diagnosed.
    pub(crate) fn diagnose_tag_hidden_by_type_name(
        &mut self,
        keyword: TagKeyword,
        name: Symbol,
        tag: DeclId,
        loc: Loc,
    ) -> bool {
        let found = self.lookup_name(name, LookupKind::Type).decls;
        let first = self.ast.first(tag);
        if found.is_empty() || found.iter().any(|&d| self.ast.first(d) == first) {
            return false;
        }
        if found.iter().any(|&d| self.typedef_names_own_tag(d, name)) {
            return false;
        }
        self.diagnose_tag_reference_non_tag(keyword, name, &found, loc)
    }

    /// Whether typedef `td` names a class or enumeration `name` of its
    /// own scope (`typedef struct A A;`), which an elaborated name looks
    /// through.
    fn typedef_names_own_tag(&self, td: DeclId, name: Symbol) -> bool {
        let Some(u) = self.ast.decls.typedef(td).map(|t| t.underlying) else { return false };
        let t = self.ast.types.canonical(u);
        let Some(x) = self.ast.types.as_record(t).or_else(|| self.ast.types.as_enum(t)) else {
            return false;
        };
        self.ast.decls.get(x).name() == DeclName::ident(name)
            && self.ast.decls.get(x).ctx().map(|c| self.ast.redecl_context(c))
                == self.ast.decls.get(td).ctx().map(|c| self.ast.redecl_context(c))
    }

    /// `friend class-key name;` whose ordinary lookup finds a typedef,
    /// type alias, class template or alias template (`found`, the last
    /// one counting) that is not `prev`, the class the friend's tag
    /// lookup chose ([dcl.type.elab]/2-4: clang `ActOnTag` sees type
    /// names in the friend's lookup too, then `err_tag_reference_non_tag`).
    /// `in_scope` is false for a name declared outside the innermost
    /// enclosing namespace (the friend then declares a new class there).
    /// True when diagnosed.
    pub(crate) fn diagnose_friend_tag_non_tag(
        &mut self,
        keyword: TagKeyword,
        name: Symbol,
        prev: Option<DeclId>,
        found: &[DeclId],
        in_scope: bool,
        loc: Loc,
    ) -> bool {
        let Some(&t) = found.last() else { return false };
        let non_tag = matches!(
            self.kind_of(t),
            DeclKind::Typedef
                | DeclKind::TypeAlias
                | DeclKind::ClassTemplate
                | DeclKind::TypeAliasTemplate
        );
        if !non_tag
            || !in_scope
            || prev.is_some_and(|p| self.ast.first(p) == self.ast.first(t))
            || self.typedef_names_own_tag(t, name)
        {
            return false;
        }
        self.diagnose_tag_reference_non_tag(keyword, name, &found[found.len() - 1..], loc)
    }

    /// An elaborated-type-specifier `keyword name` whose tag lookup found
    /// `found`, a typedef, a type alias or a template rather than a class
    /// ([dcl.type.elab]/2: clang's `err_tag_reference_non_tag`). True when
    /// diagnosed.
    pub(crate) fn diagnose_tag_reference_non_tag(
        &mut self,
        keyword: TagKeyword,
        name: Symbol,
        found: &[DeclId],
        loc: Loc,
    ) -> bool {
        let pick = found.iter().rev().find_map(|&d| {
            let sel: u32 = match self.kind_of(d) {
                DeclKind::Typedef => 4,
                DeclKind::TypeAlias => 5,
                DeclKind::ClassTemplate => 6,
                DeclKind::TypeAliasTemplate => 7,
                DeclKind::TemplateTemplateParm => 8,
                _ => return None,
            };
            Some((d, sel))
        });
        let Some((d, sel)) = pick else { return false };
        if sel == 8 {
            // A template template parameter: clang's
            // `DiagnoseTemplateParameterShadow` ([temp.local]/6).
            self.check_template_param_shadow(name, loc);
            return true;
        }
        let note = self.diag(DiagId::note_declared_at, self.conflict_loc(d));
        let msg = self
            .diag(DiagId::sema_tag_reference_non_tag, loc)
            .arg(sel)
            .arg(sym_str(self, name))
            .arg(keyword.spelling())
            .with_note(note);
        self.emit(msg);
        true
    }
}

impl Sema<'_, '_> {
    /// Where clang's note for `d` points: its name, or for an alias
    /// template its `using` keyword.
    pub(crate) fn conflict_loc(&self, d: DeclId) -> Loc {
        (self.kind_of(d) == DeclKind::TypeAliasTemplate)
            .then(|| self.ast.template_info(d).and_then(|i| i.pattern))
            .flatten()
            .and_then(|p| self.templ.alias_using_loc.get(&p).copied())
            .unwrap_or_else(|| self.ast.decls.get(d).loc())
    }

    /// `template<> struct S;` naming the non-template class `prev`
    /// ([temp.expl.spec]/1 specializes a template or a member of one):
    /// clang's `err_spec_member_not_instantiated`, after the extraneous
    /// `template<>` error.
    pub(crate) fn diag_spec_member_not_instantiated(&mut self, prev: DeclId, loc: Loc) {
        let note = self.diag(DiagId::note_specialized_decl, self.ast.decls.get(prev).loc());
        let msg = self
            .diag(DiagId::sema_spec_member_not_instantiated, loc)
            .arg(self.qualified_decl_name(prev))
            .with_note(note);
        self.emit(msg);
    }
}
