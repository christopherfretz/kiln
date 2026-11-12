//! Requires-expressions in the declaration image (spec/modules.md §11.11,
//! "Constraints", WP-1284).
//!
//! A `RequiresExpr` is a body expression: [`F::RB`] names its body, a
//! record of the [`ReqBodies`] section, and [`F::Reqs`] lists its
//! requirements. The body is no declaration record (clang lists it in no
//! context): the section carries its context, location and parameters,
//! and the invented `expr-type` parameters of its return-type
//! requirements ([expr.prim.req.compound]/1). The importer declares them
//! before it builds the bodies, so the requirements' references to the
//! parameters ([`REQP`]) and the immediately-declared constraints' first
//! arguments (a template argument of kind 2) resolve to them.
//!
//! Only a dependent requires-expression is carried. A non-dependent one
//! was evaluated by the unit ([expr.prim.req.general]/5) and one whose
//! access checks wait for an instantiation keeps Sema state the image
//! does not carry (`dependent_access`); both fall back.
//!
//! [`F::RB`]: super::body::F::RB
//! [`F::Reqs`]: super::body::F::Reqs

use kiln_ast::decl::{DeclKind, ParamData};
use kiln_ast::templates::Requirement;
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_source::fx::FxHashMap;

use super::body::{Built, Dc};
use super::{
    DECL_FLAGS, DEP, DeclHead, DeclName, Image, LEX_TU, NONE, ParamRec, QRef, Rd, Res, W, Wr,
    Writer, bad,
};
use crate::Sema;

/// A requires-expression parameter in a declaration field:
/// `PARAM | REQP | i` names the section's `i`-th parameter, counting the
/// bodies' parameters in order.
pub(super) const REQP: u32 = 0x4000_0000;

/// Requirement kinds in an [`F::Reqs`](super::body::F::Reqs) token.
const R_SIMPLE: u32 = 0;
const R_TYPE: u32 = 1;
const R_COMPOUND: u32 = 2;
const R_NESTED: u32 = 3;
/// A compound requirement with `noexcept`.
const R_COMPOUND_NOEXCEPT: u32 = 4;

/// The words of a requirement of kind `k` (`None`: no such kind).
pub(super) fn req_words(k: u32) -> Option<usize> {
    match k {
        R_SIMPLE | R_NESTED => Some(2),
        R_TYPE | R_COMPOUND | R_COMPOUND_NOEXCEPT => Some(4),
        _ => None,
    }
}

/// The requirements of an [`F::Reqs`](super::body::F::Reqs) token
/// (`w`: the body, the count, then the requirements), each with its
/// words.
pub(super) fn reqs(w: &[u32]) -> impl Iterator<Item = &[u32]> {
    let mut p = 2usize;
    std::iter::from_fn(move || {
        let n = req_words(*w.get(p)?)?;
        let r = w.get(p..p + n)?;
        p += n;
        Some(r)
    })
}

/// A requires-expression's body.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct ReqBodyRec {
    /// Its context: a declaration record or [`LEX_TU`].
    ctx: u32,
    loc: u32,
    /// [`DEP`] inside a template (its parameters carry their own).
    flags: u8,
    params: Vec<ParamRec>,
    /// The invented `expr-type` parameters: depth and location.
    invented: Vec<(u16, u32)>,
}

/// The image's requires-expression bodies.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct ReqBodies {
    pub(super) recs: Vec<ReqBodyRec>,
    /// The parameters of all bodies (derived, not encoded).
    pub(super) nparams: usize,
}

/// The writer's requires-expression state.
#[derive(Default)]
pub(super) struct ReqState {
    /// Body declaration → section record.
    bodies: FxHashMap<DeclId, u32>,
    /// Parameter → section parameter index.
    pub(super) params: FxHashMap<DeclId, u32>,
    /// Invented parameter → (section record, position).
    invented: FxHashMap<DeclId, (u32, u32)>,
    /// Body declaration → requires-expression record (built on first use).
    index: Option<FxHashMap<DeclId, u32>>,
}

impl ReqState {
    /// Drops the bodies from record `recs` and the parameters from index
    /// `nparams` on (WP-1358, a record written again).
    pub(super) fn truncate(&mut self, recs: u32, nparams: u32) {
        self.bodies.retain(|_, r| *r < recs);
        self.params.retain(|_, k| *k < nparams);
        self.invented.retain(|_, (r, _)| *r < recs);
    }
}

