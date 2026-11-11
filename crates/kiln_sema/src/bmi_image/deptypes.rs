//! Dependent and remaining non-dependent types in the declaration image
//! (spec/modules.md §11.11, "Dependent types", WP-1284).
//!
//! One [`TyRec::Dep`] variant carries every type of this module, so the
//! shared type dispatch gains one arm per site:
//!
//! - `[typename] nns::name` ([temp.res.general]/5, [temp.dep.type]/7):
//!   the keyword, the qualifier and the name;
//! - `[typename] nns::template name<args>`: the same plus the arguments
//!   as written;
//! - a type-trait transform `__remove_cv(T)` & co. ([meta.trans]): its
//!   kind, its operand and its result once the operand is not dependent;
//! - `T C::*` ([dcl.mptr]);
//! - a GNU or ext vector of a constant element count;
//! - the placeholder of a deduced class template type `C x(args)`
//!   ([dcl.type.class.deduct]), with its deduced type once deduced.
//!
//! A qualifier step naming a dependent identifier (`T::x::`,
//! [`NnsRec::Ident`]) is carried by name; the importer forms the same
//! qualifier, which instantiation resolves as replay does
//! ([temp.dep.type]/6). Types holding an expression (`decltype(e)`, an
//! array of a value-dependent bound) still fall back: the importer builds
//! types before bodies.

use kiln_ast::types::{
    Nns, TagKeyword, TemplateArg, TemplateName, TypeView, UnaryTransformKind, VectorKind,
};
use kiln_ast::{DeclId, QualType};

use super::{ArgRec, Image, NnsRec, Payload, QRef, Rd, Res, TyRec, W, Wr, Writer, bad};
use crate::Sema;

/// A type of this module.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum DepTy {
    /// `[typename] nns::name`: the keyword, the qualifier and the name
    /// (an index into the parameter names).
    Name { kw: u8, steps: Vec<NnsRec>, name: u32 },
    /// `[typename] nns::template name<args>`.
    Tmpl { kw: u8, steps: Vec<NnsRec>, name: u32, args: Vec<ArgRec> },
    /// A type-trait transform: its kind, operand and result.
    Transform { kind: u8, base: QRef, result: Option<QRef> },
    /// `pointee class::*`.
    MemPtr { pointee: QRef, class: QRef },
    /// A vector of `count` elements of kind `kind`.
    Vector { elem: QRef, count: u32, kind: u8 },
    /// A deduced class template type: the class template's record and
    /// the deduced type.
    Deduced { template: u32, deduced: Option<QRef> },
}

const T_NAME: u8 = 0;
const T_TMPL: u8 = 1;
const T_TRANSFORM: u8 = 2;
const T_MEMPTR: u8 = 3;
const T_VECTOR: u8 = 4;
const T_DEDUCED: u8 = 5;

impl Writer<'_, '_, '_> {
    /// The record of type view `v`, `None` when this module does not
    /// carry it.
    pub(super) fn dep_ty(&mut self, v: TypeView) -> W<Option<TyRec>> {
        Ok(Some(TyRec::Dep(match v {
            TypeView::DependentName { keyword, nns, name } => {
                let steps = self.nns_steps(nns)?;
                DepTy::Name { kw: keyword as u8, steps, name: self.sym_ix(name) }
            }
            TypeView::DependentTemplateSpecialization { keyword, nns, name, args } => {
                let steps = self.nns_steps(nns)?;
                let name = self.sym_ix(name);
                DepTy::Tmpl { kw: keyword as u8, steps, name, args: self.arg_recs(args)? }
            }
            TypeView::UnaryTransform { kind, base, result } => DepTy::Transform {
                kind: kind as u8,
                base: self.qref(base)?,
                result: result.map(|r| self.qref(r)).transpose()?,
            },
            TypeView::MemberPointer { pointee, class } => DepTy::MemPtr {
                pointee: self.qref(pointee)?,
                class: self.qref(QualType::unqual(class))?,
            },
            TypeView::Vector { elem, count, kind } => {
                DepTy::Vector { elem: self.qref(elem)?, count, kind: kind as u8 }
            }
            TypeView::DeducedTemplateSpecialization { name, deduced } => {
                let TemplateName::Template(td) = self.s.ast.types.template_name_view(name) else {
                    return Err("a deduced template named through a qualifier or parameter");
                };
                let template = self.template_rec(td, 1)?;
                DepTy::Deduced { template, deduced: deduced.map(|q| self.qref(q)).transpose()? }
            }
            _ => return Ok(None),
        })))
    }

    /// The parameter-name index of identifier `s`.
    pub(super) fn sym_ix(&mut self, s: kiln_source::symbol::Symbol) -> u32 {
        let s = self.s.ast.symbols().as_str(s).to_owned();
        Self::name_ix(&mut self.pnames, &mut self.img.pnames, &s)
    }
}

