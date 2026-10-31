//! Function template specializations ([temp.fct.spec], [temp.inst]/5;
//! spec/sema-templates.md §6.1 step 4, §6.5): `get_function_spec` forms
//! (and memoizes by canonical arguments) the declaration of `f<args>`
//! with its substituted signature; its body is instantiated when it is
//! odr-used (`note_used_for_instantiation`), immediately for constexpr
//! and deduced-return specializations, else at the end of the
//! translation unit.

use kiln_ast::DeclId;
use kiln_ast::QualType;
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::ids::ArgListId;
use kiln_ast::templates::SpecKind;
use kiln_ast::types::TypeView;
use kiln_source::Loc;

use kiln_ast::names::DeclName;
use kiln_diag::DiagId;

use crate::Sema;
use crate::declspec::{DeclSpec, Declarator};
use crate::templ::args::ConvertedArgs;
use crate::templ::subst::Substituter;

impl Sema<'_, '_> {
    /// The specialization of function template `tmpl` for converted
    /// arguments `args` (§6.1 step 4): found, or created with the
    /// pattern's signature substituted. In a SFINAE context (deduction)
    /// a substitution failure is captured by the caller's frame. `Err`
    /// after a diagnostic (or a captured one); the failed specialization
    /// is memoized as invalid.
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    pub fn get_function_spec(
        &mut self,
        tmpl: DeclId,
        args: &ConvertedArgs,
        loc: Loc,
    ) -> Result<DeclId, ()> {
        let tmpl = self.ast.first(tmpl);
        if let Some(s) = self.ast.templates.find_spec(tmpl, args.canonical) {
            if self.ast.decls.get(s).has(DeclFlags::INVALID) {
                return Err(());
            }
            return Ok(s);
        }
        let Some(pattern) = self.pattern_of(self.inst_template(tmpl)) else { return Err(()) };
        let kind = self.kind_of(pattern);
        if !kind.is_function() {
            return Err(());
        }
        let fspec = self.ast.decls.function(pattern).map(|x| x.spec).unwrap_or_default();
        let spec = self.builder().function_spec(
            tmpl,
            args.sugared,
            args.canonical,
            kind,
            QualType::ERROR,
            fspec,
        );
        if let Some((i, _)) = self.ast.spec_info(spec) {
            self.ast.templates.update_spec(i, |s| {
                s.kind = SpecKind::ImplicitInstantiation;
                s.from = pattern;
                s.point_of_instantiation = loc;
            });
        }
        let Some((_, margs)) = self.spec_args(spec) else { return Err(()) };
        let ctx = self.ast.decls.get(spec).ctx().unwrap_or(self.ast.tu);
        let pairs = self.inst_context_pairs(ctx);
        let r = self.in_inst_context(ctx, |s| {
            let mut sub = Substituter::new(s, &margs);
            sub.entity = Some(spec);
            sub.map_contexts(&pairs);
            sub.function_spec_signature(pattern, spec)
        });
        if r.is_err() {
            self.ast.decls.set_flags(spec, DeclFlags::INVALID);
            return Err(());
        }
        Ok(spec)
    }

    /// Whether `f` is a function template specialization (clang's
    /// `getTemplateSpecializationInfo`).
    pub(crate) fn is_function_spec(&self, f: DeclId) -> bool {
        self.ast.decls.get(f).kind().is_function()
            && self
                .ast
                .spec_info(f)
                .is_some_and(|(_, i)| self.kind_of(i.template) == DeclKind::FunctionTemplate)
    }

    /// `f<int>`: how function template specialization `spec` (of `tmpl`
    /// with `args`) prints in an instantiation note.
    pub(crate) fn function_spec_name(&self, spec: DeclId, tmpl: DeclId, args: ArgListId) -> String {
        let _ = spec;
        self.spec_name(tmpl, args)
    }

    /// clang's template argument binding string, `[with T = int, N = 3]`,
    /// for the parameters of `tmpl` and the arguments `args`.
    pub(crate) fn template_args_desc(&self, tmpl: DeclId, args: ArgListId) -> String {
        let params = self.template_params(tmpl);
        let list = self.ast.types.args(args).to_vec();
        let mut parts = Vec::with_capacity(params.len());
        for (p, a) in params.iter().zip(list) {
            if matches!(a.view(), kiln_ast::types::ArgView::Null) {
                continue;
            }
            parts.push(format!("{} = {}", self.decl_name(*p), self.arg_str(a)));
        }
        format!("[with {}]", parts.join(", "))
    }

