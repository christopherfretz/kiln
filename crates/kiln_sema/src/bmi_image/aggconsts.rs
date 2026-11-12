//! Pointer and aggregate constants (spec/modules.md §11.11, "Forms
//! carried by WP-1343").
//!
//! A body's constant may be a non-null pointer or reference into a
//! declaration the image carries (a variable, a function, a
//! lifetime-extended temporary, a `std::type_info` object or an earlier
//! constant's subobject), or a class, array or union value as the Oven
//! freezes it: its bytes, its pointer-like subobjects (each an earlier
//! constant of the section), its padding and its active union members
//! ([`kiln_ast::consts::ByteImage`]). The comparison categories'
//! values (`strong_ordering::less`), an immediate invocation's class
//! result, `source_location::current()` are such constants.
//!
//! A type's or template argument's expression section carries neither
//! a pointer nor an aggregate that names a declaration: its record bound
//! ([`super::exprs`]) counts no constant.

use kiln_ast::consts::{ByteImage, ByteRange, PtrBase, Reloc, Scalar, UnionMember};
use kiln_ast::{ConstValueId, DeclId, QualType};

use super::body::{ConstRec, Dc, dc_ok};
use super::{Image, QRef, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;

/// What a carried pointer constant points into.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum PtrRec {
    /// A variable record.
    Decl(u32),
    /// A function record.
    Function(u32),
    /// A lifetime-extended temporary record.
    Temporary(u32),
    /// The `std::type_info` object of a type.
    TypeInfo(QRef),
    /// An earlier constant of the section.
    Const(u32),
}

/// The tag of a pointer constant (after the scalars' 0-3).
pub(super) const TAG_PTR: u8 = 4;
/// The tag of an aggregate constant.
pub(super) const TAG_AGG: u8 = 5;

pub(super) fn encode(w: &mut Wr, c: &ConstRec) {
    match c {
        ConstRec::Ptr { ty, base, offset, designated, past } => {
            w.u8(TAG_PTR);
            w.q(*ty);
            match *base {
                PtrRec::Decl(x) => {
                    w.u8(0);
                    w.u32(x);
                }
                PtrRec::Function(x) => {
                    w.u8(1);
                    w.u32(x);
                }
                PtrRec::Temporary(x) => {
                    w.u8(2);
                    w.u32(x);
                }
                PtrRec::TypeInfo(q) => {
                    w.u8(3);
                    w.q(q);
                }
                PtrRec::Const(x) => {
                    w.u8(4);
                    w.u32(x);
                }
            }
            w.0.extend_from_slice(&offset.to_le_bytes());
            match designated {
                None => w.u8(0),
                Some(q) => {
                    w.u8(1);
                    w.q(*q);
                }
            }
            w.u8(u8::from(*past));
        }
        ConstRec::Agg { ty, bytes, relocs, padding, unions } => {
            w.u8(TAG_AGG);
            w.q(*ty);
            w.len(bytes.len());
            w.0.extend_from_slice(bytes);
            for v in [relocs, padding, unions] {
                w.len(v.len());
                for &(a, b) in v {
                    w.u32(a);
                    w.u32(b);
                }
            }
        }
        _ => {}
    }
}

/// Decodes the rest of a pointer or aggregate constant of tag `tag`.
pub(super) fn decode(r: &mut Rd<'_>, tag: u8, ty: QRef) -> Res<ConstRec> {
    if tag == TAG_PTR {
        let base = match r.u8()? {
            0 => PtrRec::Decl(r.u32()?),
            1 => PtrRec::Function(r.u32()?),
            2 => PtrRec::Temporary(r.u32()?),
            3 => PtrRec::TypeInfo(r.q()?),
            4 => PtrRec::Const(r.u32()?),
            _ => return bad("bad pointer constant"),
        };
        let offset = r.u64()?;
        let designated = match r.u8()? {
            0 => None,
            1 => Some(r.q()?),
            _ => return bad("bad pointer constant"),
        };
        let past = match r.u8()? {
            0 => false,
            1 => true,
            _ => return bad("bad pointer constant"),
        };
        return Ok(ConstRec::Ptr { ty, base, offset, designated, past });
    }
    let n = r.count(1)?;
    let bytes = r.take(n)?.to_vec();
    let mut pairs = || -> Res<Vec<(u32, u32)>> {
        let n = r.count(8)?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push((r.u32()?, r.u32()?));
        }
        Ok(v)
    };
    let relocs = pairs()?;
    let padding = pairs()?;
    let unions = pairs()?;
    Ok(ConstRec::Agg { ty, bytes, relocs, padding, unions })
}

