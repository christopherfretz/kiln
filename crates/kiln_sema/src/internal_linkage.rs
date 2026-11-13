//! clang `internal_linkage` (spec/sema.md §4.5; clang
//! `mergeInternalLinkageAttr`, `AttrDocs` `internal_linkage`): the
//! variable, function or class it appertains to has internal linkage, as
//! if declared `static`, and a class's members with it
//! (`kiln_abi::mangle::has_internal_linkage`, through
//! [`kiln_ast::Ast::has_internal_linkage_attr`]). The mangling follows
//! from the linkage: an internal name at namespace scope takes `L`.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::{DeclKind, TemplLinkKind};
use kiln_ast::{AttrId, DeclId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// Checks the `internal_linkage` attributes among `attrs` of `d`,
    /// dropping (with clang's diagnostic) one on a wrong subject, on a
    /// variable with automatic storage, or added by a redeclaration of a
    /// function or variable (clang `MergeFunctionDecl` /
    /// `MergeVarDecl`: `err_attribute_missing_on_first_decl`, or after
    /// the definition `warn_attribute_precede_definition`).
    ///
    /// GNU `common` is diagnosed here too: on a variable it is an error
    /// in C++ (clang `handleCommonAttr`) and dropped, so its mutual
    /// exclusion with `internal_linkage` cannot arise there; in C it is
    /// kept on a variable (a tentative definition then has common
    /// linkage even under `-fno-common`, clang
    /// `isVarDeclStrongDefinition`).
    pub(crate) fn check_internal_linkage_attrs(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        if !attrs.iter().any(|&a| {
            matches!(self.ast.attrs.get(a).kind, AttrKind::INTERNAL_LINKAGE | AttrKind::COMMON)
        }) {
            return;
        }
        let kind = self.kind_of(d);
        attrs.retain(|&a| {
            let node = *self.ast.attrs.get(a);
            if node.kind != AttrKind::COMMON {
                return true;
            }
            let name = format!("'{}'", self.attr_shown(&node, "common"));
            // `SubjectList<[Var]>`: any `VarDecl`, a parameter included.
            let var = matches!(
                kind,
                DeclKind::Var
                    | DeclKind::ParmVar
                    | DeclKind::ImplicitParam
                    | DeclKind::VarTemplateSpecialization
                    | DeclKind::VarTemplatePartialSpecialization
            );
            if var && self.is_c() {
                return true;
            }
            let msg = if var {
                self.diag(DiagId::sema_attribute_not_supported_in_cxx, node.loc).arg(name)
            } else {
                self.diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
                    .arg(name)
                    .arg("variables")
            };
            self.emit(msg);
            false
        });
        let mut dropped_redecl = false;
        let mut i = 0;
        while i < attrs.len() {
            let node = *self.ast.attrs.get(attrs[i]);
            if node.kind != AttrKind::INTERNAL_LINKAGE {
                i += 1;
                continue;
            }
            let name = format!("'{}'", self.attr_shown(&node, "internal_linkage"));
            // `SubjectList<[Var, Function, CXXRecord]>`; a `Var` of a
            // kind other than exactly `VarDecl` (a parameter, a variable
            // template specialization, a decomposition) gets the
            // hand-written list, without the serial comma.
            let what = match kind {
                k if k.is_function() => None,
                DeclKind::CXXRecord
                | DeclKind::ClassTemplateSpecialization
                | DeclKind::ClassTemplatePartialSpecialization => None,
                DeclKind::Var
                    if self.ast.decls.decomp_bindings(d).is_none()
                        && !matches!(
                            self.ast.templ_link(d).get(),
                            Some((TemplLinkKind::Specialization, _))
                        ) =>
                {
                    None
                }
                DeclKind::Var
                | DeclKind::ParmVar
                | DeclKind::ImplicitParam
                | DeclKind::VarTemplateSpecialization
                | DeclKind::VarTemplatePartialSpecialization => {
                    Some("variables, functions and classes")
                }
                _ => Some("variables, functions, and classes"),
            };
            if let Some(what) = what {
                let msg =
                    self.diag(DiagId::sema_attribute_wrong_decl_type, node.loc).arg(name).arg(what);
                self.emit(msg);
                attrs.remove(i);
                continue;
            }
            if kind == DeclKind::Var && self.has_local_storage(d) {
                let at = self.ast.decls.get(d).loc();
                self.emit(self.diag(DiagId::sema_internal_linkage_local_storage, at));
                attrs.remove(i);
                continue;
            }
            if (kind.is_function() || kind == DeclKind::Var)
                && let Some(prev) = self.ast.prev(d)
                && !self.ast.has_internal_linkage_attr(prev)
            {
                self.internal_linkage_on_redecl(d, prev, node, name, &mut dropped_redecl);
                attrs.remove(i);
                continue;
            }
            i += 1;
        }
    }

    /// [dcl.attr.noreturn]/1: the first declaration of a function
    /// specifies `[[noreturn]]` if any declaration does. clang
    /// `MergeFunctionDecl` checks it in C++ only (a `CXX11NoReturnAttr`
    /// on the new declaration, none on `prev`'s chain); C23's
    /// `[[noreturn]]` and `_Noreturn` and the GNU spelling may appear on
    /// any declaration (WP-1311). `prev` is the declaration immediately
    /// before the one carrying `attrs` (clang's `Old`), noted at `prev_at`.
    pub(crate) fn check_noreturn_on_first_decl(
        &mut self,
        prev: DeclId,
        prev_at: Loc,
        attrs: &[AttrId],
    ) {
        if self.is_c() {
            return;
        }
        let cxx11 = |s: &Self, a: AttrId| {
            let n = s.ast.attrs.get(a);
            n.kind == AttrKind::NORETURN && n.syntax == AttrSyntax::Cxx11 && !n.is_gnu_spelling()
        };
        let Some(&a) = attrs.iter().find(|&&a| cxx11(self, a)) else { return };
        let on_prev = std::iter::successors(Some(prev), |&x| self.ast.prev(x)).any(|r| {
            self.ast.lists.iter::<AttrId>(self.ast.attrs.of_decl(r)).any(|x| cxx11(self, x))
        });
        if on_prev {
            return;
        }
        let node = *self.ast.attrs.get(a);
        let name = format!("'{}'", self.attr_shown(&node, "noreturn"));
        let prev_note = self.diag(DiagId::note_previous_declaration, prev_at);
        let msg = self
            .diag(DiagId::sema_attribute_missing_on_first_decl, node.loc)
            .arg(name)
            .with_note(prev_note);
        self.emit(msg);
    }

    /// Diagnoses `internal_linkage` attribute `node` (shown as `name`)
    /// added by redeclaration `d` of `prev`, which lacks it (clang
    /// `checkNewAttributesAfterDef` first: after the definition it is
    /// ignored with a warning; else an error, once per declaration).
    fn internal_linkage_on_redecl(
        &mut self,
        d: DeclId,
        prev: DeclId,
        node: AttrNode,
        name: String,
        reported: &mut bool,
    ) {
        let def = if self.kind_of(d) == DeclKind::Var {
            self.ast.redecls_of(prev).find(|&r| r != d && self.is_var_definition(r))
        } else {
            self.ast.definition(prev).filter(|&f| f != d)
        };
        if let Some(def) = def {
            let prev_def =
                self.diag(DiagId::note_previous_definition, self.ast.decls.get(def).loc());
            let msg =
                self.diag(DiagId::sema_attribute_precede_definition, node.loc).with_note(prev_def);
            self.emit(msg);
        } else if !*reported {
            *reported = true;
            let prev_decl =
                self.diag(DiagId::note_previous_declaration, self.ast.decls.get(prev).loc());
            let msg = self
                .diag(DiagId::sema_attribute_missing_on_first_decl, self.ast.decls.get(d).loc())
                .arg(name)
                .with_note(prev_decl);
            self.emit(msg);
        }
    }
}
