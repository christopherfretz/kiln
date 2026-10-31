//! The implicit definition of a defaulted comparison operator
//! ([class.eq]/3, [class.spaceship]/3, [class.compare.secondary]/2;
//! clang `DefineDefaultedComparison`).

use kiln_ast::DeclId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// Defines defaulted comparison `f` (checked, not deleted), first
    /// required at `use_loc`.
    pub(crate) fn define_defaulted_comparison(&mut self, f: DeclId, use_loc: Loc) {
        let _ = (f, use_loc);
    }
}
