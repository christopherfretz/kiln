//! Expressions in types and template arguments (spec/modules.md §11.11,
//! "Expressions in types", WP-1300).
//!
//! A type or template argument that holds an expression carries it as an
//! *expression section*: a [`Bodies`] of its own (expressions, constants
//! and strings, no statements) whose last expression is the root,
//! written by [`Writer::texpr_section`] and indexed by `Image::texprs`.
//! The forms:
//!
//! - `decltype(e)` ([dcl.type.decltype]): the section, the computed type
//!   (the `Dependent` builtin when it cannot be computed) and whether it
//!   is instantiation-dependent, then canonically `DependentDecltype` of
//!   the expression's profile ([temp.type]/4);
//! - `T[e]` of a value-dependent bound ([temp.dep.constexpr]);
//! - `P...` of a known expansion count ([temp.variadic]);
//! - `noexcept(e)` on a function type whose operand is no `bool` literal
//!   ([except.spec]/2): the function type's exception words 7 to 9
//!   (false, true, dependent) and the section; when the operand names
//!   the function's own parameters, the function's type is carried with
//!   no exception specification and a [`LateNx`] gives the words and
//!   the operand in the image's bodies, set on the function once they
//!   are built;
//! - a template argument written as an expression ([temp.arg.nontype])
//!   that is not a literal of its converted value: the section and
//!   whether it has a profile (a dependent argument's).
//!
//! The importer builds a section when it builds the first type naming it,
//! before the declaration records that use the type (the validator's
//! per-type record bound covers every record a section names) and before
//! the image's bodies, and recomputes every profile as replay does
//! (`dependent_profile` for a type's operand, `profile_expr` for an
//! argument's). A section may not name a function or requires-expression
//! parameter or a local declaration: the importer builds types before
//! them (such a type keeps falling back).

use kiln_ast::types::{
    BuiltinType, ExcSpec, ExprOperand, FuncProto, NoexceptValue, TemplateArg, TypeKind, TypeView,
    VectorKind,
};
use kiln_ast::{DeclId, ExprId, QualType, TypeId};

use super::body::{self, Bodies};
use super::deptypes::DepTy;
use super::{ArgRec, Image, NONE, QRef, Rd, Res, TyRec, W, Wr, Writer, bad};
use crate::Sema;

/// A function type's exception words for `noexcept(e)` of a non-literal
/// operand: `noexcept(false)`, `noexcept(true)`, value-dependent.
pub(super) const NX_FALSE: u8 = 7;
pub(super) const NX_TRUE: u8 = 8;
pub(super) const NX_DEPENDENT: u8 = 9;

/// The type an expression argument reports to the shared type checks:
/// a builtin (the argument's own type is its expression's).
pub(super) const EXPR_ARG_TY: QRef = QRef { ty: BuiltinType::Int as u32, quals: 0 };

/// The `Dependent` builtin, the computed type of a type-dependent
/// `decltype` operand.
const DEPENDENT_TY: QRef = QRef { ty: BuiltinType::Dependent as u32, quals: 0 };

/// A type of this module.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum ExTy {
    /// `decltype(e)`: the section, the computed type, whether it has a
    /// profile; `canon` for the canonical `DependentDecltype` itself
    /// (written through the profile's first expression).
    Decltype { e: u32, under: QRef, dep: bool, canon: bool },
    /// `T[e]` of a value-dependent bound; `canon` for the canonical form
    /// (the bound's profile).
    DepArray { elem: QRef, e: u32, canon: bool },
    /// `P...` expanding to `n` elements.
    PackN { pattern: QRef, n: u32 },
    /// A vector of a value-dependent element count (WP-1350:
    /// `__attribute__((ext_vector_type(N))) T`), `kind` its
    /// `VectorKind`; `canon` as for [`ExTy::DepArray`].
    DepVector { elem: QRef, e: u32, kind: u8, canon: bool },
}

