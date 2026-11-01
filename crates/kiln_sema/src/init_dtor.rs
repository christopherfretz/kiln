//! The destructor checks of aggregate initialization ([dcl.init.aggr]/8,
//! [class.dtor]/15; clang `checkDestructorReference`, WP-227): the
//! destructor of each element of class type is potentially invoked from
//! the context of the initialization, so it must be accessible and not
//! deleted, and is odr-used. Kept apart from the list checking of
//! `init.rs` (spec/sema-class.md §4.8).

use kiln_ast::{QualType, decl::DeclFlags};
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// The destructor of an element of type `ty` (taken as is: an
    /// array element of array type has no destructor of its own) is
    /// referenced at `loc`: "temporary of type 'D' has private
    /// destructor" when inaccessible. True when it is deleted, which
    /// makes the initializer list invalid (clang's early return).
    pub(crate) fn check_element_dtor(&mut self, ty: QualType, loc: Loc) -> bool {
        if self.is_dependent_type(ty) || self.ast.types.is_array(ty) {
            return false;
        }
        let Some(class) = self.element_class(ty) else { return false };
        let flags = self.ast.decls.get(class).flags();
        if flags.contains(DeclFlags::INVALID) || flags.contains(DeclFlags::DEPENDENT_CTX) {
            return false;
        }
        let Some(dtor) = self.check_temporary_dtor(class, ty, loc) else { return false };
        self.is_deleted_fn(dtor)
    }

    /// [`Sema::check_element_dtor`] of a member of type `ty`: its base
    /// element type.
    pub(crate) fn check_member_dtor(&mut self, ty: QualType, loc: Loc) -> bool {
        let elem = self.base_element(ty);
        self.check_element_dtor(elem, loc)
    }

    /// The `}` the destructor checks of an aggregate (sub)list are
    /// located at: its own, or, under brace elision, the enclosing
    /// braced list's (clang checks against the written list).
    pub(crate) fn agg_dtor_end(&self, braced: bool, rbrace: Loc) -> Loc {
        if braced { rbrace } else { self.agg_list_end.unwrap_or(rbrace) }
    }

    /// The destructor checks at the list's `end` of the fields no clause
    /// initialized, `fields[next..]`, or every field when a clause was
    /// designated (clang checks those again). True when one is deleted.
    pub(crate) fn check_remaining_dtors(
        &mut self,
        fields: &[kiln_ast::DeclId],
        enabled: bool,
        designated: bool,
        next: usize,
        end: Loc,
    ) -> bool {
        if !enabled {
            return false;
        }
        let from = if designated { 0 } else { next };
        for &f in fields.iter().skip(from) {
            let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            if self.check_member_dtor(fty, end) {
                return true;
            }
        }
        false
    }

    /// Whether the entity a converting constructor initializes in
    /// context `ctx` is destroyed later, not a temporary bound now: a
    /// variable, member or array element (clang `shouldDestroyEntity`
    /// and not `shouldBindAsTemporary`).
    pub(crate) fn converted_entity_destroyed(&self, ctx: crate::conv::ConvCtx) -> bool {
        use crate::init::entity;
        ctx == crate::conv::ConvCtx::Initialization
            && !matches!(
                self.init_entity,
                Some(e) if e == entity::BASE || e == entity::NEW || e == entity::VALUE
                    || e == entity::CAPTURE
            )
    }
}
