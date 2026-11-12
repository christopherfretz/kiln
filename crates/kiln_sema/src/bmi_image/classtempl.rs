//! Explicit and partial class template specializations in the
//! declaration image (spec/modules.md §11.11, "Templates as patterns",
//! WP-1283).
//!
//! A specialization the unit declares is a `Record` record with the
//! template's name and context, listed in [`ClassSpecs`] with its
//! template and arguments:
//!
//! - an explicit specialization (`template<> struct X<int>`,
//!   [temp.expl.spec]) carries the arguments as written; the importer
//!   finds or makes the specialization through `get_class_spec`, as a use
//!   in source does, and declares it explicit there (what
//!   `act_on_explicit_class_spec` does after its checks);
//! - a partial specialization (`template<class T> struct X<T*>`,
//!   [temp.spec.partial]) is a dependent record with its own
//!   template-head (parameter records before it) and its converted
//!   arguments, which name the head's parameters; the importer builds
//!   it with `class_partial_spec`, so matching sees the same partial
//!   specializations in the same order as replay.
//!
//! Neither is in the lookup table: lookup finds the template. The writer
//! refuses what it cannot rebuild exactly: an explicit instantiation, a
//! constrained or nested head, an argument other than a type, a value or
//! a head parameter.

use kiln_ast::decl::{DeclKind, TagKind};
use kiln_ast::expr::ExprKind;
use kiln_ast::names::DeclName;
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{ArgView, BuiltinType, TemplateArg};
use kiln_ast::{DeclHead, DeclId, QualType};

use super::{ArgRec, DEP, Image, LEX_TU, Locs, NONE, Payload, QRef, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;

/// A specialization's argument.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum CArg {
    /// A type or a value.
    Arg(ArgRec),
    /// A non-type parameter of the partial specialization's head (its
    /// record), named alone, or expanded when it is a pack.
    Parm(u32),
    /// An argument pack (only at the top level).
    Pack(Vec<CArg>),
}

/// A partial specialization's template-head.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct PartialHead {
    tloc: u32,
    langle: u32,
    rangle: u32,
    params: Vec<u32>,
}

/// One explicit or partial specialization.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct ClassSpec {
    rec: u32,
    /// The class template record.
    template: u32,
    /// A partial specialization's head; `None` for an explicit one.
    head: Option<PartialHead>,
    /// A partial specialization's converted arguments, an explicit one's
    /// as written.
    args: Vec<CArg>,
}

/// The specialization section, ascending by record.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct ClassSpecs {
    specs: Vec<ClassSpec>,
}

impl ClassSpecs {
    fn get(&self, rec: u32) -> Option<&ClassSpec> {
        self.specs.binary_search_by_key(&rec, |c| c.rec).ok().map(|k| &self.specs[k])
    }

    /// Drops the entries of records from `n` on (a census rollback).
    pub(super) fn truncate(&mut self, n: u32) {
        self.specs.retain(|c| c.rec < n);
    }
}

/// The template record of `rec` when it is a carried partial
/// specialization.
pub(super) fn partial_of(img: &Image, rec: u32) -> Option<u32> {
    img.cspecs.get(rec).filter(|c| c.head.is_some()).map(|c| c.template)
}

/// The parameter count of carried partial specialization `rec`'s
/// template-head (WP-1292).
pub(super) fn partial_head_len(img: &Image, rec: u32) -> Option<usize> {
    img.cspecs.get(rec).and_then(|c| c.head.as_ref()).map(|h| h.params.len())
}

/// The template record of `rec` when it is a carried explicit
/// specialization.
pub(super) fn explicit_of(img: &Image, rec: u32) -> Option<u32> {
    img.cspecs.get(rec).filter(|c| c.head.is_none()).map(|c| c.template)
}

const PARTIAL_ARG: &str = "a partial specialization argument the image does not carry";
const PARTIAL_ARG_EXPR: &str = "a partial specialization argument the image does not carry: an expression a section cannot hold";

