//! Explicit specializations of function templates in the declaration
//! image (WP-1352, spec/modules.md §11.11 "Forms carried by WP-1352").
//!
//! `template <> double strtod_<double>(const char *, char **);`
//! ([temp.expl.spec]) is two declarations in sema: the specialization the
//! declaration matched (made by deduction, `SpecKind::Undeclared`, then
//! marked explicit) and the written declaration, a redeclaration of it
//! (`function_spec_redecl`) with its own parameters, specifiers and body,
//! listed in its lexical context but in no lookup table. The writer makes
//! no record for the first: the written declaration is a `Function`
//! record with no previous declaration, listed in the specialization
//! section (`cspecs`, as an explicit class or variable specialization)
//! with its function template's record and the converted arguments. A
//! body naming the specialization names that record. The reader finds or
//! makes the specialization through `get_function_spec`, marks it
//! explicit as `act_on_explicit_function_spec` does, and builds the
//! record as its redeclaration.
//!
//! What the unit's uses set on the first declaration (`USED`,
//! `REFERENCED`) is the `fsuses` section, (record, bits) pairs: the reader
//! marks the specialization so once the bodies are built, as replay's
//! uses did.

use kiln_ast::decl::{DeclKind, FnSpec, TemplLinkKind};
use kiln_ast::templates::SpecKind;
use kiln_ast::types::ArgView;
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_source::Loc;

use super::classtempl::{CArg, ClassSpec};
use super::{Image, Locs, Payload, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;
use crate::templ::args::ConvertedArgs;
use kiln_ast::decl::DeclFlags;

/// The census reason of an explicit specialization of a function
/// template this file does not carry, by form.
const REDECLARED: &str = "a template specialization: declared more than once";
const PACK: &str = "a template specialization: a pack argument";
const SCOPE: &str = "a template specialization: not at namespace scope";

/// The uses of explicit specializations' first declarations: (written
/// declaration's record, bit 0 `USED`, bit 1 `REFERENCED`), ascending.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct FnSpecUses {
    uses: Vec<(u32, u8)>,
}

impl FnSpecUses {
    pub(super) fn truncate(&mut self, n: u32) {
        self.uses.retain(|&(r, _)| r < n);
    }
}

pub(super) fn encode_fn_spec_uses(w: &mut Wr, s: &FnSpecUses) {
    w.len(s.uses.len());
    for &(r, b) in &s.uses {
        w.u32(r);
        w.u8(b);
    }
}

pub(super) fn decode_fn_spec_uses(r: &mut Rd) -> Res<FnSpecUses> {
    let n = r.count(5)?;
    let mut uses = Vec::with_capacity(n);
    for _ in 0..n {
        uses.push((r.u32()?, r.u8()?));
    }
    Ok(FnSpecUses { uses })
}

/// Each entry names a function record listed as an explicit
/// specialization, once, ascending, with known bits.
pub(super) fn validate_fn_spec_uses(img: &Image) -> Res<()> {
    let mut last = None;
    for &(r, b) in &img.fsuses.uses {
        let ok = last.is_none_or(|l| l < r)
            && (1..=3).contains(&b)
            && img.cspecs.get_fn(r).is_some_and(|c| {
                matches!(
                    img.decls.get(c.template_rec() as usize).map(|t| &t.payload),
                    Some(Payload::Template { kind: 0, .. })
                )
            });
        if !ok {
            return bad("bad explicit specialization use");
        }
        last = Some(r);
    }
    Ok(())
}