pub(super) fn encode_ex(w: &mut Wr, x: &ExTy) {
    match x {
        ExTy::Decltype { e, under, dep, canon } => {
            w.u8(0);
            w.u32(*e);
            w.q(*under);
            w.u8(u8::from(*dep) | u8::from(*canon) << 1);
        }
        ExTy::DepArray { elem, e, canon } => {
            w.u8(1);
            w.q(*elem);
            w.u32(*e);
            w.u8(u8::from(*canon));
        }
        ExTy::PackN { pattern, n } => {
            w.u8(2);
            w.q(*pattern);
            w.u32(*n);
        }
        ExTy::DepVector { elem, e, kind, canon } => {
            w.u8(3);
            w.q(*elem);
            w.u32(*e);
            w.u8(*kind);
            w.u8(u8::from(*canon));
        }
    }
}

pub(super) fn decode_ex(r: &mut Rd<'_>) -> Res<ExTy> {
    Ok(match r.u8()? {
        0 => {
            let (e, under) = (r.u32()?, r.q()?);
            match r.u8()? {
                f @ (0 | 1 | 3) => ExTy::Decltype { e, under, dep: f & 1 != 0, canon: f & 2 != 0 },
                _ => return bad("bad decltype type"),
            }
        }
        1 => {
            let (elem, e) = (r.q()?, r.u32()?);
            match r.u8()? {
                c @ 0..=1 => ExTy::DepArray { elem, e, canon: c != 0 },
                _ => return bad("bad dependent array type"),
            }
        }
        2 => ExTy::PackN { pattern: r.q()?, n: r.u32()? },
        3 => {
            let (elem, e, kind) = (r.q()?, r.u32()?, r.u8()?);
            if VectorKind::from_u8(kind) as u8 != kind {
                return bad("bad dependent vector type");
            }
            match r.u8()? {
                c @ 0..=1 => ExTy::DepVector { elem, e, kind, canon: c != 0 },
                _ => return bad("bad dependent vector type"),
            }
        }
        _ => return bad("bad expression type"),
    })
}

/// The types `x` names.
pub(super) fn ex_refs(x: &ExTy) -> Vec<QRef> {
    match x {
        ExTy::Decltype { under, .. } => vec![*under],
        ExTy::DepArray { elem, .. } | ExTy::DepVector { elem, .. } => vec![*elem],
        ExTy::PackN { pattern, .. } => vec![*pattern],
    }
}

/// A function's `noexcept(e)` whose operand names the function's own
/// parameters ([except.spec]/2, [basic.scope.param]): function record
/// `rec`, exception words `exc` (7 to 9) and the operand's expression
/// record `expr` in the image's bodies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct LateNx {
    pub(super) rec: u32,
    pub(super) exc: u8,
    pub(super) expr: u32,
}

pub(super) fn encode_texprs(w: &mut Wr, t: &[Bodies], late: &[LateNx]) {
    w.len(t.len());
    for b in t {
        body::encode_bodies(w, b);
    }
    w.len(late.len());
    for l in late {
        w.u32(l.rec);
        w.u8(l.exc);
        w.u32(l.expr);
    }
}

pub(super) fn decode_texprs(r: &mut Rd<'_>) -> Res<(Vec<Bodies>, Vec<LateNx>)> {
    // A section is at least its four counts.
    let n = r.count(16)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(body::decode_bodies(r)?);
    }
    let n = r.count(9)?;
    let mut late = Vec::with_capacity(n);
    for _ in 0..n {
        let (rec, exc, expr) = (r.u32()?, r.u8()?, r.u32()?);
        late.push(LateNx { rec, exc, expr });
    }
    Ok((v, late))
}

/// Each [`LateNx`] names a distinct function record not deferred whose
/// type is a function type with no exception specification, words 7 to
/// 9, and an expression of the bodies.
pub(super) fn validate_late_nx(img: &Image) -> Res<()> {
    let mut seen = kiln_source::fx::FxHashSet::default();
    for l in &img.late_nx {
        let ok = (NX_FALSE..=NX_DEPENDENT).contains(&l.exc)
            && (l.expr as usize) < img.bodies.exprs.len()
            && seen.insert(l.rec)
            && match img.decls.get(l.rec as usize).map(|d| &d.payload) {
                Some(super::Payload::Function { ty, deferred: false, .. }) => {
                    ty.quals == 0
                        && ty.ty > super::BUILTINS
                        && matches!(
                            img.types.get((ty.ty - super::BUILTINS - 1) as usize),
                            Some(TyRec::Func { exc: 0, .. })
                        )
                }
                _ => false,
            };
        if !ok {
            return bad("bad late exception specification");
        }
    }
    Ok(())
}