impl Writer<'_, '_, '_> {
    /// The types the arguments of specialization `d` name, for
    /// `prescan`.
    pub(super) fn spec_arg_types(&self, d: DeclId, out: &mut Vec<QualType>) {
        let ast = &self.s.ast;
        let list = match ast.partial_spec_info(d) {
            Some((_, p)) => p.args,
            None => match ast.templates.args_as_written(d) {
                Some(l) if ast.spec_info(d).is_some() => l,
                _ => return,
            },
        };
        for &a in ast.types.args(list) {
            match a.view() {
                ArgView::Type(q) => out.push(q),
                ArgView::Pack(l) => {
                    out.extend(ast.types.args(l).iter().filter_map(|x| {
                        if let ArgView::Type(q) = x.view() { Some(q) } else { None }
                    }));
                }
                _ => {}
            }
        }
    }

    /// Lists class `d`, record `i`, an explicit or partial
    /// specialization, before its class checks.
    pub(super) fn class_spec_entry(&mut self, d: DeclId, i: u32) -> W<()> {
        let ast = &self.s.ast;
        // A variable template's specializations are listed the same way
        // (WP-1307), with the variable's record.
        let var = ast.decls.get(d).kind() != DeclKind::CXXRecord;
        let tkind = if var { 3 } else { 1 };
        let entry = if let Some((_, p)) = ast.partial_spec_info(d) {
            let p = *p;
            let list = *ast.templates.param_list(p.params);
            // A member template's partial specialization is carried
            // (WP-1342), at its template's depth.
            let tdepth =
                ast.template_info(p.template).map(|i| ast.templates.param_list(i.params).depth);
            if tdepth != Some(list.depth) {
                return Err(MALFORMED);
            }
            // Declared, then defined: the definition's template-head
            // has parameters of its own, after the record.
            if self.s.templ.partial_def_loc.contains_key(&d) {
                return Err("a redeclared partial specialization");
            }
            // A requires-clause is carried by `constraint_roots`
            // (WP-1342).
            let template = self.template_rec(p.template, tkind)?;
            let mut params = Vec::new();
            let mut decls = Vec::new();
            for q in ast.lists.iter::<DeclId>(list.params) {
                params.push(*self.recs.get(&q).ok_or(super::tttp::PARM_OUTSIDE)?);
                decls.push(q);
            }
            let args: Vec<TemplateArg> = ast.types.args(p.args).to_vec();
            let mut out = Vec::with_capacity(args.len());
            for a in args {
                out.push(match a.view() {
                    ArgView::Pack(l) => {
                        let elems: Vec<TemplateArg> = self.s.ast.types.args(l).to_vec();
                        let mut v = Vec::with_capacity(elems.len());
                        for x in elems {
                            v.push(self.partial_arg(x, &decls, &params)?);
                        }
                        CArg::Pack(v)
                    }
                    _ => self.partial_arg(a, &decls, &params)?,
                });
            }
            let head = PartialHead {
                tloc: self.loc(list.template_loc)?,
                langle: self.loc(list.l_angle)?,
                rangle: self.loc(list.r_angle)?,
                params,
            };
            ClassSpec { rec: i, template, head: Some(head), args: out }
        } else {
            let (_, si) = ast.spec_info(d).ok_or(MALFORMED)?;
            let si = *si;
            match si.kind {
                SpecKind::ExplicitSpecialization => {}
                SpecKind::ExplicitInstantiationDeclaration
                | SpecKind::ExplicitInstantiationDefinition => {
                    return Err("an explicit instantiation");
                }
                _ => return Err("an explicit specialization or instantiation"),
            }
            if si.from != d {
                return Err("an explicit specialization or instantiation");
            }
            let template = self.template_rec(si.template, tkind)?;
            // A variable's arguments as written are not kept: the
            // printed ones (the converted, less trailing defaults) stand
            // for them, which convert and print the same.
            let written = if var {
                si.print_args
            } else {
                ast.templates.args_as_written(d).ok_or(MALFORMED)?
            };
            let written: Vec<TemplateArg> = ast.types.args(written).to_vec();
            let converted: Vec<TemplateArg> = ast.types.args(si.args).to_vec();
            let packs = converted.iter().any(|a| matches!(a.view(), ArgView::Pack(_)));
            // The parameters the written arguments leave to their
            // defaults: a non-type parameter's default is built with its
            // record when it is a section (WP-1320), else after the
            // records, too late for `get_class_spec`.
            self.spec_defaults_ok(d, si.template, written.len())?;
            let mut out = Vec::with_capacity(written.len());
            for (k, a) in written.into_iter().enumerate() {
                // `X<3>`: the converted value, which prints the same.
                let c = if packs { None } else { converted.get(k).copied() };
                let rec = match (self.written_arg(a, c), a.view()) {
                    (Some(a), _) => self.arg_rec(a).map_err(|_| EXPLICIT_ARG)?,
                    // WP-1344: any other expression as written, which the
                    // importer's `get_class_spec` converts again.
                    (None, ArgView::Expr { expr, profile }) => ArgRec::Expr(
                        self.texpr(expr).map_err(|_| EXPLICIT_ARG_EXPR)?,
                        profile.is_some(),
                    ),
                    (None, _) => return Err(EXPLICIT_ARG),
                };
                out.push(CArg::Arg(rec));
            }
            ClassSpec { rec: i, template, head: None, args: out }
        };
        self.img.cspecs.specs.push(entry);
        Ok(())
    }

    /// Whether the importer converts the defaults explicit specialization
    /// `d` of `template` leaves to its non-type parameters (after
    /// `written` arguments) as replay did (WP-1320): each default is an
    /// expression section of the head template's own parameter (one
    /// inherited from another declaration is set after the records), and
    /// what converting it instantiated (the specializations whose point
    /// of instantiation is `d`'s) is classes without static data members
    /// or member functions: their initializers and bodies come with the
    /// image's bodies, after the record.
    fn spec_defaults_ok(&self, d: DeclId, template: DeclId, written: usize) -> W<()> {
        const WHY: &str = "an explicit specialization using a non-type default argument";
        let s = self.s;
        let ast = &s.ast;
        let head = s.head_template(ast.first(template));
        let info = ast.template_info(head).ok_or(MALFORMED)?;
        let plist = ast.templates.param_list(info.params).params;
        let mut ranges = Vec::new();
        for q in ast.lists.iter::<DeclId>(plist).skip(written) {
            if ast.decls.get(q).kind() == DeclKind::TemplateTypeParm {
                continue;
            }
            let own = ast.templates.defaults.get(q).copied();
            if own.is_none() && s.template_default(q).is_some() {
                return Err(WHY);
            }
            let sect = self.recs.get(&q).is_some_and(|&r| {
                matches!(self.img.decls[r as usize].payload, Payload::Nttp { sect: true, .. })
            });
            match own.map(|a| a.view()) {
                None => {}
                Some(ArgView::Expr { expr, .. }) if sect => {
                    ranges.push((ast.decls.get(q).loc(), ast.end_loc(expr)));
                }
                Some(_) => return Err(WHY),
            }
        }
        if ranges.is_empty() {
            return Ok(());
        }
        // Everything the defaults instantiated: at the specialization, or
        // inside a default's own text. A consumer rebuilds those from the
        // section, so each must be a class the record phase completes
        // without bodies (no static data members, no member functions).
        let at = ast.decls.get(d).loc();
        for x in
            self.instantiated_where(|l| l == at || ranges.iter().any(|&(b, e)| b <= l && l <= e))
        {
            let class_ok = ast.decls.get(x).kind() == DeclKind::CXXRecord
                && ast.members(x).all(|m| {
                    !matches!(ast.decls.get(m).kind(), DeclKind::Var)
                        && ast.decls.function(m).is_none()
                });
            if !class_ok {
                return Err(WHY);
            }
        }
        Ok(())
    }

    /// The implicit instantiations whose point of instantiation satisfies
    /// `at` (WP-1320; indexed on first use).
    fn instantiated_where(&self, at: impl Fn(kiln_source::Loc) -> bool) -> Vec<DeclId> {
        use kiln_ast::RawId as _;
        let ast = &self.s.ast;
        let map = self.poi.get_or_init(|| {
            let mut m: kiln_source::fx::FxHashMap<kiln_source::Loc, Vec<DeclId>> =
                kiln_source::fx::FxHashMap::default();
            for raw in 1..ast.decls.next_id().raw() {
                let Some(x) = DeclId::from_raw(raw) else { continue };
                if let Some((_, si)) = ast.spec_info(x)
                    && si.kind == SpecKind::ImplicitInstantiation
                    && si.point_of_instantiation.is_valid()
                {
                    m.entry(si.point_of_instantiation).or_default().push(x);
                }
            }
            m
        });
        map.iter().filter(|(l, _)| at(**l)).flat_map(|(_, v)| v.iter().copied()).collect()
    }

    /// Written argument `a` as the image carries it: a type or value as
    /// is; an expression that is a `bool` or `int` literal of its
    /// converted value's type, as that value (`converted`), which prints
    /// the same; `None` for any other expression.
    pub(super) fn written_arg(
        &self,
        a: TemplateArg,
        converted: Option<TemplateArg>,
    ) -> Option<TemplateArg> {
        let ArgView::Expr { expr, .. } = a.view() else { return Some(a) };
        let c = converted?;
        let ArgView::Value { ty, .. } = c.view() else { return None };
        let ast = &self.s.ast;
        let e = ast.exprs.get(expr);
        let literal = matches!(
            (e.kind(), ast.types.builtin(ty)),
            (ExprKind::CXXBoolLiteralExpr, Some(BuiltinType::Bool))
                | (ExprKind::IntegerLiteral, Some(BuiltinType::Int))
        );
        (literal && e.ty() == ty && ty.quals_bits() == 0).then_some(c)
    }

    /// The arguments of class template specialization type `X<args>`
    /// whose specialization is `spec` (`None` while dependent), each as
    /// [`Self::written_arg`] carries it.
    pub(super) fn tst_args(
        &mut self,
        args: kiln_ast::ArgListId,
        spec: Option<DeclId>,
    ) -> W<Vec<ArgRec>> {
        let ast = &self.s.ast;
        let written: Vec<TemplateArg> = ast.types.args(args).to_vec();
        let converted: Vec<TemplateArg> = match spec.and_then(|d| ast.spec_info(ast.first(d))) {
            Some((_, si)) => ast.types.args(si.args).to_vec(),
            None => Vec::new(),
        };
        let packs = converted.iter().any(|a| matches!(a.view(), ArgView::Pack(_)));
        let mut out = Vec::with_capacity(written.len());
        for (k, a) in written.into_iter().enumerate() {
            let c = if packs { None } else { converted.get(k).copied() };
            out.push(match (self.written_arg(a, c), a.view()) {
                (Some(a), _) => self.arg_rec(a)?,
                // WP-1300: any other expression as written.
                (None, ArgView::Expr { expr, profile }) => {
                    ArgRec::Expr(self.texpr(expr)?, profile.is_some())
                }
                (None, _) => return Err(super::spec::ARG_EXPR),
            });
        }
        Ok(out)
    }

    /// Partial specialization argument `a`, with head parameters `decls`
    /// (records `params`).
    fn partial_arg(&mut self, a: TemplateArg, decls: &[DeclId], params: &[u32]) -> W<CArg> {
        let ast = &self.s.ast;
        let ArgView::Expr { expr, profile } = a.view() else {
            return Ok(CArg::Arg(self.arg_rec(a)?));
        };
        let ex = &ast.exprs;
        let mut e = expr;
        let expanded = ex.get(e).kind() == ExprKind::PackExpansionExpr;
        if expanded {
            e = ex.operand(e);
        }
        if profile.is_none() || ex.get(e).kind() != ExprKind::DeclRefExpr {
            return Err(PARTIAL_ARG);
        }
        let p = ex.decl(e);
        let k = decls.iter().position(|&x| x == p).ok_or(PARTIAL_ARG)?;
        let pack = ast.template_parm(p).is_some_and(|t| t.pack);
        // As `template_param_arg` names it, with the parameter's type (a
        // pack is not carried yet).
        let pty = ast.decls.get(p).ty();
        if ast.decls.get(p).kind() != DeclKind::NonTypeTemplateParm {
            return Err(PARTIAL_ARG);
        }
        if pack || expanded || pty != Some(ex.get(e).ty()) {
            // WP-1344: a parameter pack's expansion (`_In...`) or a
            // converted parameter, as written: an expression section.
            let i = self.texpr(expr).map_err(|_| PARTIAL_ARG_EXPR)?;
            return Ok(CArg::Arg(ArgRec::Expr(i, true)));
        }
        Ok(CArg::Parm(params[k]))
    }
}

