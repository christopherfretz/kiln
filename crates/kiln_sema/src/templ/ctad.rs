//! Class template argument deduction ([over.match.class.deduct]) —
//! a stub until WP-071b: a placeholder for a deduced class type is
//! reported as unimplemented by its callers.

use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// Deduces the template arguments of a class template from an
    /// initializer (WP-071b): reports and fails.
    pub fn deduce_class_template_args(&self, loc: Loc) -> Option<kiln_ast::QualType> {
        self.templ_unimplemented("class template argument deduction", loc);
        None
    }
}
