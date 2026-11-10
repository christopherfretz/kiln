//! Function template specializations ([temp.fct.spec], [temp.inst]/5;
//! spec/sema-templates.md §6.1 step 4, §6.5): `get_function_spec` forms
//! (and memoizes by canonical arguments) the declaration of `f<args>`
//! with its substituted signature; its body is instantiated when it is
//! odr-used (`note_used_for_instantiation`), immediately for constexpr
//! and deduced-return specializations, else at the end of the
//! translation unit.

use kiln_ast::QualType;
use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind};
use kiln_ast::ids::{ArgListId, RawId};
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{ExcSpec, NoexceptValue, TypeView};
use kiln_ast::{DeclId, ExprId};
use kiln_source::Loc;

use kiln_ast::names::{DeclName, DeclNameKind};
use kiln_diag::DiagId;

use crate::Sema;
use crate::declspec::{DeclSpec, Declarator, DeclaratorContext};
use crate::templ::args::ConvertedArgs;
use crate::templ::subst::Substituter;
use crate::type_diff::DiffArgs as _;

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
        let spec = self.get_function_spec_signature(tmpl, *args, loc)?;
        self.finish_function_spec_explicit(spec)?;
        Ok(spec)
    }

    /// [`Self::get_function_spec`] with the explicit-specifier left
    /// pending: deduction checks the associated constraints first and
    /// substitutes it after ([temp.deduct.general]/5, clang
    /// `instantiateExplicitSpecifierDeferred`), so a constraint that
    /// fails keeps an `explicit(expr)` that would be ill-formed for the
    /// arguments from being substituted (libstdc++ `tuple`'s
    /// `explicit(!__convertible_from_tuple_like<_UTuple>())`; §6.1).
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    pub(crate) fn get_function_spec_signature(
        &mut self,
        tmpl: DeclId,
        args: ConvertedArgs,
        loc: Loc,
    ) -> Result<DeclId, ()> {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("get_function_spec_signature");
        let tmpl = self.ast.first(tmpl);
        let poi = self.templ.deduce_poi.take().unwrap_or(loc);
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
        // A specialization escalates on its own (P2564): clang does not
        // carry an escalated pattern's `immediate` over.
        let fspec = self
            .ast
            .decls
            .function(pattern)
            .map(|x| x.spec.with_escalated(false))
            .unwrap_or_default();
        let sugared = self.settle_inst_sugar(args.sugared);
        let spec = self.builder().function_spec(
            tmpl,
            sugared,
            args.canonical,
            kind,
            QualType::ERROR,
            fspec,
        );
        if let Some((i, _)) = self.ast.spec_info(spec) {
            self.ast.templates.update_spec(i, |s| {
                s.kind = SpecKind::ImplicitInstantiation;
                s.from = pattern;
                s.point_of_instantiation = poi;
            });
        }
        let Some((_, margs)) = self.spec_args(spec) else { return Err(()) };
        let ctx = self.ast.decls.get(spec).ctx().unwrap_or(self.ast.tu);
        let pairs = self.inst_context_pairs(self.spec_pairs_ctx(spec, tmpl));
        // An error the enclosing SFINAE frame captures while the signature
        // is substituted (a narrowing check inside a `decltype` return
        // type) fails the specialization even when the substitution itself
        // returns `Ok`: memoize it as invalid, so the next deduction for
        // the same arguments fails too ([temp.deduct.general]/8).
        let trapped = self.sfinae_errors();
        let held = self.sfinae_warnings();
        let access = self.friend_template_access_ctx(spec);
        // A member template of a class template specialization numbers
        // in its own templated function (`S<int>::f`), not the pattern's.
        let own = self.templated_function(tmpl).unwrap_or(pattern);
        let site = self.begin_signature_mangling(tmpl, own);
        let r = self.in_inst_context(ctx, |s| {
            s.access_ctx = access;
            let mut sub = Substituter::new(s, &margs);
            sub.entity = Some(spec);
            sub.map_contexts(&pairs);
            sub.function_spec_signature(pattern, spec)
        });
        self.end_signature_mangling(site);
        let captured = self.sfinae_errors() > trapped;
        let warnings = self.take_sfinae_warnings(held);
        if !warnings.is_empty() && r.is_ok() && !captured {
            let _ = self.templ.suppressed_diags.insert(spec, warnings);
        }
        if r.is_err() || captured {
            self.ast.decls.set_flags(spec, DeclFlags::INVALID);
            return Err(());
        }
        if self.fn_spec(pattern).is_explicit()
            && self.ast.decls.cold.explicit_expr.get(pattern).is_some()
        {
            let _ = self.templ.explicit_pending.insert(spec);
        }
        Ok(spec)
    }

    /// The access context the signature of specialization `spec` of a
    /// friend function template is substituted in: the specialization
    /// itself, whose effective context continues into the befriending
    /// class ([class.friend]/7, [temp.deduct.general]/8: `-> decltype(
    /// s.priv_)` of a friend defined in the class may name its private
    /// members). clang substitutes a friend template's declaration with
    /// its lexical context as the owner and the templated declaration as
    /// the current context (`FinishTemplateArgumentDeduction`). `None`
    /// (the specialization's semantic context) for any other template.
    fn friend_template_access_ctx(&self, spec: DeclId) -> Option<DeclId> {
        self.friend_decl_of(spec).map(|_| spec)
    }

    /// Substitutes the explicit-specifier [`Self::get_function_spec_signature`]
    /// left pending on function template specialization `spec`
    /// ([temp.deduct.general]/7: a failure is a substitution failure, and
    /// the specialization is memoized as invalid). `Ok` when none is
    /// pending.
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    pub(crate) fn finish_function_spec_explicit(&mut self, spec: DeclId) -> Result<(), ()> {
        if !self.templ.explicit_pending.remove(&spec) {
            return Ok(());
        }
        let Some((_, info)) = self.ast.spec_info(spec) else { return Ok(()) };
        let tmpl = info.template;
        let Some(pattern) = self.pattern_of(self.inst_template(tmpl)) else { return Err(()) };
        let Some((_, margs)) = self.spec_args(spec) else { return Err(()) };
        let ctx = self.ast.decls.get(spec).ctx().unwrap_or(self.ast.tu);
        let pairs = self.inst_context_pairs(self.spec_pairs_ctx(spec, tmpl));
        let trapped = self.sfinae_errors();
        let access = self.friend_template_access_ctx(spec);
        let r = self.in_inst_context(ctx, |s| {
            s.access_ctx = access;
            let mut sub = Substituter::new(s, &margs);
            sub.entity = Some(spec);
            sub.map_contexts(&pairs);
            sub.function_spec_explicit(pattern, spec)
        });
        let captured = self.sfinae_errors() > trapped;
        if r.is_err() || captured {
            self.ast.decls.set_flags(spec, DeclFlags::INVALID);
            return Err(());
        }
        Ok(())
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
        if self.kind_of(spec) != DeclKind::CXXConversion {
            return self.spec_decl_name(tmpl, args);
        }
        // A conversion function template specialization is named by its
        // substituted target type, then its arguments (`operator int<int>`).
        self.qualified_decl_name(spec)
    }

    /// clang's template argument binding string, `[with T = int, N = 3]`,
    /// for the parameters of `tmpl` and the arguments `args`.
    pub(crate) fn template_args_desc(&self, tmpl: DeclId, args: ArgListId) -> String {
        self.args_desc(tmpl, args, false)
    }

    /// [`Sema::template_args_desc`] for arguments a deduction produced
    /// (the failed-candidate notes): a canonical type, deduced from inside
    /// a template-id or a base class, prints fully scoped as clang prints
    /// a bare `RecordType` (`_Container = std::deque<int>`); a type named
    /// in the call or the explicit arguments carries its written sugar
    /// ([`Sema::written_bare`]).
    pub(crate) fn deduced_args_desc(&self, tmpl: DeclId, args: ArgListId) -> String {
        self.args_desc(tmpl, args, true)
    }

    fn args_desc(&self, tmpl: DeclId, args: ArgListId, deduced: bool) -> String {
        let params = self.template_params(tmpl);
        let list = self.ast.types.args(args).to_vec();
        let mut parts = Vec::with_capacity(params.len());
        for (i, (p, a)) in params.iter().zip(list).enumerate() {
            let arg = match a.view() {
                kiln_ast::types::ArgView::Null => continue,
                kiln_ast::types::ArgView::Type(q)
                    if deduced && self.ast.types.canonical(q) == q =>
                {
                    let names = crate::names::sema_names(self);
                    let opts = kiln_ast::types::PrintOptions {
                        full_scope: true,
                        ..kiln_ast::types::PrintOptions::SUGARED
                    };
                    kiln_ast::types::print_type(&self.ast.types, &names, q, opts)
                }
                _ => self.binding_arg_str(a),
            };
            parts.push(format!("{} = {}", self.binding_param_name(*p, i), arg));
        }
        format!("[with {}]", parts.join(", "))
    }

    /// Template parameter `p` (the `i`th) as a binding names it: its
    /// name, `$i` when unnamed (clang's `getTemplateArgumentBindingsText`).
    fn binding_param_name(&self, p: DeclId, i: usize) -> String {
        let n = self.decl_name(p);
        if n.is_empty() { format!("${i}") } else { n }
    }

    /// The parameter types of `f`'s function type (`ERROR`s for a
    /// declaration without a prototype).
    pub(crate) fn proto_param_types(&self, f: DeclId) -> (Vec<QualType>, bool) {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("proto_param_types");
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
        // Already being instantiated further up (mutually recursive
        // constexpr templates, each instantiated at its use): the
        // definition is in progress (clang's `isDefined` is true once
        // `InstantiateFunctionDefinition` has started).
        if self.inst.stack.contains(crate::templ::stack::FrameKind::FunctionSpec, f) {
            return;
        }
        if !self.push_inst_frame(crate::templ::stack::FrameKind::FunctionSpec, f, tmpl, poi) {
            return;
        }
        let name_loc = self.ast.decls.get(f).loc();
        self.check_consteval_only_inst(f, pdef, name_loc);
        if let Some((i, _)) = self.ast.spec_info(f) {
            self.ast.templates.update_spec(i, |s| {
                if s.point_of_instantiation == Loc::INVALID {
                    s.point_of_instantiation = poi;
                }
            });
        }
        let ctx = self.ast.decls.get(f).ctx().unwrap_or(self.ast.tu);
        let pairs = self.inst_context_pairs(self.spec_pairs_ctx(f, tmpl));
        self.in_inst_context(ctx, |s| {
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
            // The specialization was declared from an earlier declaration
            // whose parameters may be named differently: the body sees
            // the definition's names (clang instantiates the parameters
            // from the definition), so `T e_in` then a local `T e` is no
            // redefinition of a forward declaration's `e`; an expanded
            // pack's elements take the pack's name.
            sub.rename_params_from_pattern(&pp);
            sub.sema.act_on_start_function_def(f);
            let mut inits_failed = false;
            if sub.sema.kind_of(f) == DeclKind::CXXConstructor {
                // A constructor template's ctor-initializer, as a class
                // template member's (`templ/class.rs`).
                let (inits, colon) = sub
                    .sema
                    .inst
                    .pattern_mem_inits
                    .get(&pdef)
                    .cloned()
                    .unwrap_or((Vec::new(), lbrace));
                for info in &inits {
                    match sub.mem_initializer(info) {
                        Ok(v) => {
                            for i in &v {
                                sub.sema.act_on_mem_initializer(f, i);
                            }
                        }
                        Err(_) => sub.sema.note_failed_mem_initializer(f),
                    }
                }
                inits_failed = sub.sema.instantiated_mem_inits_failed(f);
                if sub.sema.inst.failed_mem_inits.contains(&pdef) {
                    sub.sema.note_failed_mem_initializer(f);
                }
                sub.sema.act_on_mem_initializers(f, &[], colon);
            }
            let b = sub.body(body);
            // After the body: a deduced return type is known
            // ([dcl.contract.res]/2).
            sub.function_contracts(f, pdef);
            // A statement of the body failed: the specialization is
            // invalid (clang's `InstantiateFunctionDefinition` marks it
            // when the body is `StmtError`), so a later call deduces it
            // as a substitution failure and an undeduced placeholder
            // return type is not deduced `void` (spec/sema-templates.md
            // §5.5).
            // A mem-initializer that failed to instantiate does too (clang's
            // `InstantiateMemInitializers`): the constructor is never
            // constant-evaluated, so no evaluation note follows (WP-374).
            // clang 22: a constexpr specialization whose body names a
            // variable whose initializer failed is invalid too.
            let named_failed = sub.failed_init_use
                && sub.sema.clang_compat >= 22
                && sub
                    .sema
                    .ast
                    .decls
                    .function(f)
                    .is_some_and(|x| x.spec.constexpr_kind() != ConstexprKind::None);
            if sub.stmt_failed || inits_failed || named_failed {
                s.ast.decls.set_flags(f, DeclFlags::INVALID);
            }
            s.act_on_finish_function_def(f, b, rbrace);
        });
        self.replay_dependent_access(pdef, f);
        self.pop_inst_frame();
    }
}

