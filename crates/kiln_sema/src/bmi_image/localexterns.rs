//! Block-scope `extern` and function declarations in a precompiled
//! header's declaration image (spec/modules.md §11.11 "Precompiled
//! headers", WP-1293).
//!
//! A block-scope function or `extern` variable declaration names an
//! entity of its enclosing namespace, which a later declaration in that
//! namespace redeclares ([basic.link]/6, [dcl.meaning.general]/3.5).
//! Sema finds it again through [`Sema::local_externs`], not through
//! lookup. The image carries each one as a local record (its context
//! the enclosing function), written in declaration order, so that a
//! namespace-scope redeclaration in the header can name it as its
//! previous declaration. The image also lists those records in
//! `local_externs` order, so a consumer links its own declarations to
//! them as replay does. The reader also re-records the header's
//! C-linkage entities ([`Sema::extern_c`], [dcl.link]/6-7), the other
//! namespace-crossing link a declaration makes outside lookup.

use super::body::is_local;
use super::{Image, NONE, Payload, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;
use kiln_ast::DeclId;
use kiln_ast::decl::DeclKind;

/// The section: the local records of `Sema::local_externs`, in order.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct LocalExterns {
    recs: Vec<u32>,
}

impl Writer<'_, '_, '_> {
    /// Whether `d` is a block-scope declaration the image carries with
    /// its link: an entry of `local_externs` in a precompiled header.
    pub(super) fn carries_local_extern(&self, d: DeclId) -> bool {
        self.pch && self.s.local_externs.iter().any(|&(_, _, x)| x == d)
    }

    /// The previous declaration of block-scope declaration `d`: an
    /// earlier record, at namespace scope or block scope.
    pub(super) fn local_extern_prev(&self, p: DeclId) -> W<u32> {
        self.recs.get(&p).copied().ok_or("a redeclaration of an entity outside the image")
    }

    /// Local declaration `d` in declaration order: a block-scope
    /// declaration the image carries gets its record now, before the
    /// namespace-scope declarations that may redeclare it. Other locals
    /// get theirs on first use in a body.
    pub(super) fn local_in_order(&mut self, d: DeclId) -> W<()> {
        if self.carries_local_extern(d) && !self.recs.contains_key(&d) {
            self.local_rec(d)?;
        }
        Ok(())
    }

    /// Lists the records of `local_externs` (after the records). In
    /// census mode, one whose function the census counted is left out.
    pub(super) fn local_externs(&mut self) -> W<()> {
        if !self.pch {
            return Ok(());
        }
        let s = self.s;
        let mut out = Vec::with_capacity(s.local_externs.len());
        for &(_, _, d) in &s.local_externs {
            match self.recs.get(&d) {
                Some(&r) => out.push(r),
                None if self.census.is_some() => {}
                None => return Err("a block-scope declaration outside the image"),
            }
        }
        self.img.lexterns.recs = out;
        Ok(())
    }
}

pub(super) fn encode_local_externs(w: &mut Wr, s: &LocalExterns) {
    w.len(s.recs.len());
    for &i in &s.recs {
        w.u32(i);
    }
}

pub(super) fn decode_local_externs(r: &mut Rd) -> Res<LocalExterns> {
    let n = r.count(4)?;
    let mut recs = Vec::with_capacity(n);
    for _ in 0..n {
        recs.push(r.u32()?);
    }
    Ok(LocalExterns { recs })
}

/// Each listed record is a local function or variable record.
pub(super) fn validate_local_externs(img: &Image) -> Res<()> {
    let ok = img.lexterns.recs.iter().all(|&i| {
        img.decls.get(i as usize).is_some_and(|d| {
            is_local(img, d)
                && d.name != NONE
                && matches!(d.payload, Payload::Var { .. } | Payload::Function { kind: 0, .. })
        })
    });
    if ok { Ok(()) } else { bad("bad block-scope declarations") }
}