impl Writer<'_, '_, '_> {
    /// Why requires-expression `e` falls back, if it does.
    pub(super) fn requires_ok(&self, e: ExprId) -> W<()> {
        let s = self.s;
        let n = s.ast.exprs.get(e);
        if n.deps().is_none() || s.inst.concepts.expr_sat.contains_key(&e) {
            return Err("a checked requires-expression");
        }
        let rq = s.ast.requires_of(e).ok_or("a malformed requires-expression")?;
        if s.requires_access_pending(rq.body) {
            return Err("a requires-expression with a pending access check");
        }
        Ok(())
    }

    /// The body of requires-expression parameter `d`, if it is one.
    pub(super) fn req_param_body(&self, d: DeclId) -> Option<DeclId> {
        let decls = &self.s.ast.decls;
        let n = decls.get(d);
        let c = n.ctx()?;
        (n.kind() == DeclKind::ParmVar && decls.get(c).kind() == DeclKind::RequiresExprBody)
            .then_some(c)
    }

    /// The word for requires-expression parameter `d` (in body `body`).
    pub(super) fn req_param(&mut self, d: DeclId, body: DeclId) -> W<u32> {
        self.req_body(body)?;
        let p = *self.bs.req.params.get(&d).ok_or(super::outside::OUT_REQ_PARAM)?;
        Ok(super::body::PARAM | REQP | p)
    }

    /// The section record of `body`, added on first use.
    pub(super) fn req_body(&mut self, body: DeclId) -> W<u32> {
        if self.bs.texpr > 0 {
            // WP-1300: a requires-expression in a type stays refused.
            return Err("a requires-expression in a type");
        }
        if let Some(&i) = self.bs.req.bodies.get(&body) {
            return Ok(i);
        }
        let rq = self.requires_index(body)?;
        let ast = &self.s.ast;
        let n = ast.decls.get(body);
        if n.kind() != DeclKind::RequiresExprBody || n.flags().bits() & !DEP != 0 {
            return Err("a requires-expression body with flags the image does not carry");
        }
        let flags = n.flags().bits() & DEP;
        let rqd = *ast.templates.requires_data(rq).ok_or("a malformed requires-expression")?;
        let params: Vec<DeclId> = ast.lists.iter::<DeclId>(rqd.params).collect();
        for &p in &params {
            let pd = ast.decls.param(p).ok_or("a malformed requires-expression")?;
            let pn = ast.decls.get(p);
            if pd.default_arg.is_some() || pn.ctx() != Some(body) {
                return Err("a requires-expression parameter the image does not carry");
            }
        }
        let loc = self.loc(n.loc())?;
        let ctx = n.ctx().ok_or("a malformed requires-expression")?;
        let ctx = self.dref(ctx, Dc::Ctx)?;
        let base = self.img.reqs.nparams as u32;
        let mut ps = Vec::with_capacity(params.len());
        for (k, &p) in params.iter().enumerate() {
            let pr = self.param(p, base + k as u32)?;
            if pr.orig.is_some() {
                // Not in this section's encoding.
                return Err("a parameter with an adjusted type or a pack");
            }
            ps.push(pr);
        }
        let i = self.img.reqs.recs.len() as u32;
        for (k, &p) in params.iter().enumerate() {
            if self.bs.req.params.insert(p, base + k as u32).is_some() {
                return Err("a parameter shared between declarations");
            }
        }
        self.img.reqs.nparams += ps.len();
        self.img.reqs.recs.push(ReqBodyRec { ctx, loc, flags, params: ps, invented: Vec::new() });
        self.bs.req.bodies.insert(body, i);
        Ok(i)
    }

    /// The requires-expression record of `body`.
    fn requires_index(&mut self, body: DeclId) -> W<u32> {
        let t = &self.s.ast.templates;
        // A substituted requires-expression keeps its pattern's body with
        // parameters of its own (`requires_expr` in templ/concepts): the
        // body's record is its first, the parsed one (WP-1360).
        let map = self.bs.req.index.get_or_insert_with(|| {
            let mut m = FxHashMap::default();
            for i in 0..t.requires_len() {
                if let Some(r) = t.requires_data(i) {
                    let _ = m.entry(r.body).or_insert(i);
                }
            }
            m
        });
        map.get(&body).copied().ok_or("a malformed requires-expression")
    }

    /// A template argument naming type `q`, when `q` is the invented
    /// parameter of a return-type requirement: its (body, position).
    pub(super) fn invented_arg(&mut self, q: QualType) -> W<Option<(u32, u32)>> {
        let ast = &self.s.ast;
        let TypeView::TemplateTypeParm(p) = ast.types.view(q.ty()) else { return Ok(None) };
        let n = ast.decls.get(p);
        let Some(body) = n.ctx().filter(|&c| ast.decls.get(c).kind() == DeclKind::RequiresExprBody)
        else {
            return Ok(None);
        };
        if let Some(&x) = self.bs.req.invented.get(&p) {
            return Ok(Some(x));
        }
        let info = *ast.template_parm(p).ok_or("a malformed requires-expression")?;
        if q.quals_bits() != 0
            || info.index != 0
            || info.pack
            || info.typename_kw
            || info.constraint.is_some()
            || info.params.is_some()
            || n.ty() != Some(QualType::unqual(q.ty()))
            || self.ident(n.name()).as_deref() != Some(INVENTED)
        {
            return Err("a requires-expression the image does not carry");
        }
        let (loc, flags) = (self.loc(n.loc())?, n.flags().bits());
        let rb = self.req_body(body)?;
        if flags != self.img.reqs.recs[rb as usize].flags {
            return Err("a requires-expression the image does not carry");
        }
        let inv = &mut self.img.reqs.recs[rb as usize].invented;
        let k = inv.len() as u32;
        inv.push((info.depth, loc));
        self.bs.req.invented.insert(p, (rb, k));
        Ok(Some((rb, k)))
    }

    /// The words of requires-expression record `i` (an
    /// [`F::Reqs`](super::body::F::Reqs) token).
    pub(super) fn reqs_words(&mut self, i: u32, out: &mut Vec<u32>) -> W<()> {
        let ast = &self.s.ast;
        let rq = *ast.templates.requires_data(i).ok_or("a malformed requires-expression")?;
        let items: Vec<Requirement> = ast.templates.requirements(&rq).to_vec();
        let rb = self.req_body(rq.body)?;
        out.extend([rb, items.len() as u32]);
        for r in items {
            match r {
                Requirement::Simple(e) => out.extend([R_SIMPLE, self.expr_word(e)?]),
                Requirement::Nested(e) => out.extend([R_NESTED, self.expr_word(e)?]),
                Requirement::Type(t, at) => {
                    let q = self.qref(t)?;
                    out.extend([R_TYPE, q.ty, u32::from(q.quals), self.loc(at)?]);
                }
                Requirement::Compound { expr, noexcept, constraint } => {
                    let (k, l) = match noexcept {
                        Some(l) => (R_COMPOUND_NOEXCEPT, self.loc(l)?),
                        None => (R_COMPOUND, 0),
                    };
                    let c = match constraint {
                        Some(c) => self.expr_word(c)?,
                        None => NONE,
                    };
                    out.extend([k, self.expr_word(expr)?, l, c]);
                }
            }
        }
        Ok(())
    }

    /// The operand expressions of requires-expression record `i`, which
    /// the walk writes first.
    pub(super) fn reqs_kids(&self, i: u32) -> Vec<ExprId> {
        let t = &self.s.ast.templates;
        let Some(rq) = t.requires_data(i) else { return Vec::new() };
        let mut v = Vec::new();
        for r in t.requirements(rq) {
            match *r {
                Requirement::Simple(e) | Requirement::Nested(e) => v.push(e),
                Requirement::Compound { expr, constraint, .. } => {
                    v.push(expr);
                    v.extend(constraint);
                }
                Requirement::Type(..) => {}
            }
        }
        v
    }
}

