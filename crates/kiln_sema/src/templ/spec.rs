//! Class template partial and explicit specializations and explicit
//! instantiations ([temp.spec.partial], [temp.expl.spec],
//! [temp.explicit]; spec/sema-templates.md §4.4-§4.5, WP-070b).
//!
//! The parser announces `template<>` ([`Sema::act_explicit_spec_start`])
//! and `[extern] template` ([`Sema::act_explicit_inst_start`]); the class
//! template-id after the class-key then goes through
//! [`Sema::act_on_class_template_id_tag`], which also handles a partial
//! specialization (a template head is open) and a plain elaborated
//! reference (`struct X<int>`).

use kiln_ast::decl::{DeclFlags, DeclKind, TagKind};
use kiln_ast::expr::ExprKind;
use kiln_ast::ids::ArgListId;
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{ArgView, TagKeyword, TemplateArg, TypeFlags};
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_diag::DiagId;
use kiln_diag::FixIt;
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::decl::TagUse;
use crate::declspec::Declarator;
use crate::templ::deduce::{DeducedArgs, Tdk};

/// A declarator-id `v<args>` naming variable template `template` after a
/// template head or `template<>` (see
/// [`Sema::act_on_specialization_declarator_id`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecDeclaratorId {
    /// The variable template (its first declaration).
    pub template: DeclId,
    /// The arguments as written.
    pub args: Vec<TemplateArg>,
    /// Where each argument was written.
    pub arg_locs: Vec<Loc>,
    /// Where the template name was written.
    pub loc: Loc,
    /// The template-id, name to `>`.
    pub range: Span,
}

/// What a class template-id after a class-key declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TagForm {
    /// `template<params> class X<args>`.
    Partial,
    /// `template<> class X<args>` (or the same without `template<>`,
    /// diagnosed).
    ExplicitSpec,
    /// `[extern] template class X<args>;`.
    ExplicitInst {
        /// `extern template`.
        declaration: bool,
    },
    /// `class X<args>` naming a specialization.
    Reference,
}

impl Sema<'_, '_> {
    /// `template<>` was parsed: the class template-id declared next is an
    /// explicit specialization ([temp.expl.spec]/1). Paired with
    /// [`Sema::act_explicit_spec_end`].
    pub fn act_explicit_spec_start(&mut self, loc: Loc) {
        self.templ.explicit_spec = Some(loc);
        self.templ.explicit_depth = self.inst.stack.len();
    }

    /// The declaration after `template<>` ended.
    pub fn act_explicit_spec_end(&mut self) {
        self.templ.explicit_spec = None;
        self.templ.spec_headers.clear();
    }

    /// The `template` keywords of the leading `template<>` headers of the
    /// declaration whose [`Sema::act_explicit_spec_start`] or
    /// [`Sema::act_member_template_spec_start`] was just called, in
    /// source order. At namespace scope they are matched to the classes
    /// of the declaration's qualifier ([temp.expl.spec]/17; clang's
    /// `MatchTemplateParametersToScopeSpecifier`) and any left over are
    /// extraneous ([`Sema::check_extra_spec_headers`]).
    pub fn act_explicit_spec_headers(&mut self, heads: &[Loc]) {
        self.templ.spec_headers.clear();
        if matches!(self.kind_of(self.ctx), DeclKind::Namespace | DeclKind::TranslationUnit) {
            self.templ.spec_headers.extend_from_slice(heads);
            self.templ.explicit_depth = self.inst.stack.len();
        }
    }

    /// `template<>` followed by a non-empty template head was parsed
    /// (`template<> template<class U> struct A<int>::B {...}`,
    /// [temp.expl.spec]/16): the template declared next specializes a
    /// member template of an implicitly instantiated class. Paired with
    /// [`Sema::act_member_template_spec_end`].
    pub fn act_member_template_spec_start(&mut self, loc: Loc) {
        self.templ.member_spec = Some(loc);
    }

    /// The declaration after `template<> template<...>` ended.
    pub fn act_member_template_spec_end(&mut self) {
        self.templ.member_spec = None;
        self.templ.spec_headers.clear();
    }

    /// `t` redeclares, after `template<> template<...>`, a member
    /// template of an implicitly instantiated class
    /// ([temp.expl.spec]/16): the member template is specialized, so its
    /// specializations are instantiated from this declaration's
    /// definition with its own arguments only (clang's
    /// `setMemberSpecialization`; `getTemplateInstantiationPattern`
    /// stops there).
    pub(crate) fn specialize_member_template(&mut self, t: DeclId) {
        let first = self.ast.first(t);
        if self
            .ast
            .templates
            .instantiated_from_map(first)
            .is_some_and(|x| kiln_ast::templates::is_template_kind(self.kind_of(x)))
        {
            self.ast.templates.clear_instantiated_from(first);
        }
    }

    /// `[extern] template` without `<` was parsed: the class template-id
    /// declared next is explicitly instantiated ([temp.explicit]/2).
    /// Paired with [`Sema::act_explicit_inst_end`].
    pub fn act_explicit_inst_start(&mut self, extern_loc: Option<Loc>, loc: Loc) {
        self.templ.explicit_inst = Some((extern_loc, loc));
        self.templ.explicit_depth = self.inst.stack.len();
    }

    /// [temp.explicit]/3: an explicit instantiation of a function or
    /// variable has no definition. The parser met declarator `d` of one
    /// (`function`: a function declarator) followed by `at`, which is not
    /// its `;` (a body, an initializer, or a `,`; clang's
    /// `ParseDeclarationAfterDeclaratorAndAttributes` and
    /// `ParseFunctionDefinition`). For a template-id declarator-id,
    /// "explicit template instantiation cannot have a definition" at the
    /// name, recovering as an explicit specialization (`template<>`);
    /// otherwise "function/variable cannot be defined in an explicit
    /// instantiation" at `at`, recovering as a plain declaration (the
    /// `template` is ignored).
    pub fn act_explicit_inst_with_definition(&mut self, d: &Declarator, at: Loc, function: bool) {
        let Some((_, template_loc)) = self.templ.explicit_inst else { return };
        if !self.at_explicit_head() {
            return;
        }
        self.templ.explicit_inst = None;
        let template_range = Span::new(template_loc, template_loc.with_offset(8));
        if d.spec_id.is_some() {
            let e = self
                .diag(DiagId::parse_explicit_instantiation_with_definition, d.name_loc())
                .range(template_range)
                .fixit(FixIt::insert(template_loc.with_offset(8), "<>"));
            self.emit(e);
            self.templ.explicit_spec = Some(template_loc);
        } else {
            let e = self
                .diag(DiagId::parse_template_defn_explicit_instantiation, at)
                .arg(if function { 0u8 } else { 2u8 })
                .range(template_range)
                .fixit(FixIt::remove(template_range));
            self.emit(e);
        }
    }