/// A function template matched by the declarator of an explicit
/// specialization or instantiation ([temp.deduct.decl]).
#[derive(Clone, Copy)]
pub(crate) struct SpecMatch {
    /// The specialization deduced.
    pub(crate) spec: DeclId,
    /// The function template it specializes.
    pub(crate) template: DeclId,
}

impl Sema<'_, '_> {
    fn loc_of(&self, d: DeclId) -> Loc {
        self.ast.decls.get(d).loc()
    }

    /// Whether `template<>` or an explicit instantiation's `template`
    /// heads the declaration being parsed (not a declaration met while
    /// instantiating a class named in its qualifier).
    pub(crate) fn at_explicit_head(&self) -> bool {
        (self.templ.explicit_spec.is_some() || self.templ.explicit_inst.is_some())
            && self.inst.stack.len() == self.templ.explicit_depth
    }

    /// Whether declarator `d` of function type `ty` declares a function
    /// template specialization: after `template<>` or an explicit
    /// instantiation's `template`, or with a template-id naming a
    /// function template (§6.9).
    pub(crate) fn is_function_spec_declarator(&self, d: &Declarator, ty: QualType) -> bool {
        self.ast.types.is_function(ty)
            && (self.at_explicit_head()
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
    /// explicit instantiation returns `None`. `Err` when no declaration
    /// of the name precedes an explicit specialization of an unqualified
    /// name: after "no function template matches", the caller declares
    /// it as a plain, invalid function, as clang does, so a later call
    /// resolves to it (WP-318).
    pub(crate) fn act_on_function_spec_declarator(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        d: &Declarator,
        invalid: bool,
    ) -> Result<Option<DeclId>, ()> {
        let head = self.at_explicit_head();
        let spec_head = if head { self.templ.explicit_spec.take() } else { None };
        let inst = if head { self.templ.explicit_inst.take() } else { None };
        self.templ.cur = None;
        let dname = match (d.op_name, d.name) {
            (Some(op), _) => DeclName::operator(op),
            (None, Some((n, _))) => d.fn_decl_name(n),
            _ => {
                self.error(DiagId::parse_expected_unqualified_id, d.loc);
                return Ok(None);
            }
        };
        // A parameter that names no type makes the declaration invalid,
        // and clang matches no template for an invalid one
        // (`CheckFunctionTemplateSpecialization` runs only for a valid
        // declaration; WP-308).
        if invalid || d.function_params().is_some_and(|ps| ps.iter().any(|p| p.invalid)) {
            return Ok(None);
        }
        let name_loc = d.name_loc();
        // [temp.expl.spec]/2, [temp.dep.general]: a class-scope explicit
        // specialization in a dependent class with no member function
        // template of the name (clang's
        // `CheckDependentFunctionTemplateSpecialization`; WP-330). The
        // declaration is dropped.
        if inst.is_none()
            && d.ctx == DeclaratorContext::Member
            && d.qualifier.is_none()
            && self.in_template()
        {
            let found = self.current_scope_with_inline_dn(dname);
            if self.function_templates_in(&found).is_empty() {
                self.dependent_spec_no_candidate(
                    DiagId::sema_dependent_member_spec_no_candidate,
                    &found,
                    name_loc,
                );
                return Ok(None);
            }
        }
        if inst.is_none()
            && d.qualifier.is_none()
            && d.spec_id.is_none()
            && self.current_scope_with_inline_dn(dname).is_empty()
        {
            let fname = self.decl_name_str(dname);
            let msg = self.diag(DiagId::sema_fn_template_spec_no_match, name_loc).arg(fname);
            self.emit(msg);
            return Err(());
        }
        if d.spec_id.is_none()
            && let Some(m) = self.member_of_spec_candidate(d, dname, ty, inst.is_some())
        {
            let Some(m) = m else { return Ok(None) };
            // [temp.expl.spec]/1, [temp.explicit]/3: a member function
            // of an implicit class template specialization.
            return Ok(match inst {
                Some((ext, _)) => {
                    self.act_on_explicit_member_inst(m, ext.is_some(), name_loc);
                    None
                }
                None => self.act_on_explicit_member_spec(ds, ty, d, m, name_loc),
            });
        }
        let Some(m) = self.match_function_spec(d, dname, ty, inst.is_some(), name_loc) else {
            return Ok(None);
        };
        let _ = spec_head;
        if let Some((ext, _)) = inst {
            if !self.check_inst_exc_spec(m.spec, ty, ds.loc) {
                return Ok(None);
            }
            let attrs: Vec<_> = ds.attrs.iter().chain(&d.attrs).copied().collect();
            self.explicit_inst_visibility_attrs(m.spec, &attrs);
            self.act_on_explicit_function_inst(m, ext.is_some(), name_loc);
            return Ok(None);
        }
        Ok(self.act_on_explicit_function_spec(ds, ty, d, m, name_loc))
    }

    /// `[extern] template C::C(params);` / `template C::operator T*();`:
    /// an explicit instantiation whose out-of-line constructor or
    /// conversion-function declarator matched no non-template member of
    /// `class` names a specialization of one of its member function
    /// templates (`templates`), deduced from the declarator's type `ty`
    /// ([temp.explicit]/4, [temp.deduct.decl]; clang
    /// `ActOnExplicitInstantiation`, which for a conversion function
    /// considers every conversion function template, [temp.mem]/6).
    /// No match or an ambiguity is diagnosed as for a named function
    /// template. Declares nothing.
    pub(crate) fn act_on_explicit_special_template_inst(
        &mut self,
        ds: &DeclSpec,
        d: &Declarator,
        templates: &[DeclId],
        name: DeclName,
        ty: QualType,
        loc: Loc,
    ) {
        let Some((ext, _)) = self.templ.explicit_inst.take() else { return };
        self.templ.cur = None;
        let notes = !matches!(name.kind(), DeclNameKind::Conversion);
        let Some(m) = self.match_function_spec_noted(templates, None, name, ty, true, loc, notes)
        else {
            return;
        };
        if !self.check_inst_exc_spec(m.spec, ty, ds.loc) {
            return;
        }
        let attrs: Vec<_> = ds.attrs.iter().chain(&d.attrs).copied().collect();
        self.explicit_inst_visibility_attrs(m.spec, &attrs);
        self.act_on_explicit_function_inst(m, ext.is_some(), loc);
    }

    /// `template<> C::C(params) ...` / `template<> C::operator T()`: the
    /// explicit specialization counterpart of
    /// [`Self::act_on_explicit_special_template_inst`]: an out-of-line
    /// constructor, conversion-function or `operator=` declarator after
    /// `template<>` that matched no non-template member declares the
    /// specialization of one of `templates` deduced from `ty`
    /// ([temp.expl.spec]/1, [temp.deduct.decl]; clang
    /// `CheckFunctionTemplateSpecialization`). Returns the new
    /// declaration, whose body follows.
    pub(crate) fn act_on_explicit_special_template_spec(
        &mut self,
        ds: &DeclSpec,
        d: &Declarator,
        templates: &[DeclId],
        name: DeclName,
        ty: QualType,
        loc: Loc,
    ) -> Option<DeclId> {
        self.templ.explicit_spec = None;
        self.templ.cur = None;
        let notes = !matches!(name.kind(), DeclNameKind::Conversion);
        let m = self.match_function_spec_noted(templates, None, name, ty, false, loc, notes)?;
        self.act_on_explicit_function_spec(ds, ty, d, m, loc)
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
        // An explicit specialization may name a template of an inline
        // namespace ([temp.expl.spec]/3).
        let found = match d.qualifier {
            None => self.current_scope_with_inline_dn(name),
            Some(_) => self.prev_candidates_dn(name, d.qualifier),
        };
        let templates = self.function_templates_in(&found);
        self.match_function_spec_in(&templates, explicit, name, ty, inst, loc)
    }

    /// The function templates among `found` (first declarations, once
    /// each).
    pub(crate) fn function_templates_in(&self, found: &[DeclId]) -> Vec<DeclId> {
        let mut templates: Vec<DeclId> = Vec::new();
        for &c in found {
            if self.kind_of(c) == DeclKind::FunctionTemplate {
                let c = self.ast.first(c);
                if !templates.contains(&c) {
                    templates.push(c);
                }
            }
        }
        templates
    }

    /// [`Self::match_function_spec`] over the candidate `templates`, with
    /// the `explicit` template arguments written.
    pub(crate) fn match_function_spec_in(
        &mut self,
        templates: &[DeclId],
        explicit: Option<ArgListId>,
        name: DeclName,
        ty: QualType,
        inst: bool,
        loc: Loc,
    ) -> Option<SpecMatch> {
        self.match_function_spec_noted(templates, explicit, name, ty, inst, loc, true)
    }

    /// How a candidate of an ambiguous explicit instantiation or
    /// specialization is named: a conversion function template
    /// specialization by its own name (`X::operator char *<char>`, clang),
    /// any other by its template and arguments.
    fn spec_name_noted(&self, conv: bool, spec: DeclId, tmpl: DeclId, args: ArgListId) -> String {
        if conv { self.qualified_decl_name(spec) } else { self.spec_decl_name(tmpl, args) }
    }

    /// [`Self::match_function_spec_in`]; `notes` false lists no
    /// candidate that failed deduction (a conversion function template
    /// that does not deduce is not found by lookup, [temp.mem]/6, so
    /// clang notes none).
    #[allow(clippy::too_many_arguments, reason = "the matcher's inputs plus the note switch")]
    fn match_function_spec_noted(
        &mut self,
        templates: &[DeclId],
        explicit: Option<ArgListId>,
        name: DeclName,
        ty: QualType,
        inst: bool,
        loc: Loc,
        notes: bool,
    ) -> Option<SpecMatch> {
        let mut matches: Vec<SpecMatch> = Vec::new();
        let mut failures = Vec::new();
        // [temp.deduct.decl]: the exception specifications take no part
        // (clang `adjustCCAndNoReturn(..., AdjustExceptionSpec)`); the
        // caller checks the specialization's against the one written
        // ([`Self::check_inst_exc_spec`], [`Self::check_spec_exc_spec`]).
        // An explicit instantiation that writes none takes the template's
        // ([temp.explicit]/9; libstdc++'s `extern template bool
        // has_facet<...>(const locale&)` of a `throw()` template).
        let suppress = self.clang_compat >= 22 && !self.inst.suppress_access.replace(true);
        for &ft in templates {
            // clang notes the template lookup found: its most recent
            // declaration (an out-of-line definition).
            let at = self.loc_of(self.ast.latest(ft));
            match self.deduce_decl(ft, explicit, ty, loc) {
                Ok(s) => {
                    let sty = self.ast.decls.get(s).ty().unwrap_or(QualType::ERROR);
                    if self.same_ignoring_exc_spec(sty, ty) {
                        matches.push(SpecMatch { spec: s, template: ft });
                    } else if notes {
                        let n = self
                            .diag(DiagId::note_deduction_non_deduced_mismatch, at)
                            .diff_args(self, sty, ty);
                        failures.push(n);
                    }
                }
                // An invalid template is not noted (clang
                // `TemplateDeductionResult::Invalid`).
                Err(f) if f.tdk == crate::templ::deduce::Tdk::Invalid => {}
                Err(f) if notes => failures.push(self.deduction_note(ft, &f, at)),
                Err(_) => {}
            }
        }
        if suppress {
            self.inst.suppress_access.set(false);
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
            // [temp.deduct.partial]/3.3 uses the function types; for
            // conversion function templates, which have no parameters,
            // that orders them by their conversion types, as in a call
            // (clang `TPOC_Other` deduces from the whole function type).
            let conv = name.kind() == DeclNameKind::Conversion;
            let best = (0..matches.len()).find(|&i| {
                (0..matches.len()).all(|j| {
                    let (a, b) = (matches[i].template, matches[j].template);
                    i == j
                        || if conv {
                            self.more_specialized_conversion(a, b)
                        } else {
                            self.more_specialized(a, b, n)
                        }
                })
            });
            if let Some(i) = best {
                return Some(matches.swap_remove(i));
            }
            if inst {
                // [temp.explicit]/4, [temp.func.order]: clang's
                // `err_explicit_instantiation_ambiguous`, the candidates
                // first-declared first, each at the declaration its
                // specialization is instantiated from.
                let mut e = self.diag(DiagId::sema_explicit_inst_ambiguous, loc).arg(fname);
                for m in &matches {
                    let Some((_, info)) = self.ast.spec_info(m.spec).map(|(i, s)| (i, *s)) else {
                        continue;
                    };
                    let at = self.loc_of(self.ast.latest(m.template));
                    let note = self
                        .diag(DiagId::note_explicit_instantiation_candidate, at)
                        .arg(self.spec_name_noted(conv, m.spec, info.template, info.print_args))
                        .arg(self.template_args_desc(info.template, info.args));
                    e = e.with_note(note);
                }
                self.emit(e);
                return None;
            }
            // clang lists the candidates last-declared first, except
            // conversion function templates: its `LookupDirect` walks the
            // class's conversions in declaration order.
            let mut e = self.diag(DiagId::sema_fn_template_spec_ambiguous, loc).arg(fname);
            if !conv {
                matches.reverse();
            }
            for m in &matches {
                let Some((_, info)) = self.ast.spec_info(m.spec).map(|(i, s)| (i, *s)) else {
                    continue;
                };
                let note = self
                    .diag(DiagId::note_fn_template_spec_matched, self.loc_of(m.template))
                    .arg(self.spec_name_noted(conv, m.spec, info.template, info.print_args))
                    .arg(self.template_args_desc(info.template, info.args));
                e = e.with_note(note);
            }
            self.emit(e);
            return None;
        }
        matches.pop()
    }

    /// The exception specification of specialization `s`, instantiated if
    /// it is still pending ([temp.inst]/14: an explicit instantiation or
    /// specialization needs it), against the one in `ty`, its explicit
    /// instantiation's or specialization's type. None while either is
    /// still unknown (dependent) or they are compatible ([except.spec]/5:
    /// both non-throwing or both potentially-throwing, except that with
    /// `strict` no specification does not match `noexcept(false)`, as
    /// clang's explicit-instantiation check without
    /// `AllowNoexceptAllMatchWithNoSpec`); else the specialization's.
    fn spec_exc_conflict(
        &mut self,
        s: DeclId,
        ty: QualType,
        loc: Loc,
        strict: bool,
    ) -> Option<ExcSpec> {
        if !self.in_template() {
            self.resolve_exception_spec(s, loc);
        }
        let sty = self.declared_fn_type(s)?;
        let (pe, ne) = (self.written_exc_spec(sty), self.written_exc_spec(ty));
        let unknown = |e: ExcSpec| {
            matches!(
                e,
                ExcSpec::Deferred(_)
                    | ExcSpec::DependentNoexcept(_)
                    | ExcSpec::NoexceptExpr { value: NoexceptValue::Dependent(_), .. }
            )
        };
        let is_false =
            |e: ExcSpec| matches!(e, ExcSpec::NoexceptExpr { value: NoexceptValue::False, .. });
        let none_vs_false =
            strict && (pe == ExcSpec::None && is_false(ne) || is_false(pe) && ne == ExcSpec::None);
        let conflict = pe.is_nothrow() != ne.is_nothrow() || none_vs_false;
        (!unknown(pe) && !unknown(ne) && conflict).then_some(pe)
    }

    /// [except.spec]/5, [temp.explicit]/9 (clang `ActOnExplicitInstantiation`
    /// → `CheckEquivalentExceptionSpec`): an explicit instantiation of type
    /// `ty` that writes an exception specification must agree with the
    /// one of specialization `s`. On a mismatch, `exception specification
    /// in explicit instantiation does not match instantiated one` at
    /// `begin` (the decl-specifiers) with `explicit instantiation refers
    /// here` at the specialization's latest declaration, and false: the
    /// explicit instantiation has no effect.
    fn check_inst_exc_spec(&mut self, s: DeclId, ty: QualType, begin: Loc) -> bool {
        if self.written_exc_spec(ty) == ExcSpec::None
            || self.spec_exc_conflict(s, ty, begin, true).is_none()
        {
            return true;
        }
        let latest = self.ast.latest(s);
        let note = self.diag(DiagId::note_explicit_instantiation_here, self.loc_of(latest));
        let e =
            self.diag(DiagId::sema_explicit_inst_exception_spec_mismatch, begin).with_note(note);
        self.emit(e);
        false
    }

    /// [except.spec]/5, [temp.expl.spec]: an explicit specialization (or a
    /// friend naming a specialization) of type `ty` declared at `loc`
    /// redeclares specialization `s`, whose exception specification is the
    /// instantiated one (clang `MergeFunctionDecl` →
    /// `CheckEquivalentExceptionSpec`, the previous declaration being `s`,
    /// moved to `loc`). Written without one where `s` is non-throwing:
    /// `'f<int>' is missing exception specification 'noexcept'` (fix-it at
    /// `fix_at`); otherwise `exception specification in declaration does
    /// not match previous declaration`; each with `previous declaration is
    /// here` at `loc`, and false: the declaration is invalid.
    pub(crate) fn check_spec_exc_spec(
        &mut self,
        s: DeclId,
        ty: QualType,
        loc: Loc,
        fix_at: Option<Loc>,
    ) -> bool {
        let Some(pe) = self.spec_exc_conflict(s, ty, loc, false) else { return true };
        let mut e = if self.written_exc_spec(ty) == ExcSpec::None {
            // clang: the specialization unqualified with its arguments.
            let mut name = self.decl_name(s);
            if let Some(i) = self.ast.spec_info(s).map(|(_, i)| *i) {
                name = self.decl_name(i.template);
                let names = crate::names::sema_names(self);
                let canon = self.ast.types.canonical_args(i.print_args);
                kiln_ast::types::write_arg_list_scoped(&self.ast.types, &names, canon, &mut name);
            }
            let spelled = self.exc_spec_spelling(pe);
            let mut e =
                self.diag(DiagId::sema_missing_exception_spec, loc).arg(name).arg(spelled.clone());
            if let Some(at) = fix_at {
                e = e.fixit(kiln_diag::FixIt::insert(at, format!(" {spelled}")));
            }
            e
        } else {
            self.diag(DiagId::sema_exception_spec_mismatch, loc)
        };
        e = e.with_note(self.diag(DiagId::note_previous_declaration, loc));
        self.emit(e);
        false
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
        // [temp.expl.spec]/3: in a scope where the template may be
        // defined (clang's `CheckTemplateSpecializationScope`; WP-235).
        if !self.check_spec_scope(info.template, false, name_loc) {
            return None;
        }
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
            // clang: the specialization `NamedDecl`, unqualified with its
            // arguments (`f<int>`, also for a member of `N::X<short>`).
            let mut name = self.decl_name(info.template);
            let names = crate::names::sema_names(self);
            let canon = self.ast.types.canonical_args(info.print_args);
            kiln_ast::types::write_arg_list_scoped(&self.ast.types, &names, canon, &mut name);
            let e = self
                .diag(DiagId::sema_spec_after_instantiation, name_loc)
                .arg(name)
                .with_note(note);
            self.emit(e);
            return None;
        }
        if !self.check_spec_exc_spec(s, ty, name_loc, crate::except::missing_exc_spec_fixit_loc(d))
        {
            return None;
        }
        let fspec = self.explicit_spec_fnspec(ds, ty, s);
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
        Some(self.declare_explicit_spec(s, d, ty, fspec, name_loc))
    }

    /// `[extern] template R f<args>(params);` for specialization `m.spec`
    /// ([temp.explicit]; §4.5): an explicit instantiation definition
    /// instantiates the definition (now or at the end of the translation
    /// unit) and lists the specialization in the current context so that
    /// it is emitted; a declaration only records the kind.
    fn act_on_explicit_function_inst(&mut self, m: SpecMatch, declaration: bool, name_loc: Loc) {
        let s = m.spec;
        let Some((i, info)) = self.ast.spec_info(s).map(|(i, x)| (i, *x)) else { return };
        let name = self.spec_decl_name(info.template, info.print_args);
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
                let note = self.diag(
                    DiagId::note_previous_explicit_instantiation,
                    info.point_of_instantiation,
                );
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
            // Defined later: instantiated, and emitted, at the end of the
            // TU (clang `PerformPendingInstantiations`).
            self.queue_instantiation(s, name_loc);
            let first = self.ast.first(s);
            self.ast.templates.add_late_explicit_inst(first);
        }
    }