/// The name clang gives the invented parameter.
const INVENTED: &str = "expr-type";

pub(super) fn encode_requires(w: &mut Wr, r: &ReqBodies) {
    w.len(r.recs.len());
    for b in &r.recs {
        w.u32(b.ctx);
        w.u32(b.loc);
        w.u8(b.flags);
        w.len(b.params.len());
        for p in &b.params {
            w.u32(p.name);
            w.u32(p.loc);
            w.q(p.ty);
            w.u16(p.pflags);
            w.u8(p.flags);
        }
        w.len(b.invented.len());
        for &(depth, loc) in &b.invented {
            w.u16(depth);
            w.u32(loc);
        }
    }
}

pub(super) fn decode_requires(r: &mut Rd<'_>) -> Res<ReqBodies> {
    let n = r.count(17)?;
    let mut out = ReqBodies { recs: Vec::with_capacity(n), nparams: 0 };
    for _ in 0..n {
        let (ctx, loc, flags) = (r.u32()?, r.u32()?, r.u8()?);
        let np = r.count(16)?;
        let mut params = Vec::with_capacity(np);
        for _ in 0..np {
            params.push(ParamRec {
                name: r.u32()?,
                loc: r.u32()?,
                ty: r.q()?,
                pflags: r.u16()?,
                flags: r.u8()?,
                default: NONE,
                orig: None,
            });
        }
        let ni = r.count(6)?;
        let mut invented = Vec::with_capacity(ni);
        for _ in 0..ni {
            invented.push((r.u16()?, r.u32()?));
        }
        out.nparams += params.len();
        out.recs.push(ReqBodyRec { ctx, loc, flags, params, invented });
    }
    Ok(out)
}

