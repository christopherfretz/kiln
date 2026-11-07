//! Templates ([temp]; spec/sema-templates.md §11, spec/parse.md §6):
//! template heads and their parameters, template argument lists,
//! requires-clauses, requires-expressions and concept definitions, and
//! the template forms recognised but deferred to the engine (explicit
//! instantiation and specialization, partial specialization).
//!
//! The parser keeps its no-rewind model: a `<` after a name is a
//! template argument list exactly when the name classifies as a template
//! ([temp.names]/3, [`crate::parser::is_template_class`]), and the end of
//! a list is found by [`Parser::skip_template_args`] lookahead. Inside a
//! list `>` is not an operator (`greater_is_op`), and the first `>` of a
//! `>>`-like token closes it ([`Parser::split_greater`]).

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::BinaryOp;
use kiln_ast::templates::Requirement;
use kiln_ast::types::{ArgView, TagKeyword, TemplateArg, TemplateName};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_lex::{Punct, Std, TokKind};
use kiln_sema::DeclSite;
use kiln_sema::templ::actions::TemplateIdRef;
use kiln_sema::using::Qualifier;
use kiln_sema::{Declarator, DeclaratorContext, NameClass};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::expr::prec;
use crate::keywords::Kw;
use crate::parser::{ParamTypename, Parser, is_template_class};
use crate::tentative::TypeIdContext;

/// The template-declaration state of one declarator group
/// ([`Parser::template_group`], [`Parser::next_declarator`]).
pub(crate) struct TemplateGroup {
    /// What heads the declaration; `None` once its multiple declarators
    /// were reported (or for a plain declaration).
    kind: Option<TemplateDeclKind>,
    /// What each later declarator gets back.
    saved: Option<kiln_sema::templ::TemplateDeclState>,
}

/// The kind of declaration a template head, `template<>` or an
/// explicit-instantiation `template` introduces (clang's
/// `ParsedTemplateKind`), for "can only declare a single entity".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TemplateDeclKind {
    Template = 0,
    ExplicitSpecialization = 1,
    ExplicitInstantiation = 2,
}

/// The kind of a template-parameter ([temp.param]/1), decided from its
/// first tokens as clang's `isStartOfTemplateTypeParameter` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParamKind {
    /// `class T`, `typename... Ts`, `typename = int`.
    Type,
    /// `template<...> class TT`.
    Template,
    /// `C T`, `N::C<int>... Ts`: a type-constraint naming concept `.0`;
    /// `.1` is the index just past the concept's name (and arguments).
    Constrained(DeclId, usize),
    /// Everything else: a parameter-declaration ([temp.param]/2).
    NonType,
}

/// True for a token that can end a type parameter's name
/// ([temp.param]/1): `=`, `,`, `>` or `>>`.
fn ends_type_param(p: Option<Punct>) -> bool {
    matches!(p, Some(Punct::Equal | Punct::Comma | Punct::Greater | Punct::GreaterGreater))
}

impl Parser<'_, '_> {
    /// True at a token whose first character is `>` (one that can close
    /// a template argument or parameter list).
    pub(crate) fn at_greater(&self) -> bool {
        matches!(
            Self::punct_of(self.tok()),
            Some(
                Punct::Greater
                    | Punct::GreaterGreater
                    | Punct::GreaterEqual
                    | Punct::GreaterGreaterEqual
            )
        )
    }

    // -- template declarations ---------------------------------------------------------

    /// Whether the `enum` at the current token names its enumeration with
    /// a nested-name-specifier (`enum class A<int>::E`).
    fn at_qualified_enum(&self) -> bool {
        let mut n = 1;
        if matches!(self.kw_of(self.peek(1)), Some(Kw::Class | Kw::Struct)) {
            n += 1;
        }
        self.peek_is(n, Punct::ColonColon)
            || (matches!(self.peek(n).kind, TokKind::Identifier(_))
                && (self.peek_is(n + 1, Punct::ColonColon) || self.peek_is(n + 1, Punct::Less)))
    }

    /// An unqualified `enum` after `template<>` or an explicit
    /// instantiation's `template`: like clang, "enumeration cannot be a
    /// template" at the token after the name and enum-base and
    /// "declaration does not declare anything" at `enum`; the declaration
    /// is skipped.
    fn enum_template_declaration(&mut self) {
        let enum_loc = self.loc();
        let mut n = 1;
        while !matches!(
            self.peek(n).kind,
            TokKind::Punct(Punct::Semi | Punct::LBrace | Punct::RBrace) | TokKind::Eof
        ) {
            n += 1;
        }
        self.s.error(DiagId::sema_enum_template, self.peek(n).span.lo);
        self.s.error(DiagId::sema_no_declarators_error, enum_loc);
        self.skip_unsupported_declaration();
    }

    /// A declaration starting at `template` or `extern template`
    /// ([temp.pre]/1, [temp.explicit], [temp.expl.spec]) in namespace
    /// (`File`) or class (`Member`) scope.
    pub(crate) fn parse_template_declaration(&mut self, ctx: DeclaratorContext) {
        let extern_loc = self.eat_kw(Kw::Extern);
        let loc = self.bump().span.lo;
        if extern_loc.is_some() || !self.at(Punct::Less) {
            // explicit-instantiation ([temp.explicit]/1).
            if self.at_partial_specialization() || !self.at_spec_tag_or_template() {
                self.s.act_explicit_inst_start(extern_loc, loc);
                self.template_decl_kind = Some(TemplateDeclKind::ExplicitInstantiation);
                self.parse_spec_declaration(ctx);
                self.template_decl_kind = None;
                self.s.act_explicit_inst_end();
                return;
            }
            if self.at_kw(Kw::Enum) && !self.at_qualified_enum() {
                self.enum_template_declaration();
                return;
            }
            if self.at_kw(Kw::Enum) {
                // [temp.explicit]/3 names no enumerations (clang's
                // `err_explicit_instantiation_enum`).
                self.error_here(DiagId::sema_enum_explicit_instantiation);
            } else {
                let _ = self.s.act_explicit_instantiation(extern_loc, loc);
            }
            self.skip_unsupported_declaration();
            return;
        }
        if self.peek_is(1, Punct::Greater) {
            // explicit-specialization ([temp.expl.spec]/1).
            self.bump();
            self.bump();
            // `template<> template<> R C<args>::f<args>(...)`: a member
            // template of a class template specialization.
            let mut empty_heads = vec![loc];
            while self.at_kw(Kw::Template)
                && self.peek_is(1, Punct::Less)
                && self.peek_is(2, Punct::Greater)
            {
                empty_heads.push(self.bump().span.lo);
                self.bump();
                self.bump();
            }
            if self.at_kw(Kw::Template) && self.peek_is(1, Punct::Less) {
                // `template<> template<class U> struct A<int>::B {...}`: a
                // member template of an implicitly instantiated class
                // specialized as a template ([temp.expl.spec]/16).
                self.s.act_member_template_spec_start(loc);
                self.s.act_explicit_spec_headers(&empty_heads);
                self.parse_template_declaration(ctx);
                self.s.act_member_template_spec_end();
                return;
            }
            if self.at_kw(Kw::Enum) && !self.at_qualified_enum() {
                self.enum_template_declaration();
                return;
            }
            // `template<> enum class A<int>::E : int { ... };` explicitly
            // specializes a member enumeration ([temp.expl.spec]/1).
            if self.at_partial_specialization()
                || self.at_var_template_spec()
                || self.at_kw(Kw::Enum)
                || !self.at_spec_tag_or_template()
            {
                self.s.act_explicit_spec_start(loc);
                self.s.act_explicit_spec_headers(&empty_heads);
                self.template_decl_kind = Some(TemplateDeclKind::ExplicitSpecialization);
                self.parse_spec_declaration(ctx);
                self.template_decl_kind = None;
                self.s.act_explicit_spec_end();
                return;
            }
            if self.explicit_spec_of_non_template(loc, ctx) {
                return;
            }
            let _ = self.s.act_explicit_specialization(false, loc);
            self.skip_unsupported_declaration();
            return;
        }
        // [class.access]/7: the names in a namespace-scope template head
        // are checked in the context of the declared entity
        // (`template<A::I> struct A::Q`), closed by the declaration.
        let level = self.s.delayed_access_level();
        let delays = ctx != DeclaratorContext::Member && self.template_delay_level.is_none();
        if delays {
            self.s.begin_delayed_access();
        }
        let l_angle = self.bump().span.lo;
        self.s.act_on_template_params_start(loc, l_angle);
        let r_angle = self.parse_template_param_list_tail();
        let requires = if self.at_kw(Kw::Requires) {
            self.bump();
            self.parse_requires_clause()
        } else {
            None
        };
        let head = self.s.act_template_head(requires, r_angle);
        self.trace(|_| "template_head".to_owned());
        if self.failed_head_as_empty(head, loc, ctx) {
            if delays {
                self.s.end_delayed_access_to(level, None);
            }
            return;
        }
        if delays {
            self.template_delay_level = Some(level);
        }
        self.parse_templated_declaration(ctx, head, loc);
        if delays {
            self.template_delay_level = None;
            self.s.end_delayed_access_to(level, None);
        }
        self.s.act_on_finish_template_decl();
    }

