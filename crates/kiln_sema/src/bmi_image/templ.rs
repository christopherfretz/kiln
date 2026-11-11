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

use super::body::{Built, Root};
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
        if p.constraint.is_some() || t.ttp_owner.contains(d) {
            return Err("a constrained template parameter");
        }
        if p.params.is_some() {
            return Err("a template template parameter");
        }
        if p.depth != 0 {
            return Err("a member template or a template in a template");
        }
        // A default inherited from an earlier declaration belongs to a
        // redeclared template, which the image does not carry.
        if t.inherited_defaults.contains(d) {
            return Err("a default template argument");
        }
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

    /// A template record (S4): a function or class template at namespace
    /// scope.
    pub(super) fn template(&mut self, d: DeclId) -> W<Payload> {
        let s = self.s;
        let ast = &s.ast;
        let info = *ast.template_info(d).ok_or("a malformed template")?;
        let pattern = info.pattern.ok_or("a malformed template")?;
        let kind = match ast.decls.get(d).kind() {
            DeclKind::FunctionTemplate => 0,
            DeclKind::ClassTemplate => 1,
            DeclKind::TypeAliasTemplate => 2,
            _ => 3,
        };
        let redecl = |x: DeclId| ast.decls.redecl(x).is_some_and(|r| r.prev.is_some());
        if redecl(d) || redecl(pattern) {
            return Err("a redeclared template");
        }
        if info.constraint.is_some() || ast.templates.trailing_requires.contains(pattern) {
            return Err("a constrained template");
        }
        if s.templ.body_invalid.contains(&pattern) {
            return Err("a template made invalid by its body");
        }
        let pat =
            *self.recs.get(&pattern).ok_or("a template whose pattern is outside the image")?;
        let list = *ast.templates.param_list(info.params);
        if list.requires.is_some() {
            return Err("a constrained template");
        }
        if list.depth != 0 {
            return Err("a member template or a template in a template");
        }
        let mut params = Vec::new();
        for p in ast.lists.iter::<DeclId>(list.params) {
            params.push(*self.recs.get(&p).ok_or("a template parameter the image does not carry")?);
        }
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

    /// Whether `d` is the pattern of a template of kind `kind` (WP-1279:
    /// an alias template's alias declaration, a variable template's
    /// variable), carried as a dependent record before its template
    /// record.
    pub(super) fn pattern_of(&self, d: DeclId, kind: DeclKind) -> bool {
        let ast = &self.s.ast;
        matches!(ast.templ_link(d).get(), Some((TemplLinkKind::Pattern, _)))
            && ast.described_template(d).is_some_and(|t| ast.decls.get(t).kind() == kind)
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
}
