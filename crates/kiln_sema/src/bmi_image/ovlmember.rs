//! An overloaded member access in a carried body (spec/modules.md §11.11,
//! "Forms carried by WP-1368").
//!
//! A member access whose name lookup found several member functions, a
//! member function template or a static data member template
//! ([expr.ref]/7, [over.match.call]/2; `x.f`, `p->f`, an implicit
//! `this->f`, `x.f<int>`) is kiln's `UnresolvedMemberExpr`: a `MemberExpr`
//! of overloaded type naming the first candidate, whose whole set Sema
//! keeps in a side table (`ovl.members`, [`crate::overload::OvlMember`]).
//! It survives in a template pattern whose call is dependent (its
//! arguments are). The instantiation rebuilds the access from the set
//! (the forced rebuild, the explicit template arguments, the qualifier,
//! whether the static candidates make the implicit `this` unused), so
//! the image carries the set with the access as a record kind of its
//! own, [`OVL_MEMBER`]: the `MemberExpr` slots (the object, the first
//! candidate as a lookup result), then [`OVL_FIELDS`].
//!
//! A member found only in a dependent base has no set at the definition:
//! that access is a `CXXDependentScopeMemberExpr` (an empty set is never
//! recorded), so a record of this kind always has a candidate.

use kiln_ast::expr::{ExprFlags, ExprKind};
use kiln_ast::ids::RawId;
use kiln_ast::names::DeclName;
use kiln_ast::{DeclId, ExprId, ListId, NnsId, QualType};

use super::W;
use super::Writer;
use super::body::{Dc, F, Fv, Slot};
use crate::Sema;

/// The record kind byte of an overloaded member access: the second
/// `MemberExpr` entry of [`super::body::EXPR_KINDS`] (WP-1368).
pub(super) const OVL_MEMBER: u8 = 72;

/// Its slots: the object expression (a pointer for `->`) and the first
/// candidate, a declaration lookup finds ([`Dc::Lookup`]: a member
/// function, a member function or variable template, a member of a
/// specialization).
pub(super) const OVL_SHAPE: [Slot; 2] = [Slot::F(F::E), Slot::F(F::D(Dc::Lookup))];

/// The set, after the slots and the cold annotations: the candidates in
/// declaration order, what lookup found for each (the using-declaration's
/// shadow of a member a `using` brings in, else the candidate), the
/// naming class ([class.access.base]/5), the object's class, the object
/// type, the member name, the qualifier (`x.B::f`), whether the object is
/// an implicit `this`, and the explicit template arguments
/// ([temp.arg.explicit]/1).
pub(super) const OVL_FIELDS: &[F] = &[
    F::DL(Dc::Lookup),
    F::DL(Dc::Lookup),
    F::D(Dc::Ctx),
    F::D(Dc::Ctx),
    F::Qt,
    F::Name,
    F::Nns,
    F::Bool,
    F::TA,
];

/// The set does not match its access (not built by
/// `build_member_overload`).
const OVL_MISMATCH: &str = "an overloaded member access whose set does not match its expression";

/// Whether the tokens of a record of kind [`OVL_MEMBER`] hold a set: as
/// many found declarations as candidates, at least one, the first the
/// access's own.
pub(super) fn ovl_ok(toks: &[(F, &[u32])]) -> bool {
    let Some(at) = toks.len().checked_sub(OVL_FIELDS.len()) else { return false };
    let (decls, found) = (toks[at].1, toks[at + 1].1);
    decls[0] >= 1 && decls[0] == found[0] && toks.get(1).is_some_and(|t| t.1[0] == decls[1])
}

impl Writer<'_, '_, '_> {
    /// The fields of overloaded member access `e` after its slots and
    /// cold annotations ([`OVL_FIELDS`]).
    pub(super) fn ovl_member_fv(&self, e: ExprId, v: &mut Vec<Fv>) -> W<()> {
        let m = self.s.ovl.members.get(&e).ok_or(OVL_MISMATCH)?;
        let n = self.s.ast.exprs.get(e);
        if m.decls.is_empty()
            || m.found.len() != m.decls.len()
            || n.a() != m.base.raw()
            || n.b() != m.decls[0].raw()
            || n.has(ExprFlags::ARROW) != m.arrow
        {
            return Err(OVL_MISMATCH);
        }
        v.extend([
            Fv::Ovl(e, false),
            Fv::Ovl(e, true),
            Fv::D(Some(m.naming), Dc::Ctx),
            Fv::D(Some(m.def), Dc::Ctx),
            Fv::Qt(m.obj_ty),
            Fv::Name(m.name),
            Fv::Nns(m.nns),
            Fv::Raw(u32::from(m.implicit)),
            Fv::TA(m.targs),
        ]);
        Ok(())
    }

    /// The words of [`Fv::Ovl`]: the candidates (or what lookup found) of
    /// `e`, as an [`F::DL`] of [`Dc::Lookup`]. A candidate with no record
    /// refuses the body under its outside-the-image reason (WP-1360).
    pub(super) fn ovl_member_words(&mut self, e: ExprId, found: bool, out: &mut Vec<u32>) -> W<()> {
        let m = self.s.ovl.members.get(&e).ok_or(OVL_MISMATCH)?;
        let ds = if found { m.found.clone() } else { m.decls.clone() };
        out.push(u32::try_from(ds.len()).map_err(|_| "a body too large")?);
        for d in ds {
            out.push(self.dref(d, Dc::Lookup)?);
        }
        Ok(())
    }
}

impl Sema<'_, '_> {
    /// Records the set of the overloaded member access `id` the importer
    /// built from a record of kind [`OVL_MEMBER`]: `a` is its object
    /// (slot a), `bits` its expression bits, `w` the values of
    /// [`OVL_FIELDS`].
    pub(super) fn image_ovl_member(&mut self, id: ExprId, a: u32, bits: u8, w: [u32; 9]) {
        let list = |s: &Self, l: u32| -> Vec<DeclId> {
            ListId::from_raw(l).map(|l| s.ast.lists.iter::<DeclId>(l).collect()).unwrap_or_default()
        };
        let (decls, found) = (list(self, w[0]), list(self, w[1]));
        let (Some(base), Some(naming), Some(def)) =
            (ExprId::from_raw(a), DeclId::from_raw(w[2]), DeclId::from_raw(w[3]))
        else {
            return;
        };
        if decls.is_empty() || decls.len() != found.len() {
            return;
        }
        debug_assert_eq!(self.ast.exprs.get(id).kind(), ExprKind::MemberExpr);
        let m = crate::overload::OvlMember {
            name: DeclName::from_raw(w[5]).unwrap_or(DeclName::EMPTY),
            decls,
            found,
            naming,
            obj_ty: QualType::from_raw(w[4]).unwrap_or(QualType::ERROR),
            def,
            base,
            arrow: bits & ExprFlags::ARROW.bits() != 0,
            nns: NnsId::from_raw(w[6]),
            implicit: w[7] != 0,
            targs: kiln_ast::ArgListId::from_raw(w[8]),
        };
        self.ovl.members.insert(id, m);
    }
}
