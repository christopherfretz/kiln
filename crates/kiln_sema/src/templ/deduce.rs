//! Template argument deduction ([temp.deduct]; spec/sema-templates.md
//! §6). WP-071 owns it; until then deduction reports
//! `sema_unimplemented_template_feature` and fails.

use kiln_ast::{DeclId, ExprId};
use kiln_source::Loc;

use crate::Sema;
use crate::templ::args::ConvertedArgs;

impl Sema<'_, '_> {
    /// Deduces the arguments of function template `tmpl` from a call
    /// ([temp.deduct.call]). Not implemented (WP-071): reports and fails.
    pub fn deduce_call(
        &mut self,
        tmpl: DeclId,
        explicit: Option<&ConvertedArgs>,
        args: &[ExprId],
        loc: Loc,
    ) -> Option<ConvertedArgs> {
        let _ = (tmpl, explicit, args);
        self.templ_unimplemented("template argument deduction", loc);
        None
    }
}
