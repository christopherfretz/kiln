//! Where the image's dependent names were written (spec/modules.md
//! §11.11, "Dependent types", WP-1284; spec/sema-templates.md §5.8).
//!
//! Types carry no locations, so the parser records each occurrence of a
//! dependent type or qualifier component (`T::inner::`) in a side table
//! the substituter reports at. The image carries the entries whose key it
//! carries (a type already in the type section, a qualifier whose
//! components are), so an error substituting an imported pattern sits at
//! the same name, with the same caret, as after a replay.
//!
//! The same holds for the written parameters of function types inside
//! other declarators (`templ.nested_params`, spec/sema-templates.md
//! §5.8.1; WP-1859): substituting an imported pattern checks them again
//! (an address space on the parameter, a Swift-ABI parameter's type).

use kiln_ast::types::Nns;
use kiln_ast::{DeclId, NnsId, QualType};

use super::{Image, NnsRec, Payload, QRef, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;
use crate::templ::written::{NestedParam, WrittenKey};

/// One entry: its key and its locations.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct WrittenRec {
    key: Key,
    locs: Vec<u32>,
}

/// The written parameters `i` of the function types whose parameter
/// type is `ty` (`templ.nested_params`): per parameter its location, its
/// Swift parameter ABI attribute's (`NONE` when absent) and whether it
/// was declared as an array.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct NestedRec {
    ty: QRef,
    index: u16,
    params: Vec<(u32, u32, bool)>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum Key {
    Ty(QRef),
    Nns(Vec<NnsRec>),
}

impl Writer<'_, '_, '_> {
    /// Adds the written locations of the carried dependent names.
    pub(super) fn written_section(&mut self) {
        let mut out: Vec<(u32, WrittenRec)> = Vec::new();
        let table: Vec<(WrittenKey, Vec<kiln_source::Loc>)> =
            self.s.templ.written_locs.iter().map(|(k, v)| (*k, v.clone())).collect();
        for (k, v) in table {
            let key = match k {
                WrittenKey::Type(q) if self.tmemo.contains_key(&q.ty()) => match self.qref(q) {
                    Ok(q) => Key::Ty(q),
                    Err(_) => continue,
                },
                WrittenKey::Nns(n) if self.nns_carried(n) => match self.nns_steps(n) {
                    Ok(s) => Key::Nns(s),
                    Err(_) => continue,
                },
                _ => continue,
            };
            let locs: Vec<u32> = v.iter().filter_map(|&l| self.loc(l).ok()).collect();
            if let Some(&first) = locs.first() {
                out.push((first, WrittenRec { key, locs }));
            }
        }
        // Deterministic: by first location, then key.
        out.sort_by(|a, b| {
            a.0.cmp(&b.0).then_with(|| format!("{:?}", a.1).cmp(&format!("{:?}", b.1)))
        });
        self.img.written = out.into_iter().map(|(_, r)| r).collect();
        self.nested_section();
    }

    /// Adds the written nested parameters whose type is carried.
    fn nested_section(&mut self) {
        let table: Vec<((QualType, u16), Vec<NestedParam>)> =
            self.s.templ.nested_params.iter().map(|(k, v)| (*k, v.clone())).collect();
        let mut out: Vec<(u32, NestedRec)> = Vec::new();
        for ((q, index), v) in table {
            if !self.tmemo.contains_key(&q.ty()) {
                continue;
            }
            let Ok(ty) = self.qref(q) else { continue };
            let params: Vec<(u32, u32, bool)> = v
                .iter()
                .filter_map(|p| {
                    let loc = self.loc(p.loc).ok().filter(|&l| l != super::NONE)?;
                    Some((loc, self.loc(p.abi_loc).unwrap_or(super::NONE), p.array))
                })
                .collect();
            if let Some(&(first, ..)) = params.first() {
                out.push((first, NestedRec { ty, index, params }));
            }
        }
        // Deterministic: by first location, then key.
        out.sort_by(|a, b| {
            a.0.cmp(&b.0).then_with(|| format!("{:?}", a.1).cmp(&format!("{:?}", b.1)))
        });
        self.img.nested = out.into_iter().map(|(_, r)| r).collect();
    }

    /// Whether every component of qualifier `n` is already carried.
    fn nns_carried(&self, n: NnsId) -> bool {
        let mut cur = Some(n);
        for _ in 0..1024 {
            let Some(id) = cur else { return true };
            cur = match self.s.ast.types.nns_view(id) {
                Nns::Global => None,
                Nns::Type { prefix, ty, .. } if self.tmemo.contains_key(&ty.ty()) => prefix,
                Nns::Namespace { prefix, ns } if self.recs.contains_key(&ns) => prefix,
                Nns::NamespaceAlias { prefix, alias, .. } if self.recs.contains_key(&alias) => {
                    prefix
                }
                Nns::Identifier { prefix, .. } => prefix,
                _ => return false,
            };
        }
        false
    }

    /// Carries `ColdDeclData::written_decl_type` for every carried
    /// declaration that has one (spec/types.md §8.4, spec/modules.md
    /// §11.11, WP-3640), in ascending record order.
    pub(super) fn written_decl_types(&mut self) -> W<()> {
        let mut pairs: Vec<(u32, QualType)> = self
            .s
            .ast
            .decls
            .cold
            .written_decl_type
            .iter()
            .filter_map(|(d, &ty)| self.recs.get(&d).map(|&r| (r, ty)))
            .collect();
        pairs.sort_unstable_by_key(|&(r, _)| r);
        let mut rows = Vec::with_capacity(pairs.len());
        for (r, ty) in pairs {
            rows.push((r, self.qref(ty)?));
        }
        self.img.written_decls = rows;
        Ok(())
    }
}

