//! Variable template specializations in the declaration image (WP-1352,
//! spec/modules.md §11.11 "Forms carried by WP-1352").
//!
//! An implicit specialization of a carried variable template (`e_v<double>`
//! in `inline constexpr double e = e_v<double>;`) is a specialization
//! reference record, as a function's or a class's: template kind 3 and
//! the converted arguments, never complete. The importer declares it
//! while it reads the records (its type substituted, as a use in source
//! does); the initializer comes with the bodies, so it is instantiated
//! afterwards, where `image_mark_specs` notes the reference the unit
//! made (a constant one at once, as replay's use did).
//!
//! Implicit function template specializations are written in declaration
//! order too, and so is an explicit instantiation declaration of one
//! (`extern template ostream& __ostream_insert(...)`): a reference record
//! listed in the `fexpl` section, whose kind the importer sets once it has
//! made the specialization, as `act_on_explicit_function_inst` does.

use kiln_ast::decl::DeclKind;
use kiln_ast::decl::TemplLinkKind;
use kiln_ast::templates::{MemberSpecInfo, SpecInfo, SpecKind};
use kiln_ast::types::TemplateArg;
use kiln_ast::{DeclId, QualType};
use kiln_source::Loc;

use super::{Image, NONE, Payload, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;

/// The refusal of a variable template specialization the image cannot
/// name.
pub(super) const VAR_SPEC: &str = "a variable template specialization";

/// The refusal of an explicit instantiation definition of a function
/// template specialization (its definition is instantiated, and listed,
/// at the directive).
const FN_EXPL_DEF: &str = "a template specialization: an explicit instantiation definition";

/// The specialization reference records that are explicit instantiation
/// declarations, ascending.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct FnExplDecls {
    pub(super) recs: Vec<u32>,
}

impl FnExplDecls {
    pub(super) fn truncate(&mut self, n: u32) {
        self.recs.retain(|&r| r < n);
    }

    fn contains(&self, r: u32) -> bool {
        self.recs.binary_search(&r).is_ok()
    }
}

pub(super) fn encode_fn_expl_decls(w: &mut Wr, s: &FnExplDecls) {
    w.len(s.recs.len());
    for &r in &s.recs {
        w.u32(r);
    }
}

pub(super) fn decode_fn_expl_decls(r: &mut Rd) -> Res<FnExplDecls> {
    let n = r.count(4)?;
    let mut recs = Vec::with_capacity(n);
    for _ in 0..n {
        recs.push(r.u32()?);
    }
    Ok(FnExplDecls { recs })
}

/// Each listed record is a specialization reference of a function
/// template, listed once, in ascending order.
pub(super) fn validate_fn_expl_decls(img: &Image) -> Res<()> {
    let mut last = None;
    for &r in &img.fexpl.recs {
        let ok = last.is_none_or(|l| l < r)
            && img.decls.get(r as usize).is_some_and(|d| match d.payload {
                // A member function's (WP-1352): the record has the
                // directive's location.
                Payload::Member { pattern, .. } => {
                    d.loc != NONE
                        && matches!(
                            img.decls.get(pattern as usize).map(|t| &t.payload),
                            Some(Payload::Function { .. })
                        )
                }
                Payload::Spec { template, complete: false, .. } => {
                    template != NONE
                        && matches!(
                            img.decls.get(template as usize).map(|t| &t.payload),
                            Some(Payload::Template { kind: 0, .. })
                        )
                }
                _ => false,
            });
        if !ok {
            return bad("bad explicit instantiation declaration");
        }
        last = Some(r);
    }
    Ok(())
}

