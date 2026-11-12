//! The cross-unit deduplication keys of a precompiled header's
//! declarations (WP-1353, spec/modules.md §11.11 "Forms carried by
//! WP-1353", §6).
//!
//! The parser skips a namespace-scope declaration of a module unit (a
//! global module fragment, typically) whose text another unit already
//! parsed, keyed by the positions of its first and last tokens and its
//! token count, and makes the earlier parse's declarations visible in the
//! unit (`kiln_parse` `module_before_decl`). A header a consumer replays
//! records those keys as it parses; one loaded from the image did not,
//! so a module whose global module fragment includes the same header
//! declared its entities again.
//!
//! The producer's parser notes each namespace-scope declaration it parses
//! ([`Sema::pch_note_external_decl`]): its first and last tokens, its
//! token count and the declarations it made. The writer keeps one entry
//! per declaration with a token, first and last token locations as image
//! locations and the declarations as the range of records made for them:
//!
//! ```text
//! dedup := n:u32 (start:u32 end:u32 ntoks:u32 lo:u32 hi:u32)*n
//! ```
//!
//! in parse order, `lo <= hi <= ` the record count (`lo == hi`: no
//! record). The consumer's sema turns each entry into the importer's
//! locations and declaration range ([`Sema::take_pch_dedup`]) and its
//! parser records them as the replay's parse would have.

use kiln_ast::DeclId;
use kiln_source::Loc;

use super::{Image, Locs, Payload, Rd, Res, Wr, Writer, bad};
use crate::Sema;

/// One namespace-scope declaration the producer parsed: its first and
/// last tokens, its token count and the declarations it made (`lo..hi`).
/// On the consumer side, the importer's declarations made for its
/// records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExtDecl {
    /// The first token.
    pub start: Loc,
    /// The last token.
    pub end: Loc,
    /// The number of tokens.
    pub ntoks: u32,
    /// The first declaration made (a [`DeclId`] index).
    pub lo: u32,
    /// Past the last declaration made (`lo == hi`: none).
    pub hi: u32,
}

/// The image's entries (the `dedup` section).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct Dedup {
    entries: Vec<DedupRec>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct DedupRec {
    start: u32,
    end: u32,
    ntoks: u32,
    lo: u32,
    hi: u32,
}

pub(super) fn encode_dedup(w: &mut Wr, d: &Dedup) {
    w.len(d.entries.len());
    for e in &d.entries {
        for v in [e.start, e.end, e.ntoks, e.lo, e.hi] {
            w.u32(v);
        }
    }
}

pub(super) fn decode_dedup(r: &mut Rd) -> Res<Dedup> {
    let n = r.count(20)?;
    let mut entries = Vec::with_capacity(n);
    for _ in 0..n {
        let (start, end, ntoks, lo, hi) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?);
        entries.push(DedupRec { start, end, ntoks, lo, hi });
    }
    Ok(Dedup { entries })
}

/// Each entry has a token and a record range within the records.
pub(super) fn validate_dedup(img: &Image) -> Res<()> {
    let n = img.decls.len() as u32;
    if !img.dedup.entries.iter().all(|e| e.ntoks > 0 && e.lo <= e.hi && e.hi <= n) {
        return bad("bad deduplication key");
    }
    Ok(())
}

/// Whether record `r` stands for a declaration of the importer's own (a
/// builtin or implicit declaration's stub), which is no part of the
/// header's range.
fn own_stub(img: &Image, r: usize) -> bool {
    matches!(img.decls[r].payload, Payload::Implicit { kind } if kind >= super::scope::BUILTIN_KIND)
}