    /// The parameter types of `f`'s function type (`ERROR`s for a
    /// declaration without a prototype).
    pub(crate) fn proto_param_types(&self, f: DeclId) -> (Vec<QualType>, bool) {
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        // The written (sugared) prototype first, so mismatch notes print
        // the parameter as spelled; the canonical one as the fallback.
        if let TypeView::Function(p) = self.ast.types.view(ty.ty()) {
            return (self.ast.types.params(&p).to_vec(), p.info().variadic());
        }
        match self.ast.types.view(self.ast.types.canonical(ty).ty()) {
            TypeView::Function(p) => (self.ast.types.params(&p).to_vec(), p.info().variadic()),
            _ => (Vec::new(), false),
        }
    }
}

impl Sema<'_, '_> {
    /// Instantiates the definition of function template specialization
    /// `f` from the pattern's definition `pdef` (body `body`) at `poi`
    /// ([temp.inst]/5, §4.4).
    pub(crate) fn instantiate_function_spec_definition(
        &mut self,
        f: DeclId,
        pdef: DeclId,
        body: kiln_ast::StmtId,
        poi: Loc,
    ) {
        let Some((tmpl, args)) = self.spec_args(f) else { return };
        if !self.push_inst_frame(crate::templ::stack::FrameKind::FunctionSpec, f, tmpl, poi) {
            return;
        }
        if let Some((i, _)) = self.ast.spec_info(f) {
            self.ast.templates.update_spec(i, |s| {
                if s.point_of_instantiation == Loc::INVALID {
                    s.point_of_instantiation = poi;
                }
            });
        }
        let ctx = self.ast.decls.get(f).ctx().unwrap_or(self.ast.tu);
        let pairs = self.inst_context_pairs(ctx);
        self.in_inst_context(ctx, |s| {
            s.act_on_start_function_def(f);
            let pp: Vec<DeclId> = s
                .ast
                .decls
                .function(pdef)
                .map(|x| s.ast.lists.iter::<DeclId>(x.params).collect())
                .unwrap_or_default();
            let fp: Vec<DeclId> = s
                .ast
                .decls
                .function(f)
                .map(|x| s.ast.lists.iter::<DeclId>(x.params).collect())
                .unwrap_or_default();
            let lbrace = s.ast.stmts.get(body).loc();
            let rbrace = if s.ast.stmts.get(body).kind() == kiln_ast::stmt::StmtKind::CompoundStmt {
                s.ast.stmts.rbrace(body)
            } else {
                lbrace
            };
            let mut sub = Substituter::new(s, &args);
            sub.entity = Some(f);
            sub.map_contexts(&pairs);
            sub.map_params(&pp, &fp);
            let b = sub.body(body);
            s.act_on_finish_function_def(f, b, rbrace);
        });
        self.pop_inst_frame();
    }
}

/// A function template matched by the declarator of an explicit
/// specialization or instantiation ([temp.deduct.decl]).
struct SpecMatch {
    /// The specialization deduced.
    spec: DeclId,
    /// The function template it specializes.
    template: DeclId,
}

impl Sema<'_, '_> {
    fn loc_of(&self, d: DeclId) -> Loc {
        self.ast.decls.get(d).loc()
    }

    /// Whether declarator `d` of function type `ty` declares a function
    /// template specialization: after `template<>` or an explicit
    /// instantiation's `template`, or with a template-id naming a
    /// function template (§6.9).
    pub(crate) fn is_function_spec_declarator(&self, d: &Declarator, ty: QualType) -> bool {
        self.ast.types.is_function(ty)
            && (self.templ.explicit_spec.is_some()
                || self.templ.explicit_inst.is_some()
                || d.spec_id
                    .as_ref()
                    .is_some_and(|s| self.kind_of(s.template) == DeclKind::FunctionTemplate))
    }

    /// The declarator `d` (function type `ty`) of an explicit
    /// specialization `template<> R f<args>(params)` or explicit
    /// instantiation `[extern] template R f<args>(params);` of a function
    /// template (§6.9, [temp.expl.spec], [temp.explicit]): the template is
    /// found by deduction from the declared type ([temp.deduct.decl]), the
    /// most specialized of several kept, then the specialization's
    /// [`SpecKind`] moves as in §4.5. An explicit specialization is a new
    /// declaration of the specialization (returned; its body follows); an
    /// explicit instantiation returns `None`.
    pub(crate) fn act_on_function_spec_declarator(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        d: &Declarator,
        invalid: bool,
    ) -> Option<DeclId> {
        let spec_head = self.templ.explicit_spec.take();
        let inst = self.templ.explicit_inst.take();
        self.templ.cur = None;
        let dname = match (d.op_name, d.name) {
            (Some(op), _) => DeclName::operator(op),
            (None, Some((n, _))) => DeclName::ident(n),
            _ => {
                self.error(DiagId::parse_expected_unqualified_id, d.loc);
                return None;
            }
        };
        if invalid {
            return None;
        }
        let name_loc = d.name_loc();
        let m = self.match_function_spec(d, dname, ty, inst.is_some(), name_loc)?;
        match inst {
            Some((ext, _)) => {
                self.act_on_explicit_function_inst(m, ext.is_some(), name_loc);
                None
            }
            None => {
                let _ = spec_head;
                self.act_on_explicit_function_spec(ds, ty, d, m, name_loc)
            }
        }
    }