fn arg_texprs(args: &[ArgRec], out: &mut Vec<u32>) {
    out.extend(args.iter().filter_map(|a| match a {
        ArgRec::Expr(e, _) => Some(*e),
        _ => None,
    }));
}

/// The expression sections type `t` holds.
fn ty_texprs(t: &TyRec) -> Vec<u32> {
    let mut out = Vec::new();
    match t {
        TyRec::Ex(
            ExTy::Decltype { e, .. } | ExTy::DepArray { e, .. } | ExTy::DepVector { e, .. },
        ) => out.push(*e),
        TyRec::Func { exc, nx, .. } if *exc >= NX_FALSE => out.push(*nx),
        TyRec::Tst { args, .. }
        | TyRec::Dep(DepTy::Tmpl { args, .. })
        | TyRec::Placeholder { args, .. } => {
            arg_texprs(args, &mut out);
        }
        _ => {}
    }
    out
}

/// Checks the sections type `t` holds (`below`: the types before it),
/// marking them `used`; returns one past the highest record they name.
/// A section may name a function parameter (WP-1343): `owners` gives
/// each parameter's function record ([`body::param_owners`]), whose
/// record may use the type (its parameters are built before its type);
/// one naming a requires-expression parameter is used by no record.
pub(super) fn validate_ty_texprs(
    img: &Image,
    t: &TyRec,
    below: &dyn Fn(QRef) -> bool,
    used: &mut [bool],
    owners: &[u32],
) -> Res<usize> {
    let mut hi = 0usize;
    for i in ty_texprs(t) {
        let Some(b) = img.texprs.get(i as usize) else {
            return bad("bad expression section");
        };
        if !used[i as usize] {
            body::validate_section(img, b, owners.len(), below, true)?;
            used[i as usize] = true;
        }
        hi = hi.max(body::section_decl_bound(b));
        let (param, req) = body::section_max_params(b);
        if let Some(g) = param {
            hi = hi.max(owners[g as usize] as usize);
        }
        // Requires-expression parameters are declared after the records.
        if req.is_some() {
            hi = hi.max(img.decls.len());
        }
    }
    Ok(hi)
}

/// Checks the sections no type holds (the importer never builds them).
pub(super) fn validate_unused_texprs(
    img: &Image,
    used: &[bool],
    qref_ok: &dyn Fn(QRef) -> bool,
) -> Res<()> {
    let np = body::param_count(img);
    for (b, _) in img.texprs.iter().zip(used).filter(|(_, u)| !**u) {
        body::validate_section(img, b, np, qref_ok, true)?;
    }
    Ok(())
}

/// Every main-file offset the sections hold.
pub(super) fn texpr_locs(img: &Image, out: &mut Vec<u32>) {
    for b in &img.texprs {
        body::section_locs(b, out);
    }
}

// ---------------------------------------------------------------------------
// The writer
// ---------------------------------------------------------------------------