    /// The member function of an implicit class template specialization
    /// that `template<> R C<args>::f(params)` (or `template R
    /// C<args>::f(params);`) names: `None` when the qualifier is not such
    /// a specialization or names a member function template (then the
    /// declarator is a function template specialization), `Some(None)`
    /// after a diagnostic.
    #[allow(clippy::option_option, reason = "not a candidate / diagnosed / the member")]
    fn member_of_spec_candidate(
        &mut self,
        d: &Declarator,
        name: DeclName,
        ty: QualType,
        inst: bool,
    ) -> Option<Option<DeclId>> {
        let q = d.qualifier?;
        if self.kind_of(q).payload() != kiln_ast::decl::PayloadClass::Record {
            return None;
        }
        let in_spec =
            self.ast.spec_info(q).is_some_and(|(_, x)| x.kind != SpecKind::ExplicitSpecialization)
                || self.ast.member_spec_info(q).is_some();
        if !in_spec {
            return None;
        }
        let mut cands = self.prev_candidates_dn(name, Some(q));
        // An invalid out-of-line declaration is not found (clang does
        // not add it to its context).
        cands.retain(|&c| !self.ast.decls.get(c).has(DeclFlags::INVALID));
        // A non-template member that matches wins over the member
        // templates of the same name (clang's `ActOnExplicitInstantiation`
        // and `CheckMemberSpecialization` try the non-templates first;
        // libc++'s `extern template basic_string<char>&
        // basic_string<char>::replace(size_type, ...)` beside the
        // `replace` templates); explicit template arguments name a
        // template's specialization.
        let templates = cands.iter().any(|&c| self.kind_of(c) == DeclKind::FunctionTemplate);
        if templates && d.spec_id.is_some() {
            return None;
        }
        let mut exc = None;
        for &c in &cands {
            if self.kind_of(c).payload() != kiln_ast::decl::PayloadClass::Function
                || self.ast.member_spec_info(c).is_none()
            {
                continue;
            }
            let Some(pty) = self.declared_fn_type(c) else { continue };
            if self.ast.types.same_type(pty, ty) {
                return Some(Some(self.ast.first(c)));
            }
            if exc.is_none() && self.same_ignoring_exc_spec(pty, ty) {
                // [except.spec]/4 (C++11): an explicit instantiation need
                // not repeat the exception specification (libc++'s
                // extern template list omits `noexcept`; clang's
                // `adjustCCAndNoReturn(..., AdjustExceptionSpec)`).
                if inst && matches!(self.written_exc_spec(ty), ExcSpec::None) {
                    return Some(Some(self.ast.first(c)));
                }
                exc = Some(c);
            }
        }
        if templates {
            return None;
        }
        if let Some(p) = exc {
            // An implicit or explicitly defaulted member is rejected
            // first (clang `MergeFunctionDecl`).
            if !inst && let Some((id, sel)) = self.defaulted_member_definition(p) {
                self.emit(self.diag(id, d.name_loc()).arg(sel));
                return Some(None);
            }
            // [except.spec]/5.
            let e = self.diag(DiagId::sema_exception_spec_mismatch, d.name_loc());
            let e = self.note_prev(e, p, false);
            self.emit(e);
            return Some(None);
        }
        // `template<> X C<int>::x();` for a static data member `x`
        // declares a function ([dcl.ambig.res]/1), not the member's
        // value-initialized definition ([temp.expl.spec]/14; clang
        // `MergeFunctionDecl`).
        if !inst && let Some(o) = self.non_function_member(&cands) {
            let e = self
                .diag(DiagId::sema_redefinition_different_kind, d.name_loc())
                .arg(self.decl_name_str(name));
            let e = self.note_prev(e, o, true);
            self.emit(e);
            return Some(None);
        }
        if inst {
            let e = self
                .diag(DiagId::sema_explicit_inst_not_known, d.name_loc())
                .arg(self.decl_name_str(name));
            self.emit(e);
            return Some(None);
        }
        // [dcl.meaning.general]/2 (clang's `err_member_decl_does_not_match`).
        let id = if self.templ.fn_def_next {
            DiagId::sema_member_def_no_match
        } else {
            DiagId::sema_member_decl_no_match
        };
        let e = self
            .diag(id, d.name_loc())
            .arg(self.decl_name_str(name))
            .arg(self.class_context_str(q));
        let e = self.with_class_defined_note(e, q);
        self.emit(e);
        Some(None)
    }