    /// The function template specialization declarator `d` names
    /// (§6.9 steps 1-2, [temp.deduct.decl]): every function template
    /// found by redeclaration lookup of `name` is deduced against `ty`;
    /// of the matches the most specialized is kept ([temp.func.order]).
    /// Diagnoses no match and ambiguity (clang's
    /// `CheckFunctionTemplateSpecialization`).
    fn match_function_spec(
        &mut self,
        d: &Declarator,
        name: DeclName,
        ty: QualType,
        inst: bool,
        loc: Loc,
    ) -> Option<SpecMatch> {
        let explicit = d.spec_id.as_ref().map(|s| self.ast.types.arg_list(&s.args));
        let mut templates: Vec<DeclId> = Vec::new();
        for c in self.prev_candidates_dn(name, d.qualifier) {
            if self.kind_of(c) == DeclKind::FunctionTemplate {
                let c = self.ast.first(c);
                if !templates.contains(&c) {
                    templates.push(c);
                }
            }
        }
        let mut matches: Vec<SpecMatch> = Vec::new();
        let mut failures = Vec::new();
        for &ft in &templates {
            match self.deduce_funcaddr(ft, explicit, ty, loc) {
                Ok(s) => {
                    let sty = self.ast.decls.get(s).ty().unwrap_or(QualType::ERROR);
                    if self.same_ignoring_exc_spec(sty, ty) {
                        matches.push(SpecMatch { spec: s, template: ft });
                    } else {
                        let n = self
                            .diag(DiagId::note_deduction_non_deduced_mismatch, self.loc_of(ft))
                            .arg(self.ty_str(sty))
                            .arg(self.ty_str(ty));
                        failures.push(n);
                    }
                }
                Err(f) => failures.push(self.deduction_note(ft, &f, self.loc_of(ft))),
            }
        }
        let fname = self.decl_name_str(name);
        if matches.is_empty() {
            let id = if inst {
                DiagId::sema_explicit_inst_not_known
            } else {
                DiagId::sema_fn_template_spec_no_match
            };
            let mut e = self.diag(id, loc).arg(fname);
            for n in failures {
                e = e.with_note(n);
            }
            self.emit(e);
            return None;
        }
        if matches.len() > 1 {
            let n = match self.ast.types.canonical_view(ty) {
                TypeView::Function(p) => p.num_params() as usize,
                _ => 0,
            };
            let best = (0..matches.len()).find(|&i| {
                (0..matches.len()).all(|j| {
                    i == j || self.more_specialized(matches[i].template, matches[j].template, n)
                })
            });
            match best {
                Some(i) => return Some(matches.swap_remove(i)),
                None => {
                    // clang lists the candidates last-declared first.
                    let mut e = self.diag(DiagId::sema_fn_template_spec_ambiguous, loc).arg(fname);
                    for m in matches.iter().rev() {
                        let Some((_, info)) = self.ast.spec_info(m.spec).map(|(i, s)| (i, *s))
                        else {
                            continue;
                        };
                        let note = self
                            .diag(DiagId::note_fn_template_spec_matched, self.loc_of(m.template))
                            .arg(self.spec_name(info.template, info.print_args))
                            .arg(self.template_args_desc(info.template, info.args));
                        e = e.with_note(note);
                    }
                    self.emit(e);
                    return None;
                }
            }
        }
        matches.pop()
    }

