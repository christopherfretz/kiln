//! Base paths through class template specializations in the declaration
//! image (WP-1369, spec/modules.md §11.11, "Forms carried by WP-1369").
//!
//! A derived-to-base conversion keeps its path as base-specifiers
//! ([expr.ref]/7, [conv.ptr]/3: `a.fetch_sub(u)` naming a member of a
//! base of `a`'s class); the image writes each as (class record,
//! position in its base-specifier list). A class with a definition record
//! knows its base-specifiers; an implicit instantiation of a class
//! template (`atomic<long>`, a complete specialization reference) has none
//! written, since the importer instantiates its definition again
//! ([temp.inst]/2), and its base-specifiers come out in the same order as
//! the pattern's. The path element is then (specialization reference
//! record, position), and the importer takes the base-specifier at that
//! position in its own instantiation's list.

use kiln_ast::DeclId;

use super::{Image, Payload, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;

/// The member class references whose class the unit completed
/// ([temp.inst]/2: a member class of an implicit instantiation is
/// instantiated when a complete type is required, as a data member's type
/// requires); the importer completes them where it resolves them, the
/// records ascending.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct MemberClasses {
    pub(super) complete: Vec<u32>,
}

impl MemberClasses {
    /// Drops the entries of records from `n` on (a census rollback).
    pub(super) fn truncate(&mut self, n: u32) {
        self.complete.retain(|&r| r < n);
    }
}

pub(super) fn encode_member_classes(w: &mut Wr, m: &MemberClasses) {
    w.len(m.complete.len());
    for &r in &m.complete {
        w.u32(r);
    }
}

pub(super) fn decode_member_classes(r: &mut Rd) -> Res<MemberClasses> {
    let n = r.count(4)?;
    let mut complete = Vec::with_capacity(n);
    for _ in 0..n {
        complete.push(r.u32()?);
    }
    Ok(MemberClasses { complete })
}

/// Each entry is a member reference whose pattern member is a class
/// record, once, ascending.
pub(super) fn validate_member_classes(img: &Image) -> Res<()> {
    let mut last = None;
    for &r in &img.mclasses.complete {
        let ok = last.is_none_or(|l| l < r)
            && img.decls.get(r as usize).is_some_and(|d| match d.payload {
                Payload::Member { pattern, .. } => matches!(
                    img.decls.get(pattern as usize).map(|p| &p.payload),
                    Some(Payload::Record { .. })
                ),
                _ => false,
            });
        if !ok {
            return bad("bad member class completion");
        }
        last = Some(r);
    }
    Ok(())
}

impl Writer<'_, '_, '_> {
    /// The path element of base-specifier `g` of a complete class
    /// specialization reference already recorded (WP-1369): every such
    /// reference's base-specifiers are entered in `bs.bspec`, then `g`
    /// is looked up there.
    pub(super) fn spec_base(&mut self, g: u32) -> W<(u32, u32)> {
        let ast = &self.s.ast;
        let refs: Vec<(DeclId, u32)> = self
            .recs
            .iter()
            .filter(|&(_, &r)| {
                matches!(
                    self.img.decls.get(r as usize).map(|x| &x.payload),
                    Some(Payload::Spec { complete: true, .. })
                )
            })
            .map(|(&d, &r)| (d, r))
            .collect();
        for (d, r) in refs {
            let Some(rd) = ast.definition(d).and_then(|def| ast.decls.record(def)) else {
                continue;
            };
            for (pos, &b) in ast.lists.get(rd.bases).iter().enumerate() {
                self.bs.bspec.entry(b).or_insert((r, pos as u32));
            }
        }
        self.bs.bspec.get(&g).copied().ok_or(super::outside::OUT_BASE)
    }
}

/// Whether path element (`rec`, `pos`) is valid: a base of a class
/// definition record, or (WP-1369) of a complete specialization reference
/// of a class template, whose bases the importer counts.
pub(super) fn path_element_ok(img: &Image, rec: u32, pos: u32) -> bool {
    img.decls.get(rec as usize).is_some_and(|r| match &r.payload {
        Payload::Record { def: Some(df), .. } => (pos as usize) < df.bases.len(),
        Payload::Spec { template, complete: true, .. } => matches!(
            img.decls.get(*template as usize).map(|t| &t.payload),
            Some(Payload::Template { kind: 1, .. })
        ),
        _ => false,
    })
}

impl Writer<'_, '_, '_> {
    /// The record of class `d` naming its type `t`: its own, or (WP-1369)
    /// when `d` is a member class of an implicit instantiation of a
    /// carried class template (`storage<8, 8>::type`, [temp.inst]/3: the
    /// member class is declared with the specialization), its member
    /// reference, which the importer resolves in its own instantiation.
    pub(super) fn record_type_decl(&mut self, d: DeclId, t: kiln_ast::TypeId) -> W<u32> {
        let ast = &self.s.ast;
        if self.recs.contains_key(&d)
            || !self.spec_member(d)
            || ast.decls.get(d).kind() != kiln_ast::decl::DeclKind::CXXRecord
            || ast.decls.get(d).ty().is_none_or(|q| q.ty() != t || q.quals_bits() != 0)
        {
            return self.type_decl(d, t);
        }
        match self.member_rec(d)? {
            Some(r) => {
                let ast = &self.s.ast;
                let done = ast
                    .definition(d)
                    .and_then(|x| ast.decls.record(x))
                    .is_some_and(|x| x.defn.contains(kiln_ast::decl::DefinitionBits::COMPLETE));
                let list = &mut self.img.mclasses.complete;
                if done && list.last().is_none_or(|&l| l < r) {
                    list.push(r);
                }
                Ok(r)
            }
            None => self.type_decl(d, t),
        }
    }
}

impl Sema<'_, '_> {
    /// Completes the member classes the unit completed ([`MemberClasses`]),
    /// once the attributes' expressions are in: a member class's
    /// `alignas(A)` is substituted when it is instantiated.
    pub(super) fn image_member_classes(&mut self, img: &Image, ids: &[DeclId]) {
        for &r in &img.mclasses.complete {
            let Some(&m) = ids.get(r as usize) else { continue };
            if m == self.ast.tu {
                continue;
            }
            if let Some(ty) = self.ast.decls.get(m).ty() {
                let loc = self.ast.decls.get(m).loc();
                let _ = self.require_complete(ty, loc);
            }
        }
    }
}