pub(super) fn encode_written_decls(w: &mut Wr, rows: &[(u32, QRef)]) {
    w.len(rows.len());
    for &(r, q) in rows {
        w.u32(r);
        w.q(q);
    }
}

pub(super) fn decode_written_decls(r: &mut Rd<'_>) -> Res<Vec<(u32, QRef)>> {
    let n = r.count(9)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push((r.u32()?, r.q()?));
    }
    Ok(v)
}

/// Checks each `written_decls` entry: ascending distinct variable or
/// function records, each type in range.
pub(super) fn validate_written_decls(img: &Image, qref_ok: &dyn Fn(QRef) -> bool) -> Res<()> {
    let mut last = None;
    for &(r, q) in &img.written_decls {
        let ok = last.is_none_or(|l| l < r)
            && qref_ok(q)
            && img.decls.get(r as usize).is_some_and(|d| {
                matches!(d.payload, Payload::Var { .. } | Payload::Function { .. })
            });
        if !ok {
            return bad("bad written declaration type row");
        }
        last = Some(r);
    }
    Ok(())
}

pub(super) fn encode_written(w: &mut Wr, v: &[WrittenRec]) {
    w.len(v.len());
    for r in v {
        match &r.key {
            Key::Ty(q) => {
                w.u8(0);
                w.q(*q);
            }
            Key::Nns(s) => {
                w.u8(1);
                w.steps(s);
            }
        }
        w.len(r.locs.len());
        for &l in &r.locs {
            w.u32(l);
        }
    }
}

pub(super) fn decode_written(r: &mut Rd<'_>) -> Res<Vec<WrittenRec>> {
    let n = r.count(6)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let key = match r.u8()? {
            0 => Key::Ty(r.q()?),
            1 => Key::Nns(r.steps()?),
            _ => return bad("bad written-location key"),
        };
        let k = r.count(4)?;
        let mut locs = Vec::with_capacity(k);
        for _ in 0..k {
            locs.push(r.u32()?);
        }
        v.push(WrittenRec { key, locs });
    }
    Ok(v)
}

pub(super) fn encode_nested(w: &mut Wr, v: &[NestedRec]) {
    w.len(v.len());
    for r in v {
        w.q(r.ty);
        w.u16(r.index);
        w.len(r.params.len());
        for &(loc, abi, array) in &r.params {
            w.u32(loc);
            w.u32(abi);
            w.u8(u8::from(array));
        }
    }
}

pub(super) fn decode_nested(r: &mut Rd<'_>) -> Res<Vec<NestedRec>> {
    let n = r.count(10)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let ty = r.q()?;
        let index = r.u16()?;
        let k = r.count(9)?;
        let mut params = Vec::with_capacity(k);
        for _ in 0..k {
            let loc = r.u32()?;
            let abi = r.u32()?;
            let array = match r.u8()? {
                0 => false,
                1 => true,
                _ => return bad("bad nested parameter"),
            };
            params.push((loc, abi, array));
        }
        v.push(NestedRec { ty, index, params });
    }
    Ok(v)
}

/// Checks each nested parameter record's type.
pub(super) fn validate_nested(img: &Image, qref_ok: &dyn Fn(QRef) -> bool) -> Res<()> {
    if img.nested.iter().all(|r| qref_ok(r.ty)) { Ok(()) } else { bad("bad nested parameter type") }
}

