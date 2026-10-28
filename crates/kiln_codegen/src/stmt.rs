//! Statement emission (spec/codegen.md §4).
use kiln_ast::StmtId;

use crate::module::{Cg, R, err};

impl Cg<'_> {
    /// Emits a function body.
    pub(crate) fn emit_body(&mut self, body: StmtId) -> R<()> {
        let _ = body;
        err(None, "todo")
    }
}
