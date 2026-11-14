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
//! The precompiled header's image (retired by WP-1397) wrote one entry
//! per namespace-scope declaration its producer parsed: first and last
//! token locations as image locations, the token count and the
//! declarations as the range of records made for them. A module's image
//! writes the section empty; the reader still accepts it:
//!
//! ```text
//! dedup := n:u32 (start:u32 end:u32 ntoks:u32 lo:u32 hi:u32)*n
//! ```
//!
//! in parse order, `lo <= hi <= ` the record count (`lo == hi`: no
//! record).

use super::{Image, Rd, Res, Wr, bad};

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