const MALFORMED: &str = "a malformed specialization";
const EXPLICIT_ARG: &str = "an explicit specialization argument the image does not carry";
const EXPLICIT_ARG_EXPR: &str = "an explicit specialization argument the image does not carry: an expression a section cannot hold";

fn encode_arg(w: &mut Wr, a: &CArg) {
    match a {
        CArg::Arg(ArgRec::Type(q)) => {
            w.u8(0);
            w.q(*q);
        }
        CArg::Arg(ArgRec::Value(q, v)) => {
            w.u8(1);
            w.q(*q);
            w.0.extend_from_slice(&v.to_le_bytes());
        }
        CArg::Parm(p) => {
            w.u8(2);
            w.u32(*p);
        }
        // An explicit specialization's argument as written (WP-1344):
        // its expression section and whether it has a profile.
        CArg::Arg(ArgRec::Expr(e, p)) => {
            w.u8(6);
            w.u32(*e);
            w.u8(u8::from(*p));
        }
        // Never written: a flat pack marker.
        CArg::Arg(ArgRec::Pack(_)) => w.u8(u8::MAX),
        // A template (WP-1342).
        CArg::Arg(ArgRec::Template(d, k)) => {
            w.u8(4);
            w.u32(*d);
            w.u32(*k);
        }
        // A dependent template name (WP-1342).
        CArg::Arg(ArgRec::DepTemplate(t, k)) => {
            w.u8(5);
            w.u32(*t);
            w.u32(*k);
        }
        CArg::Pack(v) => {
            w.u8(3);
            w.len(v.len());
            for x in v {
                encode_arg(w, x);
            }
        }
    }
}