/// Checks the requires-expression bodies: each in a context the image
/// carries, with well-formed parameters.
pub(super) fn validate_requires(img: &Image, qref_ok: &dyn Fn(QRef) -> bool) -> Res<()> {
    let r = &img.reqs;
    if r.nparams != r.recs.iter().map(|b| b.params.len()).sum::<usize>() {
        return bad("bad requires-expression parameters");
    }
    let nparams = super::body::param_count(img);
    for b in &r.recs {
        if !super::body::dc_ok(img, nparams, b.ctx, Dc::Ctx)
            || b.flags & !DEP != 0
            || b.params.len() > usize::from(u16::MAX)
        {
            return bad("bad requires-expression body");
        }
        for p in &b.params {
            let pack = if p.flags & DEP != 0 { ParamData::PACK } else { 0 };
            if (p.name != NONE && p.name as usize >= img.pnames.len())
                || p.flags & !(DECL_FLAGS | DEP) != 0
                || p.pflags & !(ParamData::SCOPE_DEPTH_MASK | pack) != 0
                || !qref_ok(p.ty)
            {
                return bad("bad requires-expression parameter");
            }
        }
    }
    Ok(())
}

/// Every main-file offset the bodies section holds.
pub(super) fn requires_locs(img: &Image, out: &mut Vec<u32>) {
    for b in &img.reqs.recs {
        out.push(b.loc);
        out.extend(b.params.iter().map(|p| p.loc));
        out.extend(b.invented.iter().map(|x| x.1));
    }
}

/// A requires-expression body as the importer declared it.
#[derive(Default)]
pub(super) struct ReqBuilt {
    body: Option<DeclId>,
    params: Vec<DeclId>,
    invented: Vec<QualType>,
}

/// The importer's requires-expression bodies.
#[derive(Default)]
pub(super) struct ReqsBuilt {
    bodies: Vec<ReqBuilt>,
    /// The parameters in section order.
    pub(super) params: Vec<DeclId>,
}

impl ReqsBuilt {
    /// The body declaration of section record `i`.
    pub(super) fn body(&self, i: u32) -> Option<DeclId> {
        self.bodies.get(i as usize).and_then(|b| b.body)
    }

