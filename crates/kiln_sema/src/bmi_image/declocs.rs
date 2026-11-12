//! The written locations of a dependent declaration's declarator in the
//! declaration image (spec/modules.md §11.11, "Templates as patterns",
//! WP-1283).
//!
//! Sema keeps, beside a declaration of dependent type, where its first
//! type-specifier, its outermost `&`/`&&` and its innermost `*`/`[` were
//! written ([`Sema::type_spec_locs`], [`Sema::ref_locs`],
//! [`Sema::inner_chunk_locs`]): an instantiation that forms an invalid
//! type is diagnosed there (`T arr[2]` with `T = void`, at the `[`). The
//! image lists them per record, so an imported pattern's instantiation
//! reports what replay reports.

use super::{Image, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;
use kiln_ast::DeclId;
use kiln_source::Loc;

const TYPE_SPEC: u8 = 0;
const REF: u8 = 1;
const CHUNK: u8 = 2;

/// The section: (record, kind, location), ascending by record and kind.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct DeclLocs {
    entries: Vec<(u32, u8, u32)>,
}

impl Writer<'_, '_, '_> {
    /// Lists the side locations of every record written (after the
    /// records, before the roots).
    pub(super) fn decl_locs(&mut self) -> W<()> {
        let s = self.s;
        let mut recs: Vec<(u32, DeclId)> = self.recs.iter().map(|(&d, &i)| (i, d)).collect();
        recs.sort_unstable();
        let mut out = Vec::new();
        for (i, d) in recs {
            for (k, m) in
                [(TYPE_SPEC, &s.type_spec_locs), (REF, &s.ref_locs), (CHUNK, &s.inner_chunk_locs)]
            {
                let Some(&l) = m.get(&d) else { continue };
                match self.loc(l) {
                    Ok(o) => out.push((i, k, o)),
                    Err(_) => self.miss(i, "a declarator location the image does not carry")?,
                }
            }
        }
        self.img.declocs.entries = out;
        Ok(())
    }

    /// Adds the side locations of records `recs`, written after
    /// [`Writer::decl_locs`] ran (WP-1350: closure types and local
    /// classes, written when a body names them).
    pub(super) fn more_decl_locs(&mut self, recs: &[(u32, DeclId)]) -> W<()> {
        let s = self.s;
        let mut out = std::mem::take(&mut self.img.declocs.entries);
        for &(i, d) in recs {
            for (k, m) in
                [(TYPE_SPEC, &s.type_spec_locs), (REF, &s.ref_locs), (CHUNK, &s.inner_chunk_locs)]
            {
                let Some(&l) = m.get(&d) else { continue };
                match self.loc(l) {
                    Ok(o) => out.push((i, k, o)),
                    Err(_) => self.miss(i, "a declarator location the image does not carry")?,
                }
            }
        }
        // A tag written while `collect` ran is listed already.
        out.sort_by_key(|&(i, k, _)| (i, k));
        out.dedup_by_key(|e| (e.0, e.1));
        self.img.declocs.entries = out;
        Ok(())
    }
}

pub(super) fn encode_decl_locs(w: &mut Wr, s: &DeclLocs) {
    w.len(s.entries.len());
    for &(i, k, o) in &s.entries {
        w.u32(i);
        w.u8(k);
        w.u32(o);
    }
}

pub(super) fn decode_decl_locs(r: &mut Rd) -> Res<DeclLocs> {
    let n = r.count(9)?;
    let mut entries = Vec::with_capacity(n);
    for _ in 0..n {
        entries.push((r.u32()?, r.u8()?, r.u32()?));
    }
    Ok(DeclLocs { entries })
}

/// The section's records and kinds are in range and ascending.
pub(super) fn validate_decl_locs(img: &Image) -> Res<()> {
    let e = &img.declocs.entries;
    let ok = e.iter().all(|&(i, k, _)| (i as usize) < img.decls.len() && k <= CHUNK)
        && e.windows(2).all(|w| (w[0].0, w[0].1) < (w[1].0, w[1].1));
    if ok { Ok(()) } else { bad("bad declarator locations") }
}

/// The section's locations, for the decoder's offset check.
pub(super) fn decl_loc_offsets(img: &Image, offs: &mut Vec<u32>) {
    offs.extend(img.declocs.entries.iter().map(|e| e.2));
}

impl Sema<'_, '_> {
    /// Records the side locations of the built records `ids`.
    pub(super) fn image_decl_locs(&mut self, img: &Image, ids: &[DeclId], at: impl Fn(u32) -> Loc) {
        for &(i, k, o) in &img.declocs.entries {
            let Some(&d) = ids.get(i as usize) else { continue };
            let m = match k {
                TYPE_SPEC => &mut self.type_spec_locs,
                REF => &mut self.ref_locs,
                _ => &mut self.inner_chunk_locs,
            };
            m.insert(d, at(o));
        }
    }
}