/// Checks each key's type and qualifier steps.
pub(super) fn validate_written(img: &Image, qref_ok: &dyn Fn(QRef) -> bool) -> Res<()> {
    for r in &img.written {
        let ok = match &r.key {
            Key::Ty(q) => qref_ok(*q),
            Key::Nns(s) => {
                super::steps_ok(img, s, img.decls.len())
                    && s.iter().all(|st| !matches!(st, NnsRec::Ty(q, _) if !qref_ok(*q)))
            }
        };
        if !ok {
            return bad("bad written-location key");
        }
    }
    Ok(())
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Records the written locations of the image's dependent names.
    pub(super) fn image_written(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        main: &super::Locs,
    ) {
        for r in &img.written {
            let key = match &r.key {
                Key::Ty(q) => WrittenKey::Type(self.image_ty(img, tys, ids, *q)),
                Key::Nns(s) => {
                    let mut qt = |me: &mut Self, q: QRef| me.image_ty(img, tys, ids, q);
                    match self.image_nns(img, s, ids, &mut qt) {
                        Some(n) => WrittenKey::Nns(n),
                        None => continue,
                    }
                }
            };
            let v = self.templ.written_locs.entry(key).or_default();
            v.extend(r.locs.iter().map(|&l| main.get(l)).filter(|l| l.is_valid()));
            v.sort_unstable();
            v.dedup();
        }
        for r in &img.nested {
            let ty = self.image_ty(img, tys, ids, r.ty);
            let v = self.templ.nested_params.entry((ty, r.index)).or_default();
            for &(loc, abi, array) in &r.params {
                let loc = main.get(loc);
                if !loc.is_valid() {
                    continue;
                }
                let np = NestedParam { loc, abi_loc: main.get(abi), array };
                let at = v.partition_point(|p| p.loc < np.loc);
                if v.get(at).is_none_or(|p| p.loc != np.loc) {
                    v.insert(at, np);
                }
            }
        }
    }

    /// Enters `img`'s `written_decls` rows into
    /// `ColdDeclData::written_decl_type` (WP-3640).
    pub(super) fn image_written_decls(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
    ) {
        for &(rec, q) in &img.written_decls {
            let Some(&d) = ids.get(rec as usize) else { continue };
            let ty = self.image_ty(img, tys, ids, q);
            self.builder().set_written_decl_type(d, ty);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::template_sample;
    use super::super::{BUILTINS, NONE, decode, encode};
    use super::*;

    /// `template_sample` (its parameter `T` is type `t`) with the written
    /// locations of `T` and of `T::x::`.
    fn written_sample() -> Image {
        let t = QRef { ty: BUILTINS + 1 + 3, quals: 0 };
        let mut img = template_sample();
        img.written = vec![
            WrittenRec { key: Key::Ty(t), locs: vec![318] },
            WrittenRec {
                key: Key::Nns(vec![NnsRec::Ty(t, false), NnsRec::Ident(0)]),
                locs: vec![320, 330],
            },
        ];
        img
    }

    /// spec/modules.md §11.11 "Dependent types" (WP-1284;
    /// spec/sema-templates.md §5.8): written locations round-trip; a key
    /// naming a type or a parameter name the image does not have is an
    /// error.
    #[test]
    fn written_locations() {
        type Corrupt = (&'static str, fn(&mut Image));
        let img = written_sample();
        assert_eq!(decode(&encode(&img)), Ok(img));
        let corrupt: [Corrupt; 3] = [
            ("type out of range", |i| i.written[0].key = Key::Ty(QRef { ty: 9999, quals: 0 })),
            ("identifier out of range", |i| i.written[1].key = Key::Nns(vec![NnsRec::Ident(99)])),
            ("type step out of range", |i| {
                i.written[1].key = Key::Nns(vec![NnsRec::Ty(QRef { ty: 9999, quals: 0 }, false)]);
            }),
        ];
        for (what, f) in corrupt {
            let mut img = written_sample();
            f(&mut img);
            assert!(decode(&encode(&img)).is_err(), "{what}");
        }
    }

    /// spec/modules.md §11.11 "Dependent types" (WP-1859; spec/sema-templates.md
    /// §5.8.1): the written parameters of nested function types round-trip
    /// (with and without a Swift ABI attribute); a record naming a type the
    /// image does not have is an error.
    #[test]
    fn nested_params() {
        let t = QRef { ty: BUILTINS + 1 + 3, quals: 0 };
        let mut img = template_sample();
        img.nested =
            vec![NestedRec { ty: t, index: 1, params: vec![(318, NONE, true), (330, 332, false)] }];
        assert_eq!(decode(&encode(&img)), Ok(img.clone()));
        img.nested[0].ty = QRef { ty: 9999, quals: 0 };
        assert!(decode(&encode(&img)).is_err(), "type out of range");
    }

    /// spec/types.md §8.4, spec/modules.md §11.11 (WP-3640):
    /// `written_decls` round-trips; a row naming a non-var/non-function
    /// record, a duplicate record, or an out-of-range type is rejected.
    #[test]
    fn written_decl_types_round_trip() {
        type Corrupt = (&'static str, fn(&mut Image));
        let mut img = template_sample();
        let f = img
            .decls
            .iter()
            .position(|d| matches!(d.payload, Payload::Function { .. }))
            .expect("a function record") as u32;
        let int = QRef { ty: kiln_ast::types::BuiltinType::Int as u32, quals: 0 };
        img.written_decls = vec![(f, int)];
        assert_eq!(decode(&encode(&img)), Ok(img.clone()));
        let corrupt: [Corrupt; 3] = [
            ("duplicate", |i| i.written_decls.push(i.written_decls[0])),
            ("not var or fn", |i| i.written_decls[0].0 = 0),
            ("type out of range", |i| i.written_decls[0].1.ty = 9999),
        ];
        for (what, mutate) in corrupt {
            let mut bad = img.clone();
            mutate(&mut bad);
            assert!(decode(&encode(&bad)).is_err(), "{what}");
        }
    }
}
