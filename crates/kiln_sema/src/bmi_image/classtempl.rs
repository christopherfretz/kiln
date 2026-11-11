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

use super::{
    ArgRec, BUILTINS, DEP, Image, LEX_TU, Locs, NONE, Payload, QRef, Rd, Res, W, Wr, Writer, bad,
};
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

/// The template record of `rec` when it is a carried explicit
/// specialization.
pub(super) fn explicit_of(img: &Image, rec: u32) -> Option<u32> {
    img.cspecs.get(rec).filter(|c| c.head.is_none()).map(|c| c.template)
}

const PARTIAL_ARG: &str = "a partial specialization argument the image does not carry";

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
        let entry = if let Some((_, p)) = ast.partial_spec_info(d) {
            let p = *p;
            let list = *ast.templates.param_list(p.params);
            let n = ast.decls.get(d);
            let in_class = n.ctx().is_some_and(|c| ast.decls.record(c).is_some());
            if list.depth != 0 || in_class {
                return Err("a member template or a template in a template");
            }
            // Declared, then defined: the definition's template-head
            // has parameters of its own, after the record.
            if self.s.templ.partial_def_loc.contains_key(&d) {
                return Err("a redeclared partial specialization");
            }
            if list.requires.is_some() {
                return Err("a constrained template");
            }
            let template = self.template_rec(p.template, 1)?;
            let mut params = Vec::new();
            let mut decls = Vec::new();
            for q in ast.lists.iter::<DeclId>(list.params) {
                params.push(
                    *self.recs.get(&q).ok_or("a template parameter the image does not carry")?,
                );
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
            let template = self.template_rec(si.template, 1)?;
            let written = ast.templates.args_as_written(d).ok_or(MALFORMED)?;
            let written: Vec<TemplateArg> = ast.types.args(written).to_vec();
            let converted: Vec<TemplateArg> = ast.types.args(si.args).to_vec();
            let packs = converted.iter().any(|a| matches!(a.view(), ArgView::Pack(_)));
            // The parameters the written arguments leave to their
            // defaults: a non-type parameter's default is built after
            // the records, too late for `get_class_spec`.
            let info = ast.template_info(ast.first(si.template)).ok_or(MALFORMED)?;
            let plist = ast.templates.param_list(info.params).params;
            let defaulted = ast.lists.iter::<DeclId>(plist).skip(written.len()).any(|q| {
                ast.decls.get(q).kind() != DeclKind::TemplateTypeParm
                    && ast.templates.defaults.get(q).is_some()
            });
            if defaulted {
                return Err("an explicit specialization using a non-type default argument");
            }
            let mut out = Vec::with_capacity(written.len());
            for (k, a) in written.into_iter().enumerate() {
                // `X<3>`: the converted value, which prints the same.
                let c = if packs { None } else { converted.get(k).copied() };
                let a = self.written_arg(a, c).ok_or(EXPLICIT_ARG)?;
                out.push(CArg::Arg(self.arg_rec(a).map_err(|_| EXPLICIT_ARG)?));
            }
            ClassSpec { rec: i, template, head: None, args: out }
        };
        self.img.cspecs.specs.push(entry);
        Ok(())
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
            let a = self.written_arg(a, c).ok_or(super::spec::ARG)?;
            out.push(self.arg_rec(a)?);
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
        if ast.decls.get(p).kind() != DeclKind::NonTypeTemplateParm
            || pack
            || expanded
            || pty != Some(ex.get(e).ty())
        {
            return Err(PARTIAL_ARG);
        }
        Ok(CArg::Parm(params[k]))
    }
}

const MALFORMED: &str = "a malformed specialization";
const EXPLICIT_ARG: &str = "an explicit specialization argument the image does not carry";

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
        let Payload::Template { kind: 1, params: tparams, .. } = &t.payload else {
            return bad("bad class template specialization");
        };
        let ok = matches!(r.payload, Payload::Record { .. })
            && c.template < c.rec
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
            CArg::Arg(ArgRec::Value(q, _)) => q.ty <= BUILTINS && q.quals == 0,
            CArg::Parm(p) => params.contains(p),
            CArg::Pack(_) => false,
        };
        let Some(h) = &c.head else {
            if !c.args.iter().all(|a| arg_ok(a, &[])) {
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
                            Payload::Ttp { depth: 0, index, .. }
                            | Payload::Nttp { depth: 0, index, .. } => usize::from(index) == k,
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
                CArg::Pack(v) => v.iter().all(|x| arg_ok(x, &params)),
                _ => arg_ok(a, &params),
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
        if let Some(head) = &c.head {
            let ps: Vec<DeclId> = head.params.iter().map(|&p| ids[p as usize]).collect();
            let (t, l, r) = (main.get(head.tloc), main.get(head.langle), main.get(head.rangle));
            let list = self.builder().template_param_list(t, l, r, 0, &ps, None);
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

    fn image_carg(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        a: &CArg,
    ) -> TemplateArg {
        match a {
            CArg::Arg(x) => {
                let ty = self.image_ty(img, tys, ids, x.q());
                self.image_arg(ty, x)
            }
            CArg::Parm(p) => self.template_param_arg(ids[*p as usize]),
            CArg::Pack(_) => TemplateArg::NULL,
        }
    }
}
