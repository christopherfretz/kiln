//! Local structured bindings (spec/modules.md §11.11, "Forms carried by
//! WP-1343").
//!
//! A decomposition variable's record is followed by one
//! [`Payload::Binding`](super::Payload::Binding) record per binding, in
//! the order the declaration names them ([dcl.struct.bind]/1); each
//! names its decomposition, its holding variable when the binding is
//! tuple-like ([dcl.struct.bind]/4), and its binding expression. The
//! importer declares the bindings with the other locals and, once the
//! bodies exist, gives the decomposition its bindings and each binding
//! its expression.

use kiln_ast::DeclId;

use super::NONE;
use super::body::Built;
use crate::Sema;

impl Sema<'_, '_> {
    /// Links the imported bindings `bindings` (binding, decomposition
    /// record, holding variable record or [`NONE`], expression or
    /// [`NONE`], in record order) to their decompositions and
    /// expressions.
    pub(super) fn image_bindings(
        &mut self,
        ids: &[DeclId],
        bindings: &[(DeclId, u32, u32, u32)],
        built: &Built,
    ) {
        let mut at = 0;
        while at < bindings.len() {
            let v = bindings[at].1;
            let end =
                bindings[at..].iter().position(|b| b.1 != v).map_or(bindings.len(), |k| at + k);
            let list: Vec<DeclId> = bindings[at..end].iter().map(|b| b.0).collect();
            if let Some(&var) = ids.get(v as usize) {
                let _ = self.builder().set_decomposition(var, &list);
            }
            at = end;
        }
        for &(b, _, holding, expr) in bindings {
            if expr != NONE {
                let h = (holding != NONE).then(|| ids.get(holding as usize).copied()).flatten();
                let _ = self.ast.decls.set_binding(b, built.expr(expr), h);
            }
        }
    }
}