impl Writer<'_, '_, '_> {
    /// Writes the reference record of implicit variable or function
    /// template specialization `d` where the unit made it, in declaration
    /// order: the importer then makes it where replay does, its place in
    /// its namespace's member list (after the declaration whose
    /// initializer first named it, before the next) or its template's
    /// specialization list, a losing overload candidate's included.
    /// `false` when `d` is no such specialization of a carried template
    /// at namespace scope.
    pub(super) fn var_spec_at(&mut self, d: DeclId) -> W<bool> {
        let ast = &self.s.ast;
        let func = ast.decls.function(d).is_some();
        if (ast.decls.var(d).is_none() && !func) || ast.first(d) != d {
            return Ok(false);
        }
        // WP-1352: an explicit specialization's first declaration has no
        // record of its own ([`super::fnspec`]).
        if self.fn_spec_first(d) {
            // Counted as the unit's declaration, carried or not with its
            // written one.
            let root = self.census_root(d);
            if let Some(c) = self.census.as_mut() {
                let _ = c.seen.insert(root);
            }
            return Ok(true);
        }
        let Some((_, si)) = ast.spec_info(d) else { return Ok(false) };
        let explicit = func
            && ast.template_info(si.from).is_none()
            && ast.described_template(si.from).map(|t| ast.first(t))
                == Some(ast.first(si.template))
            && matches!(
                si.kind,
                SpecKind::ExplicitInstantiationDeclaration
                    | SpecKind::ExplicitInstantiationDefinition
            );
        if !(explicit || matches!(si.kind, SpecKind::ImplicitInstantiation | SpecKind::Undeclared))
            || !self.recs.contains_key(&ast.first(si.template))
            || !ast.decls.get(d).ctx().is_some_and(|c| {
                matches!(
                    ast.decls.get(c).kind(),
                    DeclKind::TranslationUnit
                        | DeclKind::Namespace
                        | DeclKind::Export
                        | DeclKind::LinkageSpec
                )
            })
        {
            return Ok(false);
        }
        if self.census_dead(d) {
            return Ok(explicit);
        }
        let before = self.marks();
        match self.spec_rec(d) {
            Ok(_) => {
                // An explicit instantiation counts as the unit's own
                // declaration.
                if explicit {
                    let root = self.census_root(d);
                    if let Some(c) = self.census.as_mut() {
                        let _ = c.seen.insert(root);
                    }
                }
                Ok(true)
            }
            // One the image cannot name is left to its users, as before
            // WP-1352: a carried body naming it is refused with the
            // reason.
            Err(_) if !explicit => {
                self.rollback(before);
                Ok(false)
            }
            // An explicit instantiation is a declaration of the unit.
            Err(why) if self.census.is_some() => {
                self.rollback(before);
                self.census_fail(d, why, true);
                Ok(true)
            }
            Err(why) => Err(why),
        }
    }

    /// The directive's location of member `d` (`m`) of an implicit class
    /// template specialization when it is a member function the unit
    /// explicitly instantiated alone with a declaration (WP-1352); `None`
    /// when it is no explicit instantiation; a definition is refused.
    pub(super) fn member_inst_loc(&self, d: DeclId, m: &MemberSpecInfo) -> W<Option<u32>> {
        if self.s.ast.decls.function(d).is_none() {
            return Ok(None);
        }
        match m.kind {
            SpecKind::ExplicitInstantiationDeclaration => {
                Ok(Some(self.loc(m.point_of_instantiation)?))
            }
            SpecKind::ExplicitInstantiationDefinition => Err(FN_EXPL_DEF),
            _ => Ok(None),
        }
    }

    /// Writes the member reference record of member function `d` of an
    /// implicit specialization of a carried class template when the unit
    /// explicitly instantiated it alone, used or not: the directive has
    /// no declaration of its own. `false` when `d` is no such member.
    pub(super) fn member_inst_at(&mut self, d: DeclId) -> W<bool> {
        let ast = &self.s.ast;
        let Some((TemplLinkKind::MemberOfSpecialization, i)) = ast.templ_link(d).get() else {
            return Ok(false);
        };
        let Some(m) = ast.templates.member_spec(i) else { return Ok(false) };
        let class = ast.decls.get(d).ctx();
        let carried = class.and_then(|c| ast.spec_info(ast.first(c))).is_some_and(|(_, s)| {
            s.kind != m.kind && self.recs.contains_key(&ast.first(s.template))
        });
        if !carried
            || ast.decls.function(d).is_none()
            || !matches!(
                m.kind,
                SpecKind::ExplicitInstantiationDeclaration
                    | SpecKind::ExplicitInstantiationDefinition
            )
            || self.recs.contains_key(&d)
        {
            return Ok(false);
        }
        let before = self.marks();
        match self.member_rec(d) {
            Ok(_) => Ok(true),
            Err(why) if self.census.is_some() => {
                self.rollback(before);
                self.census_fail(d, why, true);
                Ok(true)
            }
            Err(why) => Err(why),
        }
    }

    /// Whether function template specialization `d` (`si`), an explicit
    /// instantiation, is carried: a declaration is; a definition is
    /// refused.
    pub(super) fn fn_explicit_inst(&self, d: DeclId, si: &SpecInfo) -> W<bool> {
        if self.s.ast.decls.function(d).is_none() {
            return Ok(false);
        }
        if si.kind == SpecKind::ExplicitInstantiationDefinition {
            return Err(FN_EXPL_DEF);
        }
        Ok(true)
    }

    /// The replacement of substituted template parameter type `t` whose
    /// template the image does not carry (a variable or function template
    /// specialization's, an uncarried alias's), when unqualified: the
    /// type is written as its replacement, which is how it prints and
    /// what it is ([temp.alias]/2, [temp.inst]); `None` otherwise.
    pub(super) fn uncarried_subst(&self, t: kiln_ast::TypeId) -> Option<kiln_ast::TypeId> {
        let ast = &self.s.ast;
        let kiln_ast::types::TypeView::SubstTemplateTypeParm { replacement, associated, .. } =
            ast.types.view(t)
        else {
            return None;
        };
        if self.recs.contains_key(&ast.first(associated)) || replacement.quals_bits() != 0 {
            return None;
        }
        Some(replacement.ty())
    }

    /// Why implicit variable template specialization `d` (`si`) cannot be
    /// a reference record, `None` when it can.
    pub(super) fn var_spec_why(&self, si: &SpecInfo) -> Option<&'static str> {
        let ast = &self.s.ast;
        // The importer matches the partial specializations while it reads
        // the records, before their constraints are in: the image keeps
        // the unit's selection (WP-1358, [`super::psel`]). A constrained
        // template's constraints, checked then too, held for the unit's
        // specialization ([temp.names]/7).
        //
        // A deduced type comes from the initializer, which comes with the
        // bodies ([dcl.spec.auto.general]/11).
        let pty = ast.decls.get(si.from).ty().unwrap_or(QualType::ERROR);
        if self.s.contains_placeholder(pty) {
            return Some(VAR_SPEC);
        }
        None
    }
}

