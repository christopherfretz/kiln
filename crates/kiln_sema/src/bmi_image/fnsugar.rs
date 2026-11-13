//! Functions declared through a typedef in the declaration image
//! (WP-1369, spec/modules.md §11.11, "Forms carried by WP-1369").
//!
//! A function declared without a function declarator (`typedef int F(int);
//! F f;`, the member `struct S { F m; };`, through an alias template or
//! `decltype`/`__typeof`, [dcl.fct]/13) has the typedef's type as written:
//! sema keeps the sugar on the declaration, and builds one unnamed
//! parameter per parameter type of the function type
//! (`typedef_fn_params`). Its record is an ordinary function record whose
//! type is the function type under the sugar (so the record's parameters
//! match it, as every function record's); the `fsugar` section keeps the
//! written type, which the importer gives the declaration instead, as the
//! parser does. Redeclaration matching, default arguments a later
//! declaration adds and a later definition then behave as after replay:
//! each later declaration is its own record.

use kiln_ast::QualType;
use kiln_ast::types::TypeView;

use super::{Image, Payload, QRef, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;

/// The written types: (function record, the type its declaration was
/// written with), the records ascending.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct FnSugars {
    pub(super) sugars: Vec<(u32, QRef)>,
}

impl FnSugars {
    /// Drops the entries of records from `n` on (a census rollback).
    pub(super) fn truncate(&mut self, n: u32) {
        self.sugars.retain(|&(r, _)| r < n);
    }

    fn get(&self, rec: u32) -> Option<QRef> {
        self.sugars.binary_search_by_key(&rec, |&(r, _)| r).ok().map(|k| self.sugars[k].1)
    }
}

pub(super) fn encode_fn_sugars(w: &mut Wr, s: &FnSugars) {
    w.len(s.sugars.len());
    for &(r, q) in &s.sugars {
        w.u32(r);
        w.q(q);
    }
}

pub(super) fn decode_fn_sugars(r: &mut Rd) -> Res<FnSugars> {
    let n = r.count(9)?;
    let mut sugars = Vec::with_capacity(n);
    for _ in 0..n {
        sugars.push((r.u32()?, r.q()?));
    }
    Ok(FnSugars { sugars })
}

/// Each entry names a function record, once, ascending; its type is
/// unqualified and names records before the function's only (`below`).
pub(super) fn validate_fn_sugars(img: &Image, below: &dyn Fn(QRef, usize) -> bool) -> Res<()> {
    let mut last = None;
    for &(r, q) in &img.fsugar.sugars {
        let ok = last.is_none_or(|l| l < r)
            && q.quals == 0
            && below(q, r as usize)
            && img
                .decls
                .get(r as usize)
                .is_some_and(|d| matches!(d.payload, Payload::Function { .. }));
        if !ok {
            return bad("bad function sugar");
        }
        last = Some(r);
    }
    Ok(())
}

impl Writer<'_, '_, '_> {
    /// The function type under `written`, the type of a function
    /// declaration: `written` itself when it is one, else the first
    /// function type its sugar names (a typedef, alias template,
    /// `decltype` or parenthesized type), unqualified at every step.
    pub(super) fn fn_type_under(&self, written: QualType) -> Option<QualType> {
        let types = &self.s.ast.types;
        let mut cur = written;
        for _ in 0..256 {
            if cur.quals_bits() != 0 {
                return None;
            }
            if matches!(types.view(cur.ty()), TypeView::Function(_)) {
                return Some(cur);
            }
            let next = types.desugar_once(cur);
            if next == cur {
                return None;
            }
            cur = next;
        }
        None
    }

    /// Records that function record `i` was declared with type `written`
    /// (sugar over its record's function type).
    pub(super) fn fn_sugar(&mut self, i: u32, written: QualType) -> W<()> {
        let q = self.qref(written)?;
        if q.quals != 0 {
            return Err("a function declared through a typedef");
        }
        self.img.fsugar.sugars.push((i, q));
        Ok(())
    }
}

/// The importer's side.
impl Sema<'_, '_> {
    /// The type function record `rec` was written with, when the image
    /// records one and it is `fty` under its sugar.
    pub(super) fn image_fn_sugar(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[kiln_ast::DeclId],
        rec: u32,
        fty: QualType,
    ) -> Option<QualType> {
        let q = img.fsugar.get(rec)?;
        let t = self.image_ty(img, tys, ids, q);
        self.ast.types.same_type(t, fty).then_some(t)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::class_template_sample;
    use super::super::{BUILTINS, decode, encode};
    use super::*;

    /// spec/modules.md §11.11 "Forms carried by WP-1369": a function
    /// record's written type round-trips; one on a record that is no
    /// function, given twice, qualified or naming a later type is an
    /// error.
    #[test]
    fn function_sugars_round_trip() {
        type Corrupt = (&'static str, fn(&mut Image, u32));
        let mut img = class_template_sample();
        let f = img
            .decls
            .iter()
            .position(|d| matches!(d.payload, Payload::Function { .. }))
            .expect("a function record") as u32;
        let int = QRef { ty: kiln_ast::types::BuiltinType::Int as u32, quals: 0 };
        img.fsugar.sugars.push((f, int));
        assert_eq!(decode(&encode(&img)).as_ref(), Ok(&img));
        let corrupt: [Corrupt; 4] = [
            ("not a function", |i, f| i.fsugar.sugars[0].0 = f.saturating_sub(1)),
            ("twice", |i, f| i.fsugar.sugars.push((f, i.fsugar.sugars[0].1))),
            ("qualified", |i, _| i.fsugar.sugars[0].1.quals = 1),
            ("a type out of range", |i, _| i.fsugar.sugars[0].1.ty = BUILTINS + 100_000),
        ];
        for (what, c) in corrupt {
            let mut bad = img.clone();
            c(&mut bad, f);
            assert!(decode(&encode(&bad)).is_err(), "{what}");
        }
    }
}