    /// `class-key identifier` then `{`, `:`, `;` or `final`: a class
    /// declaration whose name is not a template-id.
    fn at_plain_tag_decl(&self) -> bool {
        if !matches!(self.kw(), Some(Kw::Class | Kw::Struct | Kw::Union)) {
            return false;
        }
        let name = self.tok_at(self.pos() + 1);
        if !matches!(name.kind, TokKind::Identifier(_)) || self.kw_of(name).is_some() {
            return false;
        }
        let t = self.tok_at(self.pos() + 2);
        matches!(Self::punct_of(t), Some(Punct::LBrace | Punct::Colon | Punct::Semi))
            || self.is_final_tok(t)
    }

    /// `template<>` (at `loc`) before a class declaration that is no
    /// template-id, or a template-id on a non-template: parsed, with the
    /// head diagnosed as extraneous or (`template<> struct NT<int> {}`)
    /// the template-id by the class-specifier, which declares `NT`
    /// ([temp.expl.spec]/1; clang's
    /// `err_template_spec_needs_template_parameters` recovery). True
    /// when handled.
    fn explicit_spec_of_non_template(&mut self, loc: Loc, ctx: DeclaratorContext) -> bool {
        if self.at_plain_tag_decl() || self.at_namespace_member_tag_decl() {
            self.extraneous_empty_template_head(loc, ctx);
        } else if self.at_non_template_spec() {
            self.parse_spec_declaration(ctx);
        } else {
            return false;
        }
        true
    }

    /// `class-key N::...::identifier` then `{`, `:`, `;` or `final`, the
    /// qualifier naming a namespace: a class declaration whose name is no
    /// template-id (`template<> struct N::P {}`, [temp.expl.spec]/1). A
    /// class qualifier (`template<> struct AI::B {}`, a member of a
    /// specialization) is not this form.
    fn at_namespace_member_tag_decl(&self) -> bool {
        if !matches!(self.kw(), Some(Kw::Class | Kw::Struct | Kw::Union)) {
            return false;
        }
        let i = self.pos() + 1;
        let Some((end, _, _, _, Some(q))) = self.scan_name_qual_at(i) else { return false };
        if (i..end).any(|j| self.tok_at(j).kind == TokKind::Punct(Punct::Less))
            || !matches!(
                self.s.ast.decls.get(q).kind(),
                DeclKind::Namespace | DeclKind::TranslationUnit
            )
        {
            return false;
        }
        let t = self.tok_at(end);
        matches!(Self::punct_of(t), Some(Punct::LBrace | Punct::Colon | Punct::Semi))
            || self.is_final_tok(t)
    }

    /// `class-key [N::]identifier <` where the name is no template (or
    /// its qualifier names nothing): a template-id on a non-template,
    /// which the class-specifier diagnoses.
    fn at_non_template_spec(&self) -> bool {
        if !matches!(self.kw(), Some(Kw::Class | Kw::Struct | Kw::Union)) {
            return false;
        }
        let i = self.pos() + 1;
        if let Some((end, _, class, _)) = self.scan_name_at(i) {
            return !matches!(class, NameClass::TypeTemplate(_))
                && end > i
                && self.ident_of(self.tok_at(end - 1)).is_some()
                && self.tok_at(end).kind == TokKind::Punct(Punct::Less);
        }
        // A qualifier naming nothing (`template<> struct NS::Z<int>`):
        // `[::] identifier (:: identifier)* <`.
        let mut j = i + usize::from(self.tok_at(i).kind == TokKind::Punct(Punct::ColonColon));
        loop {
            if self.ident_of(self.tok_at(j)).is_none() || self.kw_of(self.tok_at(j)).is_some() {
                return false;
            }
            match self.tok_at(j + 1).kind {
                TokKind::Punct(Punct::ColonColon) => j += 2,
                TokKind::Punct(Punct::Less) => return j > i,
                _ => return false,
            }
        }
    }

    /// Every parameter of `head` failed (`template<template<class> static
    /// class TT> struct D`) before a plain class declaration: the head is
    /// an empty one, parsed as `extraneous_empty_template_head`.
    fn failed_head_as_empty(
        &mut self,
        head: kiln_ast::ids::TemplateParamListId,
        loc: Loc,
        ctx: DeclaratorContext,
    ) -> bool {
        let list = self.s.ast.templates.param_list(head).params;
        if !self.s.ast.lists.is_empty(list) || !self.at_plain_tag_decl() {
            return false;
        }
        self.s.act_on_finish_template_decl();
        self.extraneous_empty_template_head(loc, ctx);
        true
    }

    /// An empty template head on a class that is no template-id ([temp.pre]/1
    /// needs parameters, [temp.expl.spec]/1 a template-id): clang's
    /// `err_template_tag_noparams`, at the `template` keyword `loc`. The class
    /// is then declared as a non-template.
    fn extraneous_empty_template_head(&mut self, loc: Loc, ctx: DeclaratorContext) {
        let key = match self.kw() {
            Some(Kw::Class) => "class",
            Some(Kw::Union) => "union",
            _ => "struct",
        };
        // The declared name: the last component of a qualified one.
        let end = self.scan_name_at(self.pos() + 1).map_or(self.pos() + 2, |e| e.0);
        let name = self
            .ident_of(self.tok_at(end.max(self.pos() + 2) - 1))
            .map_or(String::new(), |n| self.s.ast.symbols().as_str(n).to_owned());
        let d = self.s.diag(DiagId::sema_template_tag_noparams, loc).arg(key).arg(name);
        self.s.extraneous_tag_head = Some(d);
        if ctx == DeclaratorContext::Member {
            self.parse_member_declaration();
        } else {
            self.parse_simple_declaration(DeclaratorContext::File, true);
        }
        // Not taken by a class declaration (none parsed): still reported.
        if let Some(d) = self.s.extraneous_tag_head.take() {
            self.emit(d);
        }
    }

    /// The class declaration of an explicit specialization or explicit
    /// instantiation ([temp.expl.spec]/1, [temp.explicit]/1).
    fn parse_spec_declaration(&mut self, ctx: DeclaratorContext) {
        if ctx == DeclaratorContext::Member {
            self.parse_member_declaration();
        } else {
            // [temp.spec.general]/6: its decl-specifiers and declarator
            // are not access checked.
            self.s.exempt_next_delayed_access(true);
            self.parse_simple_declaration(DeclaratorContext::File, true);
            self.s.exempt_next_delayed_access(false);
        }
    }

    /// The declaration after a template head ([temp.pre]/1: the
    /// template-head's declaration).
    fn parse_templated_declaration(
        &mut self,
        ctx: DeclaratorContext,
        head: kiln_ast::ids::TemplateParamListId,
        loc: Loc,
    ) {
        match self.kw() {
            Some(Kw::Template) => return self.parse_template_declaration(ctx),
            Some(Kw::Concept) => return self.parse_concept_definition(head),
            Some(Kw::Using) => {
                self.parse_using();
                return;
            }
            Some(Kw::StaticAssert) => {
                self.error_here(DiagId::parse_expected_unqualified_id);
                self.skip_unsupported_declaration();
                return;
            }
            _ => {}
        }
        let _ = loc;
        if ctx != DeclaratorContext::Member && self.at_friend_template() {
            // [class.friend]/1: a friend declaration appears in a class
            // (clang: "'friend' used outside of class").
            self.s.templ_unimplemented("friend templates outside a class", self.loc());
            self.skip_unsupported_declaration();
            return;
        }
        self.template_decl_kind = Some(TemplateDeclKind::Template);
        if ctx == DeclaratorContext::Member {
            self.parse_member_declaration();
        } else {
            self.parse_simple_declaration(DeclaratorContext::File, true);
        }
        self.template_decl_kind = None;
    }

    /// The template-declaration state of a declarator group whose
    /// declaration is headed by `kind` (see [`Parser::next_declarator`]),
    /// taken after the decl-specifiers.
    pub(crate) fn template_group(&self, kind: Option<TemplateDeclKind>) -> TemplateGroup {
        TemplateGroup { kind, saved: kind.map(|_| self.s.save_template_decl_state()) }
    }

    /// After a declarator of a group: true if a `,` follows and the
    /// group goes on. [temp.pre]/5, [temp.expl.spec], [temp.explicit]: a
    /// template declaration, explicit specialization or explicit
    /// instantiation declares one entity; at the first `,` of such a
    /// group, clang's `err_multiple_template_declarators`. As in clang's
    /// `ParseDeclGroup`, the later declarators are still declared, each
    /// as the first one was (a template with the same heads, or the same
    /// kind of specialization or instantiation).
    pub(crate) fn next_declarator(&mut self, g: &mut TemplateGroup) -> bool {
        if !self.at(Punct::Comma) {
            return false;
        }
        if let Some(kind) = g.kind.take() {
            let d = self
                .s
                .diag(DiagId::parse_multiple_template_declarators, self.loc())
                .arg(kind as u8);
            self.emit(d);
        }
        self.bump();
        if let Some(saved) = &g.saved {
            self.s.restore_template_decl_state(saved);
        }
        true
    }

    /// A class-key, `enum` or `template` after `template<>` or an
    /// explicit-instantiation `template`: a declaration other than a
    /// function or variable one (member classes, member templates).
    fn at_spec_tag_or_template(&self) -> bool {
        matches!(self.kw(), Some(Kw::Class | Kw::Struct | Kw::Union | Kw::Enum | Kw::Template))
    }

