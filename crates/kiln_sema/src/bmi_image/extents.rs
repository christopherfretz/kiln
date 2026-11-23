//! The declaration extents of the image's records (spec/ast.md §19.6,
//! spec/modules.md §11.11; WP-2090).
//!
//! When the module unit was parsed with the declaration-extent table on,
//! each record whose declaration has a row carries it, and so does each
//! parameter of a function record (by its position): its ten locations
//! in the image's location space ([`NONE`] for an invalid one) and how
//! it ends. A declaration inside a body (a local, a lambda's parameter, a
//! block-scope class and its members, a structured binding, a condition
//! or init-statement's variable) has a record of its own once a carried
//! body names it, and so its row; a requires-expression's parameter,
//! which has no record, is carried by its index in the image's
//! requires-expression parameters ([`REQ`]; WP-2091). The importer gives
//! the declaration it builds for the record the same row, mapped back to
//! its own locations, when its own table is on. A template whose
//! pattern's row is the same shares it again.
//!
//! The unit's module declaration has a row of its own kind
//! ([`ModuleDeclExtent`]). An importer that replays the image's preamble
//! (a partition, a unit with imports) records it, and the rows of the
//! preamble's import declarations, by preprocessing; an image without a
//! preamble carries it, and the importer gives it to the module
//! declaration it enters.

use super::{Image, NONE, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;
use kiln_ast::DeclId;
use kiln_ast::decl_extents::{DeclEnd, DeclExtent, ModuleDeclExtent, ModuleDeclKind};
use kiln_source::{Loc, Span};

/// The locations of one row, in [`DeclExtent`] field order.
const LOCS: usize = 10;

/// The parameter position of a record's own row.
const OWN: u32 = NONE;

/// The record word of a requires-expression parameter's row, whose
/// position is the parameter's index in the requires section (WP-2091);
/// after every record's row.
const REQ: u32 = NONE - 1;

/// One row: (record, parameter position or [`OWN`], locations, end).
type Entry = (u32, u32, [u32; LOCS], u8);

/// The bit of an entry's end byte that makes it a later row of the
/// declaration the entry before it is a row of (a declaration written more
/// than once under one id, spec/ast.md §19.1; WP-2470).
const LATER: u8 = 0x80;

/// The locations of a module declaration's row, in
/// [`ModuleDeclExtent`] field order (seven spans).
const MLOCS: usize = 14;

/// The section, ascending by record and position.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct Extents {
    entries: Vec<Entry>,
    /// The module declaration's row, in an image without a preamble.
    module: Option<[u32; MLOCS]>,
}

fn module_locs_of(e: &ModuleDeclExtent) -> [Loc; MLOCS] {
    let mut out = [Loc::INVALID; MLOCS];
    let spans = [e.full, e.export, e.keyword, e.name, e.partition, e.attrs, e.semi];
    for (k, s) in spans.into_iter().enumerate() {
        out[2 * k] = s.lo;
        out[2 * k + 1] = s.hi;
    }
    out
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
        if !table.is_on() {
            return Ok(());
        }
        // The module declaration, which an importer of an image without
        // a preamble does not preprocess.
        if self.img.preamble == NONE
            && let Some(m) =
                table.module_decls().iter().find(|m| m.kind == ModuleDeclKind::Module).copied()
        {
            let mut locs = [NONE; MLOCS];
            for (slot, l) in locs.iter_mut().zip(module_locs_of(&m)) {
                if l.is_valid() {
                    *slot = self
                        .loc(l)
                        .map_err(|_| "a module declaration extent the image does not carry")?;
                }
            }
            self.img.extents.module = Some(locs);
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
                let table = &self.s.ast.decl_extents;
                let es: Vec<DeclExtent> = table.rows_of(x).copied().collect();
                for (n, e) in es.iter().enumerate() {
                    let later = if n == 0 { 0 } else { LATER };
                    match self.extent_locs(e) {
                        Some(locs) => out.push((i, k, locs, end_code(e.end) | later)),
                        None => self.miss(i, "a declaration extent the image does not carry")?,
                    }
                }
            }
        }
        // The requires-expression parameters, by section index.
        let mut params: Vec<(u32, DeclId)> =
            self.bs.req.params.iter().map(|(&d, &k)| (k, d)).collect();
        params.sort_unstable();
        for (k, x) in params {
            let Some(e) = self.s.ast.decl_extents.get(x).copied() else { continue };
            match self.extent_locs(&e) {
                Some(locs) => out.push((REQ, k, locs, end_code(e.end))),
                None => self.miss(NONE, "a declaration extent the image does not carry")?,
            }
        }
        self.img.extents.entries = out;
        Ok(())
    }

    /// The image offsets of `e`'s locations; `None` when one cannot be
    /// carried.
    fn extent_locs(&self, e: &DeclExtent) -> Option<[u32; LOCS]> {
        let mut locs = [NONE; LOCS];
        for (slot, l) in locs.iter_mut().zip(locs_of(e)) {
            if l.is_valid() {
                *slot = self.loc(l).ok()?;
            }
        }
        Some(locs)
    }
}

/// The bit of the row count that says a module declaration's row
/// follows the rows (so an image without one keeps the bytes it had).
const HAS_MODULE: u32 = 0x8000_0000;