    /// The explicit instantiation ended.
    pub fn act_explicit_inst_end(&mut self) {
        self.templ.explicit_inst = None;
    }

    /// `class-key X<args>` with `X` naming class template `tmpl`
    /// ([dcl.type.elab], [class.pre]/1): a partial specialization after
    /// a template head ([temp.spec.partial]), an explicit specialization
    /// after `template<>` ([temp.expl.spec]), an explicit instantiation
    /// after `[extern] template` ([temp.explicit]), else a reference to
    /// the specialization. Returns the declared record (`None` for a
    /// dependent reference) and the elaborated type. `Err` after a
    /// diagnostic (a definition's body is then skipped).
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    #[allow(clippy::too_many_arguments)]
    pub fn act_on_class_template_id_tag(
        &mut self,
        keyword: TagKeyword,
        tmpl: DeclId,
        args: &[TemplateArg],
        qualifier: Option<DeclId>,
        use_: TagUse,
        name_loc: Loc,
        (l_angle, r_angle): (Loc, Loc),
        kw_loc: Loc,
    ) -> Result<(Option<DeclId>, QualType), ()> {
        let range = Span::new(name_loc, r_angle.with_offset(1));
        let tmpl = self.ast.first(tmpl);
        let locs = std::mem::take(&mut self.templ.arg_locs);
        let declares = matches!(use_, TagUse::Definition | TagUse::Declaration);
        let form = if !declares {
            TagForm::Reference
        } else if let Some((ext, template_loc)) = self.templ.explicit_inst.take() {
            if use_ == TagUse::Definition {
                // [temp.explicit]/3: clang's
                // `err_explicit_instantiation_with_definition`, recovering
                // as an explicit specialization.
                let at = template_loc.with_offset(8);
                let d = self
                    .diag(DiagId::parse_explicit_instantiation_with_definition, name_loc)
                    .range(Span::new(template_loc, at))
                    .fixit(FixIt::insert(at, "<>"));
                self.emit(d);
                TagForm::ExplicitSpec
            } else {
                TagForm::ExplicitInst { declaration: ext.is_some() }
            }
        } else if self.templ.explicit_spec.take().is_some() {
            let _ = self.check_extra_spec_headers(qualifier, 0);
            TagForm::ExplicitSpec
        } else if self.take_template_head(qualifier).is_some() {
            TagForm::Partial
        } else if self.templ.head_mismatch {
            return Err(());
        } else {
            // [temp.expl.spec]/1: `template<>` is required.
            let d = self
                .diag(DiagId::sema_template_spec_needs_header, name_loc)
                .range(range)
                .fixit(FixIt::insert(kw_loc, "template<> "));
            self.emit(d);
            TagForm::ExplicitSpec
        };
        let name = self.template_name_for(tmpl);
        // [dcl.type.elab]/3: a specialization's class-key agrees with the
        // primary's (clang `ActOnClassTemplateSpecialization`).
        if matches!(form, TagForm::Partial | TagForm::ExplicitSpec)
            && let Some(pat) = self.pattern_of(tmpl)
            && !self.tag_matches(pat, keyword)
        {
            self.diag_wrong_tag(self.decl_name(tmpl), kw_loc, pat);
        }
        match form {
            TagForm::Partial => {
                let r = self.act_on_partial_spec(keyword, tmpl, args, &locs, use_, range);
                self.templ.cur = None;
                let p = r?;
                let list = self.ast.types.arg_list(args);
                let t = self.ast.types.template_specialization(name, list, None, false);
                Ok((Some(p), self.ast.types.elaborated(keyword, None, t)))
            }
            TagForm::ExplicitSpec => {
                let spec =
                    self.act_on_explicit_class_spec(keyword, tmpl, args, &locs, use_, range)?;
                Ok((Some(spec), self.spec_tag_type(keyword, name, args, spec)))
            }
            TagForm::ExplicitInst { declaration } => {
                let spec =
                    self.act_on_explicit_class_inst(tmpl, args, &locs, declaration, name_loc)?;
                Ok((Some(spec), self.spec_tag_type(keyword, name, args, spec)))
            }
            TagForm::Reference => {
                let list = self.ast.types.arg_list(args);
                let dependent = self
                    .ast
                    .types
                    .arg_list_flags(list)
                    .intersects(TypeFlags::DEPENDENT_INSTANTIATION);
                // [temp.friend]/9: `template<class U> friend struct
                // Y<U*>;` would declare a partial specialization.
                if use_ == TagUse::Friend && self.take_template_head(qualifier).is_some() {
                    let d = self
                        .diag(DiagId::sema_partial_spec_friend, kw_loc)
                        .range(Span::new(l_angle, r_angle.with_offset(1)));
                    self.emit(d);
                    return Err(());
                }
                // A dependent `friend struct X<T>;` ([class.friend]/3,
                // [temp.friend]) stays a dependent type in the pattern's
                // `FriendDecl`; each instantiation of the class befriends
                // the substituted specialization (§4.8).
                if dependent {
                    let t = self.ast.types.template_specialization(name, list, None, false);
                    return Ok((None, self.ast.types.elaborated(keyword, None, t)));
                }
                let spec = self.get_class_spec(tmpl, args, &locs, name_loc)?;
                if !self.tag_matches(spec, keyword) {
                    // clang `ActOnTagTemplateIdType`, naming the
                    // specialization type.
                    let rec = self.tag_type(spec);
                    let t = self.ast.types.template_specialization(name, list, Some(rec), false);
                    self.diag_wrong_tag(self.ty_str(t), kw_loc, spec);
                }
                Ok((Some(spec), self.spec_tag_type(keyword, name, args, spec)))
            }
        }
    }

