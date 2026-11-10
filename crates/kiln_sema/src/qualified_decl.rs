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
    /// Diagnosed in a function: the variable is declared unqualified and
    /// invalid in the block, as clang pushes its invalid out-of-line
    /// declaration on the block's scope chain, so a later redeclaration of
    /// the name in the block is diagnosed against it (WP-235).
    DropLocal,
}

impl Sema<'_, '_> {
    /// clang's `diagnoseQualifiedDeclaration` for a non-friend declarator
    /// whose declarator-id is qualified: what to do with the declaration.
    /// Every declarator-id is checked: an identifier, an
    /// operator-function-id, a conversion-function-id, a
    /// literal-operator-id, a constructor or destructor name (WP-235). A
    /// template-id declarator or a templated declaration (`has_head`) is
    /// checked elsewhere (clang's `CheckTemplateSpecializationScope`,
    /// [`Sema::check_spec_scope`]).
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
        // [temp.class.general]/3, [temp.spec.partial.general]/7: a
        // dependent `P<T*>::` names no partial specialization (clang's
        // `err_template_qualified_declarator_no_match`; clang 22 prints
        // the specifier without its trailing `::`).
        if let Some((t, nns)) = self.templ.unmatched_qualifier.take()
            && t == dc
        {
            let mut q = self.nns_str(nns);
            if self.clang_compat >= 22 && q.ends_with("::") {
                q.truncate(q.len() - 2);
            }
            let m = self.diag(DiagId::sema_template_qualified_declarator_no_match, d.name_loc());
            self.emit(m.arg(q));
            return QualifiedDecl::Drop;
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
        if let Some(r) = self.diagnose_exported_qualified(d, dc, cur) {
            return r;
        }
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
            let conversion =
                matches!(d.special, Some((crate::declspec::SpecialName::Conversion(_), _)));
            if is_record(cur_kind)
                && (conversion
                    || d.lit_suffix.is_some()
                    || matches!(
                        d.special,
                        Some((crate::declspec::SpecialName::LiteralOperator(_), _))
                    ))
            {
                // `struct X { X::operator int(); };` is clang's access
                // declaration parse, and a member literal operator is
                // rejected on its own; not diagnosed here.
                return QualifiedDecl::Keep;
            }
            if is_record(cur_kind) {
                let m = self.diag(DiagId::sema_member_extra_qualification, loc).arg(&n);
                self.emit(m);
                return QualifiedDecl::Unqualify;
            }
            let m = self.diag(DiagId::sema_namespace_member_extra_qualification, loc).arg(&n);
            self.emit(m);
            return QualifiedDecl::Keep;
        }
        let Some((n, loc)) = self.qualified_declarator_name(d) else {
            return QualifiedDecl::Keep;
        };
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
            let var_like = !ds.is_typedef()
                && !matches!(d.chunks.first(), Some(crate::declspec::Chunk::Function { .. }));
            if cur_kind.is_function() && !is_record(cur_kind) && var_like {
                return QualifiedDecl::DropLocal;
            }
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

    /// [module.interface]/1, /6 (WP-1159): directly in an export
    /// declaration clang's scope is the export, which encloses nothing. A
    /// member of a class is diagnosed and still declared; the global scope
    /// cannot be named; a namespace member is checked as an exported
    /// redeclaration (and gets no extra-qualification warning). A
    /// template-id declarator is exempt, as in clang. `None`: not in an
    /// export, or a namespace member of a scope other than `cur`, checked
    /// as usual.
    fn diagnose_exported_qualified(
        &mut self,
        d: &Declarator,
        dc: DeclId,
        cur: DeclId,
    ) -> Option<QualifiedDecl> {
        if d.spec_id.is_some() || !self.module_in_export_decl() {
            return None;
        }
        let Some((n, loc)) = self.qualified_declarator_name(d) else {
            return Some(QualifiedDecl::Keep);
        };
        match self.kind_of(dc) {
            DeclKind::TranslationUnit => {
                let m = self.diag(DiagId::sema_invalid_declarator_global_scope, loc).arg(&n);
                self.emit(m);
                Some(QualifiedDecl::Drop)
            }
            DeclKind::Namespace if self.ast.first(cur) == self.ast.first(dc) => {
                Some(QualifiedDecl::Keep)
            }
            DeclKind::Namespace => None,
            _ => {
                self.module_check_export_member(dc, &n, loc);
                Some(QualifiedDecl::Keep)
            }
        }
    }

    /// The declarator-id of `d` as clang prints it in the qualified
    /// declaration diagnostics, and where it is: an identifier, a
    /// constructor or destructor name, an operator-function-id, a
    /// conversion-function-id (`operator long`) or a literal-operator-id
    /// (`operator""_x`). Not a deduction-guide.
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
            Some((SpecialName::Conversion(t), loc)) => {
                Some((format!("operator {}", self.ty_str(t)), loc))
            }
            Some((SpecialName::LiteralOperator(x), loc)) => {
                Some((self.decl_name_str(DeclName::literal_operator(x)), loc))
            }
            Some((SpecialName::DeductionGuide(_), _)) => None,
            None => {
                let (name, loc) = d.name?;
                match (d.op_name, d.lit_suffix) {
                    (Some(op), _) => Some((self.decl_name_str(DeclName::operator(op)), loc)),
                    (None, Some(x)) => {
                        Some((self.decl_name_str(DeclName::literal_operator(x)), loc))
                    }
                    (None, None) => Some((sym_str(self, name), loc)),
                }
            }
        }
    }

    /// A scope as clang prints a `NamedDecl` argument here: unqualified,
    /// with a specialization's template arguments (`B`, `TT<int>`).
    fn scope_name(&self, d: DeclId) -> String {
        self.unqualified_decl_name(d)
    }
}
