//! Defaulted comparison operators in the declaration image
//! (spec/modules.md §11.11, "Defaulted comparisons"; WP-1314).
//!
//! `bool operator==(const S&) const = default;` and its friend form
//! ([class.compare.default]) are function records like any other: the
//! record holds the type, `constexpr`-ness, deletedness and deferred
//! exception specification the unit's class-completion check
//! (`check_defaulted_comparison`) gave them. What sema keeps beside the
//! declaration travels in this section, one entry per defaulted
//! comparison: the location of its `default` keyword (where the
//! synthesized body is placed and its diagnostics point,
//! `special.cmp.default_loc`) and the unqualified operator lookups its
//! subobject comparisons use, captured at the `= default`
//! (`special.cmp.lookups`, clang `lookupOperatorsForDefaultedFunction`).
//! The importer puts both back before anything can use the operator;
//! a use then defines the body exactly as after a replay
//! (`define_cmp_on_use`, the Oven's on-demand definition), and a class
//! template's instantiation copies both from the pattern as it does
//! after a replay.
//!
//! Not carried (the declaration falls back with its own reason): an
//! invalid one, one with a deduced return type (`auto operator<=>`,
//! sema's `declared_returns`), a `<=>` that declared an implicit
//! `operator==`, and a lookup that found a declaration through a
//! using-declaration or outside the image.

use kiln_ast::DeclId;

use super::locs::Locs;
use super::{Image, NONE, Payload, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;

/// One defaulted comparison.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct CmpRec {
    /// The function record.
    rec: u32,
    /// The `default` keyword's location.
    kw: u32,
    /// The captured operator lookups (records), `None` when the unit
    /// captured none (declared inside an instantiation).
    lookups: Option<Vec<u32>>,
}

/// The section, ascending by record.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct DefCmps {
    recs: Vec<CmpRec>,
}

impl DefCmps {
    /// Drops the entries of records from `n` on (the census writer's
    /// rollback).
    pub(super) fn truncate(&mut self, n: u32) {
        self.recs.retain(|c| c.rec < n);
    }
}

impl Writer<'_, '_, '_> {
    /// Records defaulted comparison `d` (a first declaration), record
    /// `rec`.
    pub(super) fn defaulted_cmp(&mut self, d: DeclId, rec: u32) -> W<()> {
        let s = self.s;
        let cmp = &s.special.cmp;
        if cmp.invalid.contains(&d) {
            return Err("an invalid defaulted comparison");
        }
        if s.declared_returns.contains_key(&d) {
            return Err("a defaulted comparison with a deduced return type");
        }
        if cmp.implicit_eq.contains_key(&d) {
            return Err("a defaulted three-way comparison that declared an equality operator");
        }
        let &(f, dl) = cmp.default_loc.get(&d).ok_or("a malformed defaulted comparison")?;
        if f != d {
            return Err("a malformed defaulted comparison");
        }
        let lookups = match cmp.lookups.get(&d) {
            None => None,
            Some(list) => {
                let mut out = Vec::with_capacity(list.len());
                for &(x, found) in list {
                    if x != found {
                        return Err(
                            "a defaulted comparison whose operator lookup found a using-declaration",
                        );
                    }
                    let r = if x == d { Some(rec) } else { self.recs.get(&x).copied() };
                    out.push(r.ok_or(
                        "a defaulted comparison whose operator lookup found a declaration outside the image",
                    )?);
                }
                Some(out)
            }
        };
        let kw = self.loc(dl)?;
        self.img.defcmps.recs.push(CmpRec { rec, kw, lookups });
        Ok(())
    }
}

pub(super) fn encode_defaulted_cmps(w: &mut Wr, s: &DefCmps) {
    w.len(s.recs.len());
    for c in &s.recs {
        w.u32(c.rec);
        w.u32(c.kw);
        match &c.lookups {
            None => w.u32(NONE),
            Some(l) => {
                w.len(l.len());
                for &x in l {
                    w.u32(x);
                }
            }
        }
    }
}

pub(super) fn decode_defaulted_cmps(r: &mut Rd) -> Res<DefCmps> {
    let n = r.count(12)?;
    let mut recs = Vec::with_capacity(n);
    for _ in 0..n {
        let (rec, kw) = (r.u32()?, r.u32()?);
        let k = r.u32()?;
        let lookups = if k == NONE {
            None
        } else {
            let k = k as usize;
            if k > r.0.len() / 4 {
                return bad("bad defaulted comparison");
            }
            let mut l = Vec::with_capacity(k);
            for _ in 0..k {
                l.push(r.u32()?);
            }
            Some(l)
        };
        recs.push(CmpRec { rec, kw, lookups });
    }
    Ok(DefCmps { recs })
}

/// The section's locations, for the decoder's offset check.
pub(super) fn defaulted_cmp_locs(img: &Image, offs: &mut Vec<u32>) {
    offs.extend(img.defcmps.recs.iter().map(|c| c.kw));
}

/// Checks the section: ascending function records, each lookup a
/// function or template record.
pub(super) fn validate_defaulted_cmps(img: &Image) -> Res<()> {
    let mut last = None;
    for c in &img.defcmps.recs {
        let func = matches!(
            img.decls.get(c.rec as usize).map(|d| &d.payload),
            Some(Payload::Function { .. })
        );
        let lookups_ok = c.lookups.iter().flatten().all(|&x| {
            matches!(
                img.decls.get(x as usize).map(|d| &d.payload),
                Some(Payload::Function { .. } | Payload::Template { .. })
            )
        });
        if !func || !lookups_ok || last.is_some_and(|l| l >= c.rec) {
            return bad("bad defaulted comparison");
        }
        last = Some(c.rec);
    }
    Ok(())
}

impl Sema<'_, '_> {
    /// Puts back what sema keeps beside the image's defaulted
    /// comparisons, before anything uses them.
    pub(super) fn image_defaulted_cmps(&mut self, img: &Image, ids: &[DeclId], main: &Locs) {
        for c in &img.defcmps.recs {
            let f = ids[c.rec as usize];
            let first = self.ast.first(f);
            let _ = self.special.cmp.default_loc.insert(first, (f, main.get(c.kw)));
            if let Some(l) = &c.lookups {
                let found: Vec<(DeclId, DeclId)> =
                    l.iter().map(|&x| (ids[x as usize], ids[x as usize])).collect();
                let _ = self.special.cmp.lookups.insert(first, found);
            }
        }
    }
}