impl Writer<'_, '_, '_> {
    /// The section of expression `e` (an index into `Image::texprs`).
    pub(super) fn texpr(&mut self, e: ExprId) -> W<u32> {
        let b = self.texpr_section(e)?;
        let i = u32::try_from(self.img.texprs.len()).map_err(|_| "too many expressions")?;
        self.img.texprs.push(b);
        Ok(i)
    }

    /// The record of type `t` (view `v`) when it is one of this module's.
    pub(super) fn ex_ty(&mut self, t: TypeId, v: TypeView) -> W<Option<TyRec>> {
        let types = &self.s.ast.types;
        let first = |p| types.profile_expr(p).ok_or("a dependent type without its expression");
        Ok(Some(TyRec::Ex(match v {
            TypeView::Decltype { expr, underlying } => {
                let canon = types.canonical(QualType::unqual(t));
                let dep = types.kind(canon.ty()) == TypeKind::DependentDecltype;
                let under = self.qref(underlying)?;
                ExTy::Decltype { e: self.texpr(expr)?, under, dep, canon: false }
            }
            TypeView::DependentDecltype(p) => {
                let expr = first(p)?;
                ExTy::Decltype { e: self.texpr(expr)?, under: DEPENDENT_TY, dep: true, canon: true }
            }
            TypeView::DependentSizedArray { elem, size } => {
                let (expr, canon) = match size {
                    ExprOperand::Expr(e) => (e, false),
                    ExprOperand::Profile(p) => (first(p)?, true),
                };
                let elem = self.qref(elem)?;
                ExTy::DepArray { elem, e: self.texpr(expr)?, canon }
            }
            TypeView::DependentVector { elem, size, kind } => {
                let (expr, canon) = match size {
                    ExprOperand::Expr(e) => (e, false),
                    ExprOperand::Profile(p) => (first(p)?, true),
                };
                let elem = self.qref(elem)?;
                ExTy::DepVector { elem, e: self.texpr(expr)?, kind: kind as u8, canon }
            }
            TypeView::PackExpansion { pattern, num_expansions: Some(n) } => {
                ExTy::PackN { pattern: self.qref(pattern)?, n }
            }
            _ => return Ok(None),
        })))
    }

    /// Function type `p` whose `noexcept` operand is no `bool` literal.
    /// Function `fty`'s type record when its `noexcept` operand names its
    /// own parameters ([`LateNx`]): the type without it, and the
    /// exception words and operand to root; `None` for any other type.
    pub(super) fn late_noexcept(
        &mut self,
        fty: QualType,
        p: &FuncProto,
    ) -> W<Option<(QRef, (u8, ExprId))>> {
        let types = &self.s.ast.types;
        let ExcSpec::NoexceptExpr { expr, value } = types.exc_spec(p) else { return Ok(None) };
        if super::noexcept_literal(&self.s.ast, expr, value).is_ok() {
            return Ok(None);
        }
        match self.qref(fty) {
            Err(body::TEXPR_LOCAL | super::outside::TEXPR_LATER) => {}
            r => return r.map(|_| None),
        }
        let exc = match value {
            NoexceptValue::False => NX_FALSE,
            NoexceptValue::True => NX_TRUE,
            NoexceptValue::Dependent(_) => NX_DEPENDENT,
        };
        let types = &self.s.ast.types;
        let ps: Vec<QualType> = types.params(p).to_vec();
        let info = p.info().bits();
        let ret = self.qref(p.ret())?;
        let params = ps.into_iter().map(|q| self.qref(q)).collect::<W<Vec<_>>>()?;
        let t = super::BUILTINS + 1 + self.img.types.len() as u32;
        self.img.types.push(TyRec::Func { ret, params, info, exc: 0, nx: NONE });
        Ok(Some((QRef { ty: t, quals: fty.quals_bits() }, (exc, expr))))
    }

    pub(super) fn noexcept_fn_ty(&mut self, p: FuncProto) -> W<TyRec> {
        let types = &self.s.ast.types;
        let ExcSpec::NoexceptExpr { expr, value } = types.exc_spec(&p) else {
            return Err("an exception specification the image does not carry");
        };
        let exc = match value {
            NoexceptValue::False => NX_FALSE,
            NoexceptValue::True => NX_TRUE,
            NoexceptValue::Dependent(_) => NX_DEPENDENT,
        };
        let ps: Vec<QualType> = types.params(&p).to_vec();
        let info = p.info().bits();
        let ret = self.qref(p.ret())?;
        let params = ps.into_iter().map(|q| self.qref(q)).collect::<W<Vec<_>>>()?;
        Ok(TyRec::Func { ret, params, info, exc, nx: self.texpr(expr)? })
    }
}

// ---------------------------------------------------------------------------
// The importer
// ---------------------------------------------------------------------------

/// A section's state in the importer.
#[derive(Clone, Copy, Debug)]
enum Sec {
    Unbuilt,
    /// Being built, or failed.
    Pending,
    Built(ExprId),
}

