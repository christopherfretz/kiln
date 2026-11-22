//! Floating-point pragma states (spec/modules.md §11.11, "Forms carried
//! by WP-1923").
//!
//! Sema keeps the FP pragma state (`kiln_ast::fp::FpOverride`, relative
//! to the command line) in two places, and the image carries both:
//!
//! - each `CompoundStmt` names its block's state (the statement's `aux`,
//!   an index into the AST's override table). A body's record keeps the
//!   kind's `aux` as an index into its own section's table
//!   ([`super::body::Bodies::fp`]: 0 for none, `i` for entry `i - 1`),
//!   and the importer interns each entry into its AST's table;
//! - the cold map `init_fp`: the state of a constructor's
//!   mem-initializers, of a field's default member initializer
//!   (WP-1904) and of a parameter's default argument (WP-1919). Its rows
//!   are `(record, parameter, state)`: the record of the constructor,
//!   field or function, and for a default argument the parameter's
//!   position in that function record ([`NONE`] otherwise).
//!
//! A state is 9 bytes: the override mask (`u16`), the flags (`u16`:
//! reassoc, arcp, nnan, ninf, nsz, afn, math-errno, `FENV_ACCESS`,
//! rounding-math, `strict_fn`, from bit 0), then one byte each for the
//! contraction, rounding mode, exception behavior (0 for none), evaluation
//! method and `CX_LIMITED_RANGE` (0 for none, 1 off, 2 on).

use kiln_ast::DeclId;
use kiln_ast::fp::{FpContract, FpEval, FpExcept, FpOptions, FpOverride, FpRound};

use super::{Image, NONE, Payload, Rd, Res, Wr, Writer, bad};
use crate::Sema;

/// One `init_fp` row: record, parameter position or [`NONE`], state.
pub(super) type FpInit = (u32, u32, FpOverride);

const CONTRACTS: [FpContract; 3] = [FpContract::Off, FpContract::On, FpContract::Fast];
const ROUNDS: [FpRound; 6] = [
    FpRound::Dynamic,
    FpRound::Nearest,
    FpRound::Upward,
    FpRound::Downward,
    FpRound::TowardZero,
    FpRound::NearestAway,
];
const EXCEPTS: [FpExcept; 3] = [FpExcept::Ignore, FpExcept::MayTrap, FpExcept::Strict];
const EVALS: [FpEval; 3] = [FpEval::Source, FpEval::Double, FpEval::Extended];
/// The flag bits a state may set.
const FLAG_BITS: u16 = (1 << 10) - 1;

fn index_of<T: PartialEq>(all: &[T], x: &T) -> u8 {
    all.iter().position(|y| y == x).map_or(0, |i| i as u8)
}

pub(super) fn encode_fp(w: &mut Wr, o: &FpOverride) {
    let v = &o.vals;
    w.u16(o.mask);
    let flags = [
        v.reassoc,
        v.arcp,
        v.nnan,
        v.ninf,
        v.nsz,
        v.afn,
        v.math_errno,
        v.fenv_access,
        v.rounding_math,
        o.strict_fn,
    ];
    w.u16(flags.iter().enumerate().fold(0, |a, (i, &b)| a | (u16::from(b) << i)));
    w.u8(index_of(&CONTRACTS, &v.contract));
    w.u8(index_of(&ROUNDS, &v.round));
    w.u8(v.except.map_or(0, |e| index_of(&EXCEPTS, &e) + 1));
    w.u8(index_of(&EVALS, &v.eval));
    w.u8(o.cx_limited.map_or(0, |b| 1 + u8::from(b)));
}

pub(super) fn decode_fp(r: &mut Rd<'_>) -> Res<FpOverride> {
    let mask = r.u16()?;
    let flags = r.u16()?;
    let (contract, round, except, eval, cx) = (r.u8()?, r.u8()?, r.u8()?, r.u8()?, r.u8()?);
    if flags & !FLAG_BITS != 0 {
        return bad("bad floating-point state flags");
    }
    let bit = |i: u16| flags & (1 << i) != 0;
    let (Some(&contract), Some(&round), Some(&eval)) = (
        CONTRACTS.get(usize::from(contract)),
        ROUNDS.get(usize::from(round)),
        EVALS.get(usize::from(eval)),
    ) else {
        return bad("bad floating-point state");
    };
    let except = match except {
        0 => None,
        k => Some(*EXCEPTS.get(usize::from(k) - 1).ok_or_else(|| {
            super::ImageError("bad floating-point exception behavior".to_owned())
        })?),
    };
    let cx_limited = match cx {
        0 => None,
        1 => Some(false),
        2 => Some(true),
        _ => return bad("bad CX_LIMITED_RANGE state"),
    };
    let vals = FpOptions {
        contract,
        reassoc: bit(0),
        arcp: bit(1),
        nnan: bit(2),
        ninf: bit(3),
        nsz: bit(4),
        afn: bit(5),
        math_errno: bit(6),
        round,
        except,
        fenv_access: bit(7),
        eval,
        rounding_math: bit(8),
    };
    Ok(FpOverride { mask, vals, cx_limited, strict_fn: bit(9) })
}