    /// A simple-declaration whose declarator-id is a template-id naming
    /// a variable template (`constexpr bool v<int> = true;`), scanned up
    /// to its initializer or `;` ([temp.expl.spec]/1).
    fn at_var_template_spec(&self) -> bool {
        let mut i = self.pos();
        loop {
            let t = self.tok_at(i);
            match t.kind {
                TokKind::Eof
                | TokKind::Punct(
                    Punct::Semi | Punct::Equal | Punct::LBrace | Punct::LParen | Punct::LSquare,
                ) => return false,
                TokKind::Identifier(_) if self.kw_of(t).is_some() => i += 1,
                TokKind::Identifier(_) | TokKind::Punct(Punct::ColonColon) => {
                    let Some((end, _, class, _)) = self.scan_name_at(i) else { return false };
                    if matches!(class, NameClass::VarTemplate(_))
                        && (i..end).any(|j| self.tok_at(j).kind == TokKind::Punct(Punct::Less))
                    {
                        return true;
                    }
                    i = end.max(i + 1);
                }
                _ => i += 1,
            }
        }
    }

    /// `class-key [attrs] name <` with `name` a class template (`attrs`
    /// including `_Nullable`): a partial
    /// specialization ([temp.spec.partial]/1).
    fn at_partial_specialization(&self) -> bool {
        if !matches!(self.kw(), Some(Kw::Class | Kw::Struct | Kw::Union)) {
            return false;
        }
        let mut i = self.pos() + 1;
        loop {
            // `class _Nullable X<int>` (clang nullability on classes).
            if self.kw_of(self.tok_at(i)) == Some(Kw::Nullable) {
                i += 1;
                continue;
            }
            // `[[attr]]`, `alignas(...)` or `__attribute__((...))`: skip to
            // the matching close.
            let (open, close) = if self.tok_at(i).kind == TokKind::Punct(Punct::LSquare) {
                (Punct::LSquare, Punct::RSquare)
            } else if matches!(self.kw_of(self.tok_at(i)), Some(Kw::Attribute | Kw::Alignas))
                && self.tok_at(i + 1).kind == TokKind::Punct(Punct::LParen)
            {
                i += 1;
                (Punct::LParen, Punct::RParen)
            } else {
                break;
            };
            let mut depth = 0usize;
            loop {
                match Self::punct_of(self.tok_at(i)) {
                    Some(p) if p == open => depth += 1,
                    Some(p) if p == close => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    _ if self.tok_at(i).kind == TokKind::Eof => return false,
                    _ => {}
                }
                i += 1;
            }
        }
        let Some((end, _, class, _)) = self.scan_name_at(i) else { return false };
        let _ = class;
        // The last component of the scanned name carries the `<`.
        // Misplaced attributes and `final` after the template-id
        // (`struct A<int> [[x]] final {`) are reported by the tag parser.
        let mut end = end;
        loop {
            if self.is_final_tok(self.tok_at(end)) {
                end += 1;
                continue;
            }
            match self.after_cxx11_attrs_at(end) {
                Some(e) if e > end => end = e,
                _ => break,
            }
        }
        (i..end).any(|j| self.tok_at(j).kind == TokKind::Punct(Punct::Less))
            && matches!(
                Self::punct_of(self.tok_at(end)),
                Some(Punct::LBrace | Punct::Colon | Punct::Semi)
            )
    }

    /// `friend` among the decl-specifiers of a templated declaration
    /// ([temp.friend]): WP-070a.
    fn at_friend_template(&self) -> bool {
        let mut i = self.pos();
        loop {
            let t = self.tok_at(i);
            match self.kw_of(t) {
                Some(Kw::Friend) => return true,
                Some(k) if crate::decl::is_decl_specifier_kw(k) => i += 1,
                _ => return false,
            }
        }
    }

    /// `concept name = constraint-expression ;` after a template head
    /// ([temp.concept]/1).
    fn parse_concept_definition(&mut self, head: kiln_ast::ids::TemplateParamListId) {
        self.bump();
        if self.at_kw(Kw::Bool) {
            // The Concepts TS form `concept bool`, diagnosed at the name
            // (clang's `err_concept_legacy_bool_keyword`).
            self.bump();
            self.error_here(DiagId::parse_concept_legacy_bool);
        }
        let Some(name) = self.ident() else {
            self.error_here(DiagId::parse_expected_unqualified_id);
            self.skip_unsupported_declaration();
            return;
        };
        let name_loc = self.bump().span.lo;
        if self.s.ast.templates.param_list(head).requires.is_some() {
            // [temp.concept]/4: a requires-clause in the template head,
            // diagnosed at the name.
            let d = self.s.diag(DiagId::parse_concept_has_constraints, name_loc);
            self.emit(d);
        }
        if self.at_kw(Kw::Requires) {
            // [temp.concept]/4.
            self.error_here(DiagId::parse_concept_has_constraints);
            self.bump();
            let _ = self.parse_requires_clause();
        }
        if self.at_attr() {
            let mut attrs = Vec::new();
            self.parse_attributes(&mut attrs);
            self.s.reject_annotations_in(&attrs, Some("a concept"));
        }
        if self.eat(Punct::Equal).is_none() {
            let d = self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("'='");
            self.emit(d);
            self.skip_unsupported_declaration();
            return;
        }
        let Some(e) = self.parse_expr_prec(prec::LOR) else {
            self.skip_to_semi();
            return;
        };
        self.s.act_concept(head, name, name_loc, e);
        self.expect_semi_after("concept definition");
    }

    // -- template parameters -----------------------------------------------------------

    /// template-parameter-list `>` after the `<` ([temp.param]/1).
    /// Returns the `>`'s location; on an error, recovers to it.
    pub(crate) fn parse_template_param_list_tail(&mut self) -> Loc {
        if !self.at_greater() {
            loop {
                // A failed parameter is skipped to the next `,` or the
                // closing `>`; the list goes on after a `,` (clang's
                // `ParseTemplateParameterList`).
                if !self.parse_template_param() {
                    self.skip_until(
                        &[Punct::Comma, Punct::Greater, Punct::GreaterGreater],
                        true,
                        false,
                    );
                }
                if self.eat(Punct::Comma).is_none() {
                    break;
                }
            }
        }
        if self.at_greater() {
            return self.split_greater();
        }
        let d = self
            .s
            .diag(DiagId::parse_expected, self.loc())
            .arg("',' or '>' in template-parameter-list");
        self.emit(d);
        self.skip_until(&[Punct::Greater], true, false);
        if self.at_greater() { self.split_greater() } else { self.prev_end() }
    }

    /// The kind of the template-parameter starting at the current token.
    pub(crate) fn template_param_kind(&self) -> ParamKind {
        let next = Self::punct_of(self.peek(1));
        match self.kw() {
            Some(Kw::Template) => return ParamKind::Template,
            Some(Kw::Class) => {
                let named = self.ident_of(self.peek(1)).is_some()
                    && self.kw_of(self.peek(1)).is_none()
                    && ends_type_param(Self::punct_of(self.peek(2)));
                return if ends_type_param(next) || next == Some(Punct::Ellipsis) || named {
                    ParamKind::Type
                } else {
                    ParamKind::NonType
                };
            }
            Some(Kw::Typename) => {
                let mut after = self.peek(1);
                if self.ident_of(after).is_some() && self.kw_of(after).is_none() {
                    after = self.peek(2);
                }
                let p = Self::punct_of(after);
                return if ends_type_param(p)
                    || p == Some(Punct::Ellipsis)
                    || matches!(self.kw_of(after), Some(Kw::Typename | Kw::Class | Kw::Typedef))
                {
                    ParamKind::Type
                } else {
                    ParamKind::NonType
                };
            }
            Some(_) => return ParamKind::NonType,
            None => {}
        }
        // A type-constraint not followed by `auto`/`decltype` (which would
        // make it a placeholder of a non-type parameter).
        if let Some((end, _, NameClass::Concept(c), _)) = self.scan_name_at(self.pos())
            && !matches!(self.kw_of(self.tok_at(end)), Some(Kw::Auto | Kw::Decltype))
        {
            return ParamKind::Constrained(c, end);
        }
        ParamKind::NonType
    }

    /// One template-parameter ([temp.param]/1). False after a syntax
    /// error (the caller recovers to the list's `>`).
    fn parse_template_param(&mut self) -> bool {
        match self.template_param_kind() {
            ParamKind::Type => self.parse_type_param(),
            ParamKind::Template => self.parse_template_template_param(),
            ParamKind::Constrained(c, _) => self.parse_constrained_param(c),
            ParamKind::NonType => self.parse_non_type_param(),
        }
    }

    /// `...`? identifier? after a type parameter's key or constraint,
    /// and where the name is or would be (an unnamed parameter's
    /// location, clang's `NameLoc`).
    fn parse_param_pack_and_name(&mut self) -> (bool, Option<(Symbol, Loc)>, Loc) {
        let pack = self.eat(Punct::Ellipsis).is_some();
        let at = self.loc();
        let name = match self.ident() {
            Some(n) if self.kw().is_none() => Some((n, self.bump().span.lo)),
            _ => None,
        };
        (pack, name, at)
    }

    /// `class|typename ...? T? (= type-id)?` ([temp.param]/1).
    fn parse_type_param(&mut self) -> bool {
        let kw = self.bump();
        let typename_kw = self.kw_of(kw) == Some(Kw::Typename);
        let (pack, name, at) = self.parse_param_pack_and_name();
        let p = self.s.act_on_type_template_param(typename_kw, kw.span.lo, pack, name, at);
        self.parse_type_param_default(p)
    }