    /// `kw X<args>` sugaring specialization `spec`'s record type.
    fn spec_tag_type(
        &mut self,
        keyword: TagKeyword,
        name: kiln_ast::ids::TemplateNameId,
        args: &[TemplateArg],
        spec: DeclId,
    ) -> QualType {
        let list = self.ast.types.arg_list(args);
        let rec = self.tag_type(spec);
        let t = self.ast.types.template_specialization(name, list, Some(rec), false);
        self.ast.types.elaborated(keyword, None, t)
    }

    /// Whether a (partial or explicit) specialization of `tmpl` may be
    /// declared in the current context ([temp.expl.spec]/3,
    /// [temp.spec.partial.general]/4: a scope in which the primary
    /// template may be defined); diagnoses otherwise. Diagnosed at
    /// namespace scope, the specialization is still declared (true, as
    /// clang's `CheckTemplateSpecializationScope` recovers; WP-1178); in
    /// a class other than the member template's own it is not (false).
    pub(crate) fn check_spec_scope(&mut self, tmpl: DeclId, partial: bool, loc: Loc) -> bool {
        let tctx = self.ast.decls.get(tmpl).ctx().unwrap_or(self.ast.tu);
        let tctx = self.ast.redecl_context(tctx);
        let cur = self.ast.redecl_context(self.ctx);
        if self.kind_of(tctx) == DeclKind::CXXRecord
            || self.kind_of(tctx) == DeclKind::ClassTemplateSpecialization
        {
            return self.check_member_spec_scope(tmpl, partial, loc);
        }
        if self.kind_of(tctx) != DeclKind::Namespace
            && self.kind_of(tctx) != DeclKind::TranslationUnit
        {
            return true;
        }
        if !matches!(self.kind_of(cur), DeclKind::Namespace | DeclKind::TranslationUnit)
            || self.encloses(cur, tctx)
        {
            return true;
        }
        let var = self.kind_of(tmpl) == DeclKind::VarTemplate;
        let func = self.kind_of(tmpl) == DeclKind::FunctionTemplate;
        let note =
            self.diag(DiagId::note_explicit_spec_template_declared, self.ast.decls.get(tmpl).loc());
        if self.kind_of(tctx) == DeclKind::TranslationUnit {
            // A template of the global namespace: clang's
            // `err_template_spec_redecl_global_scope`.
            let sel = if func { 4u32 } else { u32::from(var) * 2 + u32::from(partial) };
            let d = self
                .diag(DiagId::sema_spec_redecl_global_scope, loc)
                .arg(sel)
                .arg(self.decl_name(tmpl))
                .with_note(note);
            self.emit(d);
            return true;
        }
        if func {
            let id = DiagId::sema_function_spec_not_in_enclosing_ns;
            let d = self
                .diag(id, loc)
                .arg(self.decl_name(tmpl))
                .arg(self.decl_name(tctx))
                .with_note(note);
            self.emit(d);
            return true;
        }
        let id = match (partial, var) {
            (true, false) => DiagId::sema_partial_spec_redecl_scope,
            (false, false) => DiagId::sema_class_spec_not_in_enclosing_ns,
            (true, true) => DiagId::sema_var_partial_spec_redecl_scope,
            (false, true) => DiagId::sema_var_spec_not_in_enclosing_ns,
        };
        let d =
            self.diag(id, loc).arg(self.decl_name(tmpl)).arg(self.decl_name(tctx)).with_note(note);
        self.emit(d);
        true
    }

    /// [temp.expl.spec]/3, [temp.spec.partial.general]/4 for a member of
    /// a class (clang's `CheckTemplateSpecializationScope` when the
    /// specialized entity's context is a class): a specialization of
    /// member template `tmpl` (`template<> template<> struct
    /// N::A<int>::C<int>`), or the member specialization of member `tmpl`
    /// of a class template (`template<> struct N::A<int>::I`; `tmpl` is
    /// the pattern's member, which the note names) must be
    /// declared in that class or in a namespace enclosing it. Diagnosed
    /// at `loc` against the class; at namespace scope the specialization
    /// is still declared (true, clang's recovery), in another class it is
    /// not (false). The entity kind selects clang's
    /// `err_template_spec_redecl_out_of_scope` wording.
    pub(crate) fn check_member_spec_scope(
        &mut self,
        tmpl: DeclId,
        partial: bool,
        loc: Loc,
    ) -> bool {
        if !self.inst.stack.is_empty() {
            return true;
        }
        let tctx = self.ast.decls.get(tmpl).ctx().unwrap_or(self.ast.tu);
        let tctx = self.ast.redecl_context(tctx);
        let cur = self.ast.redecl_context(self.ctx);
        let file = matches!(self.kind_of(cur), DeclKind::Namespace | DeclKind::TranslationUnit);
        let ok = if file {
            self.encloses(cur, tctx)
        } else {
            self.ast.first(cur) == self.ast.first(tctx)
        };
        if ok {
            return true;
        }
        let kind = match self.kind_of(tmpl) {
            DeclKind::ClassTemplate => u32::from(partial),
            DeclKind::VarTemplate => 2 + u32::from(partial),
            DeclKind::FunctionTemplate => 4,
            DeclKind::Enum => 8,
            k if k.is_function() => 5,
            DeclKind::Var => 6,
            _ => 7,
        };
        let note =
            self.diag(DiagId::note_explicit_spec_template_declared, self.ast.decls.get(tmpl).loc());
        let d = self
            .diag(DiagId::sema_spec_redecl_out_of_class_scope, loc)
            .arg(kind)
            .arg(self.decl_name(tmpl))
            .arg(self.name_for_diagnostic(tctx))
            .with_note(note);
        self.emit(d);
        file
    }

    // -- partial specializations ---------------------------------------------------------

