//! A member with the name of its class ([class.mem.general]/22, WP-1015):
//! clang's `DiagnoseClassNameShadow` and the field check of
//! `CheckCompletedCXXClass` (spec/sema.md §4.4.1 "Members named as their
//! class").

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::names::DeclName;
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;

impl Sema<'_, '_> {
    /// [class.mem.general]/22: a static data member, a member type, a
    /// member template, an enumerator of an unscoped member enumeration
    /// and a member named by a using-declaration have a name different
    /// from the class. Diagnoses `name` declared as a member of `ctx`
    /// (clang `err_member_name_of_class`, at the name); true when it did.
    pub(crate) fn diagnose_class_name_shadow(
        &mut self,
        ctx: DeclId,
        name: Symbol,
        loc: Loc,
    ) -> bool {
        if !self.is_class_named(ctx, name) {
            return false;
        }
        let msg = self
            .diag(DiagId::sema_member_name_of_class, loc)
            .arg(self.ast.symbols().as_str(name).to_owned());
        self.emit(msg);
        true
    }

    /// [class.mem.general]/22: when the class has a user-declared
    /// constructor, a non-static data member has a name different from
    /// the class (clang `CheckCompletedCXXClass`, the first such member
    /// only). Run as the class definition completes.
    pub(crate) fn check_field_named_as_class(&mut self, r: DeclId) {
        let Some(name) = self.ast.decls.get(r).name().as_ident() else { return };
        let members: Vec<DeclId> = self.ast.decls.members(r).collect();
        let has_ctor = members.iter().any(|&m| {
            let n = self.ast.decls.get(m);
            let f = if n.kind() == DeclKind::FunctionTemplate {
                self.pattern_of(m).unwrap_or(m)
            } else {
                m
            };
            !n.has(DeclFlags::IMPLICIT) && self.kind_of(f) == DeclKind::CXXConstructor
        });
        if !has_ctor {
            return;
        }
        let field = members.into_iter().find(|&m| {
            let n = self.ast.decls.get(m);
            n.name() == DeclName::ident(name)
                && self.kind_of(self.ast.strip_shadow(m)) == DeclKind::Field
        });
        if let Some(f) = field {
            let loc = self.ast.decls.get(f).loc();
            let msg = self
                .diag(DiagId::sema_member_name_of_class, loc)
                .arg(self.ast.symbols().as_str(name).to_owned());
            self.emit(msg);
        }
    }
}
