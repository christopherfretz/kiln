//! A declarator-id with a nested-name-specifier must be declared in a
//! scope that encloses the qualifier's ([dcl.meaning.general]/2, clang's
//! `Sema::diagnoseQualifiedDeclaration`; WP-191): `T(::a);` in a block
//! cannot redeclare the global `a`.

use kiln_ast::DeclId;
use kiln_ast::decl::DeclKind;
use kiln_diag::DiagId;

use crate::Sema;
use crate::decl::sym_str;
use crate::declspec::{DeclSpec, Declarator};

fn is_record(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
    )
}

/// The outcome of [`Sema::diagnose_qualified_declaration`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum QualifiedDecl {
    /// Declare as written.
    Keep,
    /// Diagnosed; declare it as if it were unqualified (clang's
    /// `SS.clear()`).
    Unqualify,
    /// Diagnosed; the declaration is dropped (clang marks it invalid).
    Drop,
}

impl Sema<'_, '_> {
    /// clang's `diagnoseQualifiedDeclaration` for a non-friend declarator
    /// whose identifier is qualified: what to do with the declaration. A template-id declarator or a templated
    /// declaration (`has_head`) is checked elsewhere (clang's
    /// `CheckTemplateSpecializationScope`), and only an identifier is
    /// checked (an operator-function-id, conversion-function-id or
    /// constructor name with a wrong scope is not diagnosed here).
    pub(crate) fn diagnose_qualified_declaration(
        &mut self,
        ds: &DeclSpec,
        d: &Declarator,
        has_head: bool,
    ) -> QualifiedDecl {
        let Some(dc) = d.qualifier else { return QualifiedDecl::Keep };
        let Some((name, loc)) = d.name else { return QualifiedDecl::Keep };
        if ds.friend.is_some()
            || d.friend
            || d.special.is_some()
            || d.op_name.is_some()
            || !self.inst.stack.is_empty()
        {
            return QualifiedDecl::Keep;
        }
        let mut cur = self.ctx;
        for _ in 0..10_000 {
            if !matches!(self.kind_of(cur), DeclKind::LinkageSpec | DeclKind::Export) {
                break;
            }
            match self.ast.decls.get(cur).ctx() {
                Some(p) => cur = p,
                None => break,
            }
        }
        let n = sym_str(self, name);
        let cur_kind = self.kind_of(cur);
        if self.ast.first(cur) == self.ast.first(dc) {
            // `struct X { void X::f(); };`; the namespace case is
            // `-Wextra-qualification` (not implemented).
            if is_record(cur_kind) {
                let m = self.diag(DiagId::sema_member_extra_qualification, loc).arg(&n);
                self.emit(m);
                return QualifiedDecl::Unqualify;
            }
            return QualifiedDecl::Keep;
        }
        if !self.encloses(cur, dc) {
            if d.spec_id.is_some() || has_head {
                return QualifiedDecl::Keep;
            }
            let m = if is_record(cur_kind) {
                self.diag(DiagId::sema_member_qualification, loc).arg(&n)
            } else if self.kind_of(dc) == DeclKind::TranslationUnit {
                self.diag(DiagId::sema_invalid_declarator_global_scope, loc).arg(&n)
            } else if cur_kind.is_function() {
                self.diag(DiagId::sema_invalid_declarator_in_function, loc).arg(&n)
            } else {
                self.diag(DiagId::sema_invalid_declarator_scope, loc)
                    .arg(&n)
                    .arg(self.scope_name(cur))
                    .arg(self.scope_name(dc))
            };
            self.emit(m);
            return QualifiedDecl::Drop;
        }
        if is_record(cur_kind) {
            // [class.mem.general]/7 (`struct A { struct B { void f(); };
            // void B::f(); };`): the qualifier is dropped.
            let m = self.diag(DiagId::sema_member_qualification, loc).arg(&n);
            self.emit(m);
            return QualifiedDecl::Unqualify;
        }
        QualifiedDecl::Keep
    }

    /// A namespace as clang prints a `NamedDecl` argument (qualified).
    fn scope_name(&self, d: DeclId) -> String {
        self.qualified_decl_name(d)
    }
}