    /// `template<> R f<args>(params)` for specialization `m.spec`
    /// ([temp.expl.spec]; §4.5): diagnosed after an instantiation; else
    /// a declaration of the specialization joins its redeclarations and
    /// the specialization is never instantiated from the pattern.
    fn act_on_explicit_function_spec(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        d: &Declarator,
        m: SpecMatch,
        name_loc: Loc,
    ) -> Option<DeclId> {
        let s = m.spec;
        let (i, info) = self.ast.spec_info(s).map(|(i, x)| (i, *x))?;
        let used = self.ast.decls.get(s).has(DeclFlags::USED) || self.ast.definition(s).is_some();
        let after_inst = match info.kind {
            SpecKind::ImplicitInstantiation => used,
            SpecKind::ExplicitInstantiationDeclaration
            | SpecKind::ExplicitInstantiationDefinition => true,
            SpecKind::Undeclared | SpecKind::ExplicitSpecialization => false,
        };
        if after_inst {
            // [temp.expl.spec]/7: declared before the first use that
            // would cause an implicit instantiation.
            let explicit = u32::from(info.kind != SpecKind::ImplicitInstantiation);
            let note = self
                .diag(DiagId::note_instantiation_required_here, info.point_of_instantiation)
                .arg(explicit);
            let e = self
                .diag(DiagId::sema_spec_after_instantiation, name_loc)
                .arg(self.spec_name(info.template, info.print_args))
                .with_note(note);
            self.emit(e);
            return None;
        }
        let mut fspec = kiln_ast::decl::FnSpec::new();
        if ds.inline.is_some() {
            fspec = fspec.with_inline(true);
        }
        if ds.constexpr.is_some() {
            fspec = fspec
                .with_constexpr_kind(kiln_ast::decl::ConstexprKind::Constexpr)
                .with_implicit_inline(true);
        }
        let variadic = self.ast.types.proto(ty).is_some_and(|p| p.info().variadic());
        fspec = fspec.with_variadic(variadic);
        let old = self.ast.decls.function(s).map(|x| x.spec).unwrap_or_default();
        fspec = fspec.with_storage(old.storage());
        self.ast.templates.update_spec(i, |x| {
            x.kind = SpecKind::ExplicitSpecialization;
            x.from = info.template;
            if !used {
                x.point_of_instantiation = Loc::INVALID;
            }
        });
        if info.kind != SpecKind::ExplicitSpecialization {
            self.ast.decls.respecify_function(s, fspec);
        }
        let lexical = self.ctx;
        let f = self.builder().function_spec_redecl(s, lexical, name_loc, ty, fspec);
        let params: Vec<crate::declspec::ParamInfo> =
            d.function_params().map(<[crate::declspec::ParamInfo]>::to_vec).unwrap_or_default();
        let (defaults, _) = self.merge_default_args(None, false, &params);
        self.create_params(f, &params, &defaults);
        if self.kind_of(lexical) == DeclKind::CXXRecord {
            self.set_member_access(f);
        }
        Some(f)
    }

    /// `[extern] template R f<args>(params);` for specialization `m.spec`
    /// ([temp.explicit]; §4.5): an explicit instantiation definition
    /// instantiates the definition (now or at the end of the translation
    /// unit) and lists the specialization in the current context so that
    /// it is emitted; a declaration only records the kind.
    fn act_on_explicit_function_inst(&mut self, m: SpecMatch, declaration: bool, name_loc: Loc) {
        let s = m.spec;
        let Some((i, info)) = self.ast.spec_info(s).map(|(i, x)| (i, *x)) else { return };
        let name = self.spec_name(info.template, info.print_args);
        match (info.kind, declaration) {
            (
                SpecKind::ExplicitSpecialization | SpecKind::ExplicitInstantiationDeclaration,
                true,
            ) => return,
            (SpecKind::ExplicitSpecialization, false) => {
                // [temp.explicit]/8 (clang: no effect, a warning).
                let def = self.ast.latest(s);
                let note =
                    self.diag(DiagId::note_previous_template_specialization, self.loc_of(def));
                let e = self
                    .diag(DiagId::sema_explicit_inst_after_spec_ignored, name_loc)
                    .arg(name)
                    .with_note(note);
                self.emit(e);
                return;
            }
            (SpecKind::ExplicitInstantiationDefinition, false) => {
                // [temp.spec.general]/5: at most once.
                let note = self
                    .diag(DiagId::note_previous_explicit_instantiation, info.point_of_instantiation);
                let e = self
                    .diag(DiagId::sema_explicit_inst_duplicate, name_loc)
                    .arg(name)
                    .with_note(note);
                self.emit(e);
                return;
            }
            (SpecKind::ExplicitInstantiationDefinition, true) => {
                // [temp.explicit]/11.
                let note = self.diag(
                    DiagId::note_explicit_instantiation_definition_here,
                    info.point_of_instantiation,
                );
                let e = self
                    .diag(DiagId::sema_explicit_inst_declaration_after_definition, name_loc)
                    .with_note(note);
                self.emit(e);
                return;
            }
            _ => {}
        }
        let used = self.ast.decls.get(s).has(DeclFlags::USED);
        let kind = if declaration {
            SpecKind::ExplicitInstantiationDeclaration
        } else {
            SpecKind::ExplicitInstantiationDefinition
        };
        self.ast.templates.update_spec(i, |x| {
            x.kind = kind;
            if !used || x.point_of_instantiation == Loc::INVALID {
                x.point_of_instantiation = name_loc;
            }
        });
        if declaration {
            return;
        }
        // [temp.explicit]/10: the definition is instantiated (clang lists
        // the specialization where the explicit instantiation appears).
        let lexical = self.ctx;
        if self.ast.decls.member_list(lexical).is_some() {
            self.ast.decls.append_member(lexical, s);
        }
        self.instantiate_function_definition(s, name_loc);
        if self.ast.definition(s).is_none() {
            self.queue_instantiation(s, name_loc);
        }
    }
}