    /// Where the definition of member `m` of a class template
    /// specialization was first required: its point of instantiation,
    /// else where its instantiation was queued.
    fn member_use_loc(&self, m: DeclId, poi: Loc) -> Loc {
        if poi != Loc::INVALID {
            return poi;
        }
        self.inst.pending.iter().find(|p| p.decl == m).map_or(Loc::INVALID, |p| p.poi)
    }

    /// The [`kiln_ast::decl::FnSpec`] of an explicit specialization
    /// declared with `ds` (function type `ty`), keeping `old`'s storage
    /// class (`static` is not repeated).
    fn explicit_spec_fnspec(
        &self,
        ds: &DeclSpec,
        ty: QualType,
        old: DeclId,
    ) -> kiln_ast::decl::FnSpec {
        let mut fspec = kiln_ast::decl::FnSpec::new();
        if ds.inline.is_some() {
            fspec = fspec.with_inline(true);
        }
        if ds.constexpr.is_some() {
            fspec = fspec.with_constexpr_kind(ConstexprKind::Constexpr).with_implicit_inline(true);
        } else if ds.consteval.is_some() {
            fspec = fspec.with_constexpr_kind(ConstexprKind::Consteval).with_implicit_inline(true);
        }
        let variadic = self.ast.types.proto(ty).is_some_and(|p| p.info().variadic());
        fspec = fspec.with_variadic(variadic);
        let old = self.ast.decls.function(old).map(|x| x.spec).unwrap_or_default();
        fspec.with_storage(old.storage()).with_explicit_object(old.has_explicit_object())
    }

