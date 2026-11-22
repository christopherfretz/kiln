//! The declaration extents of the image's records (spec/ast.md §19.6,
//! spec/modules.md §11.11; WP-2090).
//!
//! When the module unit was parsed with the declaration-extent table on,
//! each record whose declaration has a row carries it, and so does each
//! parameter of a function record (by its position): its ten locations
//! in the image's location space ([`NONE`] for an invalid one) and how
//! it ends. The declarations of bodies are not carried. The importer gives the declaration it builds for the record
//! the same row, mapped back to its own locations, when its own table is
//! on. A template whose pattern's row is the same shares it again.

use super::{Image, NONE, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;
use kiln_ast::DeclId;
use kiln_ast::decl_extents::{DeclEnd, DeclExtent};
use kiln_source::{Loc, Span};

/// The locations of one row, in [`DeclExtent`] field order.
const LOCS: usize = 10;

/// The parameter position of a record's own row.
const OWN: u32 = NONE;

/// One row: (record, parameter position or [`OWN`], locations, end).
type Entry = (u32, u32, [u32; LOCS], u8);

/// The section, ascending by record and position.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct Extents {
    entries: Vec<Entry>,
}

fn locs_of(e: &DeclExtent) -> [Loc; LOCS] {
    [
        e.full.lo,
        e.full.hi,
        e.own.lo,
        e.own.hi,
        e.name.lo,
        e.name.hi,
        e.qual,
        e.body.lo,
        e.body.hi,
        e.specs_end,
    ]
}

fn end_code(e: DeclEnd) -> u8 {
    match e {
        DeclEnd::Other => 0,
        DeclEnd::Semi => 1,
        DeclEnd::Brace => 2,
    }
}

impl Writer<'_, '_, '_> {
    /// Lists the extents of every record written (after the last record
    /// is). A location the image cannot carry fails the record, as any
    /// other of its locations does.
    pub(super) fn extents_section(&mut self) -> W<()> {
        let table = &self.s.ast.decl_extents;
        if !table.is_on() || table.is_empty() {
            return Ok(());
        }
        let mut recs: Vec<(u32, DeclId)> = self.recs.iter().map(|(&d, &i)| (i, d)).collect();
        recs.sort_unstable();
        let mut out = Vec::new();
        for (i, d) in recs {
            let ast = &self.s.ast;
            let params: Vec<DeclId> = match ast.decls.function(d) {
                Some(fd) => ast.lists.iter::<DeclId>(fd.params).collect(),
                None => Vec::new(),
            };
            let rows = params.into_iter().zip(0u32..).chain(std::iter::once((d, OWN)));
            for (x, k) in rows {
                let Some(e) = self.s.ast.decl_extents.get(x).copied() else { continue };
                let mut locs = [NONE; LOCS];
                let mut ok = true;
                for (slot, l) in locs.iter_mut().zip(locs_of(&e)) {
                    if !l.is_valid() {
                        continue;
                    }
                    match self.loc(l) {
                        Ok(o) => *slot = o,
                        Err(_) => ok = false,
                    }
                }
                if ok {
                    out.push((i, k, locs, end_code(e.end)));
                } else {
                    self.miss(i, "a declaration extent the image does not carry")?;
                }
            }
        }
        self.img.extents.entries = out;
        Ok(())
    }
}

pub(super) fn encode_extents(w: &mut Wr, s: &Extents) {
    w.len(s.entries.len());
    for (i, k, locs, end) in &s.entries {
        w.u32(*i);
        w.u32(*k);
        for &o in locs {
            w.u32(o);
        }
        w.u8(*end);
    }
}

pub(super) fn decode_extents(r: &mut Rd) -> Res<Extents> {
    let n = r.count(4 * (LOCS + 2) + 1)?;
    let mut entries = Vec::with_capacity(n);
    for _ in 0..n {
        let (i, k) = (r.u32()?, r.u32()?);
        let mut locs = [NONE; LOCS];
        for slot in &mut locs {
            *slot = r.u32()?;
        }
        entries.push((i, k, locs, r.u8()?));
    }
    Ok(Extents { entries })
}

/// The section's records are in range and ascending, a parameter's
/// position is its function record's, and its ends known.
pub(super) fn validate_extents(img: &Image) -> Res<()> {
    let e = &img.extents.entries;
    let in_range = |i: u32, k: u32| match img.decls.get(i as usize) {
        None => false,
        Some(_) if k == OWN => true,
        Some(d) => matches!(&d.payload, super::Payload::Function { params, .. }
            if (k as usize) < params.len()),
    };
    let ok = e.iter().all(|&(i, k, _, end)| in_range(i, k) && end <= 2)
        && e.windows(2).all(|w| (w[0].0, w[0].1) < (w[1].0, w[1].1));
    if ok { Ok(()) } else { bad("bad declaration extents") }
}

/// The section's locations, for the decoder's offset check.
pub(super) fn extent_offsets(img: &Image, offs: &mut Vec<u32>) {
    for (_, _, locs, _) in &img.extents.entries {
        offs.extend(locs.iter().copied().filter(|&o| o != NONE));
    }
}

impl Sema<'_, '_> {
    /// Gives the built records `ids` their extents, when the table is on.
    pub(super) fn image_extents(&mut self, img: &Image, ids: &[DeclId], at: impl Fn(u32) -> Loc) {
        if !self.ast.decl_extents.is_on() {
            return;
        }
        for (i, k, locs, end) in &img.extents.entries {
            let Some(&f) = ids.get(*i as usize) else { continue };
            let d = if *k == OWN {
                f
            } else {
                let p = self.ast.decls.function(f).map(|fd| fd.params);
                match p.and_then(|l| self.ast.lists.iter::<DeclId>(l).nth(*k as usize)) {
                    Some(p) => p,
                    None => continue,
                }
            };
            let l = |k: usize| if locs[k] == NONE { Loc::INVALID } else { at(locs[k]) };
            let e = DeclExtent {
                full: Span::new(l(0), l(1)),
                own: Span::new(l(2), l(3)),
                name: Span::new(l(4), l(5)),
                qual: l(6),
                body: Span::new(l(7), l(8)),
                specs_end: l(9),
                end: match end {
                    1 => DeclEnd::Semi,
                    2 => DeclEnd::Brace,
                    _ => DeclEnd::Other,
                },
            };
            let pattern = self.ast.template_info(d).and_then(|t| t.pattern);
            match pattern {
                Some(p) if self.ast.decl_extents.get(p) == Some(&e) => {
                    self.ast.decl_extents.share(d, p);
                }
                _ => self.ast.decl_extents.set(d, e),
            }
        }
    }
}
