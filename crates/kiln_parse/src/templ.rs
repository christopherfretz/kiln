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

use kiln_ast::expr::BinaryOp;
use kiln_ast::templates::Requirement;
use kiln_ast::types::{TagKeyword, TemplateArg};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_lex::{Punct, TokKind};
use kiln_sema::templ::actions::TemplateIdRef;
use kiln_sema::{Declarator, DeclaratorContext, NameClass};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::expr::prec;
use crate::keywords::Kw;
use crate::parser::{PExpr, Parser, is_template_class};
use crate::tentative::TypeIdContext;

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

    /// A declaration starting at `template` or `extern template`
    /// ([temp.pre]/1, [temp.explicit], [temp.expl.spec]) in namespace
    /// (`File`) or class (`Member`) scope.
    pub(crate) fn parse_template_declaration(&mut self, ctx: DeclaratorContext) {
        let extern_loc = self.eat_kw(Kw::Extern);
        let loc = self.bump().span.lo;
        if extern_loc.is_some() || !self.at(Punct::Less) {
            // explicit-instantiation ([temp.explicit]/1): WP-070a.
            let _ = self.s.act_explicit_instantiation(extern_loc, loc);
            self.skip_unsupported_declaration();
            return;
        }
        if self.peek_is(1, Punct::Greater) {
            // explicit-specialization ([temp.expl.spec]/1): WP-070a.
            let _ = self.s.act_explicit_specialization(false, loc);
            self.skip_unsupported_declaration();
            return;
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
        self.parse_templated_declaration(ctx, head, loc);
        self.s.act_on_finish_template_decl();
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
        if self.at_partial_specialization() {
            let _ = self.s.act_explicit_specialization(true, loc);
            self.skip_unsupported_declaration();
            return;
        }
        if self.at_friend_template() {
            self.s.templ_unimplemented("friend templates", self.loc());
            self.skip_unsupported_declaration();
            return;
        }
        if ctx == DeclaratorContext::Member {
            self.parse_member_declaration();
        } else {
            self.parse_simple_declaration(DeclaratorContext::File, true);
        }
    }

    /// `class-key [attrs] name <` with `name` a class template: a partial
    /// specialization ([temp.spec.partial]/1).
    fn at_partial_specialization(&self) -> bool {
        if !matches!(self.kw(), Some(Kw::Class | Kw::Struct | Kw::Union)) {
            return false;
        }
        let mut i = self.pos() + 1;
        while self.tok_at(i).kind == TokKind::Punct(Punct::LSquare) {
            // `[[attr]]`: skip to the matching `]]`.
            let mut depth = 0usize;
            loop {
                match Self::punct_of(self.tok_at(i)) {
                    Some(Punct::LSquare) => depth += 1,
                    Some(Punct::RSquare) => {
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
            // The Concepts TS form `concept bool`.
            self.error_here(DiagId::parse_concept_legacy_bool);
            self.bump();
        }
        let Some(name) = self.ident() else {
            self.error_here(DiagId::parse_expected_unqualified_id);
            self.skip_unsupported_declaration();
            return;
        };
        let name_loc = self.bump().span.lo;
        if self.at_kw(Kw::Requires) {
            // [temp.concept]/4.
            self.error_here(DiagId::parse_concept_has_constraints);
            self.bump();
            let _ = self.parse_requires_clause();
        }
        if self.at_attr() {
            let mut attrs = Vec::new();
            self.parse_attributes(&mut attrs);
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
    fn parse_template_param_list_tail(&mut self) -> Loc {
        if !self.at_greater() {
            loop {
                if !self.parse_template_param() {
                    self.skip_until(&[Punct::Greater], true, false);
                    break;
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
            .diag(DiagId::parse_expected, self.expected_loc())
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
                return if ends_type_param(next) || next == Some(Punct::Ellipsis) {
                    ParamKind::Type
                } else if self.ident_of(self.peek(1)).is_some()
                    && self.kw_of(self.peek(1)).is_none()
                    && ends_type_param(Self::punct_of(self.peek(2)))
                {
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

    /// `...`? identifier? after a type parameter's key or constraint.
    fn parse_param_pack_and_name(&mut self) -> (bool, Option<(Symbol, Loc)>) {
        let pack = self.eat(Punct::Ellipsis).is_some();
        let name = match self.ident() {
            Some(n) if self.kw().is_none() => Some((n, self.bump().span.lo)),
            _ => None,
        };
        (pack, name)
    }

    /// `class|typename ...? T? (= type-id)?` ([temp.param]/1).
    fn parse_type_param(&mut self) -> bool {
        let kw = self.bump();
        let typename_kw = self.kw_of(kw) == Some(Kw::Typename);
        let (pack, name) = self.parse_param_pack_and_name();
        let p = self.s.act_on_type_template_param(typename_kw, kw.span.lo, pack, name);
        self.parse_type_param_default(p)
    }

    /// `= type-id` of a type parameter `p` ([temp.param]/10).
    fn parse_type_param_default(&mut self, p: DeclId) -> bool {
        let Some(eq) = self.eat(Punct::Equal) else { return true };
        let Some(t) = self.with_greater_op(false, |s| s.parse_type_id()) else {
            return false;
        };
        self.s.act_on_template_param_default(p, TemplateArg::ty(t), eq);
        true
    }

    /// `C ...? T? (= type-id)?`: a constrained type parameter
    /// ([temp.param]/4).
    fn parse_constrained_param(&mut self, concept: DeclId) -> bool {
        let loc = self.loc();
        let Some((_, args)) = self.parse_concept_name() else { return false };
        let (pack, name) = self.parse_param_pack_and_name();
        let p = self.s.act_on_constrained_template_param(concept, &args, loc, pack, name);
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
        let kw = self.tok();
        let typename_kw = match self.kw() {
            Some(Kw::Class) => false,
            Some(Kw::Typename) => true,
            _ => {
                let d = self.s.diag(DiagId::parse_expected, self.loc()).arg("'class'");
                self.emit(d);
                self.s.act_on_finish_template_decl();
                return false;
            }
        };
        self.bump();
        let (pack, name) = self.parse_param_pack_and_name();
        let p = self.s.act_on_template_template_param(params, typename_kw, kw.span.lo, pack, name);
        let Some(eq) = self.eat(Punct::Equal) else { return true };
        let Ok(qual) = self.parse_nested_name() else { return false };
        let Some(n) = self.ident() else {
            self.error_here(DiagId::parse_expected_unqualified_id);
            return false;
        };
        let nl = self.bump().span.lo;
        match self.s.classify_name(n, qual) {
            NameClass::TypeTemplate(d) => {
                let tn = self.s.template_name_for(d);
                self.s.act_on_template_param_default(p, TemplateArg::template(tn), eq);
            }
            _ => {
                let text = self.s.ast.symbols().as_str(n).to_owned();
                let d = self.s.diag(DiagId::sema_no_template_named, nl).arg(text);
                self.emit(d);
            }
        }
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
        self.with_greater_op(false, |s| s.parse_declarator(&mut d));
        let pack = d.ellipsis.take().is_some();
        let info = self.s.act_on_param_declarator(&ds, &d);
        let p = self.s.act_on_non_type_template_param(&info, pack);
        if let Some(eq) = self.eat(Punct::Equal) {
            let Some(e) = self.with_greater_op(false, |s| s.parse_constant_expr()) else {
                return false;
            };
            let a = self.s.builder().template_arg_expr(e);
            self.s.act_on_template_param_default(p, a, eq);
        }
        true
    }

    // -- template argument lists -------------------------------------------------------

    /// `< template-argument-list? >` at the `<` ([temp.names]/1).
    /// Returns the arguments and the `<` and `>` locations; `None` after
    /// a syntax error, recovered past the list.
    pub(crate) fn parse_template_arg_list(&mut self) -> Option<(Vec<TemplateArg>, Loc, Loc)> {
        let end = self.skip_template_args(self.pos());
        let l = self.bump().span.lo;
        let mut args = Vec::new();
        let mut ok = true;
        if !self.at_greater() {
            loop {
                match self.parse_template_argument() {
                    Some(a) => args.push(a),
                    None => {
                        ok = false;
                        break;
                    }
                }
                if self.eat(Punct::Comma).is_none() {
                    break;
                }
            }
        }
        if ok && self.at_greater() {
            let r = self.split_greater();
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
        let a = if self.is_cxx_type_id_at(self.pos(), TypeIdContext::AsTemplateArgument).0 {
            TemplateArg::ty(self.with_greater_op(false, |s| s.parse_type_id())?)
        } else if let Some(t) = self.at_template_name_arg() {
            t
        } else {
            let e = self.with_greater_op(false, |s| s.parse_constant_expr())?;
            self.s.builder().template_arg_expr(e)
        };
        match self.eat(Punct::Ellipsis) {
            Some(l) => Some(self.s.act_pack_expansion_arg(a, l)),
            None => Some(a),
        }
    }

    /// A template-argument that is an id-expression naming a class or
    /// alias template or a template template parameter
    /// ([temp.arg.template]/1): consumed, as a template name.
    fn at_template_name_arg(&mut self) -> Option<TemplateArg> {
        let (end, split, class, _) = self.scan_name_at(self.pos())?;
        let NameClass::TypeTemplate(d) = class else { return None };
        if split
            || !matches!(
                Self::punct_of(self.tok_at(end)),
                Some(
                    Punct::Comma
                        | Punct::Greater
                        | Punct::GreaterGreater
                        | Punct::Ellipsis
                        | Punct::GreaterEqual
                        | Punct::GreaterGreaterEqual
                )
            )
        {
            return None;
        }
        while self.pos() < end {
            self.bump();
        }
        Some(TemplateArg::template(self.s.template_name_for(d)))
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
        let d = match class {
            NameClass::TypeTemplate(d)
            | NameClass::VarTemplate(d)
            | NameClass::FunctionTemplate(d)
            | NameClass::Concept(d) => d,
            _ => {
                let text = self.s.ast.symbols().as_str(name).to_owned();
                let msg = self.s.diag(DiagId::sema_no_template_named, name_loc).arg(text);
                self.emit(msg);
                return Err(());
            }
        };
        let tn = self.s.template_name_for(d);
        let _ = l;
        self.s.act_template_id(tn, &args, name_loc, r)
    }

    // -- typename-specifiers -----------------------------------------------------------

    /// `typename nested-name-specifier template? name (< args >)?`
    /// ([temp.res.general]/3) at `typename`.
    pub(crate) fn parse_typename_specifier(&mut self) -> Option<QualType> {
        let kw_loc = self.bump().span.lo;
        let qual = self.parse_nested_name_nns().ok()?;
        let Some(q) = qual else {
            let d = self.s.diag(DiagId::parse_expected_qualified_after_typename, self.loc());
            self.emit(d);
            if self.ident().is_some() {
                self.bump();
            }
            return None;
        };
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
            return Some(self.s.act_dependent_name(q.nns, name, targs, TagKeyword::Typename));
        }
        let class = self.s.classify_name(name, Some(q.ctx));
        if self.at(Punct::Less) && (template_kw || is_template_class(class)) {
            return match self.parse_template_id_rest(class, name, name_loc) {
                Ok(TemplateIdRef::Type(t)) => Some(t),
                _ => None,
            };
        }
        match class {
            NameClass::Type(t, d) => {
                self.s.mark_type_referenced(d);
                let kind = self.s.ast.decls.get(d).kind();
                Some(if kiln_sema::lookup::is_tag_kind(kind) {
                    self.s.elaborated_tag_type(d, TagKeyword::None)
                } else {
                    t
                })
            }
            _ => {
                let text = self.s.ast.symbols().as_str(name).to_owned();
                let ctx = self.s.decl_name(q.ctx);
                let d = self
                    .s
                    .diag(DiagId::sema_typename_nested_not_found, name_loc)
                    .arg(text)
                    .arg(format!("'{ctx}'"));
                self.emit(d);
                let _ = kw_loc;
                None
            }
        }
    }

    // -- requires-clauses and requires-expressions -------------------------------------

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

    /// A primary-expression of a requires-clause; another binary
    /// operator binding tighter than `&&` needs parentheses (clang's
    /// `err_requires_clause_must_be_parenthesized`), recovered by parsing
    /// it anyway.
    fn parse_constraint_primary(&mut self) -> Option<ExprId> {
        let start = self.loc();
        let e = self.parse_primary_expr()?;
        if let Some(p) = self.cur_binop_prec()
            && p > prec::LAND
        {
            let d = self.s.diag(DiagId::parse_requires_clause_needs_parens, start);
            self.emit(d);
            return self.parse_rhs_from(e, prec::LAND + 1);
        }
        Some(e)
    }

    /// `requires (params)? { requirement-seq }` at `requires`
    /// ([expr.prim.req.general]/1).
    pub(crate) fn parse_requires_expression(&mut self) -> PExpr {
        let loc = self.bump().span.lo;
        let body = self.s.act_on_requires_expr_start(loc);
        let mut params = Vec::new();
        if let Some(open) = self.eat(Punct::LParen) {
            while !self.at(Punct::RParen) && !self.at_eof() {
                let start = self.loc();
                let ds = self.parse_decl_specifiers(DeclaratorContext::Prototype);
                if ds.is_empty() {
                    let e = self
                        .s
                        .diag(DiagId::parse_expected, self.loc())
                        .arg("parameter declarator");
                    self.emit(e);
                    self.skip_until(&[Punct::RParen], false, false);
                    break;
                }
                let mut d = Declarator::new(DeclaratorContext::Prototype, start);
                self.parse_declarator(&mut d);
                let p = self.s.act_on_param_declarator(&ds, &d);
                let idx = u16::try_from(params.len()).unwrap_or(u16::MAX);
                params.push(self.s.act_on_requires_param(body, &p, idx));
                if self.eat(Punct::Comma).is_none() {
                    break;
                }
            }
            self.expect_closing(Punct::RParen, Punct::LParen, open);
        }
        let mut reqs = Vec::new();
        let Some(lbrace) = self.expect(Punct::LBrace) else {
            let e = self.s.act_requires_expr(body, &params, &reqs, loc, self.prev_end());
            return Some(e);
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
        Some(self.s.act_requires_expr(body, &params, &reqs, loc, rbrace))
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
            let noexcept = self.eat_kw(Kw::Noexcept).is_some();
            let mut constraint = None;
            if self.eat(Punct::Arrow).is_some() {
                let loc = self.loc();
                let (c, args) = self.parse_concept_name()?;
                constraint = Some(self.s.act_on_return_type_requirement(c, &args, loc));
            }
            Requirement::Compound { expr, noexcept, constraint }
        } else if self.at_kw(Kw::Typename) {
            // type-requirement ([expr.prim.req.type]).
            Requirement::Type(self.parse_typename_specifier()?)
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