pub(super) fn encode_dep(w: &mut Wr, d: &DepTy) {
    match d {
        DepTy::Name { kw, steps, name } => {
            w.u8(T_NAME);
            w.u8(*kw);
            w.steps(steps);
            w.u32(*name);
        }
        DepTy::Tmpl { kw, steps, name, args } => {
            w.u8(T_TMPL);
            w.u8(*kw);
            w.steps(steps);
            w.u32(*name);
            w.args(args);
        }
        DepTy::Transform { kind, base, result } => {
            w.u8(T_TRANSFORM);
            w.u8(*kind);
            w.q(*base);
            w.u32(result.map_or(0, |q| q.ty));
            w.u8(result.map_or(0, |q| q.quals));
        }
        DepTy::MemPtr { pointee, class } => {
            w.u8(T_MEMPTR);
            w.q(*pointee);
            w.q(*class);
        }
        DepTy::Vector { elem, count, kind } => {
            w.u8(T_VECTOR);
            w.q(*elem);
            w.u32(*count);
            w.u8(*kind);
        }
        DepTy::Deduced { template, deduced } => {
            w.u8(T_DEDUCED);
            w.u32(*template);
            w.u32(deduced.map_or(0, |q| q.ty));
            w.u8(deduced.map_or(0, |q| q.quals));
        }
    }
}

fn keyword(r: &mut Rd<'_>) -> Res<u8> {
    let k = r.u8()?;
    if super::tag_keyword(k).is_none() {
        return bad("bad tag keyword");
    }
    Ok(k)
}

pub(super) fn decode_dep(r: &mut Rd<'_>) -> Res<DepTy> {
    Ok(match r.u8()? {
        T_NAME => {
            let kw = keyword(r)?;
            let steps = r.steps()?;
            DepTy::Name { kw, steps, name: r.u32()? }
        }
        T_TMPL => {
            let kw = keyword(r)?;
            let steps = r.steps()?;
            let name = r.u32()?;
            DepTy::Tmpl { kw, steps, name, args: r.args()? }
        }
        T_TRANSFORM => {
            let kind = r.u8()?;
            if UnaryTransformKind::from_u8(kind).is_none() {
                return bad("bad type transform");
            }
            let base = r.q()?;
            DepTy::Transform { kind, base, result: opt_q(r)? }
        }
        T_MEMPTR => DepTy::MemPtr { pointee: r.q()?, class: r.q()? },
        T_VECTOR => {
            let elem = r.q()?;
            let count = r.u32()?;
            let kind = r.u8()?;
            if VectorKind::from_u8(kind) as u8 != kind {
                return bad("bad vector kind");
            }
            DepTy::Vector { elem, count, kind }
        }
        T_DEDUCED => DepTy::Deduced { template: r.u32()?, deduced: opt_q(r)? },
        _ => return bad("bad dependent type"),
    })
}

/// An optional type reference: type 0 is none.
fn opt_q(r: &mut Rd<'_>) -> Res<Option<QRef>> {
    Ok(match r.q()? {
        QRef { ty: 0, quals: 0 } => None,
        QRef { ty: 0, .. } => return bad("bad type reference"),
        q => Some(q),
    })
}

