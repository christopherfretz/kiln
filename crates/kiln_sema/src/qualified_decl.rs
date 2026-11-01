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
        if ds.friend.is_some() || d.friend || !self.inst.stack.is_empty() {
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
        let cur_kind = self.kind_of(cur);
        if self.ast.first(cur) == self.ast.first(dc) {
            // DR482: a qualifier naming the scope the declaration is in.
            // `struct X { void X::f(); };` is an error (clang's
            // `err_member_extra_qualification`; the qualifier is
            // dropped), for constructor, destructor and operator names
            // too; `namespace N { void N::f(); }` is
            // `-Wextra-qualification`.
            let Some((n, loc)) = self.qualified_declarator_name(d) else {
                return QualifiedDecl::Keep;
            };
            if is_record(cur_kind) {
                let m = self.diag(DiagId::sema_member_extra_qualification, loc).arg(&n);
                self.emit(m);
                return QualifiedDecl::Unqualify;
            }
            if d.special.is_none() && d.op_name.is_none() {
                let m = self.diag(DiagId::sema_namespace_member_extra_qualification, loc).arg(&n);
                self.emit(m);
            }
            return QualifiedDecl::Keep;
        }
        let Some((name, loc)) = d.name else { return QualifiedDecl::Keep };
        if d.special.is_some() || d.op_name.is_some() {
            return QualifiedDecl::Keep;
        }
        let n = sym_str(self, name);
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
            // void B::f(); };`): clang clears the qualifier after it has
            // chosen the nested class as the declaration's context, so the
            // declaration stays a member of `B`.
            let m = self.diag(DiagId::sema_member_qualification, loc).arg(&n);
            self.emit(m);
            return QualifiedDecl::Keep;
        }
        QualifiedDecl::Keep
    }

    /// The declarator-id of `d` as clang prints it in "extra
    /// qualification on member", and where it is: an identifier, a
    /// constructor or destructor name, or an operator-function-id (a
    /// conversion-function-id is not checked).
    fn qualified_declarator_name(&self, d: &Declarator) -> Option<(String, kiln_source::Loc)> {
        use crate::declspec::SpecialName;
        use kiln_ast::names::DeclName;
        match d.special {
            Some((SpecialName::Constructor(c), loc)) => Some((sym_str(self, c), loc)),
            Some((SpecialName::Destructor(c), loc)) => {
                Some((format!("~{}", sym_str(self, c)), loc))
            }
            Some((SpecialName::Operator(op), loc)) => {
                Some((self.decl_name_str(DeclName::operator(op)), loc))
            }
            Some(_) => None,
            None => {
                let (name, loc) = d.name?;
                match d.op_name {
                    Some(op) => Some((self.decl_name_str(DeclName::operator(op)), loc)),
                    None if d.lit_suffix.is_some() => None,
                    None => Some((sym_str(self, name), loc)),
                }
            }
        }
    }

    /// A namespace as clang prints a `NamedDecl` argument (qualified).
    fn scope_name(&self, d: DeclId) -> String {
        self.qualified_decl_name(d)
    }
}
