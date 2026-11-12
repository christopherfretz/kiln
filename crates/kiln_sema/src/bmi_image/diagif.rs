//! `diagnose_if` rechecks of calls in templates (spec/modules.md §11.11,
//! "Forms carried by WP-1353").
//!
//! A call in a template definition of a function with argument-dependent
//! `diagnose_if` attributes, whose arguments are not value-dependent, is
//! checked by each instantiation that shares the call
//! (`diagnose_if_recheck`, keyed by its first argument, clang
//! `diagnoseArgDependentDiagnoseIfAttrs` on the rebuilt call). The
//! producer's parse filled that map; nothing in the image records it, so
//! the importer fills it again from the calls of the bodies it built:
//! every `CallExpr` naming such a function whose arguments are not
//! value-dependent (an entry for a call outside a template is never
//! looked up).

use kiln_ast::expr::ExprKind;

use super::body::Built;
use crate::Sema;

impl Sema<'_, '_> {
    /// Fills `diagnose_if_recheck` from the `n` built body expressions.
    pub(super) fn image_diagnose_if_rechecks(&mut self, built: &Built, n: u32) {
        if !self.any_diagnose_if {
            return;
        }
        for i in 0..n {
            let e = built.expr(i);
            if self.ast.exprs.get(e).kind() != ExprKind::CallExpr {
                continue;
            }
            let Some(f) = self.callee_decl(e) else { continue };
            if self.ast.decls.function(f).is_none() || self.diagnose_if_attrs(f, true).is_empty() {
                continue;
            }
            let args: Vec<_> = self.ast.lists.iter(self.ast.exprs.call_args(e)).collect();
            let Some(&first) = args.first() else { continue };
            if args.iter().any(|&a| self.expr_is_value_dependent(a)) {
                continue;
            }
            let loc = self.ast.exprs.end_loc(e).unwrap_or_else(|| self.ast.exprs.get(e).loc());
            self.diagnose_if_recheck.entry(first).or_insert((f, args, loc));
        }
    }
}
