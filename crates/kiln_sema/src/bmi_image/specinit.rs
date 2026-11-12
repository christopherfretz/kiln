//! In-class initializers of static data members of class template
//! patterns as expression sections (spec/modules.md §11.11, "Forms
//! carried by WP-1344").
//!
//! The importer instantiates a specialization the unit completed while
//! it builds the records ([`super::spec`]). An instantiation made then
//! may need the value of a static data member of another specialization
//! (`template <class T, bool = (A<T>::n > 2)> struct B;` named in a
//! member of the completed class): its pattern's in-class initializer
//! has to be there, so it is carried as an expression section
//! ([`super::exprs`]) built with the member's record, as the parser sets
//! it, rather than in the image's bodies. An initializer a section
//! cannot hold stays in the bodies.

use kiln_ast::{DeclId, ExprId, QualType};

use super::{Image, Payload, QRef, Res, Writer, bad, body};
use crate::Sema;

/// The `Var` record `spec` bit of an initializer that is an expression
/// section (the `VarSpec` bits stop at bit 12).
pub(super) const INIT_SECT: u32 = 1 << 31;

impl Writer<'_, '_, '_> {
    /// The expression section of initializer `e` of static data member
    /// `d` of a carried class template pattern; `None` for any other
    /// variable or an initializer a section cannot hold.
    pub(super) fn static_init_section(&mut self, d: DeclId, e: ExprId) -> Option<u32> {
        let ast = &self.s.ast;
        let member = ast.decls.get(d).ctx().is_some_and(|c| ast.decls.record(c).is_some());
        if !member || !self.dep_member(d) {
            return None;
        }
        self.texpr(e).ok()
    }
}

/// Checks the section initializer of `Var` record `i` (`payload`): a
/// section whose records and types are all before the record.
pub(super) fn validate_init_section(
    img: &Image,
    i: usize,
    payload: &Payload,
    qref_at: &dyn Fn(QRef, usize) -> bool,
) -> Res<()> {
    let Payload::Var { spec, init, .. } = payload else { return Ok(()) };
    if spec & INIT_SECT == 0 {
        return Ok(());
    }
    let Some(b) = img.texprs.get(*init as usize) else {
        return bad("bad variable initializer");
    };
    if body::section_decl_bound(b) > i {
        return bad("bad variable initializer");
    }
    body::validate_section(img, b, 0, &|q| qref_at(q, i), true)
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Gives variable `v` the initializer of section `i`, now.
    pub(super) fn image_init_section(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        v: DeclId,
        i: u32,
    ) {
        if let Some(e) = self.image_record_section(img, tys, ids, i) {
            let _ = self.builder().set_var_init(v, e);
        }
    }

    /// Builds section `i` while the records are built: the types it
    /// names first, in order, as a section cannot be built while another
    /// is (a type's own sections are built before the type).
    pub(super) fn image_record_section(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        i: u32,
    ) -> Option<ExprId> {
        let hi = std::cell::Cell::new(0u32);
        if let Some(b) = img.texprs.get(i as usize) {
            let _ = body::validate_section(
                img,
                b,
                0,
                &|q| {
                    hi.set(hi.get().max(q.ty));
                    true
                },
                true,
            );
        }
        if hi.get() > 0 {
            let _ = self.image_ty(img, tys, ids, QRef { ty: hi.get(), quals: 0 });
        }
        self.image_texpr_build(img, tys, ids, i)
    }
}
