//! The split `>` tokens of the image's locations (spec/ast.md §21.2,
//! spec/modules.md §11.11; WP-2542).
//!
//! The parser takes the first `>` of a `>>`, `>=` or `>>=` as the end of
//! a template argument list ([temp.names]/4) and records its location in
//! `Ast::split_greaters`, so sema's `token_end` and the tooling query
//! `kiln_lex::token_end_split` measure it as one character. An importer
//! that builds the image's bodies holds their locations, so the image
//! carries every split location whose range or macro expansion it
//! carries anyway (the main file always; an expansion only when another
//! location names it, so the section adds no location records), in
//! ascending order, and the importer adds them to its own set.

use super::{Image, Rd, Res, Wr, Writer, bad};
use crate::Sema;
use kiln_source::Loc;

impl Writer<'_, '_, '_> {
    /// Lists the split locations the image carries (after every other
    /// section has carried its locations).
    pub(super) fn splits_section(&mut self) {
        let sm = self.s.diags.sm();
        let lt = self.lt.borrow();
        let mut out: Vec<u32> =
            self.s.ast.split_greaters.iter().filter_map(|&l| lt.carried_loc(sm, l)).collect();
        out.sort_unstable();
        out.dedup();
        drop(lt);
        self.img.splits = out;
    }
}

pub(super) fn encode_splits(w: &mut Wr, s: &[u32]) {
    w.u32(u32::try_from(s.len()).unwrap_or(u32::MAX));
    for &o in s {
        w.u32(o);
    }
}

pub(super) fn decode_splits(r: &mut Rd) -> Res<Vec<u32>> {
    let n = r.u32()? as usize;
    if n.saturating_mul(4) > r.0.len() {
        return bad("length exceeds the section");
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(r.u32()?);
    }
    if !out.windows(2).all(|w| w[0] < w[1]) {
        return bad("split locations not ascending");
    }
    Ok(out)
}

/// The section's locations, for the decoder's offset check.
pub(super) fn split_offsets(img: &Image, offs: &mut Vec<u32>) {
    offs.extend(img.splits.iter().copied());
}

impl Sema<'_, '_> {
    /// Adds the image's split locations, mapped by `at`, to the unit's.
    pub(super) fn image_splits(&mut self, img: &Image, at: impl Fn(u32) -> Loc) {
        self.ast.split_greaters.extend(img.splits.iter().map(|&o| at(o)).filter(|l| l.is_valid()));
    }
}

#[cfg(test)]
mod tests {
    use super::{Rd, Wr, decode_splits, encode_splits};

    #[test]
    fn splits_round_trip_and_bad_ones_are_rejected() {
        let enc = |s: &[u32]| {
            let mut w = Wr(Vec::new());
            encode_splits(&mut w, s);
            w.0
        };
        for s in [&[][..], &[3], &[1, 2, 40]] {
            assert_eq!(decode_splits(&mut Rd(&enc(s))).unwrap(), s);
        }
        // Out of order, repeated, a count past the section, a short one.
        assert!(decode_splits(&mut Rd(&enc(&[2, 1]))).is_err());
        assert!(decode_splits(&mut Rd(&enc(&[2, 2]))).is_err());
        let mut long = enc(&[1]);
        long[0] = 9;
        assert!(decode_splits(&mut Rd(&long)).is_err());
        let short = enc(&[1, 2]);
        assert!(decode_splits(&mut Rd(&short[..short.len() - 1])).is_err());
    }
}