pub(super) fn encode_extents(w: &mut Wr, s: &Extents) {
    let n = u32::try_from(s.entries.len()).unwrap_or(HAS_MODULE - 1).min(HAS_MODULE - 1);
    w.u32(n | if s.module.is_some() { HAS_MODULE } else { 0 });
    for (i, k, locs, end) in &s.entries {
        w.u32(*i);
        w.u32(*k);
        for &o in locs {
            w.u32(o);
        }
        w.u8(*end);
    }
    for &o in s.module.iter().flatten() {
        w.u32(o);
    }
}

pub(super) fn decode_extents(r: &mut Rd) -> Res<Extents> {
    let word = r.u32()?;
    let n = (word & !HAS_MODULE) as usize;
    if n.saturating_mul(4 * (LOCS + 2) + 1) > r.0.len() {
        return bad("length exceeds the section");
    }
    let mut entries = Vec::with_capacity(n);
    for _ in 0..n {
        let (i, k) = (r.u32()?, r.u32()?);
        let mut locs = [NONE; LOCS];
        for slot in &mut locs {
            *slot = r.u32()?;
        }
        entries.push((i, k, locs, r.u8()?));
    }
    let module = if word & HAS_MODULE == 0 {
        None
    } else {
        let mut locs = [NONE; MLOCS];
        for slot in &mut locs {
            *slot = r.u32()?;
        }
        Some(locs)
    };
    Ok(Extents { entries, module })
}

/// The section's records are in range and ascending, a parameter's
/// position is its function record's, and its ends known.
pub(super) fn validate_extents(img: &Image) -> Res<()> {
    let e = &img.extents.entries;
    let in_range = |i: u32, k: u32| match img.decls.get(i as usize) {
        None => i == REQ && (k as usize) < img.reqs.nparams,
        Some(_) if k == OWN => true,
        Some(d) => matches!(&d.payload, super::Payload::Function { params, .. }
            if (k as usize) < params.len()),
    };
    // A later row follows a row of the same declaration.
    let ascends = |a: &Entry, b: &Entry| {
        (a.0, a.1) < (b.0, b.1) && b.3 & LATER == 0 || (a.0, a.1) == (b.0, b.1) && b.3 & LATER != 0
    };
    let ok = e.iter().all(|&(i, k, _, end)| in_range(i, k) && end & !LATER <= 2)
        && e.first().is_none_or(|f| f.3 & LATER == 0)
        && e.windows(2).all(|w| ascends(&w[0], &w[1]))
        && (img.extents.module.is_none() || img.preamble == NONE);
    if ok { Ok(()) } else { bad("bad declaration extents") }
}

/// The section's locations, for the decoder's offset check.
pub(super) fn extent_offsets(img: &Image, offs: &mut Vec<u32>) {
    for (_, _, locs, _) in &img.extents.entries {
        offs.extend(locs.iter().copied().filter(|&o| o != NONE));
    }
    offs.extend(img.extents.module.iter().flatten().copied().filter(|&o| o != NONE));
}

impl Sema<'_, '_> {
    /// The row of the module declaration an image without a preamble
    /// enters, when the table is on.
    pub(super) fn image_module_extent(&mut self, img: &Image, at: impl Fn(u32) -> Loc) {
        let Some(locs) = &img.extents.module else { return };
        if !self.ast.decl_extents.is_on() {
            return;
        }
        let l = |k: usize| if locs[k] == NONE { Loc::INVALID } else { at(locs[k]) };
        let sp = |k: usize| Span::new(l(2 * k), l(2 * k + 1));
        self.ast.decl_extents.push_module_decl(ModuleDeclExtent {
            full: sp(0),
            export: sp(1),
            keyword: sp(2),
            name: sp(3),
            partition: sp(4),
            attrs: sp(5),
            semi: sp(6),
            before: self.ast.decls.next_id().index(),
            kind: ModuleDeclKind::Module,
        });
    }

    /// Gives the built records `ids` their extents, when the table is on;
    /// then the requires-expression parameters `reqs` (built later, so
    /// a second call gives them theirs).
    pub(super) fn image_extents(
        &mut self,
        img: &Image,
        ids: &[DeclId],
        reqs: Option<&[DeclId]>,
        at: impl Fn(u32) -> Loc,
    ) {
        if !self.ast.decl_extents.is_on() {
            return;
        }
        for (i, k, locs, end) in &img.extents.entries {
            if (*i == REQ) != reqs.is_some() {
                continue;
            }
            let d = if let Some(r) = reqs {
                match r.get(*k as usize) {
                    Some(&p) => p,
                    None => continue,
                }
            } else if *k == OWN {
                let Some(&f) = ids.get(*i as usize) else { continue };
                f
            } else {
                let Some(&f) = ids.get(*i as usize) else { continue };
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
                end: match end & !LATER {
                    1 => DeclEnd::Semi,
                    2 => DeclEnd::Brace,
                    _ => DeclEnd::Other,
                },
            };
            let later = end & LATER != 0;
            let pattern = self.ast.template_info(d).and_then(|t| t.pattern);
            let table = &mut self.ast.decl_extents;
            match pattern {
                // A template shares its pattern's rows, the later ones too.
                Some(p) if !later && table.rows_of(p).next() == Some(&e) => table.share(d, p),
                Some(p) if later && table.shares(d, p) => {}
                _ => {
                    if later {
                        table.redeclare(d);
                    }
                    table.set(d, e);
                }
            }
        }
    }
}
