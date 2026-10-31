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
use kiln_ast::decl::DeclKind;
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
        let r = self.member_template_pattern(pattern);
        self.sema.act_on_finish_template_decl();
        self.sema.templ.cur = None;
        let d = r?;
        if let Some(x) = self.sema.template_of(d) {
            self.locals.insert(pattern, DeclOrPack::Decl(d));
            if !self.sema.associated_constraints(t).is_empty() {
                self.sema.inst.concepts.member_constraints.insert(x, (t, self.args.clone()));
            }
            return Ok(x);
        }
        Err(SubstFail)
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

    /// Substitutes template parameter list `list` of a member template:
    /// new parameters at their retained depth, mapped from the old ones
    /// in [`Substituter::locals`]; defaults substituted now.
    fn param_list(&mut self, list: TemplateParamListId) -> SubstResult<TemplateParamListId> {
        let l = *self.sema.ast.templates.param_list(list);
        let depth = self.args.retained_depth(l.depth);
        let old: Vec<DeclId> = self.sema.ast.lists.iter::<DeclId>(l.params).collect();
        let mut new = Vec::with_capacity(old.len());
        for p in old {
            let n = *self.sema.ast.decls.get(p);
            let Some(pi) = self.sema.ast.template_parm(p).copied() else { return Err(SubstFail) };
            let h = kiln_ast::DeclHead::new(self.sema.ctx, n.name(), n.loc());
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
                    let ty = self.ty(t, n.loc())?;
                    self.sema.builder().non_type_template_parm(h, ty, depth, pi.index, pi.pack)
                }
                DeclKind::TemplateTemplateParm => {
                    let Some(inner) = pi.params else { return Err(SubstFail) };
                    let inner = self.param_list(inner)?;
                    self.sema.builder().template_template_parm(
                        h,
                        inner,
                        depth,
                        pi.index,
                        pi.pack,
                        pi.typename_kw,
                    )
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