fn decode_arg(r: &mut Rd, top: bool) -> Res<CArg> {
    Ok(match r.u8()? {
        0 => CArg::Arg(ArgRec::Type(r.q()?)),
        1 => CArg::Arg(ArgRec::Value(r.q()?, r.i128()?)),
        2 => CArg::Parm(r.u32()?),
        4 => CArg::Arg(ArgRec::Template(r.u32()?, r.u32()?)),
        5 => CArg::Arg(ArgRec::DepTemplate(r.u32()?, r.u32()?)),
        6 => {
            let e = r.u32()?;
            match r.u8()? {
                p @ 0..=1 => CArg::Arg(ArgRec::Expr(e, p != 0)),
                _ => return bad("bad specialization argument"),
            }
        }
        3 if top => {
            let n = r.count(5)?;
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(decode_arg(r, false)?);
            }
            CArg::Pack(v)
        }
        _ => return bad("bad specialization argument"),
    })
}

pub(super) fn encode_class_specs(w: &mut Wr, s: &ClassSpecs) {
    w.len(s.specs.len());
    for c in &s.specs {
        w.u32(c.rec);
        w.u32(c.template);
        match &c.head {
            None => w.u8(0),
            Some(h) => {
                w.u8(1);
                w.u32(h.tloc);
                w.u32(h.langle);
                w.u32(h.rangle);
                w.len(h.params.len());
                for &p in &h.params {
                    w.u32(p);
                }
            }
        }
        w.len(c.args.len());
        for a in &c.args {
            encode_arg(w, a);
        }
    }
}

