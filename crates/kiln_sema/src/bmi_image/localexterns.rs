//! The `local_externs` section of a declaration image (spec/modules.md
//! §11.11, WP-1293): block-scope `extern` and function declarations
//! listed in [`Sema::local_externs`] order.
//!
//! A block-scope function or `extern` variable declaration names an
//! entity of its enclosing namespace, which a later declaration in that
//! namespace redeclares ([basic.link]/6, [dcl.meaning.general]/3.5).
//! Sema finds it again through [`Sema::local_externs`], not through
//! lookup. The precompiled header's image wrote those records (WP-1397
//! retired it); a module's image writes the section empty, and the
//! reader still accepts and links a listed record.

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
    /// The previous declaration of block-scope declaration `d`: an
    /// earlier record, at namespace scope or block scope.
    pub(super) fn local_extern_prev(&self, p: DeclId) -> W<u32> {
        self.recs.get(&p).copied().ok_or("a redeclaration of an entity outside the image")
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
}