    /// `template<params> class X<args>` ([temp.spec.partial]): checks the
    /// arguments ([temp.spec.partial.general]/8-9) and finds or creates
    /// the partial specialization. The head is in `templ.cur`.
    fn act_on_partial_spec(
        &mut self,
        keyword: TagKeyword,
        tmpl: DeclId,
        args: &[TemplateArg],
        locs: &[Loc],
        use_: TagUse,
        range: Span,
    ) -> Result<DeclId, ()> {
        let name_loc = range.lo;
        let Some(head) = self.templ.cur else { return Err(()) };
        // [temp.spec.partial.general]/9.5 (clang checks class templates
        // only): no default template arguments.
        let params: Vec<DeclId> = {
            let l = *self.ast.templates.param_list(head);
            self.ast.lists.iter::<DeclId>(l.params).collect()
        };
        for p in params {
            if let Some(at) = self.templ.default_locs.get(&p).copied() {
                self.error(DiagId::sema_default_arg_in_partial_spec, at);
            }
        }
        let conv = self.check_partial_args(
            tmpl,
            head,
            args,
            locs,
            false,
            use_ == TagUse::Definition,
            range,
        )?;
        if let Some(i) = self.find_partial_decl(tmpl, conv.canonical, head) {
            let p = self.ast.templates.partial(i).map(|p| p.partial).ok_or(())?;
            if use_ == TagUse::Definition
                && let Some(def) = self.ast.definition(p)
            {
                // The same partial specialization defined in a global
                // module fragment the current unit does not see: merged,
                // its body skipped silently (spec/modules.md §6).
                if self.module_merge_hidden_definition(def) {
                    return Err(());
                }
                let d = self
                    .diag(DiagId::sema_redefinition, name_loc)
                    .range(range)
                    .arg(self.partial_spec_name(p));
                let d = self.note_prev_partial_def(d, def);
                self.emit(d);
                return Err(());
            }
            if use_ == TagUse::Definition {
                self.templ.partial_def_loc.insert(p, name_loc);
                self.take_spec_class_key(p, keyword);
            }
            return Ok(p);
        }
        let n = *self.ast.decls.get(tmpl);
        let ctx = n.ctx().unwrap_or(self.ast.tu);
        let mut h = DeclHead::new(ctx, n.name(), name_loc);
        if self.ctx != ctx {
            h.lexical = Some(self.ctx);
        }
        let tag = Self::tag_kind_of(keyword);
        let p = self.builder().class_partial_spec(h, tmpl, head, conv.sugared, tag);
        if self.kind_of(self.ctx) == DeclKind::CXXRecord {
            self.set_member_access(p);
        }
        self.check_partial_more_specialized(p, range);
        self.check_partial_deducible(p, range);
        Ok(p)
    }

