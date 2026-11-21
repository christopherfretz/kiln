//! clang `weak_import` (spec/sema.md §4.5; `handleWeakImportAttr`,
//! `Decl::canBeWeakImported`; WP-1745): on a declaration of a variable
//! or function that is not a definition, the entity becomes a weak
//! reference when the translation unit does not define it (codegen
//! declares it `extern_weak`, [`kiln_ast::Ast::is_weak_imported`]). On a
//! variable's definition or a parameter it warns and is dropped; on
//! another declaration it warns as a wrong subject. After the entity's
//! definition it is ignored with clang's `checkNewAttributesAfterDef`
//! warning.

use kiln_ast::DeclId;
use kiln_ast::attr::{AttrKind, AttrNode};
use kiln_ast::decl::DeclKind;
use kiln_diag::DiagId;

use crate::Sema;

impl Sema<'_, '_> {
    /// The checks of `weak_import` attribute `node` on `d`: false drops
    /// it after a warning.
    pub(crate) fn check_weak_import(&mut self, d: DeclId, node: AttrNode) -> bool {
        let param = self.kind_of(d) == DeclKind::ParmVar;
        let is_var = param || self.ast.decls.var(d).is_some();
        let is_fn = self.ast.decls.function(d).is_some();
        if !is_var && !is_fn {
            let shown = self.attr_shown(&node, "weak_import");
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
                .arg(format!("'{shown}'"))
                .arg("variables and functions");
            self.emit(msg);
            return false;
        }
        // `canBeWeakImported`: a variable that this declaration defines
        // (a parameter always does). A function's body comes after its
        // attributes are checked.
        if param || is_var && self.is_var_definition(d) {
            let msg = self
                .diag(DiagId::sema_attribute_invalid_on_definition, node.loc)
                .arg("weak_import");
            self.emit(msg);
            return false;
        }
        // `checkNewAttributesAfterDef`: an entity defined before.
        let defined = |s: &Self, p: DeclId| {
            if is_var {
                s.is_var_definition(p)
            } else {
                s.ast.decls.function(p).is_some_and(|f| f.body.is_some())
            }
        };
        if !param
            && let Some(def) = std::iter::successors(self.ast.prev(d), |&p| self.ast.prev(p))
                .find(|&p| defined(self, p))
            && self.find_attr(def, AttrKind::WEAK_IMPORT).is_none()
        {
            let prev = self.diag(DiagId::note_previous_definition, self.ast.decls.get(def).loc());
            let msg =
                self.diag(DiagId::sema_attribute_precede_definition, node.loc).with_note(prev);
            self.emit(msg);
            return false;
        }
        true
    }
}