    /// The new declaration of an explicit specialization `s` (of a
    /// function template, or a member function of a class template
    /// specialization) with its parameters.
    fn declare_explicit_spec(
        &mut self,
        s: DeclId,
        d: &Declarator,
        ty: QualType,
        fspec: kiln_ast::decl::FnSpec,
        name_loc: Loc,
    ) -> DeclId {
        let lexical = self.ctx;
        let f = self.builder().function_spec_redecl(s, lexical, name_loc, ty, fspec);
        let params: Vec<crate::declspec::ParamInfo> =
            d.function_params().map(<[crate::declspec::ParamInfo]>::to_vec).unwrap_or_default();
        let (defaults, _) = self.merge_default_args(None, false, &params, true);
        self.create_params(f, &params, &defaults);
        if self.kind_of(lexical) == DeclKind::CXXRecord {
            self.set_member_access(f);
        }
        f
    }

    /// `template<> R C<args>::f(params)` for member `m` of an implicit
    /// specialization ([temp.expl.spec]/1, §4.5): the member is no longer
    /// instantiated from the pattern.
    fn act_on_explicit_member_spec(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        d: &Declarator,
        m: DeclId,
        name_loc: Loc,
    ) -> Option<DeclId> {
        let (i, info) = self.ast.member_spec_info(m).map(|(i, x)| (i, *x))?;
        let used = self.ast.decls.get(m).has(DeclFlags::USED) || self.ast.definition(m).is_some();
        let after_inst = match info.kind {
            SpecKind::ImplicitInstantiation => used,
            SpecKind::ExplicitInstantiationDeclaration
            | SpecKind::ExplicitInstantiationDefinition => true,
            SpecKind::Undeclared | SpecKind::ExplicitSpecialization => false,
        };
        if after_inst {
            // [temp.expl.spec]/7.
            let explicit = u32::from(info.kind != SpecKind::ImplicitInstantiation);
            let at = self.member_use_loc(m, info.point_of_instantiation);
            let note = self.diag(DiagId::note_instantiation_required_here, at).arg(explicit);
            let e = self
                .diag(DiagId::sema_spec_after_instantiation, name_loc)
                .arg(self.decl_name_str(self.ast.decls.get(m).name()))
                .with_note(note);
            self.emit(e);
            return None;
        }
        // An implicit `operator==` ([class.compare.default]/4) or an
        // explicitly defaulted member is not redefined (clang
        // `MergeFunctionDecl`, ahead of the `constexpr` check).
        if let Some((id, sel)) = self.defaulted_member_definition(m) {
            self.emit(self.diag(id, name_loc).arg(sel));
            return None;
        }
        let fspec = self.explicit_spec_fnspec(ds, ty, m);
        // [dcl.constexpr]/1: the member's declarations agree on
        // `constexpr`/`consteval` (clang merges the specialization with
        // the instantiated member, `err_constexpr_redecl_mismatch`).
        let (ok, nk) = (self.fn_spec(m).constexpr_kind(), fspec.constexpr_kind());
        if ok != nk {
            let e = self
                .diag(DiagId::sema_constexpr_redecl_mismatch, name_loc)
                .arg(nk as u8)
                .arg(self.decl_name_str(self.ast.decls.get(m).name()))
                .arg(ok as u8);
            let e = self.note_prev(e, m, false);
            self.emit(e);
            return None;
        }
        self.ast.templates.update_member_spec(i, |x| {
            x.kind = SpecKind::ExplicitSpecialization;
            x.point_of_instantiation = Loc::INVALID;
        });
        if info.kind != SpecKind::ExplicitSpecialization {
            // The member keeps what its in-class declaration said about
            // virtuality ([class.virtual]; clang's `isVirtual` reads the
            // canonical declaration): dropping it would drop the member's
            // vtable slot (WP-164).
            let old = self.ast.decls.function(m).map(|x| x.spec).unwrap_or_default();
            let member = fspec
                .with_virtual(old.is_virtual())
                .with_pure(old.is_pure())
                .with_virtual_written(old.virtual_written())
                .with_override(old.is_override())
                .with_final(old.is_final());
            self.ast.decls.respecify_function(m, member);
        }
        Some(self.declare_explicit_spec(m, d, ty, fspec, name_loc))
    }