    /// The invented parameter type `k` of section record `i`.
    pub(super) fn invented(&self, i: u32, k: u32) -> QualType {
        self.bodies
            .get(i as usize)
            .and_then(|b| b.invented.get(k as usize).copied())
            .unwrap_or(QualType::ERROR)
    }
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Declares the requires-expression bodies of `img`, their
    /// parameters and invented parameters, as the parser does
    /// (`act_on_requires_expr_start`, `act_on_requires_param`,
    /// `act_on_return_type_requirement`), before the bodies are built.
    pub(super) fn image_requires(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        main: &super::Locs,
    ) -> ReqsBuilt {
        let mut out = ReqsBuilt::default();
        let invented = DeclName::ident(self.intern(INVENTED));
        for b in &img.reqs.recs {
            let ctx =
                if b.ctx == LEX_TU { Some(self.ast.tu) } else { ids.get(b.ctx as usize).copied() };
            let Some(ctx) = ctx else {
                out.bodies.push(ReqBuilt::default());
                continue;
            };
            let saved_depth = self.ast.templates.dependent_depth;
            self.ast.templates.dependent_depth = saved_depth + u32::from(b.flags != 0);
            let body = self.builder().requires_body(ctx, main.get(b.loc));
            let mut ps = Vec::with_capacity(b.params.len());
            for (k, p) in b.params.iter().enumerate() {
                let pty = self.image_ty(img, tys, ids, p.ty);
                let pname = if p.name == NONE {
                    DeclName::EMPTY
                } else {
                    img.pnames
                        .get(p.name as usize)
                        .map_or(DeclName::EMPTY, |n| DeclName::ident(self.intern(n)))
                };
                let index = u16::try_from(k).unwrap_or(u16::MAX);
                self.ast.templates.dependent_depth = saved_depth + u32::from(p.flags & DEP != 0);
                let pd = self.builder().param(body, pname, main.get(p.loc), pty, index, p.pflags);
                super::image_flags(self, pd, p.flags);
                ps.push(pd);
            }
            self.ast.templates.dependent_depth = saved_depth + u32::from(b.flags != 0);
            let mut inv = Vec::with_capacity(b.invented.len());
            for &(depth, loc) in &b.invented {
                let h = DeclHead::new(body, invented, main.get(loc));
                let t = self.builder().template_type_parm(h, depth, 0, false, false);
                inv.push(self.ast.decls.get(t).ty().unwrap_or(QualType::ERROR));
            }
            self.ast.templates.dependent_depth = saved_depth;
            out.params.extend_from_slice(&ps);
            out.bodies.push(ReqBuilt { body: Some(body), params: ps, invented: inv });
        }
        out
    }

    /// Requires-expression record for an [`F::Reqs`](super::body::F::Reqs)
    /// token `w` (validated); its index.
    #[allow(clippy::too_many_arguments)] // the importer's tables
    pub(super) fn image_reqs(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        built: &Built,
        main: &super::Locs,
        w: &[u32],
    ) -> u32 {
        let (body, params) = match built.reqs.bodies.get(w[0] as usize) {
            Some(ReqBuilt { body: Some(b), params, .. }) => (*b, params.clone()),
            _ => return 0,
        };
        let opt = |x: u32| (x != NONE).then(|| built.expr(x));
        let mut items = Vec::new();
        for r in reqs(w) {
            items.push(match r[0] {
                R_SIMPLE => Requirement::Simple(built.expr(r[1])),
                R_NESTED => Requirement::Nested(built.expr(r[1])),
                R_TYPE => {
                    let t = self.image_ty(img, tys, ids, QRef { ty: r[1], quals: r[2] as u8 });
                    Requirement::Type(t, main.get(r[3]))
                }
                k => Requirement::Compound {
                    expr: built.expr(r[1]),
                    noexcept: (k == R_COMPOUND_NOEXCEPT).then(|| main.get(r[2])),
                    constraint: opt(r[3]),
                },
            });
        }
        self.builder().requires_record(body, &params, &items)
    }
}

/// Checks an [`F::Reqs`](super::body::F::Reqs) token (`e_lim` bounds its
/// operands).
pub(super) fn reqs_ok(
    img: &Image,
    w: &[u32],
    e_lim: usize,
    qref_ok: &dyn Fn(QRef) -> bool,
) -> bool {
    let e = |x: u32| (x as usize) < e_lim;
    (w[0] as usize) < img.reqs.recs.len()
        && reqs(w).all(|r| match r[0] {
            R_SIMPLE | R_NESTED => e(r[1]),
            R_TYPE => r[2] <= 7 && qref_ok(QRef { ty: r[1], quals: r[2] as u8 }),
            k => e(r[1]) && (k == R_COMPOUND_NOEXCEPT || r[2] == 0) && (r[3] == NONE || e(r[3])),
        })
}

/// Whether invented-parameter argument (`rb`, `k`) names one.
pub(super) fn invented_ok(img: &Image, rb: u32, k: u32) -> bool {
    img.reqs.recs.get(rb as usize).is_some_and(|b| (k as usize) < b.invented.len())
}

/// The main-file offsets an [`F::Reqs`](super::body::F::Reqs) token holds.
pub(super) fn reqs_locs(w: &[u32], out: &mut Vec<u32>) {
    for r in reqs(w) {
        match r[0] {
            R_TYPE => out.push(r[3]),
            R_COMPOUND_NOEXCEPT => out.push(r[2]),
            _ => {}
        }
    }
}