/// The type references and records `d` uses ([`super::ty_refs`]).
pub(super) fn dep_refs(d: &DepTy) -> (Vec<QRef>, Vec<u32>) {
    let (mut qs, mut ds) = (Vec::new(), Vec::new());
    let mut steps = |steps: &[NnsRec], qs: &mut Vec<QRef>| {
        for st in steps {
            match st {
                NnsRec::Global | NnsRec::Ident(_) => {}
                NnsRec::Ns(d) | NnsRec::Alias(d) => ds.push(*d),
                NnsRec::Ty(q, _) => qs.push(*q),
            }
        }
    };
    match d {
        DepTy::Name { steps: s, .. } => steps(s, &mut qs),
        DepTy::Tmpl { steps: s, args, .. } => {
            steps(s, &mut qs);
            qs.extend(args.iter().map(|a| a.q()));
        }
        DepTy::Transform { base, result, .. } => qs.extend(std::iter::once(*base).chain(*result)),
        DepTy::MemPtr { pointee, class } => qs.extend([*pointee, *class]),
        DepTy::Vector { elem, .. } => qs.push(*elem),
        DepTy::Deduced { template, deduced } => {
            ds.push(*template);
            qs.extend(*deduced);
        }
    }
    (qs, ds)
}

/// Whether `d`'s qualifier steps and names are well formed: the records
/// a step names were checked by [`super::validate`]'s type loop.
pub(super) fn dep_ok(img: &Image, d: &DepTy, nd: usize) -> bool {
    let name_ok = |n: u32| (n as usize) < img.pnames.len();
    match d {
        DepTy::Name { steps, name, .. } | DepTy::Tmpl { steps, name, .. } => {
            name_ok(*name) && super::steps_ok(img, steps, nd)
        }
        DepTy::Deduced { template, .. } => matches!(
            img.decls.get(*template as usize).map(|r| &r.payload),
            Some(Payload::Template { kind: 1, .. })
        ),
        DepTy::Transform { .. } | DepTy::MemPtr { .. } | DepTy::Vector { .. } => true,
    }
}

/// The importer's side.
impl Sema<'_, '_> {
    /// The type of record `d`; `qt` builds an earlier image type.
    pub(super) fn image_dep_ty(
        &mut self,
        img: &Image,
        d: &DepTy,
        ids: &[DeclId],
        qt: &mut dyn FnMut(&mut Self, QRef) -> QualType,
    ) -> QualType {
        let kw = |k: u8| super::tag_keyword(k).unwrap_or(TagKeyword::Typename);
        let name =
            |s: &Self, n: u32| s.intern(img.pnames.get(n as usize).map_or("", String::as_str));
        match d {
            DepTy::Name { kw: k, steps, name: n } => {
                let q = self.image_dep_nns(img, steps, ids, qt);
                let n = name(self, *n);
                self.ast.types.dependent_name(kw(*k), q, n)
            }
            DepTy::Tmpl { kw: k, steps, name: n, args } => {
                let q = self.image_dep_nns(img, steps, ids, qt);
                let n = name(self, *n);
                let args: Vec<TemplateArg> = args
                    .iter()
                    .map(|a| {
                        let ty = qt(self, a.q());
                        self.image_arg(ty, a)
                    })
                    .collect();
                let list = self.ast.types.arg_list(&args);
                self.ast.types.dependent_template_specialization(kw(*k), q, n, list)
            }
            DepTy::Transform { kind, base, result } => {
                let base = qt(self, *base);
                let result = result.map(|r| qt(self, r));
                match UnaryTransformKind::from_u8(*kind) {
                    Some(k) => self.ast.types.unary_transform(k, base, result),
                    None => base,
                }
            }
            DepTy::MemPtr { pointee, class } => {
                let p = qt(self, *pointee);
                let c = qt(self, *class);
                self.ast.types.member_pointer(p, c.ty())
            }
            DepTy::Vector { elem, count, kind } => {
                let e = qt(self, *elem);
                self.ast.types.vector(e, *count, VectorKind::from_u8(*kind))
            }
            DepTy::Deduced { template, deduced } => {
                let deduced = deduced.map(|q| qt(self, q));
                let t = ids.get(*template as usize).map_or(self.ast.tu, |&t| self.ast.first(t));
                let name = self.ast.types.template_name_of(t);
                self.ast.types.deduced_template_specialization(name, deduced)
            }
        }
    }

    /// A dependent type's qualifier (never empty: [`Rd::steps`]).
    fn image_dep_nns(
        &mut self,
        img: &Image,
        steps: &[NnsRec],
        ids: &[DeclId],
        qt: &mut dyn FnMut(&mut Self, QRef) -> QualType,
    ) -> kiln_ast::NnsId {
        match self.image_nns(img, steps, ids, qt) {
            Some(n) => n,
            None => self.ast.types.nns(Nns::Global),
        }
    }
}
