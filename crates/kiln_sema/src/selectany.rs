//! GNU / Microsoft `selectany` (clang `SelectAnyAttr`; WP-809).
//!
//! clang gives the attribute no subject list: on a type, a typedef, a
//! parameter or a field it is accepted and has no effect. On a function,
//! or on a variable without external linkage, it is an error
//! (`err_attribute_selectany_non_extern_data`). A variable definition
//! carrying it is emitted `weak_odr` in a comdat of its own (clang
//! `getLLVMLinkageForDeclarator`, `shouldBeInCOMDAT`), so definitions in
//! several translation units fold into one object; an inline variable
//! or a template instantiation keeps its own linkage.
//!
//! A non-inherited `selectany` also makes an `extern` declaration without
//! an initializer a definition (clang
//! `VarDecl::isThisDeclarationADefinition`); kiln rejects that form
//! rather than treat it as a declaration.

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::DeclKind;
use kiln_ast::{AttrId, DeclId};
use kiln_diag::DiagId;

use crate::Sema;

impl Sema<'_, '_> {
    /// Checks the `selectany` attributes among `attrs` of declaration
    /// `d`, dropping the rejected ones.
    pub(crate) fn check_selectany_attrs(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        if !attrs.iter().any(|&a| self.ast.attrs.get(a).kind == AttrKind::SELECTANY) {
            return;
        }
        let kind = self.kind_of(d);
        attrs.retain(|&a| {
            let node = *self.ast.attrs.get(a);
            if node.kind != AttrKind::SELECTANY {
                return true;
            }
            let var = matches!(kind, DeclKind::Var | DeclKind::VarTemplateSpecialization);
            if !(var || kind.is_function()) {
                return true;
            }
            // clang `checkNewAttributesAfterDef`: added after the
            // definition, it is ignored.
            if var
                && let Some(prev) = self.ast.prev(d)
                && !self.ast.is_selectany(prev)
                && let Some(def) =
                    self.ast.redecls_of(prev).find(|&r| r != d && self.is_var_definition(r))
            {
                let prev_def =
                    self.diag(DiagId::note_previous_definition, self.ast.decls.get(def).loc());
                let msg = self
                    .diag(DiagId::sema_attribute_precede_definition, node.loc)
                    .with_note(prev_def);
                self.emit(msg);
                return false;
            }
            if kind.is_function() || !self.has_external_linkage(d) {
                let msg = self.diag(DiagId::sema_selectany_non_extern_data, node.loc);
                self.emit(msg);
                return false;
            }
            true
        });
    }

    /// Variable `v`, declared without an initializer, is not a
    /// definition: its own `selectany` would make it one in clang.
    pub(crate) fn check_selectany_extern(&mut self, v: DeclId) {
        if !matches!(self.kind_of(v), DeclKind::Var | DeclKind::VarTemplateSpecialization)
            || self.is_in_class_static(v)
        {
            return;
        }
        let own = self
            .ast
            .lists
            .iter::<AttrId>(self.ast.attrs.of_decl(v))
            .find(|&a| self.ast.attrs.get(a).kind == AttrKind::SELECTANY);
        if let Some(a) = own {
            let msg = self
                .diag(DiagId::parse_unsupported, self.ast.attrs.get(a).loc)
                .arg("'selectany' on an 'extern' declaration without an initializer");
            self.emit(msg);
        }
    }
}
