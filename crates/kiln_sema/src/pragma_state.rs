//! Sema's pragma state at the end of a translation unit, for the
//! preprocessor-state record of a PCH (spec/modules.md §11.13 "Sema
//! pragma state"): `#pragma pack`, `#pragma ms_struct`, `#pragma GCC
//! visibility` and the FP pragmas (`float_control`, `fp_contract`,
//! `STDC FP_CONTRACT` / `CX_LIMITED_RANGE`, `clang fp`).

use kiln_ast::fp::{FpContract, FpOptions, FpOverride};
use kiln_source::Loc;

use crate::Sema;

/// The pragma state sema keeps across declarations (each field as the
/// [`Sema`] field it copies).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PragmaState {
    /// The `#pragma pack` value in effect (0: none).
    pub pack: u16,
    /// The `#pragma pack(push)` entries: `(label, value)`, innermost last.
    pub pack_stack: Vec<(Option<String>, u16)>,
    /// `#pragma ms_struct on` is in effect.
    pub ms_struct: bool,
    /// The `#pragma GCC visibility push` entries, innermost last:
    /// `(code, location)` with 0 default, 1 hidden, 2 protected (`None`:
    /// a namespace's visibility attribute).
    pub visibility: Vec<Option<(u32, Loc)>>,
    /// The FP override in effect, as [`fp_bits`].
    pub fp: u64,
    /// The `float_control(push)` entries, as [`fp_bits`].
    pub fp_stack: Vec<u64>,
}

/// An FP override as bits: the mask in bits 0-15, the contraction in
/// 16-17 (off, on, fast), `reassoc`, `arcp`, `nnan`, `ninf`, `nsz`, `afn`,
/// `math_errno` in 18-24, `CX_LIMITED_RANGE` in 25-26 (unset, off, on).
#[must_use]
pub fn fp_bits(o: &FpOverride) -> u64 {
    let v = &o.vals;
    let contract = match v.contract {
        FpContract::Off => 0,
        FpContract::On => 1,
        FpContract::Fast => 2,
    };
    let flags = [v.reassoc, v.arcp, v.nnan, v.ninf, v.nsz, v.afn, v.math_errno]
        .iter()
        .enumerate()
        .fold(0u64, |a, (i, &b)| a | (u64::from(b) << (18 + i)));
    let cx = match o.cx_limited {
        None => 0,
        Some(false) => 1,
        Some(true) => 2,
    };
    u64::from(o.mask) | (contract << 16) | flags | (cx << 25)
}

/// The FP override of [`fp_bits`] `b`; `None` for bits no override has.
#[must_use]
pub fn fp_from_bits(b: u64) -> Option<FpOverride> {
    if b >> 27 != 0 {
        return None;
    }
    let bit = |i: u32| b & (1 << i) != 0;
    let contract = match (b >> 16) & 3 {
        0 => FpContract::Off,
        1 => FpContract::On,
        2 => FpContract::Fast,
        _ => return None,
    };
    let cx_limited = match (b >> 25) & 3 {
        0 => None,
        1 => Some(false),
        2 => Some(true),
        _ => return None,
    };
    Some(FpOverride {
        mask: (b & 0xffff) as u16,
        vals: FpOptions {
            contract,
            reassoc: bit(18),
            arcp: bit(19),
            nnan: bit(20),
            ninf: bit(21),
            nsz: bit(22),
            afn: bit(23),
            math_errno: bit(24),
        },
        cx_limited,
    })
}

impl Sema<'_, '_> {
    /// The pragma state now ([`PragmaState`]).
    #[must_use]
    pub fn pragma_state(&self) -> PragmaState {
        PragmaState {
            pack: self.pack.0,
            pack_stack: self.pack.1.clone(),
            ms_struct: self.ms_struct_pragma,
            visibility: self.vis_stack.clone(),
            fp: fp_bits(&self.fp_pragma.0),
            fp_stack: self.fp_pragma.1.iter().map(fp_bits).collect(),
        }
    }

    /// Installs a precompiled header's pragma state in place of the
    /// header's pragmas (WP-1036; the state the header left, read from
    /// its preprocessor-state record). FP bits no override has (a
    /// corrupt record) leave the FP state as it is.
    pub fn install_pragma_state(&mut self, p: &PragmaState) {
        self.pack = (p.pack, p.pack_stack.clone());
        self.ms_struct_pragma = p.ms_struct;
        self.vis_stack.clone_from(&p.visibility);
        let stack: Option<Vec<FpOverride>> = p.fp_stack.iter().map(|&b| fp_from_bits(b)).collect();
        if let (Some(fp), Some(stack)) = (fp_from_bits(p.fp), stack) {
            self.fp_pragma = (fp, stack);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fp_bits_round_trip() {
        let mut o = FpOverride::NONE;
        o.mask = 0x5a;
        o.vals.contract = FpContract::Fast;
        o.vals.nsz = true;
        o.vals.math_errno = true;
        o.cx_limited = Some(true);
        assert_eq!(fp_from_bits(fp_bits(&o)), Some(o));
        assert_eq!(fp_from_bits(fp_bits(&FpOverride::NONE)), Some(FpOverride::NONE));
        assert_eq!(fp_from_bits(1 << 40), None);
    }
}