pub(super) fn decode_class_specs(r: &mut Rd) -> Res<ClassSpecs> {
    let n = r.count(13)?;
    let mut specs = Vec::with_capacity(n);
    for _ in 0..n {
        let (rec, template) = (r.u32()?, r.u32()?);
        let head = match r.u8()? {
            0 => None,
            1 => {
                let (tloc, langle, rangle) = (r.u32()?, r.u32()?, r.u32()?);
                let k = r.count(4)?;
                let mut params = Vec::with_capacity(k);
                for _ in 0..k {
                    params.push(r.u32()?);
                }
                Some(PartialHead { tloc, langle, rangle, params })
            }
            _ => return bad("bad class template specialization"),
        };
        let k = r.count(5)?;
        let mut args = Vec::with_capacity(k);
        for _ in 0..k {
            args.push(decode_arg(r, true)?);
        }
        specs.push(ClassSpec { rec, template, head, args });
    }
    Ok(ClassSpecs { specs })
}

/// The section's locations, for the decoder's offset check.
pub(super) fn class_spec_locs(img: &Image, offs: &mut Vec<u32>) {
    for h in img.cspecs.specs.iter().filter_map(|c| c.head.as_ref()) {
        offs.extend([h.tloc, h.langle, h.rangle]);
    }
}