impl Writer<'_, '_, '_> {
    /// Whether `d` is the first declaration of an explicit specialization
    /// of a carried function template at namespace scope: it has no
    /// record (its written declaration stands for it).
    pub(super) fn fn_spec_first(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        ast.decls.function(d).is_some()
            && ast.first(d) == d
            && ast.redecls_of(d).nth(1).is_some()
            && ast.spec_info(d).is_some_and(|(_, si)| {
                si.kind == SpecKind::ExplicitSpecialization
                    && self.recs.contains_key(&ast.first(si.template))
            })
    }

    /// Whether `d`'s previous declaration `p` is such a first
    /// declaration (`d` then has no previous record).
    pub(super) fn fn_spec_prev(&self, d: DeclId, p: DeclId) -> bool {
        self.s.ast.first(d) == p && self.fn_spec_first(p)
    }

    /// Lists record `i`, written declaration `d` of an explicit
    /// specialization of a function template, in the specialization
    /// section; `false` when `d` is no such declaration.
    pub(super) fn fn_spec_entry(&mut self, d: DeclId, i: u32) -> W<bool> {
        let ast = &self.s.ast;
        if !matches!(ast.templ_link(d).get(), Some((TemplLinkKind::Specialization, _))) {
            return Ok(false);
        }
        let s = ast.first(d);
        let Some((_, si)) = ast.spec_info(s) else { return Ok(false) };
        let si = *si;
        if s == d || si.kind != SpecKind::ExplicitSpecialization {
            return Ok(false);
        }
        if ast.redecls_of(s).count() != 2 {
            return Err(REDECLARED);
        }
        let ns = |c: Option<DeclId>| {
            c.is_some_and(|c| {
                matches!(
                    ast.decls.get(c).kind(),
                    DeclKind::TranslationUnit
                        | DeclKind::Namespace
                        | DeclKind::Export
                        | DeclKind::LinkageSpec
                )
            })
        };
        if !ns(ast.decls.get(d).ctx()) || !ns(ast.decls.lexical_ctx(d)) {
            return Err(SCOPE);
        }
        let template = self.template_rec(si.template, 0)?;
        let mut args = Vec::new();
        for a in ast.types.args(si.args).to_vec() {
            if matches!(a.view(), ArgView::Pack(_)) {
                return Err(PACK);
            }
            args.push(CArg::Arg(self.arg_rec(a)?));
        }
        self.img.cspecs.push_fn(ClassSpec::explicit(i, template, args));
        let n = ast.decls.get(s);
        let bits = u8::from(n.has(DeclFlags::USED)) | u8::from(n.has(DeclFlags::REFERENCED)) << 1;
        if bits != 0 {
            self.img.fsuses.uses.push((i, bits));
        }
        // A body naming the specialization names the written declaration.
        self.recs.insert(s, i);
        Ok(true)
    }
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Builds function record `i` (header `h`, type `ty`, specifiers
    /// `fspec`) as the written declaration of an explicit specialization,
    /// as `act_on_explicit_function_spec` does; `None` when the record is
    /// no listed specialization.
    #[allow(clippy::too_many_arguments, reason = "the reader's tables and the record's head")]
    pub(super) fn image_fn_spec(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        i: u32,
        h: &DeclHead,
        ty: QualType,
        fspec: FnSpec,
    ) -> Option<DeclId> {
        let c = img.cspecs.get_fn(i)?;
        let tmpl = self.ast.first(ids[c.template_rec() as usize]);
        let args = self.image_spec_args(img, tys, ids, c);
        let sugared = self.ast.types.arg_list(&args);
        let canonical = self.ast.types.canonical_args(sugared);
        let s = self.get_function_spec(tmpl, &ConvertedArgs { sugared, canonical }, h.loc).ok()?;
        let (k, info) = self.ast.spec_info(s).map(|(k, x)| (k, *x))?;
        self.ast.templates.update_spec(k, |x| {
            x.kind = SpecKind::ExplicitSpecialization;
            x.from = info.template;
            x.point_of_instantiation = Loc::INVALID;
        });
        if info.kind != SpecKind::ExplicitSpecialization {
            self.ast.decls.respecify_function(s, fspec);
            if self.ast.annots.has_list(s) {
                self.ast.annots.clear(s);
            }
        }
        let lexical = h.lexical.unwrap_or(h.ctx);
        Some(self.builder().function_spec_redecl(s, lexical, h.loc, ty, fspec))
    }

    /// Marks the explicit specializations the unit used, once the bodies
    /// are built, as replay's uses did (`image_mark_specs`).
    pub(super) fn image_fn_spec_uses(&mut self, img: &Image, ids: &[DeclId], main: &Locs) {
        for &(r, bits) in &img.fsuses.uses {
            let s = self.ast.first(ids[r as usize]);
            if s == self.ast.tu {
                continue;
            }
            let loc = main.get(img.decls[r as usize].loc);
            if bits & 1 != 0 {
                self.mark_function_referenced(s, loc);
            } else {
                self.ast.decls.set_flags(s, DeclFlags::REFERENCED);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::template_sample;
    use super::super::{ArgRec, DeclRec, NONE, QRef, decode, encode};
    use super::*;
    use kiln_ast::types::BuiltinType;

    /// The WP-1300 function template sample (`tf`, record 13) plus
    /// `template <> int tf<int, 3>(int);` (record `k`, used), and a
    /// reference record `tf<long, 4>` (`k + 1`), an explicit instantiation
    /// declaration.
    fn sample() -> (Image, usize) {
        let mut img = template_sample();
        let int = QRef { ty: BuiltinType::Int as u32, quals: 0 };
        let long = QRef { ty: BuiltinType::Long as u32, quals: 0 };
        let k = img.decls.len();
        let mut f = img.decls[2].clone();
        f.name = img.decls[13].name;
        img.decls.push(f);
        img.decls.push(DeclRec {
            flags: 0,
            owner: 1,
            access: 0,
            name: NONE,
            ctx: NONE,
            lexical: NONE,
            loc: 340,
            prev: NONE,
            payload: Payload::Spec {
                template: 13,
                complete: false,
                args: vec![ArgRec::Type(long), ArgRec::Value(int, 4)],
            },
        });
        let args = vec![CArg::Arg(ArgRec::Type(int)), CArg::Arg(ArgRec::Value(int, 3))];
        img.cspecs.push_fn(ClassSpec::explicit(k as u32, 13, args));
        img.fsuses.uses.push((k as u32, 3));
        img.fexpl.recs.push(k as u32 + 1);
        (img, k)
    }

    /// spec/modules.md §11.11 "Forms carried by WP-1352": an explicit
    /// specialization of a function template, its uses, and an explicit
    /// instantiation declaration round-trip.
    #[test]
    fn function_specializations_round_trip() {
        let (img, k) = sample();
        assert_eq!(img.cspecs.get_fn(k as u32).map(ClassSpec::template_rec), Some(13));
        assert_eq!(decode(&encode(&img)), Ok(img));
    }

    /// An entry of the wrong argument count or on another payload, a use
    /// of a record no entry lists or with unknown bits, or an explicit
    /// instantiation declaration that is no function template's reference
    /// record, is an error.
    #[test]
    fn rejects_corrupt_function_specializations() {
        type Corrupt = (&'static str, fn(&mut Image, usize));
        let corrupt: [Corrupt; 7] = [
            ("an argument short", |img, k| {
                let int = QRef { ty: BuiltinType::Int as u32, quals: 0 };
                img.cspecs = super::super::classtempl::ClassSpecs::default();
                let args = vec![CArg::Arg(ArgRec::Type(int))];
                img.cspecs.push_fn(ClassSpec::explicit(k as u32, 13, args));
            }),
            ("not a function", |img, k| img.decls[k].payload = Payload::Namespace),
            ("a use of an unlisted record", |img, k| img.fsuses.uses[0].0 = k as u32 + 1),
            ("unknown use bits", |img, _| img.fsuses.uses[0].1 = 4),
            ("no use bits", |img, _| img.fsuses.uses[0].1 = 0),
            ("an instantiation of no reference", |img, k| img.fexpl.recs[0] = k as u32),
            ("listed twice", |img, k| img.fexpl.recs.push(k as u32 + 1)),
        ];
        for (what, f) in corrupt {
            let (mut img, k) = sample();
            f(&mut img, k);
            assert!(decode(&encode(&img)).is_err(), "{what}");
        }
    }
}
