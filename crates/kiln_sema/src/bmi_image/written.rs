//! Where the image's dependent names were written (spec/modules.md
//! §11.11, "Dependent types", WP-1284; spec/sema-templates.md §5.8).
//!
//! Types carry no locations, so the parser records each occurrence of a
//! dependent type or qualifier component (`T::inner::`) in a side table
//! the substituter reports at. The image carries the entries whose key it
//! carries (a type already in the type section, a qualifier whose
//! components are), so an error substituting an imported pattern sits at
//! the same name, with the same caret, as after a replay.

use kiln_ast::types::Nns;
use kiln_ast::{DeclId, NnsId, QualType};

use super::{Image, NnsRec, QRef, Rd, Res, Wr, Writer, bad};
use crate::Sema;
use crate::templ::written::WrittenKey;

/// One entry: its key and its locations.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct WrittenRec {
    key: Key,
    locs: Vec<u32>,
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
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::template_sample;
    use super::super::{BUILTINS, decode, encode};
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
}