/// Checks the section and claims each partial specialization and its
/// parameters (dependent records a template would otherwise claim);
/// `qref_at(q, i)`: type `q` may be named by record `i`.
pub(super) fn validate_class_specs(
    img: &Image,
    qref_at: &dyn Fn(QRef, usize) -> bool,
    claimed: &mut [bool],
) -> Res<()> {
    let s = &img.cspecs.specs;
    if !s.windows(2).all(|w| w[0].rec < w[1].rec) {
        return bad("bad class template specialization");
    }
    for c in s {
        let (Some(r), Some(t)) =
            (img.decls.get(c.rec as usize), img.decls.get(c.template as usize))
        else {
            return bad("bad class template specialization");
        };
        let Payload::Template { kind: tkind @ (1 | 3), params: tparams, depth, .. } = &t.payload
        else {
            return bad("bad class template specialization");
        };
        let ok = match tkind {
            1 => matches!(r.payload, Payload::Record { .. }),
            _ => matches!(r.payload, Payload::Var { .. }),
        } && c.template < c.rec
            && r.name == t.name
            && r.ctx == t.ctx
            && r.prev == NONE
            && (r.flags & DEP != 0) == c.head.is_some()
            && !claimed[c.rec as usize];
        if !ok {
            return bad("bad class template specialization");
        }
        let at = c.rec as usize;
        let arg_ok = |a: &CArg, params: &[u32]| match a {
            CArg::Arg(ArgRec::Type(q)) => qref_at(*q, at),
            CArg::Arg(ArgRec::Value(q, _)) => super::spec::value_ty_ok(img, *q),
            CArg::Parm(p) => params.contains(p),
            CArg::Arg(ArgRec::Template(d, _)) => {
                (*d as usize) < at && super::tttp::tname_rec(img, *d)
            }
            CArg::Arg(ArgRec::DepTemplate(t, _)) => {
                qref_at(QRef { ty: *t, quals: 0 }, at) && super::tttp::dep_tname_ty(img, *t)
            }
            CArg::Pack(_) | CArg::Arg(ArgRec::Expr(..) | ArgRec::Pack(_)) => false,
        };
        // An explicit specialization's expression argument (WP-1344): a
        // section whose records and types are all before the record.
        let expr_ok = |a: &CArg| match a {
            CArg::Arg(ArgRec::Expr(e, _)) => img.texprs.get(*e as usize).is_some_and(|b| {
                super::body::section_decl_bound(b) <= at
                    && super::body::validate_section(img, b, 0, &|q| qref_at(q, at), true).is_ok()
            }),
            _ => arg_ok(a, &[]),
        };
        let Some(h) = &c.head else {
            if !c.args.iter().all(expr_ok) {
                return bad("bad class template specialization");
            }
            continue;
        };
        // At namespace scope, its parameters in the scope it is declared
        // in (as a primary template's): a reopening of its template's
        // namespace or an enclosing one.
        let scope = match r.lexical {
            NONE => r.ctx,
            LEX_TU => NONE,
            l if l < c.rec && matches!(img.decls[l as usize].payload, Payload::Namespace) => l,
            _ => return bad("bad class template specialization"),
        };
        for (k, &p) in h.params.iter().enumerate() {
            let ok = p < c.rec
                && !claimed[p as usize]
                && img.decls.get(p as usize).is_some_and(|x| {
                    x.ctx == scope
                        && match x.payload {
                            Payload::Ttp { depth: d, index, .. }
                            | Payload::Nttp { depth: d, index, .. } => {
                                d == *depth && usize::from(index) == k
                            }
                            Payload::Tttp(ref t) => t.depth == *depth && usize::from(t.index) == k,
                            _ => false,
                        }
                });
            if !ok {
                return bad("bad template parameter list");
            }
            claimed[p as usize] = true;
        }
        let nttp = |p: &u32| matches!(img.decls[*p as usize].payload, Payload::Nttp { .. });
        let params: Vec<u32> = h.params.iter().copied().filter(nttp).collect();
        let ok = c.args.len() == tparams.len()
            && c.args.iter().all(|a| match a {
                CArg::Pack(v) => v.iter().all(|x| arg_ok(x, &params) || expr_ok(x)),
                _ => arg_ok(a, &params) || expr_ok(a),
            });
        if !ok {
            return bad("bad class template specialization");
        }
        claimed[c.rec as usize] = true;
    }
    Ok(())
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Builds class specialization record `i` (header `h`, class-key
    /// `tag`), as the parser's declaration of it does.
    #[allow(clippy::too_many_arguments, reason = "the reader's tables and the record's head")]
    pub(super) fn image_class_spec(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        main: &Locs,
        i: u32,
        h: DeclHead,
        tag: TagKind,
    ) -> DeclId {
        let Some(c) = img.cspecs.get(i) else { return self.builder().record(h, tag, None) };
        let tmpl = self.ast.first(ids[c.template as usize]);
        let args = self.image_spec_args(img, tys, ids, c);
        if let Some(head) = &c.head {
            let list = self.image_partial_head(img, ids, main, c.template, head);
            let al = self.ast.types.arg_list(&args);
            return self.builder().class_partial_spec(h, tmpl, list, al, tag);
        }
        let locs = vec![h.loc; args.len()];
        let Ok(spec) = self.get_class_spec(tmpl, &args, &locs, h.loc) else {
            // Diagnosed: an unnamed class stands for it.
            let mut h = h;
            h.name = DeclName::EMPTY;
            return self.builder().record(h, tag, None);
        };
        if let Some((k, _)) = self.ast.spec_info(spec) {
            self.ast.templates.update_spec(k, |s| {
                s.kind = SpecKind::ExplicitSpecialization;
                s.from = spec;
            });
        }
        let w = self.ast.types.arg_list(&args);
        self.ast.templates.set_args_as_written(spec, w, true);
        if let Some(r) = self.ast.decls.record(spec)
            && r.tag != tag
            && (r.tag == TagKind::Union) == (tag == TagKind::Union)
            && self.ast.definition(spec).is_none()
        {
            self.ast.decls.set_definition_tag(spec, tag);
        }
        let lexical = h.lexical.unwrap_or(h.ctx);
        self.builder().declare_spec_in(spec, lexical, h.loc);
        if h.misc.access() != kiln_ast::decl::Access::None {
            self.ast.decls.set_access(spec, h.misc.access());
        }
        spec
    }

    /// Builds variable specialization record `i` (header `h`, type `ty`,
    /// specifiers `vspec`; WP-1307), as `act_on_var_spec_declarator`
    /// does; `None` when the record is no listed specialization.
    #[allow(clippy::too_many_arguments, reason = "the reader's tables and the record's head")]
    pub(super) fn image_var_spec(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        main: &Locs,
        i: u32,
        h: DeclHead,
        ty: QualType,
        vspec: kiln_ast::decl::VarSpec,
    ) -> Option<DeclId> {
        let c = img.cspecs.get(i)?;
        let tmpl = self.ast.first(ids[c.template as usize]);
        let args = self.image_spec_args(img, tys, ids, c);
        if let Some(head) = &c.head {
            let list = self.image_partial_head(img, ids, main, c.template, head);
            let al = self.ast.types.arg_list(&args);
            return Some(self.builder().var_partial_spec(h, tmpl, list, al, ty, vspec));
        }
        let locs = vec![h.loc; args.len()];
        let spec = self.find_or_create_var_spec(tmpl, &args, &locs, h.loc, ty, vspec).ok()?;
        if let Some((k, _)) = self.ast.spec_info(spec) {
            self.ast.templates.update_spec(k, |s| {
                s.kind = SpecKind::ExplicitSpecialization;
                s.from = spec;
            });
        }
        let lexical = h.lexical.unwrap_or(h.ctx);
        self.builder().declare_spec_in(spec, lexical, h.loc);
        Some(spec)
    }

    /// The arguments of listed specialization `c`.
    fn image_spec_args(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        c: &ClassSpec,
    ) -> Vec<TemplateArg> {
        let mut args = Vec::with_capacity(c.args.len());
        for a in &c.args {
            let x = match a {
                CArg::Pack(v) => {
                    let mut elems = Vec::with_capacity(v.len());
                    for x in v {
                        elems.push(self.image_carg(img, tys, ids, x));
                    }
                    TemplateArg::pack(self.ast.types.arg_list(&elems))
                }
                _ => self.image_carg(img, tys, ids, a),
            };
            args.push(x);
        }
        args
    }

    /// A partial specialization's template-head.
    fn image_partial_head(
        &mut self,
        img: &Image,
        ids: &[DeclId],
        main: &Locs,
        template: u32,
        head: &PartialHead,
    ) -> kiln_ast::TemplateParamListId {
        let ps: Vec<DeclId> = head.params.iter().map(|&p| ids[p as usize]).collect();
        let (t, l, r) = (main.get(head.tloc), main.get(head.langle), main.get(head.rangle));
        let depth = match img.decls[template as usize].payload {
            Payload::Template { depth, .. } => depth,
            _ => 0,
        };
        self.builder().template_param_list(t, l, r, depth, &ps, None)
    }

    fn image_carg(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        a: &CArg,
    ) -> TemplateArg {
        match a {
            // An explicit specialization's expression argument (WP-1344):
            // its section, built now.
            CArg::Arg(ArgRec::Expr(i, p)) => {
                let _ = self.image_record_section(img, tys, ids, *i);
                self.image_expr_arg(*i, *p)
            }
            CArg::Arg(x) => {
                let ty = self.image_ty(img, tys, ids, x.q());
                self.image_targ(ids, ty, x)
            }
            CArg::Parm(p) => self.template_param_arg(ids[*p as usize]),
            CArg::Pack(_) => TemplateArg::NULL,
        }
    }
}
