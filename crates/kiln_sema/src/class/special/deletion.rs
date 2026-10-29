//! Deleted special members (placeholder).

use kiln_ast::DeclId;
use kiln_source::Loc;

use super::SpecialKind;
use crate::Sema;

/// Why a defaulted special member is deleted.
#[derive(Clone, Debug)]
pub(crate) enum DeleteReason {
    Unsupported,
}

impl Sema<'_, '_> {
    pub(crate) fn should_delete(&mut self, f: DeclId, kind: SpecialKind, diagnose: bool) -> bool {
        let _ = (f, kind, diagnose);
        false
    }

    pub(crate) fn check_defaulted(&mut self, f: DeclId, kind: SpecialKind, loc: Loc) {
        let _ = (f, kind, loc);
    }

    pub(crate) fn check_out_of_line_defaulted(&mut self, f: DeclId, kind: SpecialKind) {
        let _ = (f, kind);
    }
}