    /// `= type-id` of a type parameter `p` ([temp.param]/10), then the
    /// end of `p` (its name is not in scope in its own default,
    /// [basic.scope.pdecl]/9).
    fn parse_type_param_default(&mut self, p: DeclId) -> bool {
        let ok = self.parse_type_param_default_arg(p);
        self.s.act_on_template_param_end(p);
        ok
    }

    fn parse_type_param_default_arg(&mut self, p: DeclId) -> bool {
        let Some(eq) = self.eat(Punct::Equal) else {
            return true;
        };
        // The default argument's location (clang's diagnostics point there).
        let at = self.loc();
        let Some(t) =
            self.with_greater_op(false, |p| p.parse_type_id_into(None, DeclSite::TypeParamDefault))
        else {
            return false;
        };
        self.s.act_on_template_param_default(p, TemplateArg::ty(t), eq, at);
        true
    }

    /// `C ...? T? (= type-id)?`: a constrained type parameter
    /// ([temp.param]/4).
    fn parse_constrained_param(&mut self, concept: DeclId) -> bool {
        let loc = self.loc();
        let Some((_, args)) = self.parse_concept_name() else { return false };
        let (pack, name, at) = self.parse_param_pack_and_name();
        let p = self.s.act_on_constrained_template_param(concept, &args, loc, pack, (name, at));
        self.parse_type_param_default(p)
    }

    /// A (qualified) concept name with optional template arguments: a
    /// type-constraint ([temp.param]/2). Returns the concept and its
    /// explicit arguments.
    pub(crate) fn parse_concept_name(&mut self) -> Option<(DeclId, Vec<TemplateArg>)> {
        let qual = self.parse_nested_name().ok()?;
        let name = self.ident()?;
        let NameClass::Concept(c) = self.s.classify_name(name, qual) else {
            self.error_here(DiagId::parse_expected_unqualified_id);
            return None;
        };
        self.bump();
        let mut args = Vec::new();
        if self.at(Punct::Less) {
            args = self.parse_template_arg_list()?.0;
        }
        Some((c, args))
    }

    /// `template < params > class|typename ...? TT? (= id-expression)?`
    /// ([temp.param]/1).
    fn parse_template_template_param(&mut self) -> bool {
        let tloc = self.bump().span.lo;
        let Some(l_angle) = self.expect(Punct::Less) else { return false };
        self.s.act_on_template_template_params_start(tloc, l_angle);
        let r_angle = self.parse_template_param_list_tail();
        let requires = if self.at_kw(Kw::Requires) {
            self.bump();
            self.parse_requires_clause()
        } else {
            None
        };
        let params = self.s.act_template_head(requires, r_angle);
        let mut keyless = false;
        let typename_kw = match self.kw() {
            Some(Kw::Class) => {
                self.bump();
                false
            }
            Some(Kw::Typename) => {
                // [temp.param]/1 before C++17: only `class` (clang
                // ext_template_template_param_typename).
                if self.std < Std::Cxx17 {
                    let d = self.s.diag(DiagId::parse_template_template_param_typename, self.loc());
                    self.emit(d);
                }
                self.bump();
                true
            }
            _ => {
                self.missing_template_template_param_key();
                keyless = true;
                false
            }
        };
        // The parameter's name, or none before `= , > >> ...`; anything
        // else fails the parameter (clang: "expected identifier").
        let named = self.ident().is_some() && self.kw().is_none();
        if !named
            && !self.at(Punct::Ellipsis)
            && !self.at(Punct::Equal)
            && !self.at(Punct::Comma)
            && !self.at_greater()
        {
            let d = self.s.diag(DiagId::parse_expected, self.loc()).arg("identifier");
            self.emit(d);
            self.s.act_on_finish_template_decl();
            return false;
        }
        let (pack, name, at) = self.parse_param_pack_and_name();
        let p = self.s.act_on_template_template_param(params, typename_kw, pack, name, at);
        if keyless {
            self.s.act_on_keyless_template_template_param(p);
        }
        // Its name is not in scope in its own default
        // ([basic.scope.pdecl]/9).
        let ok = self.parse_template_template_param_default(p);
        self.s.act_on_template_param_end(p);
        ok
    }

    /// A template template parameter's head followed by neither `class`
    /// nor `typename` ([temp.param]/1). Clang 19 always reports it; clang
    /// 22 only where the user plausibly forgot the key: before a name, `,`,
    /// `>`, `>>` or `...`, or at `struct` (replaced, so consumed). Either
    /// way the parameter recovers as a type template template parameter.
    fn missing_template_template_param_key(&mut self) {
        let is_struct = self.at_kw(Kw::Struct);
        let next = if is_struct { self.pos() + 1 } else { self.pos() };
        let t = self.tok_at(next);
        let plausible = (matches!(t.kind, TokKind::Identifier(_)) && self.kw_of(t).is_none())
            || matches!(
                Self::punct_of(t),
                Some(Punct::Comma | Punct::Greater | Punct::GreaterGreater | Punct::Ellipsis)
            );
        if self.s.clang_compat < 22 || plausible {
            let keys = if self.std >= Std::Cxx17 { "'class' or 'typename'" } else { "'class'" };
            let d =
                self.s.diag(DiagId::parse_class_on_template_template_param, self.loc()).arg(keys);
            self.emit(d);
        }
        if is_struct {
            self.bump();
        }
    }

    /// `= id-expression` of template template parameter `p`
    /// ([temp.param]/10): the default must name a class template, an
    /// alias template or a template template parameter, written as a
    /// whole template argument (clang's `ParseTemplateTemplateArgument`).
    /// Anything else is "must be a class template" at the token after
    /// what was parsed, and the parameter is skipped to `,`/`>` (false).
    /// A variable template or concept is, at clang compatibility 22, a
    /// template argument of the wrong kind (clang 22 parses it as a
    /// template name for P2841's template template parameters).
    fn parse_template_template_param_default(&mut self, p: DeclId) -> bool {
        let Some(eq) = self.eat(Punct::Equal) else {
            return true;
        };
        // The default argument's location (clang's diagnostics point there).
        let at = self.loc();
        // An invalid qualifier (`K::x` with `K` a class template) was
        // diagnosed; the name after it is still consumed, then rejected.
        let (qual, bad_qual) = match self.parse_nested_name_nns() {
            Ok(q) => (q, false),
            Err(()) => (None, true),
        };
        // `nns template name` ([temp.names]/5).
        let template_kw = qual.is_some() && self.eat_kw(Kw::Template).is_some();
        let dep = qual.as_ref().is_some_and(|q| self.s.is_dependent_ctx(q.ctx));
        // A template-id (`K<int>`) is one annotation token to clang, so the
        // error points at its start and the skip passes its arguments.
        if let Some(n) = self.ident()
            && self.kw().is_none()
            && Self::punct_of(self.peek(1)) == Some(Punct::Less)
            && !bad_qual
            && matches!(
                self.s.classify_name(n, qual.map(|q| q.ctx)),
                NameClass::TypeTemplate(_)
                    | NameClass::VarTemplate(_)
                    | NameClass::Concept(_)
                    | NameClass::FunctionTemplate(_)
            )
        {
            let d =
                self.s.diag(DiagId::parse_default_template_template_param_not_template, self.loc());
            self.bump();
            let _ = self.parse_template_arg_list();
            self.emit(d);
            return false;
        }
        let name = match self.ident() {
            Some(n) if self.kw().is_none() => Some((n, self.bump().span.lo)),
            _ => None,
        };
        let ellipsis = self.eat(Punct::Ellipsis);
        let at_end = matches!(
            Self::punct_of(self.tok()),
            Some(Punct::Comma | Punct::Greater | Punct::GreaterGreater)
        );
        let arg = match name {
            Some((n, nl)) if at_end && !bad_qual => {
                if let (true, true, Some(q)) = (dep, template_kw, qual) {
                    let tn = self
                        .s
                        .ast
                        .types
                        .template_name(TemplateName::DependentIdentifier { nns: q.nns, name: n });
                    Some(tn)
                } else {
                    match self.s.classify_name(n, qual.map(|q| q.ctx)) {
                        NameClass::TypeTemplate(d) => Some(self.s.template_name_for(d)),
                        NameClass::VarTemplate(d) | NameClass::Concept(d)
                            if self.s.clang_compat >= 22 =>
                        {
                            if ellipsis.is_none() {
                                self.s.act_on_wrong_kind_ttp_default(d, nl);
                                return true;
                            }
                            None
                        }
                        _ => None,
                    }
                }
            }
            _ => None,
        };
        // A pack expansion of a name that is not a pack makes the
        // argument invalid ([temp.variadic]/5).
        let arg = match (arg, ellipsis) {
            (Some(_), Some(l)) => {
                let d = self.s.diag(DiagId::sema_pack_expansion_without_packs, l);
                self.emit(d);
                None
            }
            (a, _) => a,
        };
        let Some(tn) = arg else {
            let d =
                self.s.diag(DiagId::parse_default_template_template_param_not_template, self.loc());
            self.emit(d);
            return false;
        };
        self.s.act_on_template_param_default(p, TemplateArg::template(tn), eq, at);
        true
    }

