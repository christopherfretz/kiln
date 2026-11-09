//! `[[carries_dependency]]`'s placement rules (WP-954; [dcl.attr.depend],
//! spec/sema.md §4.5).
//!
//! [dcl.attr.depend]/1: it appertains to the declarator-id of a function
//! declaration or lambda, or to one of their parameters: a parameter of
//! any other function declarator (a pointer to function, a typedef, a
//! returned function type) is clang's `'[[carries_dependency]]' attribute
//! only allowed on parameter in a function declaration or lambda` (clang
//! `handleDependencyAttr`, which tests `FunctionDeclarationScope`).
//! [dcl.attr.depend]/2: the first declaration of a function specifies it
//! (for the function, or for a parameter) if any declaration does: clang
//! `MergeFunctionDecl` / `mergeParamDeclAttributes`.

use kiln_ast::attr::AttrKind;
use kiln_ast::{AttrId, DeclId};
use kiln_diag::DiagId;

use crate::Sema;
use crate::declspec::ParamInfo;

impl Sema<'_, '_> {
    /// [dcl.attr.depend]/1 for parameter `p` of a function declarator that
    /// declares no function: each `carries_dependency` is an error and is
    /// dropped.
    pub fn act_on_nonfunc_param_attrs(&mut self, p: &mut ParamInfo) {
        p.attrs.retain(|&a| {
            let node = *self.ast.attrs.get(a);
            if node.kind != AttrKind::CARRIES_DEPENDENCY {
                return true;
            }
            self.error(DiagId::sema_carries_dependency_param_not_function_decl, node.loc);
            false
        });
    }

    /// [dcl.attr.depend]/2 for function redeclaration `f` with attributes
    /// `attrs`: a `carries_dependency` on `f` or on its parameter `i` that
    /// no earlier declaration (or its parameter `i`) has is an error,
    /// noted at the first declaration (or its parameter `i`).
    pub(crate) fn check_carries_dependency_redecl(&mut self, f: DeclId, attrs: &[AttrId]) {
        let has = |s: &Self, d: DeclId| {
            s.ast
                .lists
                .iter::<AttrId>(s.ast.attrs.of_decl(d))
                .any(|a| s.ast.attrs.get(a).kind == AttrKind::CARRIES_DEPENDENCY)
        };
        let params = self.param_decls(f);
        let own = attrs
            .iter()
            .copied()
            .find(|&a| self.ast.attrs.get(a).kind == AttrKind::CARRIES_DEPENDENCY);
        let marked: Vec<(usize, DeclId)> =
            params.iter().copied().enumerate().filter(|&(_, p)| has(self, p)).collect();
        if own.is_none() && marked.is_empty() {
            return;
        }
        let earlier: Vec<DeclId> = self.ast.redecls_of(f).filter(|&r| r != f).take(4096).collect();
        if earlier.is_empty() {
            return;
        }
        let first = self.ast.first(f);
        if let Some(a) = own
            && !earlier.iter().any(|&r| has(self, r))
        {
            let at = self.ast.attrs.get(a).loc;
            let note =
                self.diag(DiagId::note_carries_dependency_missing_first_decl, self.decl_loc(first));
            let msg = self
                .diag(DiagId::sema_carries_dependency_missing_on_first_decl, at)
                .arg("0")
                .with_note(note);
            self.emit(msg);
        }
        for (i, p) in marked {
            let earlier_params: Vec<Option<DeclId>> =
                earlier.iter().map(|&r| self.param_decls(r).get(i).copied()).collect();
            if earlier_params.iter().flatten().any(|&q| has(self, q)) {
                continue;
            }
            let Some(&fp) = self.param_decls(first).get(i) else { continue };
            let Some(at) = self
                .ast
                .lists
                .iter::<AttrId>(self.ast.attrs.of_decl(p))
                .find(|&a| self.ast.attrs.get(a).kind == AttrKind::CARRIES_DEPENDENCY)
                .map(|a| self.ast.attrs.get(a).loc)
            else {
                continue;
            };
            let note =
                self.diag(DiagId::note_carries_dependency_missing_first_decl, self.decl_loc(fp));
            let msg = self
                .diag(DiagId::sema_carries_dependency_missing_on_first_decl, at)
                .arg("1")
                .with_note(note);
            self.emit(msg);
        }
    }
}
