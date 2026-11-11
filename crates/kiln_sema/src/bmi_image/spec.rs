//! Specialization and member references and class template types in the
//! declaration image (spec/modules.md §11.11, "Second slice (WP-1139)").
//!
//! The writer never carries a specialization the unit made implicitly;
//! it carries a reference to it (the template and the arguments), which
//! the importer resolves through the entry points a use in source calls
//! (`get_function_spec`, `get_class_spec`), so it finds or makes the
//! same specialization replay does.

use kiln_ast::consts::Scalar;
use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits, TemplLinkKind};
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{ArgView, TemplateArg, TemplateName, TypeView};
use kiln_ast::{DeclId, QualType, TypeId};
use kiln_source::Loc;

use super::{ArgRec, DECL_FLAGS, DeclRec, Image, NONE, Payload, TyRec, W, Writer};
use crate::Sema;
use crate::templ::args::ConvertedArgs;

const ARG: &str = "a template argument the image does not carry";

impl Writer<'_, '_, '_> {
    /// Whether `d` is a member of a carried class template pattern, or of
    /// a class nested in one (S4b): dependent, claimed by its class.
    pub(super) fn dep_member(&self, d: DeclId) -> bool {
        let n = self.s.ast.decls.get(d);
        n.has(DeclFlags::DEPENDENT_CTX)
            && n.ctx().and_then(|c| self.recs.get(&c)).is_some_and(|&r| {
                let c = &self.img.decls[r as usize];
                c.flags & super::DEP != 0 && matches!(c.payload, Payload::Record { .. })
            })
    }

    /// Writes the specialization references the types of `d` name
    /// before `d`'s own record (errors are left for `record` to report).
    pub(super) fn prescan(&mut self, d: DeclId) {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        let mut qs: Vec<QualType> = Vec::new();
        if let Some(q) = n.ty() {
            match ast.types.view(q.ty()) {
                TypeView::Function(p) => {
                    qs.push(p.ret());
                    qs.extend_from_slice(ast.types.params(&p));
                }
                _ => qs.push(q),
            }
        }
        if let Some(f) = ast.decls.function(d) {
            qs.extend(ast.lists.iter::<DeclId>(f.params).filter_map(|p| ast.decls.get(p).ty()));
        }
        if let Some(t) = ast.decls.typedef(d) {
            qs.push(t.underlying);
        }
        if let Some(r) = ast.decls.record(d)
            && ast.definition(d) == Some(d)
        {
            qs.extend(ast.lists.get(r.bases).iter().map(|&g| ast.decls.base_spec(g).ty));
        }
        if let Some(ArgView::Type(q)) = ast.templates.defaults.get(d).map(TemplateArg::view) {
            qs.push(q);
        }
        for q in qs {
            let _ = self.qref(q);
        }
    }

    /// The image arguments of argument list `list`.
    pub(super) fn arg_recs(&mut self, list: kiln_ast::ArgListId) -> W<Vec<ArgRec>> {
        let args: Vec<TemplateArg> = self.s.ast.types.args(list).to_vec();
        let mut out = Vec::with_capacity(args.len());
        for a in args {
            out.push(match a.view() {
                ArgView::Type(q) => ArgRec::Type(self.qref(q)?),
                ArgView::Value { ty, value } => {
                    let v = match self.s.ast.consts.scalar(value) {
                        Some(Scalar::Int { signed, bits, .. }) => if signed {
                            bits.to_i128()
                        } else {
                            bits.to_u128().and_then(|u| i128::try_from(u).ok())
                        }
                        .ok_or(ARG)?,
                        Some(Scalar::Bool { value, .. }) => i128::from(value),
                        _ => return Err(ARG),
                    };
                    let q = self.qref(ty)?;
                    if q.ty > super::BUILTINS || q.quals != 0 {
                        return Err(ARG);
                    }
                    ArgRec::Value(q, v)
                }
                _ => return Err(ARG),
            });
        }
        Ok(out)
    }

    /// The record of template `t` (any declaration of it), a carried
    /// template of kind `kind`.
    fn template_rec(&self, t: DeclId, kind: u8) -> W<u32> {
        let t = self.s.ast.first(t);
        match self.recs.get(&t) {
            Some(&r)
                if matches!(self.img.decls[r as usize].payload,
                Payload::Template { kind: k, .. } if k == kind) =>
            {
                Ok(r)
            }
            _ => Err("a specialization of a template outside the image"),
        }
    }

    /// The specialization reference record of `d`, an implicit
    /// instantiation of a carried function or class template, written
    /// on first use; `None` when `d` is no specialization.
    pub(super) fn spec_rec(&mut self, d: DeclId) -> W<Option<u32>> {
        let ast = &self.s.ast;
        let d = ast.first(d);
        let Some((_, si)) = ast.spec_info(d) else { return Ok(None) };
        if let Some(&r) = self.recs.get(&d) {
            return Ok(Some(r));
        }
        let si = *si;
        // A class specialization only named (`using B = X<int>;`, WP-1279)
        // is `Undeclared`: the importer names it the same way.
        if !matches!(si.kind, SpecKind::ImplicitInstantiation | SpecKind::Undeclared) {
            return Err("an explicit specialization or instantiation");
        }
        let class = ast.decls.get(d).kind() == DeclKind::CXXRecord;
        if !class && ast.decls.function(d).is_none() {
            return Err("a variable template specialization");
        }
        // Instantiated (or to be) from the primary template's own
        // pattern: neither a partial specialization nor a member template.
        let from_template = ast.template_info(si.from).is_some()
            || ast.described_template(si.from).map(|t| ast.first(t)) == Some(si.template);
        if si.from_args.is_some() || !from_template {
            return Err("a specialization of a partial specialization");
        }
        let template = self.template_rec(si.template, u8::from(class))?;
        // The importer names it while it builds the records, before their
        // constraints are in (WP-1284).
        if self.constrained_template(si.template) {
            return Err("a specialization of a constrained template");
        }
        let args = self.arg_recs(si.args)?;
        let complete =
            class && ast.decls.record(d).is_some_and(|r| r.defn.contains(DefinitionBits::COMPLETE));
        let loc = self.loc(si.point_of_instantiation)?;
        let o = ast.modules.owner(d);
        let i = self.img.decls.len() as u32;
        self.img.decls.push(DeclRec {
            flags: ast.decls.get(d).flags().bits() & DECL_FLAGS,
            owner: u8::from(o.purview) | (u8::from(o.exported) << 1),
            access: 0,
            name: NONE,
            ctx: NONE,
            lexical: NONE,
            loc,
            prev: NONE,
            payload: Payload::Spec { template, complete, args },
        });
        self.recs.insert(d, i);
        Ok(Some(i))
    }

    /// The member reference record of `d`, a member of an implicit
    /// instantiation of a carried class template, written on first use;
    /// `None` when `d` is no such member.
    pub(super) fn member_rec(&mut self, d: DeclId) -> W<Option<u32>> {
        let ast = &self.s.ast;
        let class = ast.decls.get(d).ctx().ok_or("a malformed member")?;
        // Templatable members carry a link; fields and the like sit in
        // the sparse map, under a class specialization.
        let pattern = match ast.templ_link(d).get() {
            Some((TemplLinkKind::MemberOfSpecialization, i)) => {
                let m = ast.templates.member_spec(i).ok_or("a malformed member")?;
                if m.kind != SpecKind::ImplicitInstantiation {
                    return Err("an explicitly specialized member");
                }
                m.instantiated_from
            }
            None if ast.decls.get(class).kind() == DeclKind::CXXRecord
                && ast.spec_info(ast.first(class)).is_some() =>
            {
                match ast.templates.instantiated_from_map(d) {
                    Some(p) => p,
                    // An implicit special member of the specialization
                    // has no pattern to name (WP-1139 follow-up).
                    None if ast.decls.get(d).has(DeclFlags::IMPLICIT) => {
                        return Err("an implicit member of a class template specialization");
                    }
                    None => return Ok(None),
                }
            }
            _ => return Ok(None),
        };
        if let Some(&r) = self.recs.get(&d) {
            return Ok(Some(r));
        }
        if ast.decls.get(d).has(DeclFlags::IMPLICIT) {
            return Err("an implicit member of a class template specialization");
        }
        let Some(spec) = self.spec_rec(class)? else {
            return Err("a member of a nested class of a specialization");
        };
        let pat = *self.recs.get(&pattern).ok_or("a member of a template outside the image")?;
        if ast.templates.find_member(ast.first(class), pattern) != Some(d) {
            return Err("a member redeclared in a specialization");
        }
        let o = ast.modules.owner(d);
        let i = self.img.decls.len() as u32;
        self.img.decls.push(DeclRec {
            flags: ast.decls.get(d).flags().bits() & DECL_FLAGS,
            owner: u8::from(o.purview) | (u8::from(o.exported) << 1),
            access: 0,
            name: NONE,
            ctx: NONE,
            lexical: NONE,
            loc: NONE,
            prev: NONE,
            payload: Payload::Member { spec, pattern: pat },
        });
        self.recs.insert(d, i);
        Ok(Some(i))
    }

    /// A class template type (S4b): `X<args>` naming a carried class
    /// template, or the injected-class-name of its pattern; `None` for
    /// any other type.
    pub(super) fn template_ty(&mut self, t: TypeId) -> W<Option<TyRec>> {
        let ast = &self.s.ast;
        Ok(Some(match ast.types.view(t) {
            TypeView::TemplateSpecialization { name, args, underlying, alias } => {
                let TemplateName::Template(td) = ast.types.template_name_view(name) else {
                    return Err("a template named through a qualifier or parameter");
                };
                if alias {
                    // An alias template specialization (WP-1279): its
                    // underlying type is the substituted pattern.
                    let template = self
                        .template_rec(td, 2)
                        .map_err(|_| "an alias template outside the image")?;
                    let args = self.arg_recs(args)?;
                    let u = underlying.ok_or("an alias template")?;
                    return Ok(Some(TyRec::Tst { template, args, under: Some(self.qref(u)?) }));
                }
                let template = self.template_rec(td, 1)?;
                let args = self.arg_recs(args)?;
                let under = match underlying {
                    None => None,
                    Some(u) => {
                        let q = self.qref(u)?;
                        let ok = q.quals == 0
                            && q.ty > super::BUILTINS
                            && matches!(self.img.types[(q.ty - super::BUILTINS - 1) as usize],
                                TyRec::Record(r) if matches!(self.img.decls[r as usize].payload,
                                    Payload::Spec { template: st, .. } if st == template));
                        if !ok {
                            return Err("a specialization type the image does not carry");
                        }
                        Some(q)
                    }
                };
                TyRec::Tst { template, args, under }
            }
            TypeView::InjectedClassName { pattern, .. } => {
                let tmpl = ast.described_template(ast.first(pattern));
                TyRec::Injected(self.template_rec(tmpl.ok_or("a partial specialization")?, 1)?)
            }
            TypeView::Record(d) if ast.spec_info(ast.first(d)).is_some() => {
                let Some(r) = self.spec_rec(d)? else { return Err(ARG) };
                match self.s.ast.decls.get(self.s.ast.first(d)).ty() {
                    Some(q) if q.ty() == t && q.quals_bits() == 0 => TyRec::Record(r),
                    _ => return Err("a type the image does not carry"),
                }
            }
            TypeView::SubstTemplateTypeParm { replacement, associated, index, pack_index } => {
                let assoc = self.recs.get(&self.s.ast.first(associated)).copied().ok_or(ARG)?;
                if !matches!(self.img.decls[assoc as usize].payload, Payload::Template { .. }) {
                    return Err(ARG);
                }
                let pack = match pack_index {
                    None => 0,
                    Some(i) if i < 0xffff => i + 1,
                    Some(_) => return Err(ARG),
                };
                TyRec::Subst { repl: self.qref(replacement)?, assoc, index, pack }
            }
            _ => return Ok(None),
        }))
    }
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Image template argument `a`, whose type is `ty`.
    pub(super) fn image_arg(&mut self, ty: QualType, a: &ArgRec) -> TemplateArg {
        match *a {
            ArgRec::Type(_) => TemplateArg::ty(ty),
            ArgRec::Value(_, v) => {
                let c = self.int_const(ty, v);
                TemplateArg::value(ty, c)
            }
        }
    }

    /// Resolves specialization reference `r`: finds or makes the
    /// specialization a use in source would (spec/modules.md §11.11),
    /// `None` when that fails (the importer then has no declaration to
    /// name; the record stands for the translation unit).
    pub(super) fn image_spec(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        r: &DeclRec,
        loc: Loc,
    ) -> Option<DeclId> {
        let Payload::Spec { template, complete, args } = &r.payload else { return None };
        let Payload::Template { kind, .. } = img.decls.get(*template as usize)?.payload else {
            return None;
        };
        let tmpl = self.ast.first(*ids.get(*template as usize)?);
        let mut list = Vec::with_capacity(args.len());
        for a in args {
            let ty = self.image_ty(img, tys, ids, a.q());
            list.push(self.image_arg(ty, a));
        }
        let spec = if kind == 1 {
            let locs = vec![loc; list.len()];
            self.get_class_spec(tmpl, &list, &locs, loc).ok()?
        } else {
            let sugared = self.ast.types.arg_list(&list);
            let canonical = self.ast.types.canonical_args(sugared);
            self.get_function_spec(tmpl, &ConvertedArgs { sugared, canonical }, loc).ok()?
        };
        if *complete {
            let ty = self.ast.decls.get(spec).ty()?;
            let _ = self.require_complete(ty, loc);
        }
        Some(spec)
    }

    /// Resolves a member reference.
    pub(super) fn image_member(&mut self, ids: &[DeclId], r: &DeclRec) -> DeclId {
        let Payload::Member { spec, pattern } = r.payload else { return self.ast.tu };
        let s = self.ast.first(ids[spec as usize]);
        self.ast.templates.find_member(s, ids[pattern as usize]).unwrap_or(self.ast.tu)
    }

    /// Marks the specializations and members the unit used, used again
    /// once every body is built: a `constexpr` definition is instantiated
    /// at once, any other queued for the end of the translation unit, as
    /// replay's own use does.
    pub(super) fn image_mark_specs(&mut self, img: &Image, ids: &[DeclId], main: &super::Locs) {
        for (i, r) in img.decls.iter().enumerate() {
            if !matches!(r.payload, Payload::Spec { .. } | Payload::Member { .. }) {
                continue;
            }
            let d = ids[i];
            if d == self.ast.tu {
                continue;
            }
            let loc = main.get(r.loc);
            let func = self.ast.decls.function(d).is_some();
            if r.flags & DeclFlags::USED.bits() != 0 {
                if func {
                    self.mark_function_referenced(d, loc);
                } else {
                    self.mark_used(d);
                }
            } else if r.flags & DeclFlags::REFERENCED.bits() != 0 {
                self.ast.decls.set_flags(d, DeclFlags::REFERENCED);
            }
        }
    }
}