    /// A parameter-declaration as a non-type template parameter
    /// ([temp.param]/2).
    fn parse_non_type_param(&mut self) -> bool {
        let start = self.loc();
        let ds = self.parse_decl_specifiers(DeclaratorContext::Prototype);
        if ds.is_empty() {
            self.error_here(DiagId::parse_expected_template_param);
            return false;
        }
        let mut d = Declarator::new(DeclaratorContext::Prototype, start);
        d.site = DeclSite::TemplateParam;
        // An unnamed `int...` / `int&...` is a pack here: a template
        // parameter list has no C variadic ellipsis ([temp.param]/1).
        let saved = std::mem::replace(&mut self.pack_hint, true);
        self.with_greater_op(false, |s| s.parse_declarator(&mut d));
        self.pack_hint = saved;
        // A `...` after the declarator is misplaced (`T a...`, `Ts......a`):
        // diagnosed before the parameter's Sema action (clang reports it
        // inside the declarator, ahead of its type diagnostics), and the
        // parameter is the pack.
        let mut pack = d.ellipsis.take().is_some();
        let end = self.loc();
        if self.at(Punct::Ellipsis) {
            let at = self.loc();
            self.bump();
            self.misplaced_declarator_ellipsis(&d, at);
            pack = true;
        }
        let mut info = self.s.act_on_param_declarator(&ds, &d);
        // An unnamed parameter is located where its name would be
        // (clang's identifier location of an abstract declarator).
        if info.name.is_none() {
            info.loc = end;
        }
        let p = self.s.act_on_non_type_template_param(&info, pack);
        let mut ok = true;
        if let Some(eq) = self.eat(Punct::Equal) {
            let at = self.loc();
            match self.with_greater_op(false, Self::parse_constant_expr) {
                Some(e) => {
                    let a = self.s.builder().template_arg_expr(e);
                    self.s.act_on_template_param_default(p, a, eq, at);
                }
                None => ok = false,
            }
        }
        // Its name is not in scope in its own default
        // ([basic.scope.pdecl]/9).
        self.s.act_on_template_param_end(p);
        ok
    }

    // -- template argument lists -------------------------------------------------------

    /// `< template-argument-list? >` at the `<` ([temp.names]/1).
    /// Returns the arguments and the `<` and `>` locations; `None` after
    /// a syntax error, recovered past the list.
    pub(crate) fn parse_template_arg_list(&mut self) -> Option<(Vec<TemplateArg>, Loc, Loc)> {
        if !self.stack_ok() {
            return None;
        }
        let end = self.skip_template_args(self.pos());
        let l = self.bump().span.lo;
        let mut args = Vec::new();
        let mut locs = Vec::new();
        let mut ok = true;
        if !self.at_greater() {
            loop {
                locs.push(self.loc());
                let bare = self.at_bare_var_or_concept_arg();
                let name_loc = self.template_arg_name_loc();
                let Some(a) = self.parse_template_argument() else {
                    ok = false;
                    break;
                };
                args.push(a);
                // A template name argument is located at its name, after
                // any qualifier (clang's `TemplateArgumentLoc::getLocation`).
                if let (ArgView::Template(_), Some(n), Some(at)) =
                    (a.view(), name_loc, locs.last_mut())
                {
                    *at = n;
                }
                // An invalid argument ends the list (clang's
                // `ParseTemplateArgumentList`): a variable template or
                // concept named alone is invalid as an expression.
                if bare
                    && matches!(a.view(), ArgView::Expr { expr, .. } if self.s.is_error_expr(expr))
                    && let Some((close, _)) = end
                {
                    while self.pos() < close && !self.at_eof() {
                        self.bump();
                    }
                    break;
                }
                if self.eat(Punct::Comma).is_none() {
                    break;
                }
            }
        }
        if ok && self.at_greater() {
            let r = self.split_greater();
            self.s.set_template_arg_locs(locs, r);
            return Some((args, l, r));
        }
        if ok {
            self.error_here(DiagId::parse_template_args_expected_gt);
        }
        // Recover to the list's end found by lookahead.
        if let Some((close, _)) = end {
            while self.pos() < close && !self.at_eof() {
                self.bump();
            }
            if self.pos() == close && self.at_greater() {
                self.split_greater();
            }
        }
        None
    }

    /// One template-argument, with a trailing `...` ([temp.names]/1,
    /// [temp.arg]/2: a type-id wins an ambiguity).
    fn parse_template_argument(&mut self) -> Option<TemplateArg> {
        // `decltype(e)::name` (WP-119): the parenthesized operand is
        // parsed with `>` as an operator.
        self.with_greater_op(true, Self::annotate_decltype_scope_here);
        // A class template name alone is a template template argument,
        // not a deduced class type placeholder ([dcl.type.class.deduct]/2
        // allows no placeholder here). A name qualified by a namespace, or
        // by the class the template is a member of, is the template and
        // never an injected-class-name, even inside the template's own
        // definition (libstdc++ `_UnClos<__negate, std::_Expr, _Clos>`
        // in `_Expr`, [temp.local]/1).
        let ctad_name = matches!(
            self.scan_name_qual_at(self.pos()),
            Some((_, false, NameClass::TypeTemplate(d), _, q))
                if self.s.is_ctad_template(d) || q.is_some_and(|q| self.names_member_template(q, d))
        );
        let a = if ctad_name && let Some(t) = self.at_template_name_arg() {
            t
        } else if self.at_splice() {
            // [temp.arg.general]/3 (P2996): a splice-specifier here is a
            // type splice in the spike (no value or template splices).
            TemplateArg::ty(self.parse_splice_type()?)
        } else if self.is_cxx_type_id_at(self.pos(), TypeIdContext::AsTemplateArgument).0 {
            TemplateArg::ty(
                self.with_greater_op(false, |p| p.parse_type_id_into(None, DeclSite::TemplateArg))?,
            )
        } else if let Some(t) = self.at_template_name_arg() {
            t
        } else {
            let e = self.with_greater_op(false, Self::parse_constant_expr)?;
            self.s.builder().template_arg_expr(e)
        };
        match self.eat(Punct::Ellipsis) {
            Some(l) => Some(self.s.act_pack_expansion_arg(a, l)),
            None => Some(a),
        }
    }

    /// Whether the next template-argument is a variable template or
    /// concept named alone (followed by the argument's end).
    fn at_bare_var_or_concept_arg(&self) -> bool {
        matches!(
            self.scan_name_qual_at(self.pos()),
            Some((end, false, NameClass::VarTemplate(_) | NameClass::Concept(_), _, _))
                if self.bare_name_ends_at(end) && self.at_template_arg_end(end)
        )
    }

    /// Where the template name of a qualified template-argument naming a
    /// template is (`N::K` is at `K`), if the next argument is one.
    fn template_arg_name_loc(&self) -> Option<Loc> {
        match self.scan_name_qual_at(self.pos())? {
            (
                end,
                false,
                NameClass::TypeTemplate(_) | NameClass::VarTemplate(_) | NameClass::Concept(_),
                _,
                Some(_),
            ) if end > self.pos() + 1 && self.bare_name_ends_at(end) => {
                Some(self.tok_at(end - 1).span.lo)
            }
            _ => None,
        }
    }

    /// Whether the name scanned up to token `end` ends with its
    /// identifier (no template arguments follow it).
    fn bare_name_ends_at(&self, end: usize) -> bool {
        end > 0 && self.ident_of(self.tok_at(end - 1)).is_some()
    }

    /// Whether token `i` ends a template-argument.
    fn at_template_arg_end(&self, i: usize) -> bool {
        matches!(
            Self::punct_of(self.tok_at(i)),
            Some(
                Punct::Comma
                    | Punct::Greater
                    | Punct::GreaterGreater
                    | Punct::Ellipsis
                    | Punct::GreaterEqual
                    | Punct::GreaterGreaterEqual
            )
        )
    }

    /// Whether qualified lookup in `q` found class template `d` itself:
    /// `q` is a namespace or `d`'s own class, never a class whose lookup
    /// finds an injected-class-name.
    fn names_member_template(&self, q: DeclId, d: DeclId) -> bool {
        let decls = &self.s.ast.decls;
        matches!(decls.get(q).kind(), DeclKind::Namespace | DeclKind::TranslationUnit)
            || decls.get(d).ctx() == Some(q)
    }