pub(super) fn encode_fp_inits(w: &mut Wr, rows: &[FpInit]) {
    w.len(rows.len());
    for (rec, param, o) in rows {
        w.u32(*rec);
        w.u32(*param);
        encode_fp(w, o);
    }
}

pub(super) fn decode_fp_inits(r: &mut Rd<'_>) -> Res<Vec<FpInit>> {
    let n = r.count(4 + 4 + 9)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let (rec, param) = (r.u32()?, r.u32()?);
        v.push((rec, param, decode_fp(r)?));
    }
    Ok(v)
}

/// Each row names a function or field record, a parameter only of a
/// function record, in record order.
pub(super) fn validate_fp_inits(img: &Image) -> Res<()> {
    let mut last = None;
    for &(rec, param, _) in &img.fp_inits {
        let ok = match img.decls.get(rec as usize).map(|d| &d.payload) {
            Some(Payload::Function { params, .. }) => {
                param == NONE || (param as usize) < params.len()
            }
            Some(Payload::Field { .. }) => param == NONE,
            _ => false,
        };
        if !ok || last.is_some_and(|l| l >= (rec, param)) {
            return bad("bad floating-point state row");
        }
        last = Some((rec, param));
    }
    Ok(())
}

impl Writer<'_, '_, '_> {
    /// The `aux` of a block whose FP state is `o` in the section being
    /// written: its entry in [`super::body::Bodies::fp`], plus one.
    pub(super) fn fp_index(&mut self, o: FpOverride) -> Result<u16, &'static str> {
        let t = &mut self.img.bodies.fp;
        let i = if let Some(i) = t.iter().position(|x| *x == o) {
            i
        } else {
            t.push(o);
            t.len() - 1
        };
        u16::try_from(i + 1).map_err(|_| "too many floating-point pragma states")
    }

    /// The `init_fp` rows of the declarations that have records, once
    /// every record is written: a parameter's under its function's
    /// record (the parameter's own declaration, since each declaration of
    /// a function has its own parameters).
    pub(super) fn fp_inits(&mut self) {
        let ast = &self.s.ast;
        let mut rows: Vec<FpInit> = Vec::new();
        for (d, &o) in ast.decls.cold.init_fp.iter() {
            let row = match ast.decls.param(d) {
                Some(pd) => ast
                    .decls
                    .get(d)
                    .ctx()
                    .and_then(|f| self.recs.get(&f))
                    .map(|&r| (r, u32::from(pd.index), o)),
                None => self.recs.get(&d).map(|&r| (r, NONE, o)),
            };
            rows.extend(row);
        }
        // The map's order is a hash's: the image's is the records'.
        rows.sort_unstable_by_key(|&(r, p, _)| (r, p));
        self.img.fp_inits = rows;
    }
}

impl Sema<'_, '_> {
    /// Enters `img`'s `init_fp` rows for the declarations `ids` built.
    pub(super) fn image_fp_inits(&mut self, img: &Image, ids: &[DeclId]) {
        for &(rec, param, o) in &img.fp_inits {
            let Some(&d) = ids.get(rec as usize) else { continue };
            let key = if param == NONE {
                Some(d)
            } else {
                self.ast
                    .decls
                    .function(d)
                    .and_then(|f| self.ast.lists.at::<DeclId>(f.params, param as usize))
            };
            if let Some(k) = key {
                let _ = self.ast.decls.cold.init_fp.insert(k, o);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(o: &FpOverride) -> Res<FpOverride> {
        let mut w = Wr(Vec::new());
        encode_fp(&mut w, o);
        assert_eq!(w.0.len(), 9);
        let mut r = Rd(&w.0);
        let back = decode_fp(&mut r)?;
        assert!(r.0.is_empty());
        Ok(back)
    }

    #[test]
    fn states_round_trip() {
        let mut o = FpOverride::NONE;
        assert_eq!(round_trip(&o), Ok(o));
        o.mask = 0x2ff;
        o.vals.round = FpRound::NearestAway;
        o.vals.contract = FpContract::Fast;
        o.vals.except = Some(FpExcept::Strict);
        o.vals.eval = FpEval::Extended;
        o.vals.reassoc = true;
        o.vals.rounding_math = true;
        o.cx_limited = Some(false);
        o.strict_fn = true;
        assert_eq!(round_trip(&o), Ok(o));
        o.vals.except = Some(FpExcept::Ignore);
        o.cx_limited = Some(true);
        assert_eq!(round_trip(&o), Ok(o));
    }

    #[test]
    fn bad_states_are_rejected() {
        let good = {
            let mut w = Wr(Vec::new());
            encode_fp(&mut w, &FpOverride::NONE);
            w.0
        };
        // An unknown flag, contraction, rounding mode, exception
        // behavior, evaluation method, `CX_LIMITED_RANGE`; a short state.
        for (at, v) in [(3, 0x04), (4, 3), (5, 6), (6, 4), (7, 3), (8, 3)] {
            let mut b = good.clone();
            b[at] = v;
            assert!(decode_fp(&mut Rd(&b)).is_err(), "byte {at} = {v}");
        }
        assert!(decode_fp(&mut Rd(&good[..8])).is_err());
    }
}
