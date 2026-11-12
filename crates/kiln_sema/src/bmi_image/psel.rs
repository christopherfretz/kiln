//! Partial specialization selections in the declaration image (WP-1358,
//! spec/modules.md §11.11, "Forms carried by WP-1358").
//!
//! The importer completes a specialization the unit completed while it
//! reads the records, before the constraints are patched in (they are
//! expressions of the bodies, [`super::constraints`]), and before the
//! bodies a constraint's satisfaction may evaluate (a variable template's
//! initializer, a `constexpr` function). Matching the partial
//! specializations of a template with a constrained one
//! ([temp.spec.partial.match]/2) cannot be repeated then, so the image
//! keeps the unit's result: per complete specialization reference of such
//! a template, the partial specialization its definition was instantiated
//! from, or none (the primary template). The importer's matching for that
//! specialization deduces the arguments of the recorded partial
//! specialization only ([temp.spec.partial.match]/1: the arguments
//! matched it, so deduction succeeds), or takes the primary template.

use kiln_ast::DeclId;
use kiln_ast::templates::{SpecInfo, SpecKind};
use kiln_source::fx::FxHashSet;

use super::{Image, NONE, Payload, Rd, Res, W, Wr, bad, classtempl};
use crate::Sema;

/// The selections: (specialization reference record, partial
/// specialization record or [`NONE`] for the primary template), the
/// specialization records ascending.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct PartialSels {
    pub(super) sels: Vec<(u32, u32)>,
}

impl PartialSels {
    /// Drops the entries of records from `n` on (a census rollback).
    pub(super) fn truncate(&mut self, n: u32) {
        self.sels.retain(|&(r, _)| r < n);
    }

    /// The selection recorded for specialization reference `rec`.
    fn get(&self, rec: u32) -> Option<u32> {
        self.sels.binary_search_by_key(&rec, |&(r, _)| r).ok().map(|k| self.sels[k].1)
    }
}

pub(super) fn encode_partial_sels(w: &mut Wr, s: &PartialSels) {
    w.len(s.sels.len());
    for &(r, p) in &s.sels {
        w.u32(r);
        w.u32(p);
    }
}

pub(super) fn decode_partial_sels(r: &mut Rd) -> Res<PartialSels> {
    let n = r.count(8)?;
    let mut sels = Vec::with_capacity(n);
    for _ in 0..n {
        sels.push((r.u32()?, r.u32()?));
    }
    Ok(PartialSels { sels })
}

/// Each entry names a complete specialization reference of a class
/// template, once, ascending; its partial specialization, when not
/// [`NONE`], is a carried partial specialization of that template
/// recorded before it.
pub(super) fn validate_partial_sels(img: &Image) -> Res<()> {
    let mut last = None;
    for &(r, p) in &img.psels.sels {
        let ok = last.is_none_or(|l| l < r)
            && img.decls.get(r as usize).is_some_and(|d| match d.payload {
                // A class template's complete specialization, or a
                // variable template's.
                Payload::Spec { template, complete, .. } => {
                    matches!(
                        (img.decls.get(template as usize).map(|t| &t.payload), complete),
                        (Some(Payload::Template { kind: 1, .. }), true)
                            | (Some(Payload::Template { kind: 3, .. }), false)
                    ) && (p == NONE || p < r && classtempl::partial_of(img, p) == Some(template))
                }
                _ => false,
            });
        if !ok {
            return bad("bad partial specialization selection");
        }
        last = Some(r);
    }
    Ok(())
}

impl super::Writer<'_, '_, '_> {
    /// Records the selection for complete specialization reference `rec`:
    /// partial specialization record `partial`, or [`NONE`].
    pub(super) fn partial_sel_commit(&mut self, rec: u32, partial: u32) {
        self.img.psels.sels.push((rec, partial));
    }

    /// The selection to record for variable template specialization
    /// `si` (WP-1358): the importer declares it, matching the partial
    /// specializations, while it reads the records ([temp.inst]/7), so
    /// every specialization of a template with partial specializations
    /// keeps the unit's result. `Err` when the partial specialization it
    /// was instantiated from has no record.
    pub(super) fn var_partial_sel(&self, si: &SpecInfo) -> W<Option<u32>> {
        let ast = &self.s.ast;
        if ast.templates.partials_of(ast.first(si.template)).is_empty() {
            return Ok(None);
        }
        if si.from_args.is_none() {
            return Ok(Some(NONE));
        }
        match self.recs.get(&ast.first(si.from)) {
            Some(&p) if classtempl::partial_of(&self.img, p).is_some() => Ok(Some(p)),
            _ => Err(super::varspec::VAR_SPEC),
        }
    }

    /// Whether specialization `si` (of a variable template when `var`)
    /// was instantiated from a carried partial specialization whose
    /// definition is recorded and complete (WP-1358: one declared before
    /// its definition, `iterator_concepts.h:75`, has its definition's
    /// record later).
    pub(super) fn instantiated_from_partial(&mut self, si: &SpecInfo, var: bool) -> bool {
        let ast = &self.s.ast;
        si.from_args.is_some()
            && ast.partial_spec_info(si.from).is_some()
            && self
                .recs
                .get(&ast.first(si.from))
                .is_some_and(|&p| classtempl::partial_of(&self.img, p).is_some())
            && self.class_rec(ast.first(si.from)).is_some_and(|p| {
                matches!(&self.img.decls[p as usize].payload,
                    Payload::Record { def: Some(df), .. } if df.end != NONE)
                    // A variable template's (WP-1358).
                    || var && matches!(self.img.decls[p as usize].payload, Payload::Var { .. })
            })
    }

    /// Writes, before the reference of complete class specialization
    /// `d`, the references of the classes its completion completes in
    /// turn (its bases and the classes of its non-static data members, at
    /// any depth) that need a selection: the importer completes them
    /// while it completes `d`, and would match their partial
    /// specializations without the constraints. Each is then completed
    /// first, with its selection.
    pub(super) fn pre_record_sels(&mut self, d: DeclId) -> W<()> {
        let s = self.s;
        let ast = &s.ast;
        let mut stack = vec![d];
        let mut seen: FxHashSet<DeclId> = FxHashSet::default();
        let mut need = Vec::new();
        while let Some(c) = stack.pop() {
            if seen.len() > 4096 || !seen.insert(c) {
                continue;
            }
            let Some(def) = ast.definition(c) else { continue };
            let Some(r) = ast.decls.record(def) else { continue };
            let mut classes: Vec<DeclId> = ast
                .lists
                .get(r.bases)
                .iter()
                .filter_map(|&g| s.base_class(ast.decls.base_spec(g)))
                .collect();
            classes.extend(
                ast.members(def)
                    .filter(|&m| ast.decls.field(m).is_some())
                    .filter_map(|m| ast.decls.get(m).ty().and_then(|q| s.element_class(q))),
            );
            for x in classes.into_iter().rev() {
                let x = ast.first(x);
                if x == ast.first(d) || self.recs.contains_key(&x) {
                    continue;
                }
                if ast.spec_info(x).is_some_and(|(_, si)| {
                    matches!(si.kind, SpecKind::ImplicitInstantiation)
                        && self.constrained_partials(si.template)
                }) {
                    need.push(x);
                }
                stack.push(x);
            }
        }
        for x in need {
            if !self.recs.contains_key(&x) {
                self.spec_rec(x)?;
            }
        }
        Ok(())
    }
}