    /// `template<> T C<args>::s ...;` for static data member `m` of an
    /// implicit class template specialization ([temp.expl.spec]/1): the
    /// member's definition is no longer instantiated from the pattern
    /// (clang `CheckMemberSpecialization`). False after a diagnostic
    /// ([temp.expl.spec]/7).
    pub(crate) fn act_on_explicit_member_var_spec(&mut self, m: DeclId, name_loc: Loc) -> bool {
        let first = self.ast.first(m);
        let Some((i, info)) = self.ast.member_spec_info(first).map(|(i, x)| (i, *x)) else {
            return true;
        };
        let after_inst = match info.kind {
            SpecKind::ImplicitInstantiation => self.ast.decls.get(first).has(DeclFlags::USED),
            SpecKind::ExplicitInstantiationDeclaration
            | SpecKind::ExplicitInstantiationDefinition => true,
            SpecKind::Undeclared | SpecKind::ExplicitSpecialization => false,
        };
        if after_inst {
            let explicit = u32::from(info.kind != SpecKind::ImplicitInstantiation);
            let at = self.member_use_loc(first, info.point_of_instantiation);
            let note = self.diag(DiagId::note_instantiation_required_here, at).arg(explicit);
            let e = self
                .diag(DiagId::sema_spec_after_instantiation, name_loc)
                .arg(self.decl_name_str(self.ast.decls.get(first).name()))
                .with_note(note);
            self.emit(e);
            return false;
        }
        self.ast.templates.update_member_spec(i, |x| {
            x.kind = SpecKind::ExplicitSpecialization;
            x.point_of_instantiation = Loc::INVALID;
        });
        self.inst.member_defs.remove(&first);
        self.inst.deferred_inits.remove(&first);
        true
    }

