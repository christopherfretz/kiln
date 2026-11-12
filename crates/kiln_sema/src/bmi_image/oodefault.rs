//! Special members defaulted outside their class (spec/modules.md
//! §11.11, "Forms carried by WP-1353").
//!
//! `inline S::S() = default;` after `struct S { S(); };`
//! ([dcl.fct.def.default]/5): the redeclaration is user-provided and
//! defined where it is written (clang `SetDeclDefaulted` →
//! `DefineDefaultedFunction`). Its record is an ordinary `Function`
//! redeclaration of a non-dependent class's member with the defaulted
//! bit and no body; the importer runs `act_on_defaulted` on the
//! declaration it built, as the parser does after the declarator, which
//! defines it again.
//!
//! Not carried: one of a class template's member (its definition is the
//! instantiations'), an out-of-line defaulted non-special member.

use kiln_ast::DeclId;
use kiln_ast::decl::DeclFlags;
use kiln_source::Loc;

use super::Writer;
use crate::Sema;

impl Writer<'_, '_, '_> {
    /// Whether `d`, defaulted on a redeclaration, is a special member of
    /// a class that is not dependent.
    pub(super) fn out_of_line_defaulted_ok(&self, d: DeclId) -> bool {
        let n = self.s.ast.decls.get(d);
        !n.has(DeclFlags::DEPENDENT_CTX)
            && self.s.ast.described_template(d).is_none()
            && self.s.special_kind_of(d).is_some()
    }
}

impl Sema<'_, '_> {
    /// Defines out-of-line defaulted redeclaration `f` (at `loc`) again.
    pub(super) fn image_out_of_line_defaulted(&mut self, f: DeclId, loc: Loc) {
        self.act_on_defaulted(f, loc);
    }
}