impl Writer<'_, '_, '_> {
    /// The `dedup` section of a precompiled header from the producer's
    /// notes (after the records are written). A declaration whose tokens
    /// the image cannot locate gets no entry.
    pub(super) fn dedup_section(&mut self, notes: &[ExtDecl]) {
        let mut entries = Vec::with_capacity(notes.len());
        for x in notes {
            let (Ok(start), Ok(end)) = (self.loc(x.start), self.loc(x.end)) else { continue };
            let (mut lo, mut hi) = (u32::MAX, 0);
            for i in x.lo..x.hi {
                if let Some(&r) = self.recs.get(&DeclId::new(i)) {
                    lo = lo.min(r);
                    hi = hi.max(r + 1);
                }
            }
            if lo > hi {
                (lo, hi) = (0, 0);
            }
            entries.push(DedupRec { start, end, ntoks: x.ntoks, lo, hi });
        }
        self.img.dedup = Dedup { entries };
    }
}

impl Sema<'_, '_> {
    /// Notes the producer's namespace-scope declarations for the
    /// precompiled header's deduplication keys (WP-1353).
    pub fn record_pch_decl_keys(&mut self) {
        self.bmi.pch_ext_decls = Some(Vec::new());
    }

    /// Whether the parser should note its namespace-scope declarations
    /// ([`Self::pch_note_external_decl`]).
    #[must_use]
    pub fn records_pch_decl_keys(&self) -> bool {
        self.bmi.pch_ext_decls.is_some()
    }

    /// Notes namespace-scope declaration `x`, just parsed by the producer
    /// of a precompiled header.
    pub fn pch_note_external_decl(&mut self, x: ExtDecl) {
        if let Some(v) = &mut self.bmi.pch_ext_decls {
            v.push(x);
        }
    }

    /// The deduplication keys of the precompiled header the consumer
    /// loaded from its image, once, in the importer's locations and
    /// declarations.
    pub fn take_pch_dedup(&mut self) -> Vec<ExtDecl> {
        std::mem::take(&mut self.bmi.pch_dedup)
    }

    /// Turns the image's `dedup` section into the importer's keys, with
    /// `ids` its records' declarations.
    pub(super) fn image_dedup(&mut self, img: &Image, main: &Locs, ids: &[DeclId]) {
        let mut out = Vec::with_capacity(img.dedup.entries.len());
        for e in &img.dedup.entries {
            let (start, end) = (main.get(e.start), main.get(e.end));
            if !start.is_valid() || !end.is_valid() {
                continue;
            }
            let mut range: Option<(u32, u32)> = None;
            for r in e.lo as usize..e.hi as usize {
                let Some(&d) = ids.get(r) else { continue };
                if own_stub(img, r) {
                    continue;
                }
                let i = d.index();
                range = Some(range.map_or((i, i + 1), |(a, b)| (a.min(i), b.max(i + 1))));
            }
            let (lo, hi) = range.unwrap_or((0, 0));
            out.push(ExtDecl { start, end, ntoks: e.ntoks, lo, hi });
        }
        self.bmi.pch_dedup = out;
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::sample;
    use super::super::{decode, encode};
    use super::*;

    fn with_keys() -> Image {
        let mut img = sample();
        let n = img.decls.len() as u32;
        img.dedup.entries = vec![
            DedupRec { start: 20, end: 25, ntoks: 6, lo: 0, hi: n },
            DedupRec { start: 30, end: 30, ntoks: 1, lo: 0, hi: 0 },
        ];
        img
    }

    /// spec/modules.md §11.11 "Forms carried by WP-1353": the `dedup`
    /// section round-trips.
    #[test]
    fn dedup_round_trip() {
        let img = with_keys();
        assert_eq!(decode(&encode(&img)), Ok(img));
    }

    /// An entry without tokens or with a reversed or out-of-range record
    /// range is an error.
    #[test]
    fn rejects_corrupt_dedup() {
        type Corrupt = (&'static str, fn(&mut Image));
        let corrupt: [Corrupt; 3] = [
            ("no tokens", |i| i.dedup.entries[0].ntoks = 0),
            ("reversed", |i| i.dedup.entries[0].lo = 1 + i.dedup.entries[0].hi),
            ("past the records", |i| i.dedup.entries[0].hi = i.decls.len() as u32 + 1),
        ];
        for (what, f) in corrupt {
            let mut img = with_keys();
            f(&mut img);
            assert!(decode(&encode(&img)).is_err(), "{what}");
        }
    }
}
