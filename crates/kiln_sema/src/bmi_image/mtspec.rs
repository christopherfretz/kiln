//! Specializations of member function templates of class specializations
//! in the declaration image (WP-1358, spec/modules.md §11.11, "Forms
//! carried by WP-1358").
//!
//! `chrono::hours(__h)` names `duration<long, ratio<3600>>::duration<long>`,
//! a specialization of the constructor template the class specialization
//! declares, instantiated from the class template's member template
//! ([temp.mem]/1, [temp.inst]/4). Sema makes its template a member of the
//! specialization (the instantiated member template, `instantiated_from`
//! the pattern's) and the specialization's arguments that template's own.
//! The image names the member template by a member reference (S4b) of
//! the complete class specialization, its pattern the member template's
//! record, and the specialization by a specialization reference whose
//! template is that member reference; the importer finds the member
//! template in its specialization and makes the specialization with the
//! same arguments, as the replay's call did.

use kiln_ast::DeclId;
use kiln_ast::decl::DeclKind;
use kiln_ast::templates::{SpecInfo, SpecKind};

use super::{Image, Payload, W, Writer};

/// The census reason of a specialization of a constrained member
/// function template of a class specialization.
pub(super) const CONSTRAINED: &str =
    "a specialization of a constrained member template of a class specialization";

/// The parameters of the member function template a member reference
/// `r` names, when it names one.
pub(super) fn params(img: &Image, r: u32) -> Option<&[u32]> {
    let Payload::Member { pattern, .. } = img.decls.get(r as usize)?.payload else { return None };
    match &img.decls.get(pattern as usize)?.payload {
        Payload::Template { kind: 0, params, .. } => Some(params),
        _ => None,
    }
}

/// The pattern function record of the member function template member
/// reference `r` names.
pub(super) fn pattern_fn(img: &Image, r: u32) -> Option<u32> {
    let Payload::Member { pattern, .. } = img.decls.get(r as usize)?.payload else { return None };
    match img.decls.get(pattern as usize)?.payload {
        Payload::Template { kind: 0, pattern, .. } => Some(pattern),
        _ => None,
    }
}

impl Writer<'_, '_, '_> {
    /// Writes the specialization reference of `d`, an implicit
    /// specialization of a member function template of a class
    /// specialization, where the walk meets it: after its class's
    /// members, in creation order, so the importer makes the
    /// specializations of each member template in replay's order (a
    /// losing overload candidate's included, as `var_spec_at` does at
    /// namespace scope). `false` when `d` is no such specialization, or
    /// one the image cannot name (left to its users).
    pub(super) fn mt_spec_at(&mut self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        if ast.decls.function(d).is_none() || ast.first(d) != d || self.recs.contains_key(&d) {
            return false;
        }
        let Some((_, si)) = ast.spec_info(d) else { return false };
        if !matches!(si.kind, SpecKind::ImplicitInstantiation | SpecKind::Undeclared)
            || ast.decls.get(si.template).kind() != DeclKind::FunctionTemplate
        {
            return false;
        }
        let Some(class) = ast.decls.get(si.template).ctx().map(|c| ast.first(c)) else {
            return false;
        };
        // A class specialization first named in a body (`dur<long>(h)`)
        // is written here, before its member template's specializations,
        // as replay made it before them.
        let recorded = self.recs.contains_key(&class);
        let named = ast.decls.get(class).kind() == DeclKind::CXXRecord
            && ast.spec_info(class).is_some()
            && !self.census_dead(class);
        if !recorded && !named || self.census_dead(d) {
            return false;
        }
        let before = self.marks();
        let class_rec = if recorded { Ok(None) } else { self.spec_rec(class) };
        if class_rec.and_then(|_| self.spec_rec(d)).is_err() {
            self.rollback(before);
            return false;
        }
        true
    }

    /// The member reference record of `t` when it is a member function
    /// template of a class specialization instantiated from the class
    /// template's (the template a substituted parameter type names).
    pub(super) fn subst_member_template(&mut self, t: DeclId) -> W<Option<u32>> {
        let ast = &self.s.ast;
        if ast.decls.get(t).kind() != DeclKind::FunctionTemplate
            || ast.templates.instantiated_from_map(t).is_none()
            || ast.decls.get(t).ctx().is_none_or(|c| ast.spec_info(ast.first(c)).is_none())
        {
            return Ok(None);
        }
        let Some(r) = self.member_rec(t)? else { return Ok(None) };
        Ok(params(&self.img, r).is_some().then_some(r))
    }

    /// The member reference record of the template of function
    /// specialization `si` when that template is a member template of a
    /// class specialization instantiated from the class template's member
    /// template, and the specialization from that template's pattern;
    /// `None` for any other specialization.
    pub(super) fn member_template_of(&mut self, si: &SpecInfo) -> W<Option<u32>> {
        let ast = &self.s.ast;
        let t = si.template;
        if ast.decls.get(t).kind() != DeclKind::FunctionTemplate || si.from_args.is_some() {
            return Ok(None);
        }
        let Some(class) = ast.decls.get(t).ctx() else { return Ok(None) };
        if ast.decls.get(class).kind() != DeclKind::CXXRecord
            || ast.spec_info(ast.first(class)).is_none()
        {
            return Ok(None);
        }
        let Some(pt) = ast.templates.instantiated_from_map(t) else { return Ok(None) };
        if ast.described_template(si.from).map(|x| ast.first(x)) != Some(ast.first(pt)) {
            return Ok(None);
        }
        // Its mangled name has the instantiated head's constraints
        // (`Tk`, `Q`), which the importer's completion leaves out.
        if self.head_constrained(pt) {
            return Err(CONSTRAINED);
        }
        let Some(r) = self.member_rec(t)? else { return Ok(None) };
        Ok(params(&self.img, r).is_some().then_some(r))
    }
}