/// What a declaration image recorded a specialization was instantiated
/// from.
#[derive(Clone, Copy)]
pub(crate) enum PartialChoice {
    /// The primary template.
    Primary,
    /// The partial specialization (its first declaration).
    Partial(DeclId),
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Notes the selection the image records for reference `rec`, which
    /// resolved to class specialization `spec`, before the importer
    /// completes it.
    pub(super) fn image_partial_sel(
        &mut self,
        img: &Image,
        ids: &[DeclId],
        rec: u32,
        spec: DeclId,
    ) {
        let Some(p) = img.psels.get(rec) else { return };
        let partial = (p != NONE).then(|| ids.get(p as usize).map(|&d| self.ast.first(d)));
        let Some(partial) = partial.map_or(Some(None), |x| x.map(Some)) else { return };
        self.image_texprs.partial_sels.insert(spec, partial);
    }

    /// Notes the selection the image records for variable template
    /// specialization reference `rec` of template `tmpl`, for the
    /// declaration [`Self::image_var_template_spec`] makes next (its
    /// partial matching names the template); `true` when there is one,
    /// to drop with [`Self::image_var_sel_done`].
    pub(super) fn image_var_sel(
        &mut self,
        img: &Image,
        ids: &[DeclId],
        rec: u32,
        tmpl: DeclId,
    ) -> bool {
        let Some(p) = img.psels.get(rec) else { return false };
        let partial = if p == NONE {
            None
        } else {
            match ids.get(p as usize) {
                Some(&d) => Some(self.ast.first(d)),
                None => return false,
            }
        };
        self.image_texprs.partial_sels.insert(tmpl, partial);
        true
    }

    /// Drops the selection [`Self::image_var_sel`] noted for `tmpl`.
    pub(super) fn image_var_sel_done(&mut self, tmpl: DeclId) {
        self.image_texprs.partial_sels.remove(&tmpl);
    }

    /// The selection a declaration image being built recorded for class
    /// specialization `spec` ([`PartialSels`]); `None` when it recorded
    /// none.
    pub(crate) fn image_partial_choice(&self, spec: DeclId) -> Option<PartialChoice> {
        self.image_texprs.partial_sels.get(&spec).map(|&p| match p {
            None => PartialChoice::Primary,
            Some(p) => PartialChoice::Partial(p),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::class_template_sample;
    use super::super::{decode, encode};
    use super::*;

    /// spec/modules.md §11.11 "Forms carried by WP-1358": a selection of
    /// the primary template for a complete specialization reference
    /// round-trips; one naming no complete class specialization
    /// reference, a record that is no partial specialization of its
    /// template, or given twice, is an error.
    #[test]
    fn partial_selections_round_trip() {
        type Corrupt = (&'static str, fn(&mut Image));
        // Record 19 is `S<int>`, complete; 16 its template, 15 the pattern.
        let mut img = class_template_sample();
        img.psels.sels.push((19, NONE));
        assert_eq!(decode(&encode(&img)).as_ref(), Ok(&img));
        let corrupt: [Corrupt; 4] = [
            ("a member reference", |i| i.psels.sels[0].0 = 20),
            ("not a partial specialization", |i| i.psels.sels[0].1 = 15),
            ("a later record", |i| i.psels.sels[0].1 = 21),
            ("twice", |i| i.psels.sels.push((19, NONE))),
        ];
        for (what, f) in corrupt {
            let mut bad = img.clone();
            f(&mut bad);
            assert!(decode(&encode(&bad)).is_err(), "{what}");
        }
    }
}
