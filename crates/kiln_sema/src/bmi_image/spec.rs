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

pub(super) const ARG: &str = "a template argument the image does not carry";
// The forms of `ARG`, named for the census (WP-1344).
const ARG_VALUE: &str =
    "a template argument the image does not carry: a value that is not an integer";
const ARG_VALUE_TY: &str =
    "a template argument the image does not carry: a value of a type that is not builtin";
const ARG_PACK: &str = "a template argument the image does not carry: a pack in an argument list";
pub(super) const ARG_EXPR: &str = "a template argument the image does not carry: an expression";
const ARG_SPEC: &str =
    "a template argument the image does not carry: a class that is no specialization";
const ARG_SUBST: &str = "a template argument the image does not carry: a substituted parameter of an uncarried template";

impl Writer<'_, '_, '_> {
    /// Whether `d` is a member of a carried class template pattern, or of
    /// a class nested in one (S4b): dependent, claimed by its class.
    pub(super) fn dep_member(&self, d: DeclId) -> bool {
        let n = self.s.ast.decls.get(d);
        n.has(DeclFlags::DEPENDENT_CTX)
            && n.ctx().and_then(|c| self.recs.get(&c)).is_some_and(|&r| {
                let c = &self.img.decls[r as usize];
                // A member enumeration's enumerators too (WP-1310).
                c.flags & super::DEP != 0
                    && matches!(c.payload, Payload::Record { .. } | Payload::Enum { .. })
            })
    }

    /// Whether `d` is a member of a class template specialization.
    fn spec_member(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        ast.decls.get(d).ctx().is_some_and(|c| {
            ast.decls.get(c).kind() == DeclKind::CXXRecord && ast.spec_info(ast.first(c)).is_some()
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
                    // WP-1344: and its exception specification's
                    // expression (`noexcept(noexcept(declval<T>()))`).
                    qs.push(q);
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
            && self.late_split(d).is_none()
        {
            qs.extend(ast.lists.get(r.bases).iter().map(|&g| ast.decls.base_spec(g).ty));
        }
        if let Some(ArgView::Type(q)) = ast.templates.defaults.get(d).map(TemplateArg::view) {
            qs.push(q);
        }
        self.spec_arg_types(d, &mut qs);
        for q in qs {
            let _ = self.qref(q);
        }
    }

    /// The image arguments of a specialization's converted argument list
    /// `list`: an argument pack as [`ArgRec::Pack`] followed by its
    /// elements (WP-1344, `_List<unsigned char, ...>`).
    pub(super) fn spec_arg_recs(&mut self, list: kiln_ast::ArgListId) -> W<Vec<ArgRec>> {
        let args: Vec<TemplateArg> = self.s.ast.types.args(list).to_vec();
        let mut out = Vec::with_capacity(args.len());
        for a in args {
            if let ArgView::Pack(l) = a.view() {
                let elems: Vec<TemplateArg> = self.s.ast.types.args(l).to_vec();
                out.push(ArgRec::Pack(u32::try_from(elems.len()).map_err(|_| ARG)?));
                for x in elems {
                    out.push(self.arg_rec(x)?);
                }
            } else {
                out.push(self.arg_rec(a)?);
            }
        }
        Ok(out)
    }

    /// The image arguments of a type's argument list `list` as written:
    /// an expression argument as its expression (WP-1300, [`super::exprs`]).
    pub(super) fn written_arg_recs(&mut self, list: kiln_ast::ArgListId) -> W<Vec<ArgRec>> {
        let args: Vec<TemplateArg> = self.s.ast.types.args(list).to_vec();
        let mut out = Vec::with_capacity(args.len());
        for a in args {
            out.push(match a.view() {
                ArgView::Expr { expr, profile } => {
                    ArgRec::Expr(self.texpr(expr)?, profile.is_some())
                }
                _ => self.arg_rec(a)?,
            });
        }
        Ok(out)
    }

    /// The image argument of `a`: a type, or an integral or `bool` value
    /// of a builtin type.
    pub(super) fn arg_rec(&mut self, a: TemplateArg) -> W<ArgRec> {
        Ok(match a.view() {
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
                    _ => return Err(ARG_VALUE),
                };
                let q = self.qref(ty)?;
                // An enumeration's value too (WP-1344).
                if !value_ty_ok(&self.img, q) {
                    return Err(ARG_VALUE_TY);
                }
                ArgRec::Value(q, v)
            }
            // WP-1342: a template, or a pack expansion of one.
            ArgView::Template(n) => self.tname_arg(n, 0)?,
            ArgView::TemplateExpansion { name, num_expansions } => {
                let k = match num_expansions {
                    None => 1,
                    Some(k) => k.checked_add(2).ok_or(ARG)?,
                };
                self.tname_arg(name, k)?
            }
            ArgView::Pack(_) => return Err(ARG_PACK),
            ArgView::Expr { .. } => return Err(ARG_EXPR),
            _ => return Err(ARG),
        })
    }

    /// The record of template `t` (any declaration of it), a carried
    /// template of kind `kind`.
    pub(super) fn template_rec(&self, t: DeclId, kind: u8) -> W<u32> {
        let t = self.s.ast.first(t);
        if kind == 1
            && let Some(r) = self.fwd_template(t)
        {
            return Ok(r);
        }
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
        let class = ast.decls.get(d).kind() == DeclKind::CXXRecord;
        // An explicitly instantiated class (WP-1307) is named as an
        // implicit one: the importer's explicit instantiation runs after
        // the records (`explinst.rs`).
        let explicit_inst = matches!(
            si.kind,
            SpecKind::ExplicitInstantiationDeclaration | SpecKind::ExplicitInstantiationDefinition
        ) && (class || self.fn_explicit_inst(d, &si)?);
        // An explicit specialization named before its definition, which
        // is recorded later (WP-1345): named as the replay's use named it,
        // never instantiated.
        let deferred = class && self.deferred.contains(&d);
        // An explicit specialization the image does not carry has its
        // own reason (WP-1352): this is a cascade of it.
        if si.kind == SpecKind::ExplicitSpecialization && !deferred {
            return Err(if class {
                "a type declared outside the image"
            } else {
                super::body::OUTSIDE
            });
        }
        if !explicit_inst
            && !deferred
            && !matches!(si.kind, SpecKind::ImplicitInstantiation | SpecKind::Undeclared)
        {
            return Err("an explicit specialization or instantiation");
        }
        // A variable template's (WP-1352, [`super::varspec`]).
        let var = !class && ast.decls.var(d).is_some();
        if !class && !var && ast.decls.function(d).is_none() {
            return Err(super::varspec::VAR_SPEC);
        }
        if var && let Some(why) = self.var_spec_why(&si) {
            return Err(why);
        }
        // Instantiated (or to be) from the primary template's own
        // pattern, or from a carried partial specialization whose
        // definition is complete (WP-1283: the importer matches the same
        // partial specializations, built in the same order); not from a
        // member template.
        let from_template = ast.template_info(si.from).is_some()
            || ast.described_template(si.from).map(|t| ast.first(t)) == Some(si.template);
        let from_partial = si.from_args.is_some()
            && ast.partial_spec_info(si.from).is_some()
            && self.recs.get(&si.from).is_some_and(|&p| {
                super::classtempl::partial_of(&self.img, p).is_some()
                    && matches!(&self.img.decls[p as usize].payload,
                        Payload::Record { def: Some(df), .. } if df.end != NONE)
            });
        if !deferred && !from_partial && (si.from_args.is_some() || !from_template) {
            return Err("a specialization of a partial specialization");
        }
        let template = self.template_rec(si.template, if var { 3 } else { u8::from(class) })?;
        let complete = class
            && !deferred
            && ast.decls.record(d).is_some_and(|r| r.defn.contains(DefinitionBits::COMPLETE));
        // The importer names it while it builds the records, before their
        // constraints are in (WP-1284); WP-1344 carries what that cannot
        // change ([`super::constrspec`]).
        if let Some(why) = self.constrained_spec_why(si.template, complete) {
            return Err(why);
        }
        let args = self.spec_arg_recs(si.args)?;
        // The importer instantiates a complete one while it reads the
        // records, before the pattern's bit-field widths and default
        // member initializers come with the bodies: the specialization
        // would lose them (WP-1300; static data member initializers are
        // instantiated again afterwards, `image_spec_member_inits`).
        if complete
            && ast.definition(si.from).is_some_and(|p| {
                ast.members(p).any(|m| {
                    ast.decls.field(m).is_some_and(|f| f.bit_width.is_some() || f.init.is_some())
                })
            })
        {
            return Err(
                "a specialization of a class template with a bit-field or a default member initializer",
            );
        }
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
        if complete {
            self.complete_specs.push((d, i));
        }
        if explicit_inst && !class {
            self.img.fexpl.recs.push(i);
        }
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
        // WP-1352: an explicit instantiation declaration of the member
        // alone (`extern template C<int>::C(const C &);`), its directive.
        let mut member_inst = None;
        let pattern = match ast.templ_link(d).get() {
            Some((TemplLinkKind::MemberOfSpecialization, i)) => {
                let m = ast.templates.member_spec(i).ok_or("a malformed member")?;
                // A member of an explicitly instantiated class has the
                // class's kind, which the importer's explicit
                // instantiation gives it again (WP-1307).
                let class_kind = ast.spec_info(ast.first(class)).map(|(_, s)| s.kind);
                let of_explicit_inst = matches!(
                    m.kind,
                    SpecKind::ExplicitInstantiationDeclaration
                        | SpecKind::ExplicitInstantiationDefinition
                ) && class_kind == Some(m.kind);
                if !of_explicit_inst {
                    member_inst = self.member_inst_loc(d, m)?;
                }
                if m.kind != SpecKind::ImplicitInstantiation
                    && !of_explicit_inst
                    && member_inst.is_none()
                {
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
                    // WP-1345: an implicit special member, declared again
                    // by the importer (`members.rs`).
                    None if ast.decls.get(d).has(DeclFlags::IMPLICIT) => {
                        return self.implicit_spec_member(d, class).map(Some);
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
            loc: member_inst.unwrap_or(NONE),
            prev: NONE,
            payload: Payload::Member { spec, pattern: pat },
        });
        self.recs.insert(d, i);
        if member_inst.is_some() {
            self.img.fexpl.recs.push(i);
        }
        Ok(Some(i))
    }

    /// Why underlying type `q` of a `X<args>` naming template record
    /// `template` is not one the image carries, or `None` when it is (a
    /// specialization reference or carried explicit specialization of
    /// `template`). WP-1320 names the form for the census.
    fn tst_under_form(&self, q: super::QRef, template: u32) -> Option<&'static str> {
        if q.quals != 0 {
            return Some("a specialization type: a qualified underlying type");
        }
        if q.ty <= super::BUILTINS {
            return Some("a specialization type: a builtin underlying type");
        }
        let r = match &self.img.types[(q.ty - super::BUILTINS - 1) as usize] {
            TyRec::Record(r) => *r,
            // A dependent `X<T>` completed with its defaults, `X<T, d>`
            // (`dependent_spec_canon`, [temp.arg.general]/9).
            TyRec::Tst { template: t, under: None, .. } if *t == template => return None,
            TyRec::Tst { .. } => {
                return Some("a specialization type: another template's specialization");
            }
            TyRec::Injected(_) => {
                return Some("a specialization type: a partial specialization's own type");
            }
            _ => return Some("a specialization type: an underlying type that is not a class"),
        };
        match self.img.decls[r as usize].payload {
            Payload::Spec { template: st, .. } if st == template => None,
            _ if super::classtempl::explicit_of(&self.img, r) == Some(template) => None,
            Payload::Spec { .. } => {
                Some("a specialization type: a specialization of another template record")
            }
            _ if super::classtempl::explicit_of(&self.img, r).is_some() => {
                Some("a specialization type: an explicit specialization of another template record")
            }
            _ if super::classtempl::partial_of(&self.img, r).is_some() => {
                Some("a specialization type: a partial specialization")
            }
            Payload::Record { .. } => Some("a specialization type: a plain class"),
            _ => Some("a specialization type: another record"),
        }
    }

    /// A class template type (S4b): `X<args>` naming a carried class
    /// template, or the injected-class-name of its pattern; `None` for
    /// any other type.
    pub(super) fn template_ty(&mut self, t: TypeId) -> W<Option<TyRec>> {
        let ast = &self.s.ast;
        Ok(Some(match ast.types.view(t) {
            TypeView::TemplateSpecialization { name, args, underlying, alias } => {
                // `TT<args>` of a template template parameter (WP-1342):
                // dependent, its arguments as written.
                if let Some(template) = self.tst_ttp(name)? {
                    if underlying.is_some() || alias {
                        return Err(
                            "a template template parameter's specialization with an underlying type",
                        );
                    }
                    let args = self.written_arg_recs(args)?;
                    return Ok(Some(TyRec::Tst { template, args, under: None }));
                }
                let TemplateName::Template(td) = ast.types.template_name_view(name) else {
                    return Err("a template named through a qualifier or parameter");
                };
                if alias {
                    // An alias template specialization (WP-1279): its
                    // underlying type is the substituted pattern.
                    let template = self
                        .template_rec(td, 2)
                        .map_err(|_| "an alias template outside the image")?;
                    let args = self.written_arg_recs(args)?;
                    let u = underlying.ok_or("an alias template")?;
                    return Ok(Some(TyRec::Tst { template, args, under: Some(self.qref(u)?) }));
                }
                // A dependent builtin template's (WP-1344).
                if let Some(template) = self.builtin_template_rec(td)? {
                    if underlying.is_some() || alias {
                        return Err("a builtin template's specialization that is not dependent");
                    }
                    let args = self.written_arg_recs(args)?;
                    return Ok(Some(TyRec::Tst { template, args, under: None }));
                }
                let template = self.template_rec(td, 1)?;
                let spec = underlying.and_then(|u| match ast.types.view(u.ty()) {
                    TypeView::Record(d) => Some(d),
                    _ => None,
                });
                let args = self.tst_args(args, spec)?;
                let under = match underlying {
                    None => None,
                    Some(u) => {
                        let q = self.qref(u)?;
                        if let Some(why) = self.tst_under_form(q, template) {
                            return Err(why);
                        }
                        Some(q)
                    }
                };
                TyRec::Tst { template, args, under }
            }
            TypeView::InjectedClassName { pattern, .. } => {
                let p = ast.first(pattern);
                match ast.described_template(p) {
                    Some(tmpl) => TyRec::Injected(self.template_rec(tmpl, 1)?),
                    // A carried partial specialization's (WP-1283).
                    None => match self.recs.get(&p) {
                        Some(&r) if super::classtempl::partial_of(&self.img, r).is_some() => {
                            TyRec::Injected(r)
                        }
                        _ => return Err("an explicit or partial specialization"),
                    },
                }
            }
            TypeView::Record(d) if ast.spec_info(ast.first(d)).is_some() => {
                let Some(r) = self.spec_rec(d)? else { return Err(ARG_SPEC) };
                match self.s.ast.decls.get(self.s.ast.first(d)).ty() {
                    Some(q) if q.ty() == t && q.quals_bits() == 0 => TyRec::Record(r),
                    _ => return Err("a type the image does not carry"),
                }
            }
            // A member typedef of a class specialization (WP-1344).
            TypeView::Typedef { decl, .. }
                if !self.recs.contains_key(&decl) && self.spec_member(decl) =>
            {
                match self.member_rec(decl)? {
                    Some(r) => TyRec::Typedef(r),
                    None => return Err("a type declared outside the image"),
                }
            }
            TypeView::SubstTemplateTypeParm { replacement, associated, index, pack_index } => {
                let assoc =
                    self.recs.get(&self.s.ast.first(associated)).copied().ok_or(ARG_SUBST)?;
                // A template's parameter, or a carried partial
                // specialization's (WP-1283).
                if !matches!(self.img.decls[assoc as usize].payload, Payload::Template { .. })
                    && super::classtempl::partial_of(&self.img, assoc).is_none()
                {
                    return Err(ARG_SUBST);
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

/// Whether `q` is the type of a value argument: an unqualified builtin
/// or (WP-1344) enumeration type.
pub(super) fn value_ty_ok(img: &Image, q: super::QRef) -> bool {
    q.quals == 0
        && (q.ty <= super::BUILTINS
            || matches!(
                q.ty.checked_sub(super::BUILTINS + 1).and_then(|k| img.types.get(k as usize)),
                Some(TyRec::Enum(_))
            ))
}

/// Whether specialization reference `spec` is one the importer
/// completes, so that a member typedef of it may name a base class.
pub(super) fn member_typedef_base(img: &Image, spec: u32) -> bool {
    matches!(
        img.decls.get(spec as usize).map(|r| &r.payload),
        Some(Payload::Spec { complete: true, .. })
    )
}

/// Whether specialization reference arguments `args` (record `at`) fit
/// template parameters `params`: one argument per parameter, a pack
/// parameter's an [`ArgRec::Pack`] followed by its elements (WP-1344).
pub(super) fn spec_args_ok(
    img: &Image,
    params: &[u32],
    args: &[ArgRec],
    at: usize,
    qref_at: &dyn Fn(super::QRef, usize) -> bool,
) -> bool {
    let one = |p: &Payload, a: &ArgRec| match (p, a) {
        (Payload::Ttp { .. }, ArgRec::Type(q)) => qref_at(*q, at),
        (Payload::Tttp(_), ArgRec::Template(d, 0)) => {
            (*d as usize) < at && super::tttp::tname_rec(img, *d)
        }
        (Payload::Nttp { .. }, ArgRec::Value(q, _)) => value_ty_ok(img, *q),
        _ => false,
    };
    let mut k = 0;
    for &p in params {
        let Some(pr) = img.decls.get(p as usize) else { return false };
        let pack = match &pr.payload {
            Payload::Ttp { pack, .. } | Payload::Nttp { pack, .. } => *pack,
            Payload::Tttp(t) => t.pack,
            _ => return false,
        };
        match args.get(k) {
            Some(ArgRec::Pack(n)) if pack => {
                let n = *n as usize;
                let Some(elems) = args.get(k + 1..k + 1 + n) else { return false };
                if !elems.iter().all(|a| one(&pr.payload, a)) {
                    return false;
                }
                k += 1 + n;
            }
            Some(a) if one(&pr.payload, a) => k += 1,
            _ => return false,
        }
    }
    k == args.len()
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
            ArgRec::Expr(i, p) => self.image_expr_arg(i, p),
            // Built by `image_targ`, which has the records.
            ArgRec::Template(..) | ArgRec::DepTemplate(..) | ArgRec::Pack(_) => TemplateArg::NULL,
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
        // A pack's elements (WP-1344): written one by one for a class
        // template, which converts them again, as one pack for a
        // function template's converted list.
        let mut list = Vec::with_capacity(args.len());
        let mut k = 0;
        while k < args.len() {
            if let ArgRec::Pack(n) = args[k] {
                let elems = args.get(k + 1..k + 1 + n as usize)?;
                let mut v = Vec::with_capacity(elems.len());
                for a in elems {
                    let ty = self.image_ty(img, tys, ids, a.q());
                    v.push(self.image_targ(ids, ty, a));
                }
                if kind == 1 || kind == 3 {
                    list.extend(v);
                } else {
                    list.push(TemplateArg::pack(self.ast.types.arg_list(&v)));
                }
                k += 1 + n as usize;
                continue;
            }
            let ty = self.image_ty(img, tys, ids, args[k].q());
            list.push(self.image_targ(ids, ty, &args[k]));
            k += 1;
        }
        let spec = if kind == 1 {
            let locs = vec![loc; list.len()];
            self.get_class_spec(tmpl, &list, &locs, loc).ok()?
        } else if kind == 3 {
            self.image_var_template_spec(tmpl, &list, loc)?
        } else {
            let sugared = self.ast.types.arg_list(&list);
            let canonical = self.ast.types.canonical_args(sugared);
            self.get_function_spec(tmpl, &ConvertedArgs { sugared, canonical }, loc).ok()?
        };
        if *complete {
            self.image_complete_spec(spec, loc, false);
        }
        Some(spec)
    }

    /// Completes class specialization `spec`, which the unit completed.
    /// A specialization named before its template's definition (`using
    /// XI = X<int>;` after `template<class T> struct X;`, WP-1307) has
    /// its record before the definition's: it is completed when a member
    /// reference or the end of the image needs it (`late`), as a replay
    /// completes it at its first use after the definition.
    fn image_complete_spec(&mut self, spec: DeclId, loc: Loc, late: bool) {
        if self.ast.decls.record(spec).is_none_or(|r| r.defn.contains(DefinitionBits::COMPLETE)) {
            return;
        }
        if !late && let Some((_, si)) = self.ast.spec_info(spec) {
            let tmpl = self.inst_template(si.template);
            if self.pattern_of(tmpl).and_then(|p| self.ast.definition(p)).is_none() {
                return;
            }
        }
        if let Some(ty) = self.ast.decls.get(spec).ty() {
            let _ = self.require_complete(ty, loc);
        }
    }

    /// Resolves a member reference.
    pub(super) fn image_member(&mut self, ids: &[DeclId], r: &DeclRec) -> DeclId {
        let Payload::Member { spec, pattern } = r.payload else { return self.ast.tu };
        let s = self.ast.first(ids[spec as usize]);
        if s != self.ast.tu {
            let loc = self.ast.decls.get(s).loc();
            self.image_complete_spec(s, loc, true);
        }
        self.ast.templates.find_member(s, ids[pattern as usize]).unwrap_or(self.ast.tu)
    }

    /// The in-class initializers of the static data members of class
    /// specialization `spec` that its instantiation, made while the
    /// image's declarations were read, could not see: the pattern's
    /// initializers come with the bodies. A non-deferred one
    /// (`static const int w = sizeof(T);`, [temp.inst]/3) is
    /// instantiated now, as replay's instantiation of the class did.
    fn image_spec_member_inits(&mut self, spec: DeclId, loc: Loc) {
        if self.ast.decls.get(spec).kind() != DeclKind::CXXRecord {
            return;
        }
        let missing: Vec<DeclId> = self
            .ast
            .members(spec)
            .filter(|&m| {
                self.ast.decls.var(m).is_some_and(|v| v.init.is_none())
                    && self
                        .ast
                        .instantiated_from(m)
                        .and_then(|p| self.ast.decls.var(p))
                        .is_some_and(|p| p.init.is_some())
            })
            .collect();
        for m in missing {
            self.instantiate_missing_static_member_init(m, loc);
        }
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
            if let Payload::Spec { complete: true, .. } = r.payload {
                self.image_complete_spec(d, loc, true);
            }
            if matches!(r.payload, Payload::Spec { .. }) {
                self.image_spec_member_inits(d, loc);
            }
            let func = self.ast.decls.function(d).is_some();
            let used = r.flags & DeclFlags::USED.bits() != 0;
            if self.ast.decls.var(d).is_some()
                && (used || r.flags & DeclFlags::REFERENCED.bits() != 0)
            {
                // A static data member (or variable template
                // specialization) is instantiated as replay's reference
                // did: one usable in constant expressions at once, even
                // when only named (a non-odr-use, `B<2>::v`), its value
                // being needed; any other only when odr-used.
                self.unevaluated += u32::from(!used);
                self.note_var_referenced(d, loc);
                self.unevaluated -= u32::from(!used);
            }
            if used {
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