/// The importer's sections while one image is built.
#[derive(Debug, Default)]
pub(crate) struct TexprState {
    /// The image's main-file locations.
    locs: Option<super::Locs>,
    /// Per section: its state (a built one's root).
    built: Vec<Sec>,
    /// The function parameters built so far, in image order (WP-1343):
    /// a section naming one is built only once it exists.
    pub(super) params: Vec<DeclId>,
    /// The requires-expression parameters, once declared (WP-1343).
    pub(super) req_params: Vec<DeclId>,
}

impl Sema<'_, '_> {
    /// Starts building `img`; returns the state to restore after.
    pub(super) fn image_texprs_enter(&mut self, img: &Image, main: &super::Locs) -> TexprState {
        let fresh = TexprState {
            locs: Some(main.clone()),
            built: vec![Sec::Unbuilt; img.texprs.len()],
            params: Vec::new(),
            req_params: Vec::new(),
        };
        std::mem::replace(&mut self.image_texprs, fresh)
    }

    /// Builds the sections type `k` holds, before the type.
    pub(super) fn image_texprs_of(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        k: usize,
    ) {
        let Some(t) = img.types.get(k) else { return };
        for i in ty_texprs(t) {
            self.image_texpr_build(img, tys, ids, i);
        }
    }

    /// Builds section `i` unless it is built (or being built); its root.
    /// Every record a section names is built before its holder (a type,
    /// or since WP-1320 a non-type parameter's default); checked again,
    /// never trusted.
    pub(super) fn image_texpr_build(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        i: u32,
    ) -> Option<ExprId> {
        let k = i as usize;
        if matches!(self.image_texprs.built.get(k), Some(Sec::Unbuilt)) {
            self.image_texprs.built[k] = Sec::Pending;
            let b = &img.texprs[k];
            let (np, nr) = (self.image_texprs.params.len(), self.image_texprs.req_params.len());
            let (param, req) = body::section_max_params(b);
            if body::section_decl_bound(b) <= ids.len()
                && param.is_none_or(|g| (g as usize) < np)
                && req.is_none_or(|g| (g as usize) < nr)
                && let Some(locs) = self.image_texprs.locs.take()
            {
                let root = self.image_texpr_section(img, tys, ids, b, &locs);
                self.image_texprs.locs = Some(locs);
                if let Some(r) = root {
                    self.image_texprs.built[k] = Sec::Built(r);
                }
            }
        }
        self.image_texpr(i)
    }

    /// The root of section `i`, once built.
    pub(super) fn image_texpr(&self, i: u32) -> Option<ExprId> {
        match self.image_texprs.built.get(i as usize) {
            Some(&Sec::Built(e)) => Some(e),
            _ => None,
        }
    }

    /// The template argument of expression record `(i, profile)`.
    pub(super) fn image_expr_arg(&mut self, i: u32, profile: bool) -> TemplateArg {
        match self.image_texpr(i) {
            Some(e) => TemplateArg::expr(e, profile.then(|| self.ast.profile_expr(e))),
            None => TemplateArg::ty(QualType::ERROR),
        }
    }

    /// The exception specification of function type words `exc`, `nx`.
    pub(super) fn image_func_exc(&mut self, exc: u8, nx: u32) -> ExcSpec {
        if exc < NX_FALSE {
            return self.image_exc_spec(exc);
        }
        let expr = (nx != NONE).then(|| self.image_texpr(nx)).flatten();
        self.image_nx(exc, expr)
    }

    /// WP-1360: a [`LateNx`]'s function waits for its exception
    /// specification as one whose declarator the parser saved for a
    /// complete-class context does ([`Sema::defer_late_noexcept`]): a
    /// specialization the image instantiates before the bodies are built
    /// (a header's `box<int>`) defers its own ([temp.inst]/14) instead
    /// of copying a type with none.
    pub(super) fn image_defer_late_nx(&mut self, f: DeclId) {
        let Some(ty) = self.ast.decls.get(f).ty() else { return };
        if self.ast.types.proto(ty).is_none() {
            return;
        }
        let t = self.with_exc_spec(ty, ExcSpec::Deferred(f));
        self.ast.decls.set_ty(f, t);
        self.unparsed_exc.insert(f);
    }