/// Whether pointer or aggregate constant `c` (index `k` of a section)
/// is sound: its declarations are of their class, its earlier constants
/// earlier and pointer-like, its byte ranges inside its bytes. A type's
/// or template argument's section (`texpr`) names no declaration.
pub(super) fn validate(
    img: &Image,
    nparams: usize,
    consts: &[ConstRec],
    k: usize,
    texpr: bool,
    qref_ok: &dyn Fn(QRef) -> bool,
) -> bool {
    let decl = |w: u32, dc: Dc| !texpr && w & super::body::PARAM == 0 && dc_ok(img, nparams, w, dc);
    let earlier_ptr = |x: u32| {
        (x as usize) < k
            && matches!(consts.get(x as usize), Some(ConstRec::Ptr { .. } | ConstRec::Null { .. }))
    };
    match consts.get(k) {
        Some(ConstRec::Ptr { base, designated, .. }) => {
            designated.is_none_or(qref_ok)
                && match *base {
                    PtrRec::Decl(x) => decl(x, Dc::Var),
                    PtrRec::Function(x) => decl(x, Dc::Value),
                    PtrRec::Temporary(x) => decl(x, Dc::Let),
                    PtrRec::TypeInfo(q) => qref_ok(q),
                    PtrRec::Const(x) => {
                        (x as usize) < k
                            && matches!(consts.get(x as usize), Some(ConstRec::Agg { .. }))
                    }
                }
        }
        Some(ConstRec::Agg { bytes, relocs, padding, unions, .. }) => {
            let n = bytes.len() as u64;
            relocs.iter().all(|&(at, v)| u64::from(at) < n && earlier_ptr(v))
                && padding.iter().all(|&(at, len)| len != 0 && u64::from(at) + u64::from(len) <= n)
                && unions.iter().all(|&(at, f)| u64::from(at) < n && decl(f, Dc::Field))
                && (!texpr || unions.is_empty())
        }
        _ => true,
    }
}

impl Writer<'_, '_, '_> {
    /// The record of pointer or aggregate constant `c` (none when the
    /// image does not carry it).
    pub(super) fn agg_const(&mut self, c: ConstValueId) -> W<Option<ConstRec>> {
        let s = self.s;
        let consts = &s.ast.consts;
        if let Some(Scalar::Pointer { ty, base, offset, designated, past }) = consts.scalar(c) {
            if self.bs.texpr > 0 {
                return Ok(None);
            }
            let base = match base {
                PtrBase::Decl(d) if s.ast.decls.var(d).is_some() => {
                    PtrRec::Decl(self.dref(d, Dc::Var)?)
                }
                PtrBase::Function(d) => PtrRec::Function(self.dref(d, Dc::Value)?),
                PtrBase::Temporary(d) => PtrRec::Temporary(self.dref(d, Dc::Let)?),
                PtrBase::TypeInfo(q) => PtrRec::TypeInfo(self.qref(q)?),
                PtrBase::Const(v) => match self.cnst_full(v)? {
                    Some(i) => PtrRec::Const(i),
                    None => return Ok(None),
                },
                _ => return Ok(None),
            };
            let designated = match designated {
                Some(q) => Some(self.qref(q)?),
                None => None,
            };
            let ty = self.qref(ty)?;
            return Ok(Some(ConstRec::Ptr { ty, base, offset, designated, past }));
        }
        let Some(a) = consts.aggregate(c) else { return Ok(None) };
        if self.bs.texpr > 0 && !(a.relocs.is_empty() && a.unions.is_empty()) {
            return Ok(None);
        }
        let mut relocs = Vec::with_capacity(a.relocs.len());
        for r in a.relocs {
            match self.cnst_full(r.value)? {
                Some(i) => relocs.push((r.at, i)),
                None => return Ok(None),
            }
        }
        let mut unions = Vec::with_capacity(a.unions.len());
        for u in a.unions {
            unions.push((u.at, self.dref(u.field, Dc::Field)?));
        }
        let padding = a.padding.iter().map(|p| (p.start, p.len)).collect();
        let ty = self.qref(a.ty)?;
        Ok(Some(ConstRec::Agg { ty, bytes: a.bytes.to_vec(), relocs, padding, unions }))
    }
}

impl Sema<'_, '_> {
    /// The value of pointer or aggregate constant `c` of type `ty`;
    /// `consts` are the section's constants before it. `None` when the
    /// constant store rejects it (checked, never trusted).
    pub(super) fn image_agg_const(
        &mut self,
        c: &ConstRec,
        ty: QualType,
        consts: &[ConstValueId],
        ids: &[DeclId],
        q: &mut dyn FnMut(&mut Self, QRef) -> QualType,
    ) -> Option<ConstValueId> {
        let id = |w: u32| ids.get(w as usize).copied();
        match c {
            ConstRec::Ptr { base, offset, designated, past, .. } => {
                let base = match *base {
                    PtrRec::Decl(x) => PtrBase::Decl(self.ast.first(id(x)?)),
                    PtrRec::Function(x) => PtrBase::Function(id(x)?),
                    PtrRec::Temporary(x) => PtrBase::Temporary(id(x)?),
                    PtrRec::TypeInfo(t) => PtrBase::TypeInfo(q(self, t)),
                    PtrRec::Const(x) => PtrBase::Const(*consts.get(x as usize)?),
                };
                let designated = designated.map(|t| q(self, t));
                let s = Scalar::Pointer { ty, base, offset: *offset, designated, past: *past };
                self.ast.consts.push_scalar(s).ok()
            }
            ConstRec::Agg { bytes, relocs, padding, unions, .. } => {
                let relocs: Vec<Reloc> = relocs
                    .iter()
                    .map(|&(at, v)| consts.get(v as usize).map(|&value| Reloc { at, value }))
                    .collect::<Option<_>>()?;
                let padding: Vec<ByteRange> =
                    padding.iter().map(|&(start, len)| ByteRange { start, len }).collect();
                let unions: Vec<UnionMember> = unions
                    .iter()
                    .map(|&(at, f)| id(f).map(|field| UnionMember { at, field }))
                    .collect::<Option<_>>()?;
                let img =
                    ByteImage { ty, bytes, relocs: &relocs, padding: &padding, unions: &unions };
                self.ast.consts.push_aggregate(img).ok()
            }
            _ => None,
        }
    }
}