    /// A template-argument that is an id-expression naming a class or
    /// alias template or a template template parameter
    /// ([temp.arg.template]/1): consumed, as a template name.
    fn at_template_name_arg(&mut self) -> Option<TemplateArg> {
        let (end, split, class, _, qual) = self.scan_name_qual_at(self.pos())?;
        // `nns template name` with a dependent `nns` ([temp.names]/5,
        // [temp.arg.template]/1). A member template of a dependent
        // specialization that is not the current instantiation (`X<T>::
        // template M`, found in `X`'s primary pattern by the lookahead)
        // is a dependent template name too: the member is looked up again
        // in the specialization `X<T>` becomes ([temp.dep.type]/6).
        let dep_qual = qual.is_some_and(|q| self.s.is_dependent_ctx(q));
        let dep_name = (matches!(class, NameClass::Dependent)
            && end >= 2
            && self.kw_of(self.tok_at(end - 2)) == Some(Kw::Template))
            || (dep_qual && matches!(class, NameClass::TypeTemplate(_)));
        let d = match class {
            _ if dep_name => None,
            NameClass::TypeTemplate(d) => Some(d),
            // A variable template or concept alone is a template name at
            // clang 22 (P2841, `ParseTemplateTemplateArgument`); the
            // argument check reports it against its parameter.
            // (The scan takes a variable template's arguments: `v<T>`
            // is an expression.)
            NameClass::VarTemplate(_) | NameClass::Concept(_) if !self.bare_name_ends_at(end) => {
                return None;
            }
            NameClass::VarTemplate(d) | NameClass::Concept(d) if self.s.clang_compat >= 22 => {
                Some(d)
            }
            NameClass::Concept(_) => None,
            _ => return None,
        };
        if split || !self.at_template_arg_end(end) {
            return None;
        }
        // Before clang 22 a concept alone is a concept-id missing its
        // arguments: "too few template arguments for concept".
        if let NameClass::Concept(c) = class
            && d.is_none()
            && !dep_name
        {
            let loc = self.tok_at(end - 1).span.lo;
            while self.pos() < end {
                self.bump();
            }
            return Some(self.s.act_bare_concept_template_arg(c, loc));
        }
        let Some(d) = d else {
            let qual = self.parse_nested_name_nns().ok()??;
            let template_kw = self.eat_kw(Kw::Template).is_some();
            let n = self.ident()?;
            self.bump();
            // `X<T>::template M` inside `X` names the current
            // instantiation's member ([temp.dep.type]/1), found now.
            if !self.s.is_dependent_ctx(qual.ctx)
                && let NameClass::TypeTemplate(d) = self.s.classify_name(n, Some(qual.ctx))
            {
                let underlying = self.s.template_name_for(d);
                let tn = self.s.ast.types.template_name(TemplateName::Qualified {
                    nns: qual.nns,
                    template_kw,
                    underlying,
                });
                return Some(TemplateArg::template(tn));
            }
            let tn = self
                .s
                .ast
                .types
                .template_name(TemplateName::DependentIdentifier { nns: qual.nns, name: n });
            return Some(TemplateArg::template(tn));
        };
        // `nns template name` naming a member template found now keeps
        // its qualifier and keyword as written ([temp.names]/5).
        let template_kw = end >= 2 && self.kw_of(self.tok_at(end - 2)) == Some(Kw::Template);
        let underlying = self.s.template_name_for(d);
        // A qualified variable template or concept keeps its qualifier
        // for the argument check's "use of variable template 'N::v'".
        let var_or_concept = matches!(class, NameClass::VarTemplate(_) | NameClass::Concept(_));
        if (template_kw || var_or_concept)
            && qual.is_some()
            && let Ok(Some(q)) = self.parse_nested_name_nns()
        {
            self.eat_kw(Kw::Template);
            self.bump();
            let tn = self.s.ast.types.template_name(TemplateName::Qualified {
                nns: q.nns,
                template_kw,
                underlying,
            });
            return Some(TemplateArg::template(tn));
        }
        while self.pos() < end {
            self.bump();
        }
        Some(TemplateArg::template(underlying))
    }

    /// The template-id `name < args >` with `name` (already classified as
    /// `class`, at `name_loc`) at the current `<`. `Err` after an error.
    pub(crate) fn parse_template_id_rest(
        &mut self,
        class: NameClass,
        name: Symbol,
        name_loc: Loc,
    ) -> Result<TemplateIdRef, ()> {
        let (args, l, r) = self.parse_template_arg_list().ok_or(())?;
        let (NameClass::TypeTemplate(d)
        | NameClass::VarTemplate(d)
        | NameClass::FunctionTemplate(d)
        | NameClass::Concept(d)) = class
        else {
            let text = self.s.ast.symbols().as_str(name).to_owned();
            let msg = self.s.diag(DiagId::sema_no_template_named, name_loc).arg(text);
            self.emit(msg);
            return Err(());
        };
        let tn = self.s.template_name_for(d);
        let _ = l;
        self.s.act_template_id(tn, &args, name_loc, r)
    }

    /// At `typename name ;` or `typename name<args> ;` in a
    /// type-requirement: the type-name has no nested-name-specifier.
    fn unqualified_type_requirement(&self) -> bool {
        let p = self.pos() + 1;
        if self.ident_of(self.tok_at(p)).is_none() {
            return false;
        }
        match self.tok_at(p + 1).kind {
            // A pack-index-specifier is no type-name: "expected a
            // qualified name after 'typename'".
            TokKind::Punct(Punct::ColonColon | Punct::Ellipsis) => false,
            TokKind::Punct(Punct::Less) => match self.skip_template_args(p + 1) {
                Some((close, false)) => {
                    self.tok_at(close + 1).kind != TokKind::Punct(Punct::ColonColon)
                }
                _ => false,
            },
            _ => true,
        }
    }

    // -- typename-specifiers -----------------------------------------------------------

    /// `typename nested-name-specifier template? name (< args >)?`
    /// ([temp.res.general]/3) at `typename`.
    pub(crate) fn parse_typename_specifier(&mut self) -> Option<QualType> {
        self.parse_typename_specifier_in(None)
    }

