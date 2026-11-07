//! Member templates of class template specializations ([temp.mem];
//! spec/sema-templates.md §4.6): when `A<int>` is instantiated, each
//! member template of `A`'s pattern is redeclared in it with its
//! template parameter list substituted (the outer levels replaced, its
//! own parameters one or more levels shallower), as clang's
//! `TemplateDeclInstantiator` does. Specializations of the redeclared
//! template are instantiated from the original pattern with the
//! enclosing specialization's arguments as the outer levels
//! ([`Sema::base_levels`]).

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclKind, FriendTarget};
use kiln_ast::ids::TemplateParamListId;

use crate::declspec::DeclaratorContext;
use crate::scope::{Ns, Scope, ScopeKind};
use crate::templ::HeadBuild;
use crate::templ::subst::{DeclOrPack, SubstFail, SubstResult, Substituter};

impl Substituter<'_, '_, '_> {
    /// Redeclares member template `t` of the pattern class in the
    /// specialization being instantiated; returns the new template decl.
    /// Its constraints stay the pattern's: they are recorded in
    /// `member_constraints` with this specialization's levels.
    pub(crate) fn member_template(&mut self, t: DeclId) -> SubstResult<DeclId> {
        let Some(info) = self.sema.ast.template_info(t).copied() else { return Err(SubstFail) };
        let Some(pattern) = info.pattern else { return Err(SubstFail) };
        self.open_substituted_head(info.params)?;
        let r = self.member_template_pattern(pattern);
        self.sema.act_on_finish_template_decl();
        self.sema.templ.cur = None;
        let d = r?;
        if let Some(x) = self.sema.template_of(d) {
            self.locals.insert(pattern, DeclOrPack::Decl(d));
            if !self.sema.associated_constraints(t).is_empty() {
                self.sema.inst.concepts.member_constraints.insert(x, (t, self.args.clone()));
            }
            self.out_of_line_member_partials(t);
            return Ok(x);
        }
        Err(SubstFail)
    }

    /// Redeclares explicit specialization `m` of a member variable
    /// template declared in the pattern class (`template<> constexpr bool
    /// v<int> = true;`, [temp.expl.spec]/3, CWG 727; clang's
    /// `VisitVarTemplateSpecializationDecl`): its arguments and type
    /// substituted, it specializes the member template the pattern's was
    /// redeclared as, and its initializer is instantiated with it.
    pub(crate) fn member_explicit_var_spec(&mut self, m: DeclId) -> SubstResult<DeclId> {
        let Some((_, info)) = self.sema.ast.spec_info(m).map(|(i, s)| (i, *s)) else {
            return Err(SubstFail);
        };
        let Some(pattern) = self.sema.pattern_of(info.template) else { return Err(SubstFail) };
        let Some(&DeclOrPack::Decl(new_pattern)) = self.locals.get(&pattern) else {
            return Err(SubstFail);
        };
        let Some(new_tmpl) = self.sema.template_of(new_pattern) else { return Err(SubstFail) };
        let loc = self.sema.ast.decls.get(m).loc();
        let t = self.decl_ty(m);
        let ty = self.named_decl_ty(m, t, loc)?;
        let args = self.args(info.args, loc)?;
        let args = self.sema.ast.types.args(args).to_vec();
        let mut ds = self.pattern_declspec(m, ty);
        // Static as a member, not as written.
        ds.storage = None;
        let mut dr = self.pattern_declarator(m, DeclaratorContext::Member)?;
        if let Some(x) = self.sema.ast.decls.var(m)
            && x.init.is_some()
        {
            dr.init_style = Some(x.spec.init_style());
        }
        let arg_locs = vec![loc; args.len()];
        dr.spec_id = Some(crate::templ::spec::SpecDeclaratorId {
            template: new_tmpl,
            args,
            arg_locs,
            loc,
            range: kiln_source::Span::new(loc, loc),
        });
        let Some(v) = self.sema.act_on_declarator(&ds, ty, &dr) else { return Err(SubstFail) };
        self.copy_begin_loc(m, v, ty);
        self.locals.insert(m, DeclOrPack::Decl(v));
        self.class_attrs(v, m);
        self.var_init(m, v)?;
        Ok(v)
    }

    /// Redeclares partial specialization `m` of a member class or
    /// variable template of the pattern class for the member template it was redeclared as
    /// ([temp.spec.partial.member]/2, [temp.mem]; clang's
    /// `InstantiateClassTemplatePartialSpecialization`): its parameter
    /// list and arguments substituted, so matching sees the enclosing
    /// specialization's arguments (`In<T, B>` in `Outer<int>` matches
    /// `In<int, char>`). A specialization instantiated from it uses the
    /// original partial's definition, recorded as its pattern, with the
    /// enclosing arguments as the outer levels (§4.6).
    pub(crate) fn member_partial_spec(&mut self, m: DeclId) -> SubstResult<()> {
        let Some((_, info)) = self.sema.ast.partial_spec_info(m) else { return Err(SubstFail) };
        let (tmpl, params, args) = (info.template, info.params, info.args);
        let n = *self.sema.ast.decls.get(m);
        let Some(pattern) = self.sema.pattern_of(tmpl) else { return Err(SubstFail) };
        let Some(&DeclOrPack::Decl(new_pattern)) = self.locals.get(&pattern) else {
            return Err(SubstFail);
        };
        let Some(new_tmpl) = self.sema.template_of(new_pattern) else { return Err(SubstFail) };
        self.open_substituted_head(params)?;
        let r = (|| {
            let list = self.sema.templ.heads.last().and_then(|h| h.list).ok_or(SubstFail)?;
            let args = self.args(args, n.loc())?;
            let ctx = self.sema.ast.decls.get(new_tmpl).ctx().unwrap_or(self.sema.ast.tu);
            let h = kiln_ast::DeclHead::new(ctx, n.name(), n.loc());
            let p = if let Some(v) = self.sema.ast.decls.var(m).map(|v| v.spec) {
                // A member variable template's partial specialization
                // (clang's `InstantiateVarTemplatePartialSpecialization`):
                // its type substituted too; the original stays the
                // pattern of the type and initializer of a specialization
                // instantiated from it.
                let ty = self.ty(n.ty().unwrap_or(kiln_ast::QualType::ERROR), n.loc())?;
                self.sema.builder().var_partial_spec(h, new_tmpl, list, args, ty, v)
            } else {
                let tag = self
                    .sema
                    .ast
                    .decls
                    .record(m)
                    .map_or(kiln_ast::decl::TagKind::Struct, |r| r.tag);
                self.sema.builder().class_partial_spec(h, new_tmpl, list, args, tag)
            };
            self.sema.ast.templates.set_instantiated_from(p, m);
            // Its constraints stay the original's, checked with this
            // specialization's levels outside its own (as for a member
            // template, `member_constraints`).
            let pl = *self.sema.ast.templates.param_list(params);
            let mut cs = Vec::new();
            self.sema.head_constraints(pl.params, pl.requires, &mut cs);
            if !cs.is_empty() {
                self.sema.inst.concepts.member_constraints.insert(p, (m, self.args.clone()));
            }
            Ok(())
        })();
        self.sema.act_on_finish_template_decl();
        self.sema.templ.cur = None;
        r
    }

    /// Redeclares the partial specializations of member class or variable
    /// template `t` declared out of line (`template<class T> template<class U>
    /// struct Outer<T>::Inner<U*> {...}`, [temp.spec.partial.member]/1):
    /// they are not in the pattern class's member list, so the member
    /// walk does not reach them (clang's `VisitClassTemplateDecl`
    /// instantiates every partial specialization of the member
    /// template). Those declared in the class come with the walk.
    fn out_of_line_member_partials(&mut self, t: DeclId) {
        if !matches!(self.sema.kind_of(t), DeclKind::ClassTemplate | DeclKind::VarTemplate) {
            return;
        }
        let class = self.sema.ast.decls.get(t).ctx();
        let partials: Vec<DeclId> = self
            .sema
            .ast
            .templates
            .partials_of(t)
            .iter()
            .filter_map(|&i| self.sema.ast.templates.partial(i))
            .map(|p| p.partial)
            .filter(|&p| self.sema.ast.decls.lexical_ctx(p) != class)
            .collect();
        for p in partials {
            // A partial specialization whose redeclaration fails is not
            // matched; the primary template is used, as for one that
            // fails in the member walk.
            let _ = self.member_partial_spec(p);
        }
    }

    /// Opens the head of a member template (or a generic lambda's call
    /// operator) being redeclared: template parameter list `params`
    /// substituted ([`Self::param_list`]), its parameters in scope, one
    /// more dependent level. The declaration takes the head.
    pub(crate) fn open_substituted_head(&mut self, params: TemplateParamListId) -> SubstResult<()> {
        let list = self.param_list(params)?;
        let l = *self.sema.ast.templates.param_list(list);
        let params: Vec<DeclId> = self.sema.ast.lists.iter::<DeclId>(l.params).collect();
        self.sema.templ.heads.push(HeadBuild {
            template_loc: l.template_loc,
            l_angle: l.l_angle,
            depth: l.depth,
            params: params.clone(),
            list: Some(list),
            taken: false,
            invented: false,
        });
        let mut s = Scope::new(ScopeKind::TemplateParams, None);
        for &p in &params {
            if let Some(n) = self.sema.ast.decls.get(p).name().as_ident() {
                s.locals.push((n, p, Ns::Ordinary));
            }
        }
        self.sema.scopes.push(s);
        self.sema.ast.templates.dependent_depth += 1;
        Ok(())
    }

    /// `template<class U> friend R f(U);` in a class template
    /// ([temp.friend]/1): its head is substituted like a member
    /// template's (to the retained depth), and the pattern redeclared as a
    /// friend, which matches (or hiddenly declares) the namespace scope
    /// template. Returns the `FriendDecl`.
    pub(crate) fn friend_function_template(
        &mut self,
        t: DeclId,
        loc: kiln_source::Loc,
    ) -> SubstResult<DeclId> {
        let Some(info) = self.sema.ast.template_info(t).copied() else { return Err(SubstFail) };
        let Some(pattern) = info.pattern else { return Err(SubstFail) };
        let list = self.param_list(info.params)?;
        let l = *self.sema.ast.templates.param_list(list);
        let params: Vec<DeclId> = self.sema.ast.lists.iter::<DeclId>(l.params).collect();
        self.sema.templ.heads.push(HeadBuild {
            template_loc: l.template_loc,
            l_angle: l.l_angle,
            depth: l.depth,
            params: params.clone(),
            list: Some(list),
            taken: false,
            invented: false,
        });
        let mut s = Scope::new(ScopeKind::TemplateParams, None);
        for &p in &params {
            if let Some(n) = self.sema.ast.decls.get(p).name().as_ident() {
                s.locals.push((n, p, Ns::Ordinary));
            }
        }
        self.sema.scopes.push(s);
        self.sema.ast.templates.dependent_depth += 1;
        let ol = *self.sema.ast.templates.param_list(info.params);
        let old: Vec<DeclId> = self.sema.ast.lists.iter::<DeclId>(ol.params).collect();
        let (cmp, apart) = self.friend_cmp_constraints(&old, ol.requires, pattern);
        let saved_cmp = std::mem::replace(&mut self.sema.friend_cmp_constraints, cmp);
        let saved_apart = std::mem::replace(&mut self.sema.friend_template_apart, apart);
        let saved = std::mem::replace(&mut self.sema.instantiating_friend_template, true);
        let r = self.function_decl(pattern, DeclaratorContext::Member, Some(loc));
        self.sema.instantiating_friend_template = saved;
        self.sema.friend_cmp_constraints = saved_cmp;
        self.sema.friend_template_apart = saved_apart;
        self.sema.act_on_finish_template_decl();
        self.sema.templ.cur = None;
        let fd = r?;
        if let FriendTarget::Decl(x) = self.sema.ast.decls.friend_target(fd)
            && let Some(p) = self.sema.pattern_of(x)
        {
            if self.sema.ast.instantiated_from(p).is_none() {
                self.member_link(p, pattern);
            }
            // Defined in the class: its specializations are instantiated
            // from the class template's pattern with this specialization's
            // arguments as the enclosing level, as a member template's
            // ([temp.friend]/1, [temp.inst]/4; clang's
            // `getTemplateInstantiationPattern` through
            // `getInstantiatedFromMemberTemplate`). A body still saved
            // for the end of the class (the specialization used in a
            // member's body) counts: it is there by the time a
            // specialization of the friend is instantiated.
            if (self.sema.ast.decls.function(pattern).is_some_and(|f| f.body.is_some())
                || self.sema.delayed_bodies.contains(&pattern))
                && self.sema.ast.first(x) == x
                && self.sema.ast.templates.instantiated_from_map(x).is_none()
            {
                self.sema.ast.templates.set_instantiated_from(x, t);
            }
            // A friend template constrained on the enclosing class's
            // parameters ([temp.friend]/9) keeps the pattern's constraints,
            // checked with this specialization's levels outside its own,
            // as a member template's (clang `getTemplateInstantiationArgs`
            // through the friend's lexical context; WP-231).
            if !self.sema.associated_constraints(t).is_empty()
                && !self.sema.inst.concepts.member_constraints.contains_key(&x)
            {
                self.sema.inst.concepts.member_constraints.insert(x, (t, self.args.clone()));
            }
        }
        Ok(fd)
    }

    /// The constraints of friend template pattern `pattern` (those of
    /// its template parameters `old`, its head's requires-clause `req`
    /// and its trailing requires-clause) mapped to their substitution,
    /// for redeclaration matching only (`Sema::friend_cmp_constraints`):
    /// the redeclared friend keeps the pattern's constraints, whose own
    /// parameters are one level deeper than the namespace scope
    /// template's (`template<class T1, three_way_comparable C> friend
    /// auto operator<=>(const stack<T1, C>&, ...)` in libc++ 22's
    /// `stack`). A constraint that substitutes to a non-dependent one
    /// names only the enclosing class's parameters ([temp.friend]/9):
    /// it is not mapped, so the friend stays apart. The flag: some
    /// constraint names an enclosing template's parameter (or the
    /// injected-class-name), so the friend declares its own template
    /// ([temp.friend]/9; `Sema::friend_template_apart`).
    fn friend_cmp_constraints(
        &mut self,
        old: &[DeclId],
        req: Option<kiln_ast::ExprId>,
        pattern: DeclId,
    ) -> (kiln_source::fx::FxHashMap<kiln_ast::ExprId, kiln_ast::ExprId>, bool) {
        let mut cs: Vec<kiln_ast::ExprId> = old
            .iter()
            .filter_map(|&p| self.sema.ast.template_parm(p).and_then(|i| i.constraint))
            .collect();
        cs.extend(req);
        cs.extend(self.sema.ast.templates.trailing_requires.get(pattern).copied());
        let mut m = kiln_source::fx::FxHashMap::default();
        let mut apart = false;
        let saved_used = self.outer_used.replace(false);
        for c in cs {
            let r = self.expr(c);
            apart |= self.outer_used.replace(false);
            if let Ok(e) = r
                && self
                    .sema
                    .ast
                    .exprs
                    .get(e)
                    .deps()
                    .intersects(kiln_ast::expr::ExprDeps::TYPE_VALUE_INST)
            {
                m.insert(c, e);
            }
        }
        self.outer_used.set(saved_used);
        (m, apart)
    }

    /// Declares the pattern of a member template (the head is open).
    fn member_template_pattern(&mut self, p: DeclId) -> SubstResult<DeclId> {
        let loc = self.sema.ast.decls.get(p).loc();
        match self.sema.kind_of(p) {
            DeclKind::CXXRecord => self.member_class_decl(p),
            DeclKind::TypeAlias | DeclKind::Typedef => self.typedef_decl(p),
            DeclKind::Var => self.member_var_template_pattern(p),
            k if k.payload() == kiln_ast::decl::PayloadClass::Function => {
                self.function_decl(p, DeclaratorContext::Member, None)
            }
            k => self.unimplemented(&format!("member templates of kind {k:?}"), loc),
        }
    }

    /// The pattern of a member variable template: declared without its
    /// initializer, which each specialization instantiates from the
    /// original pattern (as clang: `VarDecl v 'const int' static
    /// constexpr`).
    fn member_var_template_pattern(&mut self, p: DeclId) -> SubstResult<DeclId> {
        let loc = self.sema.ast.decls.get(p).loc();
        let t = self.decl_ty(p);
        let ty = self.ty(t, loc)?;
        let mut ds = self.pattern_declspec(p, ty);
        ds.inline = None;
        let dr = self.pattern_declarator(p, DeclaratorContext::Member)?;
        let saved = std::mem::replace(&mut self.sema.inst.deferring_init, true);
        let v = self.sema.act_on_declarator(&ds, ty, &dr);
        self.sema.inst.deferring_init = saved;
        v.ok_or(SubstFail)
    }

    /// The substituted type of a member template's non-type parameter
    /// declared with type `t`. A pack whose type is an expansion of an
    /// enclosing level's pack (`template<Ts... a>` in `O<int, char>`) is
    /// an expanded parameter pack ([temp.variadic]/5, [temp.param]/17;
    /// clang's `isExpandedParameterPack`): one element type per element
    /// of `Ts`, returned as a type-argument list; its declared type keeps
    /// the expansion with the known pack (§5.3).
    fn nttp_param_ty(
        &mut self,
        t: kiln_ast::QualType,
        pack: bool,
        loc: kiln_source::Loc,
    ) -> SubstResult<(kiln_ast::QualType, Option<kiln_ast::ids::ArgListId>)> {
        use crate::templ::pack::{Expansion, type_pattern};
        use kiln_ast::types::TemplateArg;
        let Some(pattern) = type_pattern(self.sema, t).filter(|_| pack) else {
            return Ok((self.ty(t, loc)?, None));
        };
        match self.expand_pattern(|s| s.ty(pattern, loc))? {
            Expansion::Unexpanded(p) => Ok((self.sema.ast.types.pack_expansion(p, None), None)),
            Expansion::Elements(v) => {
                let kept = self.kept_pack_pattern(|s| s.ty(pattern, loc))?;
                let ty = self.sema.ast.types.pack_expansion(kept, None);
                let v: Vec<TemplateArg> = v.into_iter().map(TemplateArg::ty).collect();
                Ok((ty, Some(self.sema.ast.types.arg_list(&v))))
            }
        }
    }

    /// Substitutes template parameter list `list` of a member template:
    /// new parameters at their retained depth, mapped from the old ones
    /// in [`Substituter::locals`]; defaults substituted now.
    pub(crate) fn param_list(
        &mut self,
        list: TemplateParamListId,
    ) -> SubstResult<TemplateParamListId> {
        let l = *self.sema.ast.templates.param_list(list);
        let depth = self.args.retained_depth(l.depth);
        let old: Vec<DeclId> = self.sema.ast.lists.iter::<DeclId>(l.params).collect();
        let mut new = Vec::with_capacity(old.len());
        for p in old {
            let n = *self.sema.ast.decls.get(p);
            let Some(pi) = self.sema.ast.template_parm(p).copied() else { return Err(SubstFail) };
            let mut h = kiln_ast::DeclHead::new(self.sema.ctx, n.name(), n.loc());
            // An invented parameter ([dcl.fct]/22, a generic lambda's
            // `auto`) stays implicit, so it still prints as `auto`.
            if n.flags().contains(kiln_ast::decl::DeclFlags::IMPLICIT) {
                h = h.flags(kiln_ast::decl::DeclFlags::IMPLICIT);
            }
            let np = match n.kind() {
                DeclKind::TemplateTypeParm => self.sema.builder().template_type_parm(
                    h,
                    depth,
                    pi.index,
                    pi.pack,
                    pi.typename_kw,
                ),
                DeclKind::NonTypeTemplateParm => {
                    let t = self.decl_ty(p);
                    let (ty, expanded) = self.nttp_param_ty(t, pi.pack, n.loc())?;
                    let np =
                        self.sema.builder().non_type_template_parm(h, ty, depth, pi.index, pi.pack);
                    if let Some(l) = expanded {
                        self.sema.templ.expanded_packs.insert(np, l);
                    }
                    np
                }
                DeclKind::TemplateTemplateParm => {
                    let Some(inner) = pi.params else { return Err(SubstFail) };
                    let inner = self.param_list(inner)?;
                    let np = self.sema.builder().template_template_parm(
                        h,
                        inner,
                        depth,
                        pi.index,
                        pi.pack,
                        pi.typename_kw,
                    );
                    self.sema.builder().set_ttp_kind(np, pi.ttp_kind);
                    np
                }
                _ => return Err(SubstFail),
            };
            // The constraints stay unsubstituted, as in clang: they are
            // checked with the specialization's levels through
            // `member_constraints`, and keep redeclarations that differ
            // only in them apart ([temp.over.link]/6).
            if let Some(c) = pi.constraint {
                self.sema.builder().set_parm_constraint(np, c);
            }
            self.locals.insert(p, DeclOrPack::Decl(np));
            if let Some(a) = self.sema.template_default(p) {
                // Substituted now (clang's `SubstDefaultArgument` when
                // the member template is redeclared).
                let a = self.template_arg(a, n.loc())?;
                let _ = self.sema.builder().set_template_default(np, a);
                if let Some(&dl) = self.sema.templ.default_locs.get(&p) {
                    self.sema.templ.default_locs.insert(np, dl);
                }
            }
            new.push(np);
        }
        Ok(self.sema.builder().template_param_list(
            l.template_loc,
            l.l_angle,
            l.r_angle,
            depth,
            &new,
            l.requires,
        ))
    }
}
