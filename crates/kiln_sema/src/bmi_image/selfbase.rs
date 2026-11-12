//! Class template patterns whose bases name their own template
//! (spec/modules.md §11.11, "Forms carried by WP-1344"):
//! `template <class T> struct chain : chain<T*> {};`.
//!
//! A class template's record is written just after its pattern's
//! ([`super::templ`]), so a base of the pattern that names the template
//! names a later record. The pattern's definition then has bit
//! [`BASES_AFTER`]: its bases are checked as if they were the template's
//! record's, and the importer gives the class its bases once it has
//! built the template, before any member, as the parser had the
//! template when it read the base-clause.

use kiln_ast::{DeclId, QualType};

use super::{Image, Payload, RecDef, Writer, ty_refs};
use crate::Sema;

/// The `RecDef` bit of a class template pattern whose bases name the
/// template's record, the next one (`DefinitionBits` stop at bit 61).
pub(super) const BASES_AFTER: u64 = 1 << 63;

impl Writer<'_, '_, '_> {
    /// The record template `t` will have, when `t` is the class template
    /// whose pattern is being written as record `at - 1`.
    pub(super) fn fwd_template(&self, t: DeclId) -> Option<u32> {
        self.fwd_tmpl.filter(|&(x, _)| x == t).map(|(_, r)| r)
    }

    /// Writes the bases of pattern `i` (of template `t`) with `f`, the
    /// template's record (`i + 1`) nameable; whether a type made names
    /// it.
    pub(super) fn with_fwd_template<R>(
        &mut self,
        t: DeclId,
        i: u32,
        f: impl FnOnce(&mut Self) -> R,
    ) -> (R, bool) {
        let mark = self.img.types.len();
        let saved = self.fwd_tmpl.replace((self.s.ast.first(t), i + 1));
        let r = f(self);
        self.fwd_tmpl = saved;
        let named = self.img.types[mark..].iter().any(|t| ty_refs(t).1.contains(&(i + 1)));
        (r, named)
    }
}

/// Whether record `i` (a class definition `df`) may name record `i + 1`
/// in its bases: it has [`BASES_AFTER`] and that record is its class
/// template.
pub(super) fn bases_after_ok(img: &Image, i: usize, df: &RecDef) -> bool {
    df.bits & BASES_AFTER == 0
        || matches!(
            img.decls.get(i + 1).map(|r| &r.payload),
            Some(Payload::Template { kind: 1, pattern, .. }) if *pattern as usize == i
        )
}

/// The record index the bases of record `i` (`payload`) are checked at.
pub(super) fn bases_at(i: usize, payload: &Payload) -> usize {
    match payload {
        Payload::Record { def: Some(df), .. } if df.bits & BASES_AFTER != 0 => i + 2,
        _ => i,
    }
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Gives class `c`, pattern record `r`, its bases now that its
    /// template is built.
    pub(super) fn image_deferred_bases(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        main: &dyn Fn(u32) -> kiln_source::Loc,
        c: DeclId,
        r: usize,
    ) {
        let Payload::Record { def: Some(df), .. } = &img.decls[r].payload else { return };
        let bases: Vec<kiln_ast::decl::BaseSpec> = df
            .bases
            .iter()
            .map(|b| kiln_ast::decl::BaseSpec {
                ty: self.image_ty(img, tys, ids, b.ty),
                loc: main(b.loc),
                is_virtual: b.virt,
                access: super::access_of(b.access),
                pack_expansion: b.pack,
            })
            .collect();
        let _ = self.ast.decls.set_bases(c, &bases, &mut self.ast.lists);
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::template_sample;
    use super::super::{
        ArgRec, BUILTINS, BaseRec, DEP, DeclRec, NONE, QRef, TyRec, builtintempl, decode, encode,
    };
    use super::*;
    use kiln_ast::types::BuiltinType;

    /// `template_sample` plus `template <class T> struct Ch : Ch<T *> {};`
    /// and the dependent `__make_integer_seq<Ch, int>` (a builtin
    /// template's stub, record 17); the pattern's record.
    fn sample() -> (Image, usize) {
        let q = |ty| QRef { ty, quals: 0 };
        let mut img = template_sample();
        let i = img.decls.len() as u32;
        let name = img.names.len() as u32;
        img.names.push("Ch".into());
        let pname = img.pnames.len() as u32;
        img.pnames.push("T".into());
        let t = BUILTINS + 1 + img.types.len() as u32;
        img.types.extend([
            TyRec::Ttp(i),
            TyRec::Pointer(q(t)),
            TyRec::Tst { template: i + 2, args: vec![ArgRec::Type(q(t + 1))], under: None },
            TyRec::Tst {
                template: i + 3,
                args: vec![ArgRec::Template(i + 2, 0), ArgRec::Type(q(BuiltinType::Int as u32))],
                under: None,
            },
        ]);
        let head = |flags, name, loc, payload| DeclRec {
            flags,
            owner: 3,
            access: 0,
            name,
            ctx: 1,
            lexical: NONE,
            loc,
            prev: NONE,
            payload,
        };
        let base = BaseRec { ty: q(t + 2), loc: 430, access: 0, virt: false, pack: false };
        img.decls.extend([
            head(
                DEP,
                pname,
                402,
                Payload::Ttp {
                    depth: 0,
                    index: 0,
                    typename_kw: false,
                    pack: false,
                    begin: 396,
                    default: None,
                },
            ),
            head(
                DEP,
                name,
                420,
                Payload::Record {
                    tag: 0,
                    def: Some(RecDef {
                        bits: BASES_AFTER,
                        end: i + 3,
                        key: NONE,
                        bases: vec![base],
                        friends: vec![],
                    }),
                },
            ),
            head(
                0,
                name,
                420,
                Payload::Template {
                    kind: 1,
                    pattern: i + 1,
                    tloc: 390,
                    using: NONE,
                    langle: 398,
                    rangle: 404,
                    depth: 0,
                    params: vec![i],
                },
            ),
            DeclRec {
                flags: 0,
                owner: 0,
                access: 0,
                name: NONE,
                ctx: NONE,
                lexical: NONE,
                loc: NONE,
                prev: NONE,
                payload: Payload::Implicit { kind: builtintempl::MAKE_INTEGER_SEQ_KIND },
            },
        ]);
        (img, i as usize + 1)
    }

    /// spec/modules.md §11.11 "Forms carried by WP-1344": a pattern whose
    /// base names its template and a builtin template's specialization
    /// round-trip.
    #[test]
    fn self_bases_and_builtin_templates_round_trip() {
        let (img, _) = sample();
        assert_eq!(decode(&encode(&img)), Ok(img));
    }

    /// A pattern naming a later record without the bit, the bit on a
    /// class whose next record is not its template, and a builtin
    /// template stub with flags are errors.
    #[test]
    fn rejects_corrupt_self_bases_and_builtin_templates() {
        type Corrupt = (&'static str, fn(&mut Image, usize));
        let corrupt: [Corrupt; 3] = [
            ("base naming the template without the bit", |img, i| {
                if let Payload::Record { def: Some(df), .. } = &mut img.decls[i].payload {
                    df.bits = 0;
                }
            }),
            ("the bit, the next record no template", |img, i| {
                img.decls[i + 1].payload = Payload::Implicit { kind: 0 };
            }),
            ("a builtin template stub with flags", |img, i| img.decls[i + 2].flags = DEP),
        ];
        for (what, f) in corrupt {
            let (mut img, i) = sample();
            f(&mut img, i);
            assert!(decode(&encode(&img)).is_err(), "{what}");
        }
    }
}
