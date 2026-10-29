//! Partial ordering and partial specialization matching
//! ([temp.func.order], [temp.spec.partial.match]; spec/sema-templates.md
//! §7). WP-072 owns it; until then the queries report
//! `sema_unimplemented_template_feature`.

use kiln_ast::DeclId;
use kiln_ast::ids::ArgListId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// The partial specialization of `tmpl` that matches `args`
    /// ([temp.spec.partial.match]). Not implemented (WP-072): with
    /// partial specializations declared, reports and uses the primary
    /// template; without any, `None` (the primary) silently.
    pub(crate) fn match_partial_specs(
        &mut self,
        tmpl: DeclId,
        args: ArgListId,
        loc: Loc,
    ) -> Option<DeclId> {
        let _ = args;
        if self.has_partial_specs(tmpl) {
            self.templ_unimplemented("class template partial specialization matching", loc);
        }
        None
    }

    /// Whether `tmpl` has declared partial specializations.
    pub(crate) fn has_partial_specs(&self, tmpl: DeclId) -> bool {
        self.ast.templates.specs_of(tmpl).iter().any(|&s| {
            self.ast.decls.get(s).kind() == kiln_ast::decl::DeclKind::ClassTemplatePartialSpecialization
        })
    }

    /// Whether function template `a` is at least as specialized as `b`
    /// ([temp.func.order]). Not implemented (WP-072): reports and says no.
    pub fn more_specialized(&mut self, a: DeclId, b: DeclId, loc: Loc) -> bool {
        let _ = (a, b);
        self.templ_unimplemented("partial ordering of function templates", loc);
        false
    }
}