    /// After `template<>`, an out-of-line redeclaration `p` of a
    /// constructor or destructor of an implicit class template
    /// specialization explicitly specializes it ([temp.expl.spec]/1,
    /// §4.5): it is no longer instantiated from the pattern (clang
    /// `CheckMemberSpecialization`). Static data members go through
    /// `act_on_explicit_member_var_spec`. The head is consumed: the
    /// declarations in the body that follows are not under it.
    pub(crate) fn mark_member_explicit_spec(&mut self, p: DeclId) {
        if self.templ.explicit_spec.is_none() || !self.at_explicit_head() {
            return;
        }
        self.templ.explicit_spec = None;
        let Some((i, info)) = self.ast.member_spec_info(self.ast.first(p)).map(|(i, x)| (i, *x))
        else {
            return;
        };
        if info.kind == SpecKind::ImplicitInstantiation {
            self.ast.templates.update_member_spec(i, |x| {
                x.kind = SpecKind::ExplicitSpecialization;
                x.point_of_instantiation = Loc::INVALID;
            });
        }
    }

    /// `[extern] template R C<args>::f(params);` for member `m` of a
    /// class template specialization ([temp.explicit]/3, §4.5).
    pub(crate) fn act_on_explicit_member_inst(
        &mut self,
        m: DeclId,
        declaration: bool,
        name_loc: Loc,
    ) {
        let Some((i, info)) = self.ast.member_spec_info(m).map(|(i, x)| (i, *x)) else { return };
        let name = self.qualified_decl_name(m);
        match (info.kind, declaration) {
            (
                SpecKind::ExplicitSpecialization | SpecKind::ExplicitInstantiationDeclaration,
                true,
            ) => return,
            (SpecKind::ExplicitSpecialization, false) => {
                let def = self.ast.latest(m);
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
                let note = self.diag(
                    DiagId::note_previous_explicit_instantiation,
                    info.point_of_instantiation,
                );
                let e = self
                    .diag(DiagId::sema_explicit_inst_duplicate, name_loc)
                    .arg(name)
                    .with_note(note);
                self.emit(e);
                return;
            }
            (SpecKind::ExplicitInstantiationDefinition, true) => {
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
            // [temp.explicit]/10: clang hands the definition to codegen
            // where the explicit instantiation appears.
            let lexical = self.ctx;
            if let Some(list) = self.ast.decls.member_list(lexical) {
                let after = list.last.unwrap_or(lexical);
                self.ast.templates.add_explicit_member_inst(after, m);
            }
            self.instantiate_function_definition(m, name_loc);
            if self.ast.definition(m).is_none() {
                self.queue_instantiation(m, name_loc);
                let first = self.ast.first(m);
                self.ast.templates.add_late_explicit_inst(first);
            }
        }
    }
}

impl Sema<'_, '_> {
    /// `base.template name<targs>` / `base->template name`
    /// ([temp.names]/5): on a type-dependent object a
    /// `CXXDependentScopeMemberExpr`; on any other object the member
    /// access, which must name a member template
    /// ([`Self::act_on_member_template_args`] takes `targs`). Also forms
    /// a dependent member access again at instantiation.
    #[allow(clippy::too_many_arguments, reason = "the parts of a member template-id")]
    pub fn act_on_template_member(
        &mut self,
        base: ExprId,
        name: DeclName,
        targs: Option<ArgListId>,
        template_kw: bool,
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> ExprId {
        self.act_on_template_member_q(base, None, name, targs, template_kw, loc, op_loc, arrow)
    }

    /// [`Sema::act_on_template_member`] with an optional
    /// nested-name-specifier before the name (`e.N::template f<T>`,
    /// WP-121).
    #[allow(clippy::too_many_arguments, reason = "the operands of a member access")]
    pub fn act_on_template_member_q(
        &mut self,
        base: ExprId,
        qual: Option<crate::using::Qualifier>,
        name: DeclName,
        targs: Option<ArgListId>,
        template_kw: bool,
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> ExprId {
        self.template_member_q(base, qual, name, targs, (template_kw, false), loc, op_loc, arrow)
    }

    /// [`Sema::act_on_template_member_q`] for a member name the parser
    /// read after `template` (`e.template f`, `e.N::template f<T>`):
    /// without a template argument list a function or variable template
    /// (or a dependent name) is ill-formed ([temp.names]/6, C++26; clang's
    /// `missing_template_arg_list_after_template_kw`, an error by
    /// default). Not re-diagnosed at instantiation.
    #[allow(clippy::too_many_arguments, reason = "the operands of a member access")]
    pub fn act_on_template_kw_member_q(
        &mut self,
        base: ExprId,
        qual: Option<crate::using::Qualifier>,
        name: DeclName,
        targs: Option<ArgListId>,
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> ExprId {
        let no_args = targs.is_none();
        self.template_member_q(base, qual, name, targs, (true, no_args), loc, op_loc, arrow)
    }

    /// `(template_kw, no_args)`: `no_args` diagnoses a missing template
    /// argument list ([`Sema::act_on_template_kw_member_q`]).
    #[allow(clippy::too_many_arguments, reason = "the operands of a member access")]
    fn template_member_q(
        &mut self,
        base: ExprId,
        qual: Option<crate::using::Qualifier>,
        name: DeclName,
        targs: Option<ArgListId>,
        (template_kw, no_args): (bool, bool),
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> ExprId {
        if let Some(e) =
            self.try_dependent_member_q(base, qual, name, targs, template_kw, loc, arrow)
        {
            if no_args {
                self.diag_missing_template_arg_list(loc);
            }
            if op_loc != loc {
                let _ = self.ast.templates.member_op_loc.insert(e, op_loc);
            }
            return e;
        }
        let e = self.act_on_member_q(base, qual, name, loc, op_loc, arrow);
        if self.is_err_expr(e) {
            return e;
        }
        if !self.is_member_template_ref(e) {
            if targs.is_none() && !template_kw {
                return e;
            }
            let mut d = self
                .diag(DiagId::sema_template_kw_refers_to_non_template, loc)
                .arg(self.decl_name_str(name));
            if self.ast.exprs.get(e).kind() == kiln_ast::expr::ExprKind::MemberExpr {
                let at = self.ast.decls.get(self.ast.exprs.decl(e)).loc();
                d = d.with_note(self.diag(DiagId::note_declared_non_template, at));
            }
            self.emit(d);
            return self.recover_err(&[e], loc);
        }
        if no_args {
            self.diag_missing_template_arg_list(loc);
        }
        match targs {
            Some(l) => self.act_on_member_template_args(e, l),
            None => self.act_on_member_without_template_args(e),
        }
    }

    /// clang's `missing_template_arg_list_after_template_kw` at the name
    /// after `template` ([temp.names]/6).
    pub fn diag_missing_template_arg_list(&mut self, loc: Loc) {
        let d = self.diag(DiagId::parse_missing_template_arg_list_after_template_kw, loc);
        self.emit(d);
    }

    /// [temp.names]/3.2: whether the dependent member access `e`
    /// (`this->f`, `x.f` on an object of type `X<P...>` inside `X`, an unqualified name that lookup in the current
    /// instantiation finds with more than one declaration, so the access
    /// stays dependent) names a member template, so a `<` after it starts
    /// a template argument list without `template` (libstdc++ variant's
    /// `this->emplace<__index>(...)`).
    pub fn dependent_member_names_template(&self, e: ExprId) -> bool {
        let n = self.ast.exprs.get(e);
        if n.kind() != kiln_ast::expr::ExprKind::CXXDependentScopeMemberExpr
            || n.aux() & 2 != 0
            || self.ast.templates.expr_targs.get(e).is_some()
            || self.ast.templates.expr_nns.get(e).is_some()
        {
            return false;
        }
        let Some(base) = ExprId::from_raw(n.a()) else { return false };
        let bt = self.ety(base);
        let obj = if n.aux() & 1 != 0 {
            if !self.ast.types.is_pointer(bt) {
                return false;
            }
            self.pointee_sugared(bt)
        } else {
            Some(bt)
        };
        // `X<P...>` written inside `X` names the current instantiation as
        // its injected-class-name does ([temp.dep.type]/1.2).
        let rec = obj.and_then(|o| {
            self.ast.types.as_record(o).or_else(|| self.friend_current_instantiation(o))
        });
        let Some(rec) = rec else { return false };
        let name = self.ast.dependent_member_name(e);
        self.lookup_in_class_dn(rec, name, crate::lookup::LookupKind::Ordinary).decls.iter().any(
            |&d| {
                matches!(
                    self.kind_of(d),
                    DeclKind::FunctionTemplate
                        | DeclKind::ClassTemplate
                        | DeclKind::VarTemplate
                        | DeclKind::TypeAliasTemplate
                )
            },
        )
    }

    /// The dependent member access `e` (see
    /// [`Self::dependent_member_names_template`]) with the template
    /// arguments `targs`: formed again, lookup and all, at instantiation.
    pub fn act_on_dependent_member_template_args(&mut self, e: ExprId, targs: ArgListId) -> ExprId {
        let n = self.ast.exprs.get(e);
        let (base, arrow, loc) = (ExprId::from_raw(n.a()), n.aux() & 1 != 0, n.loc());
        let name = self.ast.dependent_member_name(e);
        let op = self.ast.templates.member_op_loc.get(e).copied();
        let x = self.builder().dependent_member(base, arrow, name, false, None, Some(targs), loc);
        if let Some(op) = op {
            let _ = self.ast.templates.member_op_loc.insert(x, op);
        }
        x
    }

    /// The member template-id `e<targs>` of the overloaded member access
    /// `e` naming a member function template ([temp.names]/3,
    /// [temp.arg.explicit]): the arguments are recorded for the call.
    /// Dependent arguments make it a dependent member access
    /// (`CXXDependentScopeMemberExpr`, clang's type-dependent
    /// `UnresolvedMemberExpr`) formed again, lookup and all, at
    /// instantiation.
    pub fn act_on_member_template_args(&mut self, e: ExprId, targs: ArgListId) -> ExprId {
        let dependent = self
            .ast
            .types
            .arg_list_flags(targs)
            .intersects(kiln_ast::types::TypeFlags::DEPENDENT_INSTANTIATION);
        if !dependent {
            if let Some(x) = self.member_overload(e)
                && self
                    .ovl
                    .members
                    .get(&x)
                    .is_some_and(|m| self.kind_of(m.decls[0]) == DeclKind::VarTemplate)
            {
                return self.member_var_template_id(x, targs);
            }
            let _ = self.set_member_template_args(e, targs);
            return e;
        }
        let Some(x) = self.member_overload(e) else { return e };
        let Some(m) = self.ovl.members.remove(&x) else { return e };
        let loc = self.ast.exprs.get(x).loc();
        // `p->f<T>` through `operator->` ([over.ref]/1): the chain is
        // formed again from the class object at instantiation.
        let base = if m.arrow { self.arrow_chain_object(m.base) } else { m.base };
        // `this->Base::g<T>()` keeps its qualifier: at instantiation `g`
        // is looked up in `Base`, not in the object's class, which may
        // declare a non-template `g` of its own ([basic.lookup.qual]/1;
        // lld's `ObjFile<ELFT>::getObj`).
        self.builder().dependent_member(Some(base), m.arrow, m.name, false, m.nns, Some(targs), loc)
    }
}
