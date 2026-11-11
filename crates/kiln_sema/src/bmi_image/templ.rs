//! Template parameters and templates in the declaration image
//! (spec/modules.md §11.11, "Templates as patterns"; the third slice,
//! WP-1279): parameter packs, default arguments of non-type parameters,
//! pack expansions in patterns.
//!
//! The writer refuses what it cannot rebuild exactly (the importer then
//! replays the source): a pattern that instantiates differently from
//! replay is a miscompile, so every form here is carried whole or not at
//! all.

use kiln_ast::decl::{DeclKind, TemplLinkKind};
use kiln_ast::types::{ArgView, TemplateArg};
use kiln_ast::{DeclId, ExprId};

use kiln_ast::templates::{TemplateInfo, TemplateParamList};

use super::body::{Built, Root};
use super::constraints::CONCEPT_KIND;
use super::{NONE, Payload, W, Writer};
use crate::Sema;

impl Writer<'_, '_, '_> {
    /// A template parameter record (S4): a type or non-type parameter of
    /// a namespace-scope template, unconstrained; a pack, a type
    /// parameter's default type or a non-type parameter's default
    /// expression (WP-1279) are carried.
    pub(super) fn template_parm(&mut self, d: DeclId) -> W<Payload> {
        let ast = &self.s.ast;
        let t = &ast.templates;
        let p = *ast.template_parm(d).ok_or("a malformed template parameter")?;
        // A constraint is carried by `constraint_roots` (WP-1284).
        if p.params.is_some() {
            return Err("a template template parameter");
        }
        // A default inherited from an earlier declaration is rebuilt by
        // the importer from the redeclared template's ([`Sema::image_inherited_defaults`]).
        let n = ast.decls.get(d);
        let default = t.defaults.get(d).map(TemplateArg::view);
        if n.kind() == DeclKind::NonTypeTemplateParm {
            let (expr, profile) = match default {
                None => (None, false),
                Some(ArgView::Expr { expr, profile }) => (Some(expr), profile.is_some()),
                Some(_) => return Err("a default template argument of a non-type parameter"),
            };
            if let Some(e) = expr {
                self.bs.roots.push(Root::TemplDefault(self.img.decls.len() as u32, e));
            }
            let ty = self.qref(n.ty().ok_or("a malformed template parameter")?)?;
            return Ok(Payload::Nttp {
                ty,
                depth: p.depth,
                index: p.index,
                pack: p.pack,
                default: NONE,
                profile,
            });
        }
        let default = match default {
            None => None,
            Some(ArgView::Type(q)) => Some(self.qref(q)?),
            Some(_) => return Err("a default template argument"),
        };
        let begin = match ast.decls.cold.begin_loc.get(d) {
            Some(&l) => self.loc(l)?,
            None => NONE,
        };
        Ok(Payload::Ttp {
            depth: p.depth,
            index: p.index,
            typename_kw: p.typename_kw,
            pack: p.pack,
            begin,
            default,
        })
    }

    /// A template record (S4): a function, class, alias or variable
    /// template, a member template or a template in a template (WP-1292);
    /// a concept (WP-1284), with no pattern. Constraints are carried by
    /// `constraint_roots`, a redeclaration by `template_prev`.
    pub(super) fn template(&mut self, d: DeclId) -> W<Payload> {
        let s = self.s;
        let ast = &s.ast;
        let info = *ast.template_info(d).ok_or("a malformed template")?;
        if ast.decls.get(d).kind() == DeclKind::Concept {
            return self.concept(d, &info);
        }
        let pattern = info.pattern.ok_or("a malformed template")?;
        let kind = match ast.decls.get(d).kind() {
            DeclKind::FunctionTemplate => 0,
            DeclKind::ClassTemplate => 1,
            DeclKind::TypeAliasTemplate => 2,
            _ => 3,
        };
        if s.templ.body_invalid.contains(&pattern) {
            return Err("a template made invalid by its body");
        }
        let pat =
            *self.recs.get(&pattern).ok_or("a template whose pattern is outside the image")?;
        let list = *ast.templates.param_list(info.params);
        let params = self.template_params(&list)?;
        if kind != 0 && pat + 1 != self.img.decls.len() as u32 {
            return Err("a class template whose pattern is not just before it");
        }
        let using = match s.templ.alias_using_loc.get(&pattern) {
            Some(&l) if kind == 2 => self.loc(l)?,
            _ => NONE,
        };
        Ok(Payload::Template {
            kind,
            pattern: pat,
            tloc: self.loc(list.template_loc)?,
            using,
            langle: self.loc(list.l_angle)?,
            rangle: self.loc(list.r_angle)?,
            depth: list.depth,
            params,
        })
    }

    /// The previous declaration of template `d` (WP-1292): its pattern's
    /// previous declaration's template, which `finish_template_pattern`
    /// passes the new template as the declaration it replaces in lookup
    /// (template declarations carry no redeclaration link of their own).
    pub(super) fn template_prev(&self, d: DeclId) -> W<u32> {
        let ast = &self.s.ast;
        let pattern = ast.template_info(d).and_then(|i| i.pattern).ok_or("a malformed template")?;
        let Some(p) = ast.prev(pattern) else { return Ok(NONE) };
        let t = ast.described_template(p).ok_or("a redeclared template")?;
        self.recs.get(&t).copied().ok_or("a redeclaration of an entity outside the image")
    }

    /// The parameter records of template head `list`.
    fn template_params(&self, list: &TemplateParamList) -> W<Vec<u32>> {
        let mut params = Vec::new();
        for p in self.s.ast.lists.iter::<DeclId>(list.params) {
            params.push(*self.recs.get(&p).ok_or("a template parameter the image does not carry")?);
        }
        Ok(params)
    }

    /// A concept's record (WP-1284): a `Template` record of kind
    /// [`CONCEPT_KIND`] without a pattern; its constraint-expression is
    /// a constraint record.
    fn concept(&mut self, d: DeclId, info: &TemplateInfo) -> W<Payload> {
        let ast = &self.s.ast;
        if ast.decls.redecl(d).is_some_and(|r| r.prev.is_some()) {
            return Err("a redeclared template");
        }
        let list = *ast.templates.param_list(info.params);
        let params = self.template_params(&list)?;
        Ok(Payload::Template {
            kind: CONCEPT_KIND,
            pattern: NONE,
            tloc: self.loc(list.template_loc)?,
            using: NONE,
            langle: self.loc(list.l_angle)?,
            rangle: self.loc(list.r_angle)?,
            depth: list.depth,
            params,
        })
    }

    /// Whether `d` is the pattern of a template of kind `kind` (WP-1279:
    /// an alias template's alias declaration, a variable template's
    /// variable), carried as a dependent record before its template
    /// record.
    pub(super) fn pattern_of(&self, d: DeclId, kind: DeclKind) -> bool {
        let ast = &self.s.ast;
        matches!(ast.templ_link(d).get(), Some((TemplLinkKind::Pattern, _)))
            && ast.described_template(d).is_some_and(|t| ast.decls.get(t).kind() == kind)
    }

    /// Why declaration `d`, in a dependent context and neither a pattern
    /// nor a member of a carried class pattern, is refused (WP-1279
    /// census triage): a partial specialization; a member (or an
    /// out-of-line member definition) of a class the image does not
    /// carry, which is a cascade; or another declaration in a template.
    pub(super) fn in_template_reason(&self, d: DeclId) -> &'static str {
        let ast = &self.s.ast;
        if matches!(ast.templ_link(d).get(), Some((TemplLinkKind::PartialSpecialization, _))) {
            return "an explicit or partial specialization";
        }
        let ctx = ast.decls.get(d).ctx();
        if ctx.is_some_and(|c| ast.decls.record(c).is_some() && !self.recs.contains_key(&c)) {
            return "a member of a class outside the image";
        }
        "a declaration in a template"
    }

    /// Whether unresolved name `e` is carried (WP-1279): what it found
    /// at the definition is namespace-scope functions and function or
    /// variable templates, with nothing sema keeps beside it (a
    /// using-shadow or naming class, an implicit member reference).
    pub(super) fn lookup_ok(&self, e: ExprId) -> W<()> {
        let s = self.s;
        let ast = &s.ast;
        if s.ovl.ule_found.contains_key(&e) || s.member_refs.contains_key(&e) {
            return Err("a template expression");
        }
        for d in ast.unresolved_decls(e) {
            let n = ast.decls.get(d);
            let member = n.ctx().is_some_and(|c| ast.decls.record(c).is_some());
            if member
                || !matches!(
                    n.kind(),
                    DeclKind::Function | DeclKind::FunctionTemplate | DeclKind::VarTemplate
                )
            {
                return Err("a template expression");
            }
        }
        Ok(())
    }

    /// Patches non-type parameter record `rec`'s default argument to
    /// expression record `i` ([`Writer::walk_roots`]).
    pub(super) fn set_nttp_default(&mut self, rec: u32, i: u32) {
        if let Payload::Nttp { default, .. } = &mut self.img.decls[rec as usize].payload {
            *default = i;
        }
    }
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Gives each non-type parameter its default argument once the
    /// bodies are built: the expression, with its canonical profile when
    /// the writer's argument had one (as `act_on_template_default`
    /// stores it).
    pub(super) fn image_nttp_defaults(&mut self, defaults: &[(DeclId, u32, bool)], built: &Built) {
        for &(p, e, profile) in defaults {
            let e: ExprId = built.expr(e);
            let profile = profile.then(|| self.ast.profile_expr(e));
            let _ = self.builder().set_template_default(p, TemplateArg::expr(e, profile));
        }
    }

    /// Rebuilds the default arguments redeclared templates `ts` (in
    /// declaration order) inherit, once every default is set, as
    /// `finish_template_pattern` records them ([temp.param]/12).
    pub(super) fn image_inherited_defaults(&mut self, ts: &[DeclId]) {
        for &t in ts {
            let Some(info) = self.ast.template_info(t).copied() else { continue };
            let Some(prev) = info
                .pattern
                .and_then(|p| self.ast.prev(p))
                .and_then(|p| self.ast.described_template(p))
            else {
                continue;
            };
            self.merge_template_defaults(t);
            // A friend class template declared in a class template is
            // compared with neither side (`finish_template_pattern`'s
            // `dependent_friend`): its head is deeper (WP-1307).
            let dependent_friend = self.ast.decls.get(t).kind() == DeclKind::ClassTemplate
                && self.ast.decls.get(t).misc().friend() != kiln_ast::decl::FriendKind::None
                && self.ast.templates.param_list(info.params).depth > 0;
            if dependent_friend {
                continue;
            }
            let old = self
                .head_comparable_decl(prev)
                .and_then(|p| self.ast.template_info(p))
                .map(|i| i.params);
            if let Some(old) = old {
                self.record_inherited_defaults(info.params, old);
            }
        }
    }
}
