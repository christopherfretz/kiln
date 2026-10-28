//! Constant emission (spec/codegen.md §5).
use kiln_ast::{ConstValueId, ExprId, QualType};
use kiln_llvm::Value;

use crate::module::{Cg, R, err};

impl Cg<'_> {
    /// Lowers a folded constant of type `ty`.
    pub(crate) fn const_value(&mut self, c: ConstValueId, ty: QualType, e: Option<ExprId>, top: bool) -> R<Value> {
        let _ = (c, ty, e, top);
        err(None, "todo")
    }
}
