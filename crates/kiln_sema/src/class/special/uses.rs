//! Uses of deleted functions (placeholder).

use kiln_ast::{DeclId, ExprId};
use kiln_source::Loc;

/// A reference to a deleted function waiting to see whether it is called.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PendingDeleted {
    pub expr: ExprId,
    pub decl: DeclId,
    pub loc: Loc,
    pub unqualified: bool,
}