impl Sema<'_, '_> {
    /// Declares the specialization of variable template `tmpl` for
    /// `written` at `loc`, as `get_var_spec` does, without its
    /// initializer (instantiated once the bodies are in).
    pub(super) fn image_var_template_spec(
        &mut self,
        tmpl: DeclId,
        written: &[TemplateArg],
        loc: Loc,
    ) -> Option<DeclId> {
        let locs = vec![loc; written.len()];
        self.get_var_spec(tmpl, written, &locs, loc).ok()
    }

    /// Member `m` of member reference record `i`: an explicit
    /// instantiation declaration when the image lists it, at the record's
    /// location, as `act_on_explicit_member_inst` records it.
    pub(super) fn image_member_expl_decl(&mut self, img: &Image, i: u32, m: DeclId, loc: Loc) {
        if m == self.ast.tu || !img.fexpl.contains(i) {
            return;
        }
        let Some((k, _)) = self.ast.member_spec_info(m) else { return };
        self.ast.templates.update_member_spec(k, |x| {
            x.kind = SpecKind::ExplicitInstantiationDeclaration;
            x.point_of_instantiation = loc;
        });
    }

    /// Specialization `s` of reference record `i`: an explicit
    /// instantiation declaration when the image lists it there, its kind
    /// and point of instantiation the directive's (`loc`), as
    /// `act_on_explicit_function_inst` records them.
    pub(super) fn image_fn_expl_decl(&mut self, img: &Image, i: u32, s: DeclId, loc: Loc) {
        if s == self.ast.tu || !img.fexpl.contains(i) {
            return;
        }
        let Some((k, _)) = self.ast.spec_info(s) else { return };
        self.ast.templates.update_spec(k, |x| {
            x.kind = SpecKind::ExplicitInstantiationDeclaration;
            x.point_of_instantiation = loc;
        });
    }
}
