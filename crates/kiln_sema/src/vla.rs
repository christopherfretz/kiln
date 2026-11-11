//! Variable length array and array-parameter helpers shared by C and the
//! C++ GNU extension (C17 6.7.6.2, 6.7.6.3; spec/sema.md §4.2): the `[*]`
//! bound, the `[*]`-decayed parameter types of a function type (clang
//! `getVariableArrayDecayedType`), and `-Warray-parameter` (clang
//! `EquivalentArrayTypes`).

use kiln_ast::types::{TypeFlags, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::declspec::ParamInfo;

impl Sema<'_, '_> {
    /// The shared bound of every `[*]` array type (C17 6.7.6.2p4), a
    /// `size_t` literal `0` (`TypeContext::vla_star_bound`; the type
    /// prints as `T[*]`). Never evaluated for a real object: `[*]` is
    /// only allowed in a prototype that is not a definition.
    pub(crate) fn vla_star_bound(&mut self) -> ExprId {
        if let Some(b) = self.ast.types.vla_star_bound {
            return b;
        }
        let st = self.size_type();
        let b = self.builder().int_literal(st, 0, kiln_ast::expr::Radix::Decimal, Loc::INVALID);
        self.ast.types.vla_star_bound = Some(b);
        b
    }

    /// `q` with every variable length array bound replaced by `[*]`
    /// (clang `getVariableArrayDecayedType`), through pointers and arrays.
    pub(crate) fn vla_star_decayed(&mut self, q: QualType) -> QualType {
        if !self.ast.types.qflags(q).contains(TypeFlags::VARIABLY_MODIFIED) {
            return q;
        }
        let quals = q.quals();
        let r = match self.ast.types.canonical_view(q) {
            TypeView::Pointer(p) => {
                let p = self.vla_star_decayed(p);
                self.ast.types.pointer_to(p)
            }
            TypeView::VariableArray { elem, .. } => {
                let e = self.vla_star_decayed(elem);
                let b = self.vla_star_bound();
                self.ast.types.variable_array(e, b)
            }
            TypeView::ConstantArray(a) => {
                let e = self.vla_star_decayed(a.elem);
                self.ast.types.constant_array(e, a.size, None)
            }
            TypeView::IncompleteArray(elem) => {
                let e = self.vla_star_decayed(elem);
                self.ast.types.incomplete_array(e)
            }
            _ => return q,
        };
        self.ast.types.qualify(r, quals)
    }

    /// `q` contains a `[*]` array derivation (C17 6.7.6.3p12; clang
    /// `diagnoseArrayStarInParamType`), through pointers and arrays.
    pub(crate) fn has_vla_star(&self, q: QualType) -> bool {
        let Some(star) = self.ast.types.vla_star_bound else { return false };
        let mut q = q;
        for _ in 0..64 {
            match self.ast.types.canonical_view(q) {
                TypeView::VariableArray { size, .. } if size == star => return true,
                TypeView::VariableArray { elem, .. }
                | TypeView::IncompleteArray(elem)
                | TypeView::Pointer(elem) => q = elem,
                TypeView::ConstantArray(a) => q = a.elem,
                _ => return false,
            }
        }
        false
    }

    /// The array form of a parameter as written: no size information
    /// (`T[]`, `T *`, `T[*]`), a constant bound, or a runtime one.
    fn array_form(&self, q: QualType) -> ArrayForm {
        let star = self.ast.types.vla_star_bound;
        match self.ast.types.canonical_view(q) {
            TypeView::IncompleteArray(_) | TypeView::Pointer(_) => ArrayForm::NoSize,
            // clang does not tell `[*]` from a bound when the element type
            // is variably modified too (`int[*][n]` is like `int[n][n]`).
            TypeView::VariableArray { size, elem }
                if Some(size) == star
                    && !self.ast.types.qflags(elem).contains(TypeFlags::VARIABLY_MODIFIED) =>
            {
                ArrayForm::NoSize
            }
            TypeView::VariableArray { .. } => ArrayForm::Variable,
            TypeView::ConstantArray(a) => ArrayForm::Constant(a.size),
            TypeView::DependentSizedArray { .. } => ArrayForm::Dependent,
            _ => ArrayForm::Other,
        }
    }

    /// clang `EquivalentArrayTypes`: `T[]`, `T *` and `T[*]` are all
    /// alike; two runtime bounds are alike unless just one is `[*]`;
    /// constant bounds compare by value.
    fn equivalent_array_forms(&self, old: QualType, new: QualType) -> bool {
        use ArrayForm::{Constant, Dependent, NoSize, Variable};
        match (self.array_form(old), self.array_form(new)) {
            (NoSize, NoSize) | (Variable, Variable) | (Dependent, Dependent) => true,
            (Constant(a), Constant(b)) => a == b,
            _ => self.ast.types.same_type(old, new),
        }
    }

    /// `-Warray-parameter` (clang `mergeParamDeclAttributes`, in
    /// `-Wall`): a redeclaration's array parameter whose bound differs
    /// from the latest earlier declaration's, at the new parameter,
    /// noting the old one with its type as written.
    pub(crate) fn check_array_param_form(&mut self, prev: DeclId, params: &[ParamInfo]) {
        if !self.diags.is_enabled(DiagId::sema_array_parameter_mismatch) {
            return;
        }
        let Some(latest) = self.ast.redecls_of(prev).next() else { return };
        let old: Vec<DeclId> = self
            .ast
            .decls
            .function(latest)
            .map(|f| self.ast.lists.iter::<DeclId>(f.params).collect())
            .unwrap_or_default();
        if old.len() != params.len() {
            return;
        }
        for (p, &o) in params.iter().zip(&old) {
            let (Some(new_t), Some(old_t)) =
                (p.original_ty, self.ast.decls.param(o).and_then(|x| x.original_ty))
            else {
                continue;
            };
            if !self.ast.types.is_array(new_t)
                || !self.ast.types.is_array(old_t)
                || self.equivalent_array_forms(old_t, new_t)
            {
                continue;
            }
            let name = p.name.map(|(n, _)| crate::decl::sym_str(self, n)).unwrap_or_default();
            let at = p.decl.map_or(p.loc, |d| self.ast.decls.get(d).loc());
            let note = self
                .diag(DiagId::note_previous_declaration_as, self.ast.decls.get(o).loc())
                .arg(self.ty_arg(old_t));
            let msg = self
                .diag(DiagId::sema_array_parameter_mismatch, at)
                .arg(name)
                .arg(self.ty_arg(new_t))
                .with_note(note);
            self.emit(msg);
        }
    }
}

/// See [`Sema::array_form`].
#[derive(Clone, Copy)]
enum ArrayForm {
    NoSize,
    Variable,
    Constant(u64),
    Dependent,
    Other,
}