    /// The checks every partial specialization `tmpl<args>` with head
    /// `head` passes ([temp.spec.partial.general]/8-9,
    /// [temp.spec.partial.general]/4): not fully specialized, not the
    /// primary template's own arguments, in an enclosing scope. `var`: a
    /// variable template's; `def`: a definition. The converted
    /// arguments, `Err` after a diagnostic.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn check_partial_args(
        &mut self,
        tmpl: DeclId,
        head: kiln_ast::ids::TemplateParamListId,
        args: &[TemplateArg],
        locs: &[Loc],
        var: bool,
        def: bool,
        range: Span,
    ) -> Result<crate::templ::args::ConvertedArgs, ()> {
        let name_loc = range.lo;
        let conv = self.check_template_args(tmpl, args, locs, name_loc)?;
        let types = &self.ast.types;
        // A member template of a templated class has a dependent name
        // (`Outer<T>::Inner`), so its non-dependent arguments are left to
        // the deducibility check (clang's `Name.isDependent()` in
        // `ActOnClassTemplateSpecialization`).
        let dependent_name = self.ast.templates.param_list(head).depth > 0;
        if !dependent_name
            && !types.arg_list_flags(conv.canonical).intersects(TypeFlags::DEPENDENT_INSTANTIATION)
        {
            let d = self
                .diag(DiagId::sema_partial_spec_fully_specialized, name_loc)
                .arg(self.decl_name(tmpl));
            self.emit(d);
            return Err(());
        }
        // A constrained partial specialization may repeat the primary's
        // arguments ([temp.spec.partial.general]/9.3: it is more
        // specialized by its constraints).
        let constrained = {
            let mut c = Vec::new();
            let hl = *self.ast.templates.param_list(head);
            self.head_constraints(hl.params, hl.requires, &mut c);
            !c.is_empty()
        };
        // [temp.spec.partial.general]/9.3: the arguments are not the
        // primary template's own parameters in order. They are the head
        // declaration's: a friend first declaration inside a class
        // template has deeper parameters, which no argument names.
        let params = self.template_params(self.head_template(tmpl));
        let conv_args: Vec<TemplateArg> = self.ast.types.args(conv.sugared).to_vec();
        if !constrained
            && params.len() == conv_args.len()
            && params.iter().zip(&conv_args).all(|(&p, &a)| {
                // A parameter pack's argument is converted to a pack
                // of one expansion (`G<Ts...>`), not of one element
                // (`E<M>` for `template <int... N> struct E`).
                let a = match a.view() {
                    ArgView::Pack(l) => match self.ast.types.args(l) {
                        &[e] if self.is_pack_arg(e) => e,
                        _ => return false,
                    },
                    _ => a,
                };
                self.arg_names_param(p, a) && self.same_nttp_type(p, a)
            })
        {
            let d = self
                .diag(DiagId::sema_partial_spec_args_match_primary, name_loc)
                .range(range)
                .arg(u32::from(var))
                .arg(u32::from(def));
            self.emit(d);
            if def
                && !var
                && let Some(prev) = self.pattern_of(tmpl).and_then(|p| self.ast.definition(p))
            {
                let d = self.diag(DiagId::sema_redefinition, name_loc).arg(self.decl_name(tmpl));
                let d = self.note_prev(d, prev, true);
                self.emit(d);
            }
            return Err(());
        }
        if !self.check_spec_scope(tmpl, true, name_loc) {
            return Err(());
        }
        Ok(conv)
    }

    /// Whether argument `a` that names non-type parameter `p` of the
    /// primary template is a parameter of the same type: `template<auto K>
    /// class C; template<class T, T *K> class C<K>` specializes `K`
    /// ([temp.over.link]/6: equivalent non-type template parameters have
    /// equivalent types; clang profiles the parameter's type). True for
    /// any other argument.
    fn same_nttp_type(&self, p: DeclId, a: TemplateArg) -> bool {
        let ArgView::Expr { expr, .. } = a.view() else { return true };
        let mut e = expr;
        if self.ast.exprs.get(e).kind() == ExprKind::PackExpansionExpr {
            e = self.ast.exprs.operand(e);
        }
        if self.ast.exprs.get(e).kind() != ExprKind::DeclRefExpr {
            return true;
        }
        let d = self.ast.exprs.decl(e);
        let ty = |x: DeclId| {
            let t = self.ast.decls.get(x).ty().unwrap_or(QualType::ERROR);
            self.ast.types.canonical(t)
        };
        ty(d) == ty(p)
    }

    /// [temp.spec.partial.general]/9.2 (DR1495; clang's
    /// `checkMoreSpecializedThanPrimary`): partial specialization `p` is
    /// more specialized than its primary template, else
    /// `-Winvalid-partial-specialization` (an error by default) at its
    /// name, with the primary's location noted.
    pub(crate) fn check_partial_more_specialized(&mut self, p: DeclId, range: Span) {
        let loc = range.lo;
        let Some((_, info)) = self.ast.partial_spec_info(p) else { return };
        let tmpl = info.template;
        if !self.not_more_specialized_than_primary(p, loc) {
            return;
        }
        let var = self.kind_of(p) == DeclKind::Var;
        let note = self.diag(DiagId::note_template_decl_here, self.ast.decls.get(tmpl).loc());
        let mut d = self
            .diag(DiagId::sema_partial_spec_not_more_specialized, loc)
            .range(range)
            .arg(u32::from(var))
            .with_note(note);
        for n in self.primary_constraint_notes(p) {
            d = d.with_note(n);
        }
        self.emit(d);
    }

    /// [temp.spec.partial.general]/8 (clang's
    /// `checkTemplatePartialSpecialization`): every parameter of partial
    /// specialization `p` is deducible from its arguments, else it can
    /// never match (`-Wunusable-partial-specialization`). Clang checks a
    /// member template's partial specialization at its definition and
    /// again in each instantiation of the enclosing class, so the
    /// diagnostic is `per_instantiation` (spec/diag.md §7.4).
    pub(crate) fn check_partial_deducible(&mut self, p: DeclId, range: Span) {
        let loc = range.lo;
        let Some((_, info)) = self.ast.partial_spec_info(p) else { return };
        let info = *info;
        let list = *self.ast.templates.param_list(info.params);
        let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(list.params).collect();
        let args: Vec<TemplateArg> = self.ast.types.args(info.canonical).to_vec();
        // Deducing the arguments from themselves leaves exactly the
        // parameters of no deduced context undeduced. A pack expansion
        // of `A` is deduced as a whole (`Ts *...` from `Ts *...`), and one
        // that is not last makes the whole list non-deduced
        // ([temp.deduct.type]/9: `F<T..., int>` deduces no `T`); a function
        // parameter pack that is not last is skipped, the rest still
        // deduced ([temp.deduct.type]/5.7: `X<void(Ts..., T)>` deduces `T`).
        let mut d = DeducedArgs::new(list.depth, params.len());
        d.expansions = true;
        d.self_deduce = true;
        self.inst.sfinae.borrow_mut().push(crate::templ::stack::SfinaeState::default());
        let r = self.deduce_args(&args, &args, true, &mut d, loc);
        let failed = self.inst.sfinae.borrow_mut().pop().is_some_and(|s| s.errors > 0);
        if r != Tdk::Success || failed {
            return;
        }
        let missing: Vec<DeclId> = params
            .iter()
            .zip(&d.args)
            .filter(|(_, a)| matches!(a.view(), ArgView::Null))
            .map(|(&p, _)| p)
            .collect();
        if missing.is_empty() {
            return;
        }
        let var = self.kind_of(p) == DeclKind::Var;
        let mut msg = self
            .diag(DiagId::sema_partial_spec_not_deducible, loc)
            .per_instantiation()
            .range(range)
            .arg(u32::from(var))
            .arg(u32::from(missing.len() > 1));
        for &m in &missing {
            let note = self
                .diag(DiagId::note_non_deducible_parameter, self.ast.decls.get(m).loc())
                .arg(self.decl_name(m));
            msg = msg.with_note(note);
        }
        self.emit(msg);
    }

    /// `X<T *>`: how partial specialization `p` prints as a diagnostic
    /// argument (its unqualified name and its arguments as written, as
    /// clang's `getNameForDiagnostic` without the `q` modifier).
    pub(crate) fn partial_spec_name(&self, p: DeclId) -> String {
        let mut s = self.unqualified_decl_name(p);
        if let Some((_, info)) = self.ast.partial_spec_info(p) {
            let names = crate::names::sema_names(self);
            kiln_ast::types::write_arg_list(&self.ast.types, &names, info.args, &mut s);
        }
        s
    }

    /// `[with T = int, N = 3]`: the deduced arguments `args` of partial
    /// specialization `p` by parameter name.
    pub(crate) fn partial_spec_bindings(&self, p: DeclId, args: ArgListId) -> String {
        let Some((_, info)) = self.ast.partial_spec_info(p) else { return String::new() };
        let list = *self.ast.templates.param_list(info.params);
        let mut s = String::from("[with ");
        let elems = self.ast.types.args(args);
        for (i, (p, &a)) in self.ast.lists.iter::<DeclId>(list.params).zip(elems).enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&self.decl_name(p));
            s.push_str(" = ");
            s.push_str(&self.binding_arg_str(a));
        }
        s.push(']');
        s
    }

    /// The deduction frame's description: `class template partial
    /// specialization 'X<T *>' [with T = int]`.
    pub(crate) fn partial_spec_desc(&self, p: DeclId, args: Option<ArgListId>) -> String {
        let what = if self.kind_of(p) == DeclKind::Var { "variable" } else { "class" };
        let mut s =
            format!("{what} template partial specialization '{}'", self.partial_spec_name(p));
        if let Some(a) = args {
            s.push(' ');
            s.push_str(&self.partial_spec_bindings(p, a));
        }
        s
    }

    // -- explicit specializations ----------------------------------------------------------

    /// `template<> class X<args>` ([temp.expl.spec]): the specialization,
    /// checked against its previous state (clang's
    /// `CheckSpecializationInstantiationRedecl`).
    fn act_on_explicit_class_spec(
        &mut self,
        keyword: TagKeyword,
        tmpl: DeclId,
        args: &[TemplateArg],
        locs: &[Loc],
        use_: TagUse,
        range: Span,
    ) -> Result<DeclId, ()> {
        let name_loc = range.lo;
        let spec = self.get_class_spec(tmpl, args, locs, name_loc)?;
        if !self.check_spec_scope(tmpl, false, name_loc) {
            return Err(());
        }
        let (i, info) = self.ast.spec_info(spec).map(|(i, s)| (i, *s)).ok_or(())?;
        match info.kind {
            SpecKind::Undeclared => {}
            SpecKind::ImplicitInstantiation
            | SpecKind::ExplicitInstantiationDeclaration
            | SpecKind::ExplicitInstantiationDefinition => {
                // [temp.expl.spec]/7: declared before the first use that
                // would cause an implicit instantiation.
                let explicit = u32::from(info.kind != SpecKind::ImplicitInstantiation);
                let note = self
                    .diag(DiagId::note_instantiation_required_here, info.point_of_instantiation)
                    .arg(explicit);
                // clang: `Context.getTypeDeclType(PrevDecl)`, fully scoped
                // (`N::X<short>::M<int>`).
                let t = self.tag_type(spec);
                let d = self
                    .diag(DiagId::sema_spec_after_instantiation, name_loc)
                    .range(range)
                    .arg(self.decl_ty_str(t))
                    .with_note(note);
                self.emit(d);
                return Err(());
            }
            SpecKind::ExplicitSpecialization => {
                if use_ == TagUse::Definition
                    && let Some(def) = self.ast.definition(spec)
                {
                    // [basic.def.odr]/15: the same explicit specialization
                    // defined in a global module fragment the current
                    // unit does not see is merged, not redefined; the
                    // body is skipped silently (spec/modules.md §6).
                    if self.module_merge_hidden_definition(def) {
                        return Err(());
                    }
                    let t = self.tag_type(spec);
                    let d = self
                        .diag(DiagId::sema_redefinition, name_loc)
                        .range(range)
                        .arg(self.ty_str(t));
                    let d = self.note_prev(d, def, true);
                    self.emit(d);
                    return Err(());
                }
                // clang 19 `ActOnClassTemplateSpecialization` resets the
                // written arguments at the definition; clang 22 keeps the
                // first declaration's.
                if use_ == TagUse::Definition && self.clang_compat < 22 {
                    let w = self.ast.types.arg_list(args);
                    self.ast.templates.set_args_as_written(spec, w, true);
                }
                self.take_spec_class_key(spec, keyword);
                return Ok(spec);
            }
        }
        self.ast.templates.update_spec(i, |s| {
            s.kind = SpecKind::ExplicitSpecialization;
            s.from = spec;
        });
        // clang `setTemplateArgsAsWritten`: the type printer prints these
        // (`std::coroutine_handle<void>`, not `<>`).
        let w = self.ast.types.arg_list(args);
        self.ast.templates.set_args_as_written(spec, w, true);
        self.take_spec_class_key(spec, keyword);
        let lexical = self.ctx;
        self.builder().declare_spec_in(spec, lexical, name_loc);
        if self.kind_of(lexical) == DeclKind::CXXRecord {
            self.set_member_access(spec);
        }
        Ok(spec)
    }

    /// An explicit or partial specialization not yet defined takes the
    /// class-key of the class-head declaring it, not the primary
    /// template's (spec/sema-class.md §2.1, WP-611): the key of its
    /// definition sets its default member and base access
    /// ([class.access.base]/2, [class.access]/3). A union/class mismatch
    /// is not a re-keying (keys stay as they were).
    fn take_spec_class_key(&mut self, spec: DeclId, keyword: TagKeyword) {
        let tag = Self::tag_kind_of(keyword);
        if self.ast.definition(spec).is_none()
            && let Some(r) = self.ast.decls.record(spec)
            && r.tag != tag
            && (r.tag == TagKind::Union) == (tag == TagKind::Union)
        {
            self.ast.decls.set_definition_tag(spec, tag);
        }
    }

    // -- explicit instantiations -----------------------------------------------------------

    /// `[extern] template class X<args>;` ([temp.explicit]): the
    /// specialization's definition is instantiated, and (an explicit
    /// instantiation definition, [temp.explicit]/10) the definitions of
    /// its members.
    pub(crate) fn act_on_explicit_class_inst(
        &mut self,
        tmpl: DeclId,
        args: &[TemplateArg],
        locs: &[Loc],
        declaration: bool,
        name_loc: Loc,
    ) -> Result<DeclId, ()> {
        let spec = self.get_class_spec(tmpl, args, locs, name_loc)?;
        let (i, info) = self.ast.spec_info(spec).map(|(i, s)| (i, *s)).ok_or(())?;
        let t = self.tag_type(spec);
        match (info.kind, declaration) {
            (
                SpecKind::ExplicitSpecialization | SpecKind::ExplicitInstantiationDeclaration,
                true,
            ) => {
                return Ok(spec);
            }
            (SpecKind::ExplicitSpecialization, false) => {
                // [temp.explicit]/8 (clang: no effect, a warning).
                let note = self.diag(
                    DiagId::note_previous_template_specialization,
                    self.ast.decls.get(spec).loc(),
                );
                let d = self
                    .diag(DiagId::sema_explicit_inst_after_spec_ignored, name_loc)
                    .arg(self.ty_str(t))
                    .with_note(note);
                self.emit(d);
                return Ok(spec);
            }
            (SpecKind::ExplicitInstantiationDefinition, false) => {
                // [temp.spec.general]/5: at most once.
                let note = self.diag(
                    DiagId::note_previous_explicit_instantiation,
                    info.point_of_instantiation,
                );
                let d = self
                    .diag(DiagId::sema_explicit_inst_duplicate, name_loc)
                    .arg(self.ty_str(t))
                    .with_note(note);
                self.emit(d);
                return Err(());
            }
            (SpecKind::ExplicitInstantiationDefinition, true) => {
                // [temp.explicit]/11.
                let note = self.diag(
                    DiagId::note_explicit_instantiation_definition_here,
                    info.point_of_instantiation,
                );
                let d = self
                    .diag(DiagId::sema_explicit_inst_declaration_after_definition, name_loc)
                    .with_note(note);
                self.emit(d);
                return Err(());
            }
            _ => {}
        }
        let first_explicit =
            matches!(info.kind, SpecKind::Undeclared | SpecKind::ImplicitInstantiation);
        if first_explicit {
            self.builder().relocate_spec(spec, name_loc);
            // clang `ActOnExplicitInstantiation`: the first explicit
            // instantiation's written arguments.
            let w = self.ast.types.arg_list(args);
            self.ast.templates.set_args_as_written(spec, w, false);
        }
        if self.ast.definition(spec).is_none() && !self.instantiate_explicit(spec, name_loc) {
            return Err(());
        }
        let kind = if declaration {
            SpecKind::ExplicitInstantiationDeclaration
        } else {
            SpecKind::ExplicitInstantiationDefinition
        };
        self.ast.templates.update_spec(i, |s| {
            s.kind = kind;
            if s.point_of_instantiation == Loc::INVALID {
                s.point_of_instantiation = name_loc;
            }
        });
        let lexical = self.ctx;
        if first_explicit {
            self.builder().declare_spec_in(spec, lexical, name_loc);
        } else if !declaration && let Some(list) = self.ast.decls.member_list(lexical) {
            // After an explicit instantiation declaration, clang hands
            // the members to codegen here, not at the declaration.
            let after = list.last.unwrap_or(lexical);
            self.ast.templates.add_moved_class_inst(after, spec);
        }
        self.instantiate_members_of(spec, name_loc, declaration, 0);
        // The vtable of an explicit instantiation definition is emitted
        // with it, so its virtual members are odr-used: an implicit
        // virtual destructor is defined here (clang
        // `ActOnExplicitInstantiation` -> `MarkVTableUsed(..., true)`;
        // LLVM's `template class cl::opt<std::string>`).
        if !declaration {
            self.mark_vtable_used(spec, name_loc);
        }
        Ok(spec)
    }

    /// `template<> struct C<args>::I` / `template<> enum class
    /// C<args>::E` for member `p` of an implicit class template
    /// specialization ([temp.expl.spec]/7; clang
    /// `CheckMemberSpecialization`): false, after "explicit
    /// specialization of 'I' after instantiation", when the member was
    /// already defined from its pattern or explicitly instantiated, or
    /// when it is declared in a class other than its own
    /// ([temp.expl.spec]/3, [`Sema::check_member_spec_scope`]; in a
    /// class, the qualifier is diagnosed first). Declared outside a
    /// namespace enclosing the class, it is diagnosed and still
    /// specialized (true, clang's recovery).
    pub(crate) fn check_member_tag_spec(&mut self, p: DeclId, name_loc: Loc) -> bool {
        let first = self.ast.first(p);
        let Some(info) = self.ast.member_spec_info(first).map(|(_, x)| *x) else { return true };
        if self.inst.stack.is_empty() && self.kind_of(self.ctx) == DeclKind::CXXRecord {
            // In a class (clang's `diagnoseQualifiedDeclaration` with
            // `IsMemberSpecialization`): the qualifier is diagnosed, then
            // the scope below.
            let n = self.decl_name(first);
            self.emit(self.diag(DiagId::sema_member_qualification, name_loc).arg(n));
        }
        let after_inst = match info.kind {
            SpecKind::ImplicitInstantiation => self.ast.definition(first).is_some(),
            SpecKind::ExplicitInstantiationDeclaration
            | SpecKind::ExplicitInstantiationDefinition => true,
            SpecKind::Undeclared | SpecKind::ExplicitSpecialization => false,
        };
        if !after_inst {
            // Then its scope (WP-1178).
            return self.check_member_spec_scope(info.instantiated_from, false, name_loc);
        }
        let explicit = u32::from(info.kind != SpecKind::ImplicitInstantiation);
        // An unscoped member enumeration was defined with its class: the
        // class's point of instantiation.
        let at = if info.point_of_instantiation == Loc::INVALID {
            self.enclosing_poi(first)
        } else {
            info.point_of_instantiation
        };
        let note = self.diag(DiagId::note_instantiation_required_here, at).arg(explicit);
        let e = self
            .diag(DiagId::sema_spec_after_instantiation, name_loc)
            .arg(self.decl_name_str(self.ast.decls.get(first).name()))
            .with_note(note);
        self.emit(e);
        false
    }

    /// The point of instantiation of the innermost instantiated class
    /// enclosing member `m` of a class template specialization.
    fn enclosing_poi(&self, m: DeclId) -> Loc {
        let mut c = self.ast.decls.get(m).ctx();
        while let Some(x) = c.filter(|&x| self.kind_of(x) == DeclKind::CXXRecord) {
            let poi = match (self.ast.spec_info(x), self.ast.member_spec_info(x)) {
                (Some((_, s)), _) => s.point_of_instantiation,
                (None, Some((_, s))) => s.point_of_instantiation,
                (None, None) => Loc::INVALID,
            };
            if poi != Loc::INVALID {
                return poi;
            }
            c = self.ast.decls.get(x).ctx();
        }
        Loc::INVALID
    }

    /// `[extern] template struct C<args>::I;`, the qualified class `p`
    /// declared after an explicit instantiation's `template`: when `p` is
    /// a member class of a class template specialization
    /// ([temp.explicit]/3), it is explicitly instantiated (true).
    pub(crate) fn act_on_explicit_member_class_inst(&mut self, p: DeclId, name_loc: Loc) -> bool {
        let m = self.ast.first(p);
        if !self.at_explicit_head() || self.ast.member_spec_info(m).is_none() {
            return false;
        }
        let Some((ext, _)) = self.templ.explicit_inst.take() else { return false };
        self.explicit_member_class_inst(m, ext.is_some(), name_loc);
        true
    }

    /// `[extern] template struct C<args>::I;` for member class `m` of a
    /// class template specialization ([temp.explicit]/3, /10): its
    /// definition is instantiated and, for a definition, the definitions
    /// of its members, handed to codegen here (clang
    /// `ActOnExplicitInstantiation` for a member class).
    fn explicit_member_class_inst(&mut self, m: DeclId, declaration: bool, name_loc: Loc) {
        let Some((i, info)) = self.ast.member_spec_info(m).map(|(i, x)| (i, *x)) else { return };
        match (info.kind, declaration) {
            // [temp.explicit]/8 (clang: no effect); a repeated
            // declaration is harmless.
            (SpecKind::ExplicitSpecialization, _)
            | (SpecKind::ExplicitInstantiationDeclaration, true) => return,
            (SpecKind::ExplicitInstantiationDefinition, false) => {
                // [temp.spec.general]/5: at most once.
                let note = self.diag(
                    DiagId::note_previous_explicit_instantiation,
                    info.point_of_instantiation,
                );
                let t = self.tag_type(m);
                let d = self
                    .diag(DiagId::sema_explicit_inst_duplicate, name_loc)
                    .arg(self.ty_str(t))
                    .with_note(note);
                self.emit(d);
                return;
            }
            (SpecKind::ExplicitInstantiationDefinition, true) => {
                // [temp.explicit]/11.
                let note = self.diag(
                    DiagId::note_explicit_instantiation_definition_here,
                    info.point_of_instantiation,
                );
                let d = self
                    .diag(DiagId::sema_explicit_inst_declaration_after_definition, name_loc)
                    .with_note(note);
                self.emit(d);
                return;
            }
            _ => {}
        }
        if !self.instantiate_class(m, name_loc, false) {
            return;
        }
        let kind = if declaration {
            SpecKind::ExplicitInstantiationDeclaration
        } else {
            SpecKind::ExplicitInstantiationDefinition
        };
        self.ast.templates.update_member_spec(i, |x| {
            x.kind = kind;
            x.point_of_instantiation = name_loc;
        });
        if !declaration {
            let lexical = self.ctx;
            if let Some(list) = self.ast.decls.member_list(lexical) {
                let after = list.last.unwrap_or(lexical);
                self.ast.templates.add_explicit_member_inst(after, m);
            }
        }
        self.instantiate_members_of(m, name_loc, declaration, 0);
        if !declaration {
            self.mark_vtable_used(m, name_loc);
        }
    }

    /// Instantiates the definition of `spec` for an explicit
    /// instantiation, diagnosing an undefined template
    /// ([temp.explicit]/5).
    fn instantiate_explicit(&mut self, spec: DeclId, loc: Loc) -> bool {
        if self.instantiate_class_as(spec, loc, false, false) {
            return true;
        }
        if self.inst.undefined_diagnosed.get() == Some(spec) {
            return false;
        }
        let Some((_, info)) = self.ast.spec_info(spec) else { return false };
        let tmpl = info.template;
        let from = info.from;
        let at = if self.ast.partial_spec_info(from).is_some() {
            self.ast.decls.get(from).loc()
        } else {
            self.ast.decls.get(tmpl).loc()
        };
        let t = self.tag_type(spec);
        let note = self.diag(DiagId::note_template_decl_here, at);
        let d = self
            .diag(DiagId::sema_explicit_inst_undefined_template, loc)
            .arg(self.ty_str(t))
            .with_note(note);
        self.emit(d);
        self.inst.undefined_diagnosed.set(Some(spec));
        false
    }

    /// Instantiates the definitions of the members of `class` (an
    /// explicitly instantiated specialization or a member class of one,
    /// [temp.explicit]/10): member functions, static data members and,
    /// recursively, member classes. Member templates are not. For an
    /// explicit instantiation `declaration` ([temp.explicit]/12) only
    /// the member classes are instantiated, recursively, taking that
    /// kind (clang `InstantiateClassMembers` with
    /// `TSK_ExplicitInstantiationDeclaration`); their member functions
    /// and static data members keep their definitions uninstantiated.
    fn instantiate_members_of(&mut self, class: DeclId, loc: Loc, declaration: bool, depth: u32) {
        let Some(def) = self.ast.definition(class) else { return };
        if depth > 64 {
            return;
        }
        let members: Vec<DeclId> = self.ast.decls.members(def).collect();
        for m in members {
            if self.inst.fatal {
                return;
            }
            let n = *self.ast.decls.get(m);
            // A member that is still dependent (the partial specialization
            // of a member class template, [temp.spec.partial.member]) is
            // a template: not instantiated (WP-812).
            if n.flags().contains(DeclFlags::IMPLICIT)
                || n.flags().contains(DeclFlags::DEPENDENT_CTX)
                || self.ast.instantiated_from(m).is_none()
                || self.ast.excluded_from_explicit_instantiation(m)
            {
                continue;
            }
            // An explicitly specialized member is not instantiated from
            // the pattern ([temp.explicit]/10, clang
            // `InstantiateClassMembers` skips `TSK_ExplicitSpecialization`).
            if self
                .ast
                .member_spec_info(self.ast.first(m))
                .is_some_and(|(_, x)| x.kind == SpecKind::ExplicitSpecialization)
            {
                continue;
            }
            match n.kind() {
                DeclKind::Function
                | DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
                    if !declaration =>
                {
                    self.instantiate_function_definition(m, loc);
                }
                DeclKind::Var if !declaration => {
                    // In-class initializer, else the out-of-line
                    // definition (clang `InstantiateVariableDefinition`).
                    self.instantiate_static_member_init(m, loc);
                    self.instantiate_static_member_def(m, loc);
                }
                DeclKind::CXXRecord => {
                    // A member class the explicit instantiation defines is
                    // itself explicitly instantiated (clang
                    // `InstantiateClassMembers` sets its kind; one already
                    // defined stays implicit: its vtable is not required).
                    // A definition upgrades a member class an earlier
                    // declaration instantiated, requiring its vtable.
                    let fresh = self.ast.definition(m).is_none();
                    if !self.instantiate_class(m, loc, false) {
                        continue;
                    }
                    let kind = if declaration {
                        SpecKind::ExplicitInstantiationDeclaration
                    } else {
                        SpecKind::ExplicitInstantiationDefinition
                    };
                    let mut defined = false;
                    if let Some((i, _)) = self.ast.member_spec_info(self.ast.first(m)) {
                        self.ast.templates.update_member_spec(i, |x| {
                            let upgrade = !declaration
                                && x.kind == SpecKind::ExplicitInstantiationDeclaration;
                            if upgrade
                                || fresh
                                    && matches!(
                                        x.kind,
                                        SpecKind::Undeclared | SpecKind::ImplicitInstantiation
                                    )
                            {
                                x.kind = kind;
                                defined = !declaration;
                            }
                        });
                    }
                    self.instantiate_members_of(m, loc, declaration, depth + 1);
                    if defined {
                        self.mark_vtable_used(m, loc);
                    }
                }
                _ => {}
            }
        }
    }
}