    /// [`Self::parse_typename_specifier`] in the decl-specifier-seq of a
    /// declaration in context `ctx`, which selects clang's recovery from
    /// an unqualified name (spec/parse.md, typename-specifiers).
    pub(crate) fn parse_typename_specifier_in(
        &mut self,
        ctx: Option<DeclaratorContext>,
    ) -> Option<QualType> {
        self.bump();
        if self.at_splice() && self.decltype_scope_end(self.pos()).is_none() {
            return self.parse_splice_type();
        }
        let Ok(qual) = self.parse_nested_name_nns() else {
            // clang annotates the whole (invalid) typename-specifier: the
            // components after a qualifier that failed (an incomplete
            // class, [temp.inst]/2) are consumed without a second error.
            self.skip_after_failed_qualifier();
            return None;
        };
        let Some(q) = qual else {
            let at = self.loc();
            let d = self.s.diag(DiagId::parse_expected_qualified_after_typename, at);
            self.emit(d);
            if self.ident().is_none() {
                self.typename_failed = ctx.is_some();
                return None;
            }
            let pack_index = self.peek_is(1, Punct::Ellipsis) && self.peek_is(2, Punct::LSquare);
            if ctx == Some(DeclaratorContext::Prototype)
                && let Some(first) = self.param_recovery
                && (pack_index || first)
            {
                // clang's tentative parse of the first parameter reports
                // the name once more; the parameter list ends at it.
                if first {
                    let d = self.s.diag(DiagId::parse_expected_qualified_after_typename, at);
                    self.emit(d.allow_duplicate());
                }
                self.param_abort = true;
                return None;
            }
            if !pack_index && ctx.is_some() {
                // clang recovers by skipping the `typename` when the name
                // is a type; otherwise the name is left for the
                // declarator and the decl-specifier-seq ends
                // (`template <typename static U>`: "expected ',' or '>'").
                let n = self.ident()?;
                if let NameClass::Type(ty, _) = self.s.classify_name(n, None) {
                    self.bump();
                    return Some(ty);
                }
                self.typename_failed = true;
                return None;
            }
            self.bump();
            if pack_index {
                // clang consumes the pack-index-specifier `T...[i]` with
                // the name.
                self.bump();
                let mut depth = 0usize;
                while !self.at_eof() {
                    let p = Self::punct_of(self.tok());
                    self.bump();
                    match p {
                        Some(Punct::LSquare | Punct::LParen | Punct::LBrace) => depth += 1,
                        Some(Punct::RSquare | Punct::RParen | Punct::RBrace) => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                self.typename_pack_index = Some((at, self.expected_loc()));
            }
            return None;
        };
        self.parse_qualified_type_rest(q, TagKeyword::Typename)
    }

    /// The `template? name (< args >)?` after qualifier `q` naming a type:
    /// a dependent name type (with keyword `kw`) when `q` is dependent,
    /// else the type found by qualified lookup.
    pub(crate) fn parse_qualified_type_rest(
        &mut self,
        q: Qualifier,
        kw: TagKeyword,
    ) -> Option<QualType> {
        let nns_end = self.prev_end();
        let template_kw = self.eat_kw(Kw::Template).is_some();
        let Some(name) = self.ident() else {
            self.error_here(DiagId::parse_expected_unqualified_id);
            return None;
        };
        let name_loc = self.bump().span.lo;
        if self.s.is_dependent_ctx(q.ctx) {
            let targs = if self.at(Punct::Less) {
                let (args, _, _) = self.parse_template_arg_list()?;
                Some(self.s.ast.types.arg_list(&args))
            } else {
                None
            };
            return Some(self.s.act_dependent_name_at(q.nns, name, targs, kw, name_loc));
        }
        let mut class = self.s.classify_name(name, Some(q.ctx));
        if !template_kw
            && !self.at(Punct::Less)
            && matches!(class, NameClass::TypeTemplate(_))
            && let Some(c) = self.s.injected_specialization_name(name, Some(q.ctx))
        {
            class = c;
        }
        if self.at(Punct::Less) && (template_kw || is_template_class(class)) {
            if let NameClass::TypeTemplate(d) = class {
                self.s.check_type_name_access(name, Some(q.ctx), d, name_loc);
            }
            return match self.parse_template_id_rest(class, name, name_loc) {
                // The qualifier as written is sugar: `'O::I<int>'`.
                Ok(TemplateIdRef::Type(t)) => {
                    let mut named = match self.s.ast.types.view(t.ty()) {
                        kiln_ast::types::TypeView::Elaborated { nns: None, named, .. } => named,
                        _ => t,
                    };
                    if template_kw {
                        named = self.template_kw_name(q.nns, named);
                    }
                    Some(self.s.ast.types.elaborated(kw, Some(q.nns), named))
                }
                _ => None,
            };
        }
        let NameClass::Type(t, d) = class else {
            // `typename D::X` for the current instantiation `D` with a
            // dependent base: looked up at instantiation ([temp.dep.type]/6).
            if let Some(t) = self.s.act_unknown_specialization_type(q, name, kw, name_loc) {
                return Some(t);
            }
            // A reachable type that is not visible ([module.reach]): the
            // missing-import error, then the type is used.
            if class == NameClass::Undeclared
                && !self.at(Punct::Less)
                && let Some(NameClass::Type(t, d)) =
                    self.s.module_hidden_type(name, Some(q.ctx), name_loc)
            {
                self.s.mark_type_referenced(d);
                let kind = self.s.ast.decls.get(d).kind();
                let named =
                    if kiln_sema::lookup::is_tag_kind(kind) { self.s.tag_type(d) } else { t };
                return Some(self.s.ast.types.elaborated(kw, Some(q.nns), named));
            }
            let text = self.s.ast.symbols().as_str(name).to_owned();
            let ctx = self.s.qualifier_desc(q.ctx);
            // clang underlines the nested-name-specifier (WP-421).
            let qual = kiln_source::Span::new(q.loc, nns_end);
            if self.at(Punct::Less) && matches!(class, NameClass::Undeclared | NameClass::Value(_))
            {
                // `std::codecvt_utf8<wchar_t> c;` with no such member, or
                // `R::f<int> z;` with `f` no template ([temp.names]/3:
                // only a template name is followed by `<`): clang's
                // `DiagnoseUnknownTypeName` "no template named 'X' in
                // 'N'", then its argument list is skipped.
                let d = self
                    .s
                    .diag(DiagId::sema_no_member_template, name_loc)
                    .arg(text)
                    .arg(ctx)
                    .range(qual);
                self.emit(d);
                let _ = self.parse_template_arg_list();
                return None;
            }
            let d = self
                .s
                .diag(DiagId::sema_typename_nested_not_found, name_loc)
                .arg(text)
                .arg(ctx)
                .range(qual);
            self.emit(d);
            return None;
        };
        self.s.diagnose_ambiguous_name(name, Some(q.ctx), name_loc);
        self.s.mark_type_referenced(d);
        self.s.check_type_name_access(name, Some(q.ctx), d, name_loc);
        let kind = self.s.ast.decls.get(d).kind();
        let named = if kiln_sema::lookup::is_tag_kind(kind) { self.s.tag_type(d) } else { t };
        // The qualifier as written is sugar: `'S::P':'int *'`.
        Some(self.s.ast.types.elaborated(kw, Some(q.nns), named))
    }

    /// The specialization `t` named as `nns::template J<...>`: its
    /// template name becomes the qualified name with the `template`
    /// keyword, which clang keeps as written and prints
    /// (`'O::template J<SA>' (aka 'template J<ns::S>')`, [temp.names]/5).
    /// Sugar only; the canonical type is unchanged.
    fn template_kw_name(&mut self, nns: kiln_ast::NnsId, t: QualType) -> QualType {
        let types = &mut self.s.ast.types;
        let kiln_ast::types::TypeView::TemplateSpecialization { name, args, underlying, alias } =
            types.view(t.ty())
        else {
            return t;
        };
        let underlying_name = match types.template_name_view(name) {
            TemplateName::Qualified { underlying, .. } => underlying,
            _ => name,
        };
        let qn = types.template_name(TemplateName::Qualified {
            nns,
            template_kw: true,
            underlying: underlying_name,
        });
        let r = types.template_specialization(qn, args, underlying, alias);
        QualType::new(r.ty(), t.quals_bits())
    }

    /// True at a type-constraint followed by `auto` or `decltype(auto)`
    /// (a constrained placeholder, [dcl.spec.auto.general]/1).
    pub(crate) fn at_constrained_placeholder(&self) -> bool {
        let Some((end, false, NameClass::Concept(_), _)) = self.scan_name_at(self.pos()) else {
            return false;
        };
        match self.kw_of(self.tok_at(end)) {
            Some(Kw::Auto) => true,
            Some(Kw::Decltype) => {
                Self::punct_of(self.tok_at(end + 1)) == Some(Punct::LParen)
                    && self.kw_of(self.tok_at(end + 2)) == Some(Kw::Auto)
            }
            _ => false,
        }
    }

    /// True at a type-name that [`Parser::parse_template_type_name`]
    /// parses: one with a template argument list or `template` keyword,
    /// or a dependent qualified name that is a type here (followed by a
    /// declarator, [temp.res.general]/4).
    pub(crate) fn at_template_type_name(&self, ctx: DeclaratorContext) -> bool {
        let start = self.pos();
        let Some((end, split, class, _)) = self.scan_name_at(start) else { return false };
        let templ = split
            || (start..end).any(|j| {
                let t = self.tok_at(j);
                Self::punct_of(t) == Some(Punct::Less) || self.kw_of(t) == Some(Kw::Template)
            });
        match class {
            NameClass::Type(..) => templ,
            NameClass::Dependent if end > start + 1 => {
                if split {
                    return true;
                }
                let next = self.tok_at(end);
                // `T::m x`, `T::m const x`: no expression continues so.
                if self.ident_of(next).is_some()
                    || self.kw_of(next).is_some_and(crate::decl::is_decl_specifier_kw)
                {
                    return true;
                }
                match ctx {
                    DeclaratorContext::TypeName => true,
                    DeclaratorContext::Block
                    | DeclaratorContext::Condition
                    | DeclaratorContext::ForInit => false,
                    _ => matches!(
                        Self::punct_of(next),
                        Some(
                            Punct::Star
                                | Punct::Amp
                                | Punct::AmpAmp
                                | Punct::Comma
                                | Punct::RParen
                                | Punct::Ellipsis
                                | Punct::Greater
                                | Punct::GreaterGreater
                                | Punct::Equal
                        )
                    ),
                }
            }
            _ => false,
        }
    }

    /// A type-name spelled with a template-id or a dependent qualifier in
    /// a decl-specifier-seq: `X<args>`, `N::X<args>::Y`, `T::type`. A
    /// dependent qualified name without `typename` is a type only where
    /// [temp.res.general]/4 (P0634) makes it one; elsewhere (block scope,
    /// a parameter of a function declared with an unqualified
    /// declarator-id at namespace or block scope) it is diagnosed and
    /// parsed as if `typename` were written.
    pub(crate) fn parse_template_type_name(&mut self, ctx: DeclaratorContext) -> Option<QualType> {
        let start = self.name_begin();
        let q = self.parse_nested_name_nns().ok()?;
        match q {
            None => {
                let name = self.ident()?;
                let name_loc = self.bump().span.lo;
                let class = self.s.classify_name(name, None);
                match self.parse_template_id_rest(class, name, name_loc) {
                    Ok(TemplateIdRef::Type(t)) => Some(t),
                    _ => None,
                }
            }
            Some(q) => {
                if self.s.is_dependent_ctx(q.ctx) {
                    self.diagnose_missing_typename(q, ctx, start);
                }
                self.parse_qualified_type_rest(q, TagKeyword::None)
            }
        }
    }

    /// Whether a dependent qualified type name in context `ctx` needs
    /// `typename` ([temp.res.general]/4, P0634: block scope, a condition,
    /// a for-init-statement, a parameter of a function declared with an
    /// unqualified declarator-id at namespace or block scope).
    pub(crate) fn typename_required(&self, ctx: DeclaratorContext) -> bool {
        match ctx {
            DeclaratorContext::Block
            | DeclaratorContext::Condition
            | DeclaratorContext::ForInit => true,
            DeclaratorContext::Prototype => self.param_typename == ParamTypename::Required,
            _ => false,
        }
    }

    /// "missing 'typename' prior to dependent type name" for the
    /// dependent qualified name `q::name` at `start` (the name follows),
    /// where `ctx` requires `typename`; it is parsed as if written.
    fn diagnose_missing_typename(&mut self, q: Qualifier, ctx: DeclaratorContext, start: Loc) {
        if !self.typename_required(ctx) {
            return;
        }
        let k = usize::from(self.at_kw(Kw::Template));
        if let Some(name) = self.ident_of(self.tok_at(self.pos() + k)) {
            let text = format!("{}{}", self.s.nns_str(q.nns), self.s.ast.symbols().as_str(name));
            let d = self.s.diag(DiagId::parse_missing_typename, start).arg(text);
            self.emit(d);
            if ctx == DeclaratorContext::Prototype {
                self.param_typename = ParamTypename::Missing;
            }
        }
    }

    /// A type found in a class that is dependent but not the current
    /// instantiation (`L::U` for a local class `L` of a function template,
    /// outside `L`; [temp.dep.type]/1) is a dependent qualified name: it
    /// needs `typename` where one is required (clang classifies it as a
    /// dependent name; kiln keeps the type it found).
    pub(crate) fn check_missing_typename_in_class(
        &mut self,
        q: Qualifier,
        ctx: DeclaratorContext,
        start: Loc,
    ) {
        if self.typename_required(ctx) && self.s.is_unentered_dependent_class(q.ctx) {
            self.diagnose_missing_typename(q, ctx, start);
        }
    }

    // -- requires-clauses and requires-expressions -------------------------------------

    /// The trailing requires-clause of function declarator `d`
    /// ([dcl.decl.general]/1: `declarator requires-clause`).
    pub(crate) fn parse_trailing_requires(&mut self, d: &mut Declarator) {
        if !self.at_kw(Kw::Requires) || !d.is_function_declarator() {
            return;
        }
        self.bump();
        let scope = self.s.act_on_start_trailing_requires(d);
        d.trailing_requires = self.parse_requires_clause();
        self.s.act_on_end_trailing_requires(scope);
    }

    /// The constraint-logical-or-expression of a requires-clause after
    /// `requires` ([temp.pre]/1, [temp.pre]/9): primary expressions
    /// joined by `&&` and `||`.
    pub(crate) fn parse_requires_clause(&mut self) -> Option<ExprId> {
        let mut lhs = self.parse_constraint_and()?;
        while self.at(Punct::PipePipe) {
            let l = self.bump().span.lo;
            let rhs = self.parse_constraint_and()?;
            lhs = self.s.act_on_binary(BinaryOp::LOr, lhs, rhs, l);
        }
        Some(lhs)
    }

    fn parse_constraint_and(&mut self) -> Option<ExprId> {
        let mut lhs = self.parse_constraint_primary()?;
        while self.at(Punct::AmpAmp) {
            let l = self.bump().span.lo;
            let rhs = self.parse_constraint_primary()?;
            lhs = self.s.act_on_binary(BinaryOp::LAnd, lhs, rhs, l);
        }
        Some(lhs)
    }

    /// A primary-expression of a requires-clause ([temp.pre]/9). A unary
    /// expression, a postfix operator other than a call, or a binary
    /// operator binding tighter than `&&` needs parentheses (clang's
    /// `err_unparenthesized_non_primary_expr_in_requires_clause`); it is
    /// recovered by parsing the postfix suffix and the operators above
    /// `&&`, and diagnosed at the result's expression location.
    fn parse_constraint_primary(&mut self) -> Option<ExprId> {
        let non_primary = self.at_non_primary_constraint_start();
        let e = if non_primary {
            self.parse_cast_expr()?
        } else if let Some(crate::keywords::BuiltinKw::Trait(t)) = self.builtin_kw_at(self.pos()) {
            // A builtin type trait is a primary-expression here, as in
            // clang's `ParseCastExpression(PrimaryExprOnly)`:
            // `requires __is_trivially_destructible(T)`.
            self.parse_type_trait(t)
        } else {
            self.parse_primary_expr()?
        };
        let postfix = self.at(Punct::Period)
            || self.at(Punct::PlusPlus)
            || self.at(Punct::MinusMinus)
            || (self.at(Punct::LSquare) && !self.peek_is(1, Punct::LSquare));
        let binary = self.cur_binop_prec().is_some_and(|p| p > prec::LAND);
        if !(non_primary || postfix || binary) {
            // A non-`bool` atom makes the clause invalid (clang's
            // `CheckConstraintExpression` from the parser).
            if !self.s.check_constraint_expression(e) {
                let loc = self.s.ast.exprs.get(e).loc();
                return Some(self.s.act_on_error_expr(&[e], loc));
            }
            return Some(e);
        }
        let e = if postfix { self.parse_postfix_suffix(e)? } else { e };
        let e = self.parse_rhs_from(e, prec::LAND + 1)?;
        let loc = self.s.ast.exprs.get(e).loc();
        let d = self.s.diag(DiagId::parse_requires_clause_needs_parens, loc);
        self.emit(d);
        Some(e)
    }

    /// Whether the current token starts an expression that is not a
    /// primary-expression (clang's `NotPrimaryExpression` in
    /// `PrimaryExprOnly` mode): a unary operator, `sizeof`, `alignof`,
    /// `noexcept`, `new`, `delete`, a named cast or `typeid`.
    fn at_non_primary_constraint_start(&self) -> bool {
        if let Some(p) = Self::punct_of(self.tok()) {
            return matches!(
                p,
                Punct::PlusPlus
                    | Punct::MinusMinus
                    | Punct::Amp
                    | Punct::Star
                    | Punct::Plus
                    | Punct::Minus
                    | Punct::Tilde
                    | Punct::Exclaim
            );
        }
        matches!(
            self.kw(),
            Some(
                Kw::Sizeof
                    | Kw::Alignof
                    | Kw::GnuAlignof
                    | Kw::DataSizeof
                    | Kw::Noexcept
                    | Kw::New
                    | Kw::Delete
                    | Kw::StaticCast
                    | Kw::ConstCast
                    | Kw::ReinterpretCast
                    | Kw::DynamicCast
                    | Kw::Typeid
            )
        )
    }

    /// `requires (params)? { requirement-seq }` at `requires`
    /// ([expr.prim.req.general]/1).
    pub(crate) fn parse_requires_expression(&mut self) -> ExprId {
        let loc = self.bump().span.lo;
        let body = self.s.act_on_requires_expr_start(loc);
        let mut params = Vec::new();
        if let Some(open) = self.eat(Punct::LParen) {
            self.s.act_on_start_requires_params();
            while !self.at(Punct::RParen) && !self.at_eof() {
                let start = self.loc();
                let ds = self.parse_decl_specifiers(DeclaratorContext::Prototype);
                if ds.is_empty() {
                    let e =
                        self.s.diag(DiagId::parse_expected, self.loc()).arg("parameter declarator");
                    self.emit(e);
                    self.skip_until(&[Punct::RParen], false, false);
                    break;
                }
                let mut d = Declarator::new(DeclaratorContext::Prototype, start);
                d.site = DeclSite::RequiresParam;
                self.parse_declarator(&mut d);
                let p = self.s.act_on_param_declarator(&ds, &d);
                let idx = u16::try_from(params.len()).unwrap_or(u16::MAX);
                params.push(self.s.act_on_requires_param(body, &p, idx));
                if self.eat(Punct::Comma).is_none() {
                    break;
                }
            }
            self.s.act_on_end_requires_params();
            self.expect_closing(Punct::RParen, Punct::LParen, open);
        }
        let mut reqs = Vec::new();
        let Some(lbrace) = self.expect(Punct::LBrace) else {
            return self.s.act_requires_expr(body, &params, &reqs, loc, self.prev_end());
        };
        while !self.at(Punct::RBrace) && !self.at_eof() {
            match self.parse_requirement() {
                Some(r) => reqs.push(r),
                None => {
                    self.skip_until(&[Punct::Semi], false, true);
                }
            }
        }
        let rbrace = self.expect_closing(Punct::RBrace, Punct::LBrace, lbrace);
        self.s.act_requires_expr(body, &params, &reqs, loc, rbrace)
    }

    /// One requirement ([expr.prim.req.general]/1) with its `;`.
    fn parse_requirement(&mut self) -> Option<Requirement> {
        let r = if self.at(Punct::LBrace) {
            // compound-requirement ([expr.prim.req.compound]).
            let open = self.bump().span.lo;
            let Some(expr) = self.parse_expression() else {
                self.skip_until(&[Punct::RBrace], false, true);
                return None;
            };
            self.expect_closing(Punct::RBrace, Punct::LBrace, open);
            let noexcept = self.eat_kw(Kw::Noexcept);
            let mut constraint = None;
            if self.eat(Punct::Arrow).is_some() {
                let loc = self.loc();
                let (c, args) = self.parse_concept_name()?;
                constraint = Some(self.s.act_on_return_type_requirement(c, &args, loc));
            }
            Requirement::Compound { expr, noexcept, constraint }
        } else if self.at_kw(Kw::Typename) {
            // type-requirement ([expr.prim.req.type]): `typename
            // nested-name-specifier? type-name ;`; an unqualified
            // type-name (`typename X<T>;`) is a plain type.
            if self.unqualified_type_requirement() {
                self.bump();
                let at = self.loc();
                // clang keeps the keyword as `ElaboratedType 'typename X'`.
                let t = self.parse_type_id()?;
                let t = if self.s.is_error_type(t) {
                    t
                } else {
                    self.s.ast.types.elaborated(TagKeyword::Typename, None, t)
                };
                Requirement::Type(t, at)
            } else {
                let at = self.tok_at(self.pos() + 1).span.lo;
                Requirement::Type(self.parse_typename_specifier()?, at)
            }
        } else if self.at_kw(Kw::Requires) {
            // nested-requirement ([expr.prim.req.nested]).
            self.bump();
            Requirement::Nested(self.parse_expr_prec(prec::LOR)?)
        } else {
            Requirement::Simple(self.parse_expression()?)
        };
        self.expect_semi_after("requirement");
        Some(r)
    }
}

#[cfg(test)]
mod tests {
    use super::ends_type_param;
    use kiln_lex::Punct;

    /// [temp.param]/1: the tokens that end a type parameter's name.
    #[test]
    fn type_param_terminators() {
        for p in [Punct::Equal, Punct::Comma, Punct::Greater, Punct::GreaterGreater] {
            assert!(ends_type_param(Some(p)));
        }
        for p in [Punct::Star, Punct::ColonColon, Punct::Less, Punct::GreaterEqual] {
            assert!(!ends_type_param(Some(p)));
        }
        assert!(!ends_type_param(None));
    }
}
