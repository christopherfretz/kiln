//! Variable template specializations ([temp.spec.general],
//! [temp.inst]/7; spec/sema-templates.md §4.7): naming one
//! (`get_var_spec`) declares it with the substituted type of its pattern
//! (the primary template's variable or the most specialized matching
//! partial specialization, [temp.spec.partial.match]); its initializer
//! is instantiated when the variable is referenced and may be usable in
//! constant expressions, else at the end of the translation unit
//! ([temp.point]/7), as a static data member's out-of-line definition.

use kiln_ast::DeclHead;
use kiln_ast::decl::{DeclFlags, DeclKind, VarSpec};
use kiln_ast::ids::{ArgListId, TemplateParamListId};
use kiln_ast::templates::SpecKind;
use kiln_ast::types::Quals;
use kiln_ast::types::TemplateArg;
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::declspec::{DeclSpec, Declarator};
use crate::templ::spec::SpecDeclaratorId;
use crate::templ::stack::FrameKind;
use crate::templ::subst::Substituter;

impl Sema<'_, '_> {
    /// The specialization of variable template `tmpl` for `written`
    /// (§4.7), declared: found, or created and its declaration
    /// instantiated. `Err` after a diagnostic.
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    pub fn get_var_spec(
        &mut self,
        tmpl: DeclId,
        written: &[TemplateArg],
        arg_locs: &[Loc],
        loc: Loc,
    ) -> Result<DeclId, ()> {
        let tmpl = self.ast.first(tmpl);
        let conv = self.check_template_args(tmpl, written, arg_locs, loc)?;
        self.ensure_template_constraints(tmpl, conv.sugared, 2, loc)?;
        if let Some(s) = self.ast.templates.find_spec(tmpl, conv.canonical) {
            if self.ast.decls.get(s).has(DeclFlags::INVALID) {
                return Err(());
            }
            return Ok(s);
        }
        let s =
            self.instantiate_var_spec_decl(tmpl, conv.sugared, conv.canonical, written.len(), loc);
        if let Ok(v) = s
            && self.is_undeduced_auto(self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR))
        {
            // [dcl.spec.auto.general]/11: the type comes from the
            // initializer, needed now.
            self.instantiate_var_spec_init(v, loc);
        }
        s
    }

    /// The specialization of variable template `tmpl` for `written`,
    /// created `Undeclared` with type `ty` and specifiers `vspec` if new
    /// (an explicit specialization's declaration).
    fn find_or_create_var_spec(
        &mut self,
        tmpl: DeclId,
        written: &[TemplateArg],
        arg_locs: &[Loc],
        loc: Loc,
        ty: QualType,
        vspec: VarSpec,
    ) -> Result<DeclId, ()> {
        let tmpl = self.ast.first(tmpl);
        let conv = self.check_template_args(tmpl, written, arg_locs, loc)?;
        if let Some(s) = self.ast.templates.find_spec(tmpl, conv.canonical) {
            return Ok(s);
        }
        let spec = self.builder().var_spec(tmpl, conv.sugared, conv.canonical, ty, vspec);
        let print = self.suppress_default_args(tmpl, conv.sugared, written.len(), loc);
        if let Some((i, _)) = self.ast.spec_info(spec) {
            self.ast.templates.update_spec(i, |s| s.print_args = print);
        }
        Ok(spec)
    }

    /// Declares the implicit specialization of `tmpl` for converted
    /// arguments `args` (canonical `canonical`, the first `written` of
    /// them written) at `poi` ([temp.inst]/7): the most specialized
    /// matching partial specialization or the primary template supplies
    /// the pattern, whose type is substituted.
    fn instantiate_var_spec_decl(
        &mut self,
        tmpl: DeclId,
        args: ArgListId,
        canonical: ArgListId,
        written: usize,
        poi: Loc,
    ) -> Result<DeclId, ()> {
        let matched = self.match_partial_args(tmpl, args, tmpl, poi);
        // A specialization of an instantiated member template is
        // instantiated from the member template's pattern ([temp.mem]);
        // it is listed with the instantiated one.
        let (pattern, listed) = match matched {
            Ok(Some(m)) => (Some(m.partial), Some(m.partial)),
            Ok(None) => {
                // The redeclaration with the initializer (one after
                // `template<> template<class U>` defines a specialized
                // member template, [temp.expl.spec]/16).
                let p = self.pattern_of(self.inst_template(tmpl)).map(|p| {
                    self.ast
                        .redecls_of(p)
                        .find(|&x| self.ast.decls.var(x).is_some_and(|v| v.init.is_some()))
                        .unwrap_or(p)
                });
                (p, self.pattern_of(tmpl))
            }
            Err(()) => (None, None),
        };
        let pspec = pattern.and_then(|p| self.ast.decls.var(p)).map(|x| x.spec).unwrap_or_default();
        let spec = self.builder().var_spec(tmpl, args, canonical, QualType::ERROR, pspec);
        let print = self.suppress_default_args(tmpl, args, written, poi);
        let i = self.ast.spec_info(spec).map(|(i, _)| i);
        if let Some(i) = i {
            self.ast.templates.update_spec(i, |s| s.print_args = print);
        }
        let Some(pattern) = pattern.filter(|_| !self.inst.fatal) else {
            // Ambiguous (diagnosed) or no pattern.
            self.ast.decls.set_flags(spec, DeclFlags::INVALID);
            return Err(());
        };
        let from_args = matched.ok().flatten().map(|m| m.args);
        if let Some(i) = i {
            self.ast.templates.update_spec(i, |s| {
                s.kind = SpecKind::ImplicitInstantiation;
                s.from = pattern;
                s.from_args = from_args;
                s.point_of_instantiation = poi;
            });
        }
        let ploc = self.ast.decls.get(pattern).loc();
        self.builder().list_implicit_spec(spec, listed.unwrap_or(pattern));
        let Some((_, args)) = self.spec_args(spec) else { return Err(()) };
        if !self.push_inst_frame(FrameKind::VarTemplateInstantiation, spec, tmpl, poi) {
            self.ast.decls.set_flags(spec, DeclFlags::INVALID);
            return Err(());
        }
        let pty = self.ast.decls.get(pattern).ty().unwrap_or(QualType::ERROR);
        let ctx = self.ast.decls.get(spec).ctx().unwrap_or(self.ast.tu);
        let pairs = self.inst_context_pairs(ctx);
        let ty = self.in_inst_context(ctx, |s| {
            let mut sub = Substituter::new(s, &args);
            sub.map_contexts(&pairs);
            sub.ty(pty, ploc)
        });
        self.pop_inst_frame();
        let Ok(ty) = ty else {
            self.ast.decls.set_flags(spec, DeclFlags::INVALID);
            return Err(());
        };
        self.ast.decls.set_ty(spec, ty);
        if self.ast.decls.var(pattern).is_some_and(|x| x.init.is_some()) {
            self.inst.member_defs.insert(spec);
        }
        Ok(spec)
    }

    /// Instantiates the initializer of implicit variable template
    /// specialization `v` from its pattern's ([temp.inst]/7). `poi` is
    /// the point of instantiation.
    pub(crate) fn instantiate_var_spec_init(&mut self, v: DeclId, poi: Loc) {
        if self.inst.fatal || !self.inst.member_defs.remove(&v) {
            return;
        }
        let Some((_, info)) = self.ast.spec_info(v) else { return };
        let (tmpl, pattern) = (info.template, info.from);
        let Some((_, args)) = self.spec_args(v) else { return };
        if !self.push_inst_frame(FrameKind::VarTemplateInit, v, tmpl, poi) {
            return;
        }
        let ctx = self.ast.decls.get(v).ctx().unwrap_or(self.ast.tu);
        let pairs = self.inst_context_pairs(ctx);
        self.in_inst_context(ctx, |s| {
            let mut sub = Substituter::new(s, &args);
            sub.map_contexts(&pairs);
            sub.sema.initializing = Some(v);
            // A failure was diagnosed; the variable is left uninitialized.
            let _ = sub.var_init(pattern, v);
        });
        self.pop_inst_frame();
    }

    /// The defining declaration of the pattern of variable template
    /// specialization `v`, if one has been seen.
    fn var_pattern_def(&self, v: DeclId) -> Option<DeclId> {
        let (_, info) = self.ast.spec_info(v)?;
        self.ast.redecls_of(info.from).find(|&r| self.is_var_definition(r))
    }

    /// The pending instantiation of variable template specialization `v`,
    /// explicitly instantiated before its template's definition: the
    /// initializer is instantiated from the definition seen since
    /// ([temp.explicit]/10, clang `InstantiateVariableDefinition`).
    pub(crate) fn instantiate_late_var_spec(&mut self, v: DeclId, poi: Loc) {
        let Some(def) = self.var_pattern_def(v) else { return };
        let Some((i, info)) = self.ast.spec_info(v).map(|(i, s)| (i, *s)) else { return };
        if info.from == def || self.ast.decls.var(v).is_none_or(|x| x.init.is_some()) {
            return;
        }
        self.ast.templates.update_spec(i, |s| s.from = def);
        if self.ast.decls.var(def).is_some_and(|x| x.init.is_some()) {
            self.inst.member_defs.insert(v);
        }
        self.instantiate_var_spec_init(v, poi);
    }

    /// `v<int, 3>`: how the specialization of variable template `tmpl`
    /// with arguments `args` prints (its qualified name).
    pub(crate) fn spec_name(&self, tmpl: DeclId, args: ArgListId) -> String {
        let mut s = self.qualified_decl_name(tmpl);
        let names = crate::names::SemaNames::new(self);
        kiln_ast::types::write_arg_list(&self.ast.types, &names, args, &mut s);
        s
    }

    /// How a specialization declaration of `tmpl` with arguments `args`
    /// names itself (clang `getNameForDiagnostic`): its canonical
    /// arguments, every type scoped (`std::v<std::D<int>>`, not the
    /// typedef `v<_Del>` the arguments were written with).
    pub(crate) fn spec_decl_name(&self, tmpl: DeclId, args: ArgListId) -> String {
        let mut s = self.qualified_decl_name(tmpl);
        let names = crate::names::SemaNames::new(self);
        let canon = self.ast.types.canonical_args(args);
        kiln_ast::types::write_arg_list_scoped(&self.ast.types, &names, canon, &mut s);
        s
    }

    /// Whether `d` is a variable template specialization (clang's
    /// `VarTemplateSpecializationDecl`).
    pub(crate) fn is_var_spec(&self, d: DeclId) -> bool {
        self.kind_of(d) == DeclKind::Var && self.ast.spec_info(d).is_some()
    }

    /// Whether the declarator declares a definition ([basic.def]/2: not
    /// `extern` without an initializer).
    fn declares_definition(d: &Declarator, vspec: VarSpec) -> bool {
        d.init_style.is_some() || vspec.storage() != kiln_ast::decl::VarStorage::Extern
    }

    // -- declarations --------------------------------------------------------------------

    /// A declarator whose declarator-id is `v<args>` ([`SpecDeclaratorId`]):
    /// a partial specialization after a template head `head`
    /// ([temp.spec.partial]) or an explicit specialization after
    /// `template<>` ([temp.expl.spec]). `ty` is the declarator's type.
    /// The caller supplies the initializer.
    pub(crate) fn act_on_var_spec_declarator(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        d: &Declarator,
        sid: &SpecDeclaratorId,
        head: Option<TemplateParamListId>,
    ) -> Option<DeclId> {
        // [dcl.constexpr]/6: a constexpr object is const.
        let ty = if ds.constexpr.is_some() && !self.ast.types.is_reference(ty) {
            self.ast.types.qualify(ty, Quals::CONST)
        } else {
            ty
        };
        let vspec = self.var_spec(ds, d);
        let r = if let Some(head) = head {
            self.act_on_var_partial_spec(ty, vspec, d, sid, head)
        } else {
            self.templ.explicit_spec = None;
            // [temp.expl.spec]/2 (C++20 wording removed in C++23):
            // clang accepts a storage class as an extension.
            if let Some((s, l)) = ds.storage {
                let len = u32::try_from(s.spelling().len()).unwrap_or(0);
                let w = self
                    .diag(DiagId::sema_explicit_spec_storage_class, l)
                    .range(kiln_source::Span::new(l, l.with_offset(len)));
                self.emit(w);
            }
            self.act_on_explicit_var_spec(ty, vspec, d, sid)
        };
        self.templ.cur = None;
        let v = r.ok()?;
        if ds.invalid || d.invalid {
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
        }
        Some(v)
    }

    /// `template<params> T v<args> = init;` ([temp.spec.partial]): the
    /// checks of [`Sema::check_partial_args`], then the partial
    /// specialization is found or created.
    fn act_on_var_partial_spec(
        &mut self,
        ty: QualType,
        vspec: VarSpec,
        d: &Declarator,
        sid: &SpecDeclaratorId,
        head: TemplateParamListId,
    ) -> Result<DeclId, ()> {
        let tmpl = sid.template;
        let range = sid.range;
        let def = d.init_style.is_some();
        let conv =
            self.check_partial_args(tmpl, head, &sid.args, &sid.arg_locs, true, def, range)?;
        if let Some(i) = self.find_partial_decl(tmpl, conv.canonical, head) {
            let p = self.ast.templates.partial(i).map(|p| p.partial).ok_or(())?;
            if Self::declares_definition(d, vspec) && self.is_var_definition(p) {
                let msg =
                    self.diag(DiagId::sema_redefinition, sid.loc).arg(self.partial_spec_name(p));
                let msg = self.note_prev_partial_def(msg, p);
                self.emit(msg);
                return Err(());
            }
            if Self::declares_definition(d, vspec) {
                self.templ.partial_def_loc.insert(p, sid.loc);
            }
            return Ok(p);
        }
        let n = *self.ast.decls.get(tmpl);
        let ctx = n.ctx().unwrap_or(self.ast.tu);
        let mut h = DeclHead::new(ctx, n.name(), sid.loc);
        if self.ctx != ctx {
            h.lexical = Some(self.ctx);
        }
        let p = self.builder().var_partial_spec(h, tmpl, head, conv.sugared, ty, vspec);
        if self.kind_of(self.ctx) == DeclKind::CXXRecord {
            self.set_member_access(p);
        }
        self.check_partial_deducible(p, range);
        Ok(p)
    }

    /// `template<> T v<args> = init;` ([temp.expl.spec]): the
    /// specialization, checked against its previous state (clang's
    /// `CheckSpecializationInstantiationRedecl`).
    fn act_on_explicit_var_spec(
        &mut self,
        ty: QualType,
        vspec: VarSpec,
        d: &Declarator,
        sid: &SpecDeclaratorId,
    ) -> Result<DeclId, ()> {
        let tmpl = sid.template;
        let loc = sid.loc;
        let spec = self.find_or_create_var_spec(tmpl, &sid.args, &sid.arg_locs, loc, ty, vspec)?;
        if !self.check_spec_scope(tmpl, false, loc) {
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
                // clang names the template here, unqualified.
                let msg = self
                    .diag(DiagId::sema_spec_after_instantiation, loc)
                    .range(sid.range)
                    .arg(self.decl_name(tmpl))
                    .with_note(note);
                self.emit(msg);
                return Err(());
            }
            SpecKind::ExplicitSpecialization => {
                if Self::declares_definition(d, vspec) && self.is_var_definition(spec) {
                    let msg = self
                        .diag(DiagId::sema_redefinition, loc)
                        .arg(self.spec_decl_name(tmpl, info.print_args));
                    let msg = self.note_prev(msg, spec, true);
                    self.emit(msg);
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
        self.builder().declare_spec_in(spec, lexical, loc);
        if self.kind_of(lexical) == DeclKind::CXXRecord {
            self.set_member_access(spec);
        }
        Ok(spec)
    }
}

impl Sema<'_, '_> {
    /// `[extern] template T v<args>;` or `[extern] template T
    /// C<args>::m;` ([temp.explicit]/3): an explicit instantiation of a
    /// variable template specialization or of a static data member of a
    /// class template specialization. A definition instantiates the
    /// variable's definition (initializer) now and keeps it for emission
    /// ([temp.explicit]/12); a declaration only records the kind
    /// ([temp.explicit]/14).
    pub(crate) fn act_on_explicit_var_inst(
        &mut self,
        d: &Declarator,
        declaration: bool,
        invalid: bool,
    ) {
        self.templ.cur = None;
        if invalid {
            return;
        }
        let name_loc = d.name_loc();
        let kind = if declaration {
            SpecKind::ExplicitInstantiationDeclaration
        } else {
            SpecKind::ExplicitInstantiationDefinition
        };
        if let Some(sid) = &d.spec_id {
            let Ok(v) = self.get_var_spec(sid.template, &sid.args, &sid.arg_locs, sid.loc) else {
                return;
            };
            let Some((i, info)) = self.ast.spec_info(v).map(|(i, s)| (i, *s)) else { return };
            match info.kind {
                SpecKind::ExplicitSpecialization | SpecKind::ExplicitInstantiationDefinition => {
                    return;
                }
                SpecKind::ExplicitInstantiationDeclaration if declaration => return,
                _ => {}
            }
            self.ast.templates.update_spec(i, |s| {
                s.kind = kind;
                s.point_of_instantiation = name_loc;
            });
            if !declaration {
                self.instantiate_var_spec_init(v, name_loc);
                self.mark_used(v);
                if self.var_pattern_def(v).is_none() {
                    // Defined later: instantiated, and emitted, at the end
                    // of the TU (clang `PerformPendingInstantiations`).
                    self.queue_instantiation(v, name_loc);
                    let first = self.ast.first(v);
                    self.ast.templates.add_late_explicit_inst(first);
                }
            }
            return;
        }
        // clang does not compare the declared type with the member's
        // (GCC does); kiln follows clang.
        let found = d.name.and_then(|(n, _)| {
            let q = d.qualifier;
            if q.is_some_and(|q| self.kind_of(q).payload() != kiln_ast::decl::PayloadClass::Record)
            {
                return None;
            }
            self.prev_candidates_dn(kiln_ast::names::DeclName::ident(n), q)
                .into_iter()
                .find(|&c| self.kind_of(c) == DeclKind::Var)
        });
        let Some(m) = found.filter(|&c| self.ast.instantiated_from(c).is_some()) else {
            let name = d.name.map_or(String::new(), |(n, _)| crate::decl::sym_str(self, n));
            let mut e = self.diag(DiagId::sema_explicit_inst_not_known, name_loc).arg(name);
            if let Some(c) = found {
                let note = self
                    .diag(DiagId::note_explicit_instantiation_here, self.ast.decls.get(c).loc());
                e = e.with_note(note);
            }
            self.emit(e);
            return;
        };
        let m = self.ast.first(m);
        if let Some((i, info)) = self.ast.member_spec_info(m).map(|(i, x)| (i, *x)) {
            match info.kind {
                SpecKind::ExplicitSpecialization | SpecKind::ExplicitInstantiationDefinition => {
                    return;
                }
                SpecKind::ExplicitInstantiationDeclaration if declaration => return,
                _ => {}
            }
            self.ast.templates.update_member_spec(i, |x| {
                x.kind = kind;
                x.point_of_instantiation = name_loc;
            });
        }
        if declaration {
            return;
        }
        self.instantiate_static_member_init(m, name_loc);
        self.instantiate_static_member_def(m, name_loc);
        if self.inst.member_defs.contains(&m) {
            // Defined later, or an error at the end of the TU (clang
            // `PerformPendingInstantiations`, spec/sema-templates.md §4.4).
            self.queue_instantiation(m, name_loc);
        }
        self.mark_used(m);
    }
}