    /// Sets each [`LateNx`]'s exception specification on its function
    /// (`ids`) once the bodies are built, as its declarator had it.
    pub(super) fn image_late_nx(&mut self, img: &Image, ids: &[DeclId], built: &body::Built) {
        for l in &img.late_nx {
            let f = ids[l.rec as usize];
            let exc = self.image_nx(l.exc, Some(built.expr(l.expr)));
            if self.unparsed_exc.contains(&f) {
                self.set_member_exception_spec(f, exc);
                continue;
            }
            let Some(fty) = self.ast.decls.get(f).ty() else { continue };
            let q = self.with_exc_spec(fty, exc);
            self.ast.decls.set_ty(f, q);
        }
    }

    /// `noexcept(expr)` of words `exc` (7 to 9); none for a missing
    /// operand.
    fn image_nx(&mut self, exc: u8, expr: Option<ExprId>) -> ExcSpec {
        let Some(expr) = expr else {
            return ExcSpec::default();
        };
        let value = match exc {
            NX_FALSE => NoexceptValue::False,
            NX_TRUE => NoexceptValue::True,
            _ => NoexceptValue::Dependent(self.dependent_profile(expr)),
        };
        ExcSpec::NoexceptExpr { expr, value }
    }

    /// Builds type record `x`.
    pub(super) fn image_ex_ty(
        &mut self,
        x: &ExTy,
        qt: &mut dyn FnMut(&mut Self, QRef) -> QualType,
    ) -> QualType {
        match *x {
            ExTy::Decltype { e, under, dep, canon } => {
                let under = qt(self, under);
                let Some(e) = self.image_texpr(e) else { return QualType::ERROR };
                let p = dep.then(|| self.dependent_profile(e));
                let q = self.ast.types.decltype_type(e, under, p);
                if canon { self.ast.types.canonical(q) } else { q }
            }
            ExTy::DepArray { elem, e, canon } => {
                let elem = qt(self, elem);
                let Some(e) = self.image_texpr(e) else { return QualType::ERROR };
                let p = self.dependent_profile(e);
                let q = self.ast.types.dependent_sized_array(elem, e, p);
                if canon { self.ast.types.canonical(q) } else { q }
            }
            ExTy::PackN { pattern, n } => {
                let q = qt(self, pattern);
                self.ast.types.pack_expansion(q, Some(n))
            }
            ExTy::DepVector { elem, e, kind, canon } => {
                let elem = qt(self, elem);
                let Some(e) = self.image_texpr(e) else { return QualType::ERROR };
                let p = self.dependent_profile(e);
                let q = self.ast.types.dependent_vector(elem, e, p, VectorKind::from_u8(kind));
                if canon { self.ast.types.canonical(q) } else { q }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(x: &ExTy) -> ExTy {
        let mut w = Wr(Vec::new());
        encode_ex(&mut w, x);
        let mut r = Rd(&w.0);
        let y = decode_ex(&mut r).expect("decodes");
        assert!(r.0.is_empty());
        y
    }

    #[test]
    fn ex_types_round_trip() {
        let q = QRef { ty: 3, quals: 1 };
        for x in [
            ExTy::Decltype { e: 4, under: q, dep: true, canon: false },
            ExTy::Decltype { e: 0, under: DEPENDENT_TY, dep: true, canon: true },
            ExTy::Decltype { e: 1, under: q, dep: false, canon: false },
            ExTy::DepArray { elem: q, e: 2, canon: true },
            ExTy::PackN { pattern: q, n: 7 },
            ExTy::DepVector { elem: q, e: 3, kind: 3, canon: false },
        ] {
            assert_eq!(round(&x), x);
        }
    }

    #[test]
    fn bad_ex_types_are_rejected() {
        // A canonical decltype without a profile, a bad flag, a bad tag.
        for bytes in [
            &[0u8, 0, 0, 0, 0, 3, 0, 0, 0, 0, 2][..],
            &[0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 4],
            &[1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 2],
            &[3],
        ] {
            assert!(decode_ex(&mut Rd(bytes)).is_err());
        }
    }
}