/// A local function record (record `i`): a declaration without a body,
/// whose type and parameters check as a namespace-scope function's.
pub(super) fn local_fn_ok(
    img: &Image,
    i: usize,
    qref_at: &dyn Fn(super::QRef, usize) -> bool,
) -> bool {
    let Some(d) = img.decls.get(i) else { return false };
    let Payload::Function { kind: 0, ty, spec, deferred: false, params, body, inits, overrides } =
        &d.payload
    else {
        return false;
    };
    let proto = (ty.ty > super::BUILTINS)
        .then(|| img.types.get((ty.ty - super::BUILTINS - 1) as usize))
        .flatten();
    let Some(super::TyRec::Func { params: pt, .. }) = proto else { return false };
    matches!(body, super::FnBody::None)
        && inits.is_empty()
        && overrides.is_empty()
        && ty.quals == 0
        && pt.len() == params.len()
        && super::fn_spec(*spec).is_some()
        && qref_at(*ty, i)
        && params.iter().all(|p| {
            qref_at(p.ty, i)
                && (p.name == NONE || (p.name as usize) < img.pnames.len())
                && p.flags & !super::DECL_FLAGS == 0
                && p.pflags & !kiln_ast::decl::ParamData::SCOPE_DEPTH_MASK == 0
        })
}

impl Sema<'_, '_> {
    /// Links the built block-scope declarations of the image for the
    /// consumer's redeclarations, as their declarations did
    /// ([`Sema::local_externs`]).
    pub(super) fn image_local_externs(&mut self, img: &Image, ids: &[DeclId]) {
        for &i in &img.lexterns.recs {
            let Some(&d) = ids.get(i as usize) else { continue };
            let Some(name) = self.ast.decls.get(d).name().as_ident() else { continue };
            let mut ns = self.ast.decls.get(d).ctx();
            for _ in 0..10_000 {
                let Some(c) = ns else { break };
                if matches!(
                    self.ast.decls.get(c).kind(),
                    DeclKind::Namespace | DeclKind::TranslationUnit
                ) {
                    break;
                }
                ns = self.ast.decls.get(c).ctx();
            }
            if let Some(ns) = ns {
                let ns = self.ast.first(ns);
                self.local_externs.push((ns, name, d));
            }
        }
    }

    /// Records the image's namespace-scope functions and variables with C
    /// language linkage in the consumer's "extern C context"
    /// ([`Sema::extern_c`], [dcl.link]/6-7), as their declarations did
    /// in replay (`Sema::record_extern_c`), so a consumer's C-linkage
    /// declaration in another namespace redeclares or conflicts with
    /// them. A precompiled header's image only (WP-1293).
    pub(super) fn image_extern_c(&mut self, img: &Image, ids: &[DeclId]) {
        if !img.module.is_empty() {
            return;
        }
        for (rec, &d) in img.decls.iter().zip(ids) {
            if !matches!(rec.payload, Payload::Var { .. } | Payload::Function { kind: 0, .. }) {
                continue;
            }
            let n = self.ast.decls.get(d);
            let Some(name) = n.name().as_ident() else { continue };
            let at_ns = n.ctx().is_some_and(|c| {
                matches!(
                    self.ast.decls.get(self.ast.redecl_context(c)).kind(),
                    DeclKind::Namespace | DeclKind::TranslationUnit
                )
            });
            if !at_ns
                || self.ast.described_template(d).is_some()
                || n.flags().contains(kiln_ast::decl::DeclFlags::INVALID)
                || !kiln_abi::mangle::is_extern_c(&self.ast, d)
                || kiln_abi::mangle::has_internal_linkage(&self.ast, d)
                || !self.ast.pass_object_size_params(d).is_empty()
            {
                continue;
            }
            match self.extern_c.get(&name) {
                Some(&old) if self.ast.first(old) != self.ast.first(d) => {}
                _ => {
                    self.extern_c.insert(name, d);
                }
            }
        }
    }
}
