//! The parser side of the written-reference table (spec/ast.md §18,
//! WP-1930): an id-expression or member access notes the span of the
//! name it consumed ([`Parser::note_wname`]) and records it once its
//! expression is built ([`Parser::with_written_name`]). Callers test
//! [`kiln_sema::Sema::records_written_refs`] first, so recording off
//! costs one branch per site.

use kiln_ast::written_refs::{RefMark, RefQualifier, RefRole};
use kiln_ast::{DeclId, QualType};
use kiln_source::{Loc, Span};

use crate::parser::{PExpr, Parser};

impl Parser<'_, '_> {
    /// The name being parsed began at `lo` and ended at the last token
    /// consumed, after qualifier `q`.
    #[inline]
    pub(crate) fn note_wname(&mut self, lo: Loc, q: RefQualifier) {
        if self.s.records_written_refs() {
            self.wname = Some((Span::new(lo, self.prev_end()), q));
        }
    }

    /// Runs `f` (an id-expression or member access parse) and records the
    /// name it noted with the expression it built; the site of an
    /// enclosing parse is kept across it.
    pub(crate) fn with_written_name(
        &mut self,
        role: RefRole,
        f: impl FnOnce(&mut Self) -> PExpr,
    ) -> PExpr {
        let outer = self.wname.take();
        let outer_role = self.wname_role.take();
        let r = f(self);
        let member = role == RefRole::Member;
        let role = self.wname_role.take().unwrap_or(role);
        if let Some((range, q)) = self.wname.take() {
            self.s.note_written_name(r, range, role, q, member);
        }
        self.wname = outer;
        self.wname_role = outer_role;
        r
    }

    /// Records the type or template name written at `name` (one token,
    /// or the tokens of the name) with qualifier `q`, naming `ty` (the
    /// template `tmpl` for a template name; spec/ast.md §18.4). Callers
    /// test [`kiln_sema::Sema::records_written_refs`] first.
    pub(crate) fn note_type_name(
        &mut self,
        name: Span,
        q: RefQualifier,
        ty: Option<QualType>,
        tmpl: Option<DeclId>,
        role: RefRole,
    ) {
        if let Some(id) = self.s.note_written_type(name, ty, tmpl, role, q) {
            self.last_tname = Some(id);
        }
    }

    /// Records the unqualified type name at `name` (role `type`).
    pub(crate) fn note_unqualified_type_name(&mut self, name: Span, ty: QualType) {
        if self.s.records_written_refs() {
            self.note_type_name(name, RefQualifier::NONE, Some(ty), None, RefRole::Type);
        }
    }

    /// The outermost type name recorded since `m` becomes the name of a
    /// base-specifier or mem-initializer-id (`role`).
    pub(crate) fn rerole_type_name(&mut self, m: Option<RefMark>, role: RefRole) -> bool {
        if let (Some(m), Some(id)) = (m, self.last_tname)
            && id.index() >= m.entries as usize
            && self.s.records_written_refs()
        {
            self.s.ast.written_refs.set_role(id, role);
            return true;
        }
        false
    }
}
