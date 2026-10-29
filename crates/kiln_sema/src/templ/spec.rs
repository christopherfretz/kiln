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

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::ids::ArgListId;
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{ArgView, TagKeyword, TemplateArg, TypeFlags};
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_diag::DiagId;
use kiln_diag::FixIt;
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::decl::TagUse;
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
    }

    /// The declaration after `template<>` ended.
    pub fn act_explicit_spec_end(&mut self) {
        self.templ.explicit_spec = None;
    }

    /// `[extern] template` without `<` was parsed: the class template-id
    /// declared next is explicitly instantiated ([temp.explicit]/2).
    /// Paired with [`Sema::act_explicit_inst_end`].
    pub fn act_explicit_inst_start(&mut self, extern_loc: Option<Loc>, loc: Loc) {
        self.templ.explicit_inst = Some((extern_loc, loc));
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
        r_angle: Loc,
        kw_loc: Loc,
    ) -> Result<(Option<DeclId>, QualType), ()> {
        let range = Span::new(name_loc, r_angle.with_offset(1));
        let tmpl = self.ast.first(tmpl);
        let locs = std::mem::take(&mut self.templ.arg_locs);
        let declares = matches!(use_, TagUse::Definition | TagUse::Declaration);
        let form = if !declares {
            TagForm::Reference
        } else if let Some((ext, _)) = self.templ.explicit_inst.take() {
            TagForm::ExplicitInst { declaration: ext.is_some() }
        } else if self.templ.explicit_spec.take().is_some() {
            TagForm::ExplicitSpec
        } else if self.take_template_head(qualifier).is_some() {
            TagForm::Partial
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
                let spec = self.act_on_explicit_class_spec(tmpl, args, &locs, use_, range)?;
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
                if dependent || use_ == TagUse::Friend && self.in_template() {
                    if use_ == TagUse::Friend {
                        self.templ_unimplemented("friend class template specialization", name_loc);
                        return Err(());
                    }
                    let t = self.ast.types.template_specialization(name, list, None, false);
                    return Ok((None, self.ast.types.elaborated(keyword, None, t)));
                }
                let spec = self.get_class_spec(tmpl, args, &locs, name_loc)?;
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
    /// template may be defined); diagnoses otherwise.
    pub(crate) fn check_spec_scope(&mut self, tmpl: DeclId, partial: bool, loc: Loc) -> bool {
        let tctx = self.ast.decls.get(tmpl).ctx().unwrap_or(self.ast.tu);
        let tctx = self.ast.redecl_context(tctx);
        let cur = self.ast.redecl_context(self.ctx);
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
        let id = match (partial, var) {
            (true, false) => DiagId::sema_partial_spec_redecl_scope,
            (false, false) => DiagId::sema_class_spec_not_in_enclosing_ns,
            (true, true) => DiagId::sema_var_partial_spec_redecl_scope,
            (false, true) => DiagId::sema_var_spec_not_in_enclosing_ns,
        };
        let note =
            self.diag(DiagId::note_explicit_spec_template_declared, self.ast.decls.get(tmpl).loc());
        let d =
            self.diag(id, loc).arg(self.decl_name(tmpl)).arg(self.decl_name(tctx)).with_note(note);
        self.emit(d);
        false
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
        if let Some(i) = self.ast.templates.find_partial(tmpl, conv.canonical) {
            let p = self.ast.templates.partial(i).map(|p| p.partial).ok_or(())?;
            if use_ == TagUse::Definition
                && let Some(def) = self.ast.definition(p)
            {
                let d = self
                    .diag(DiagId::sema_redefinition, name_loc)
                    .range(range)
                    .arg(self.partial_spec_name(p));
                let d = self.note_prev(d, def, true);
                self.emit(d);
                return Err(());
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
        if !types.arg_list_flags(conv.canonical).intersects(TypeFlags::DEPENDENT_INSTANTIATION) {
            let d = self
                .diag(DiagId::sema_partial_spec_fully_specialized, name_loc)
                .arg(self.decl_name(tmpl));
            self.emit(d);
            return Err(());
        }
        if self.ast.templates.param_list(head).requires.is_some() {
            // Constrained partial specializations order by constraints
            // ([temp.spec.partial.order]/1, [temp.constr.order]: WP-073)
            // and may repeat the primary's arguments
            // ([temp.spec.partial.general]/9.3).
            let what = if var { "variable" } else { "class" };
            self.templ_unimplemented(
                &format!("constrained {what} template partial specialization"),
                name_loc,
            );
            return Err(());
        }
        // [temp.spec.partial.general]/9.3: the arguments are not the
        // primary template's own parameters in order.
        let params = self.template_params(tmpl);
        let conv_args: Vec<TemplateArg> = self.ast.types.args(conv.sugared).to_vec();
        if params.len() == conv_args.len()
            && params.iter().zip(&conv_args).all(|(&p, &a)| self.arg_names_param(p, a))
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

    /// [temp.spec.partial.general]/8 (clang's
    /// `checkTemplatePartialSpecialization`): every parameter of partial
    /// specialization `p` is deducible from its arguments, else it can
    /// never match (`-Wunusable-partial-specialization`).
    pub(crate) fn check_partial_deducible(&mut self, p: DeclId, range: Span) {
        let loc = range.lo;
        let Some((_, info)) = self.ast.partial_spec_info(p) else { return };
        let info = *info;
        if info.has_pack {
            return;
        }
        let list = *self.ast.templates.param_list(info.params);
        let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(list.params).collect();
        let args: Vec<TemplateArg> = self.ast.types.args(info.canonical).to_vec();
        if args.iter().any(|&a| self.is_pack_arg(a)) {
            return;
        }
        let mut d = DeducedArgs::new(list.depth, params.len());
        if self.deduce_args(&args, &args, true, &mut d, loc) != Tdk::Success {
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

    /// `X<T *>`: how partial specialization `p` prints (its qualified
    /// name and its arguments as written).
    pub(crate) fn partial_spec_name(&self, p: DeclId) -> String {
        let mut s = self.qualified_decl_name(p);
        if let Some((_, info)) = self.ast.partial_spec_info(p) {
            let names = crate::names::SemaNames::new(self);
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
            s.push_str(&self.arg_str(a));
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
                let t = self.tag_type(spec);
                let d = self
                    .diag(DiagId::sema_spec_after_instantiation, name_loc)
                    .range(range)
                    .arg(self.ty_str(t))
                    .with_note(note);
                self.emit(d);
                return Err(());
            }
            SpecKind::ExplicitSpecialization => {
                if use_ == TagUse::Definition
                    && let Some(def) = self.ast.definition(spec)
                {
                    let t = self.tag_type(spec);
                    let d = self
                        .diag(DiagId::sema_redefinition, name_loc)
                        .range(range)
                        .arg(self.ty_str(t));
                    let d = self.note_prev(d, def, true);
                    self.emit(d);
                    return Err(());
                }
                return Ok(spec);
            }
        }
        self.ast.templates.update_spec(i, |s| {
            s.kind = SpecKind::ExplicitSpecialization;
            s.from = spec;
        });
        let lexical = self.ctx;
        self.builder().declare_spec_in(spec, lexical, name_loc);
        if self.kind_of(lexical) == DeclKind::CXXRecord {
            self.set_member_access(spec);
        }
        Ok(spec)
    }

    // -- explicit instantiations -----------------------------------------------------------

    /// `[extern] template class X<args>;` ([temp.explicit]): the
    /// specialization's definition is instantiated, and (an explicit
    /// instantiation definition, [temp.explicit]/10) the definitions of
    /// its members.
    fn act_on_explicit_class_inst(
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
        if first_explicit {
            let lexical = self.ctx;
            self.builder().declare_spec_in(spec, lexical, name_loc);
        }
        if !declaration {
            self.instantiate_members_of(spec, name_loc, 0);
        }
        Ok(spec)
    }

    /// Instantiates the definition of `spec` for an explicit
    /// instantiation, diagnosing an undefined template
    /// ([temp.explicit]/5).
    fn instantiate_explicit(&mut self, spec: DeclId, loc: Loc) -> bool {
        if self.instantiate_class(spec, loc, false) {
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
    /// recursively, member classes. Member templates are not.
    fn instantiate_members_of(&mut self, class: DeclId, loc: Loc, depth: u32) {
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
            if n.flags().contains(DeclFlags::IMPLICIT) || self.ast.instantiated_from(m).is_none() {
                continue;
            }
            match n.kind() {
                DeclKind::Function
                | DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion => self.instantiate_function_definition(m, loc),
                DeclKind::Var => self.instantiate_static_member_init(m, loc),
                DeclKind::CXXRecord if self.instantiate_class(m, loc, false) => {
                    self.instantiate_members_of(m, loc, depth + 1);
                }
                _ => {}
            }
        }
    }
}
