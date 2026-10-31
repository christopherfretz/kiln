//! Class-scope allocation and deallocation function lookup
//! ([expr.new]/12, /19, /28; [expr.delete]/9-10; [class.free]),
//! spec/sema-overload.md §10.
//!
//! clang's `Sema::FindAllocationFunctions` / `FindDeallocationFunction`:
//! unless the expression is written `::new` / `::delete`, the name
//! `operator new` (`delete`, `new[]`, `delete[]`) is looked up in the
//! scope of the allocated class first, and only when that finds nothing
//! in the global scope. Selection among the functions found is done by
//! `new_delete.rs`.

use kiln_ast::decl::DeclKind;
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::lookup::LookupKind;

/// The allocation or deallocation functions a new- or delete-expression
/// considers.
pub(crate) struct AllocFns {
    /// The functions found.
    pub fns: Vec<DeclId>,
    /// Found in the scope of the allocated class.
    pub in_class: bool,
}

impl Sema<'_, '_> {
    /// The functions named `operator op` for an object of type `elem`:
    /// the class's own (with those of its bases) unless `global`, else
    /// the global ones. Function templates are not candidates yet
    /// (WP-071).
    pub(crate) fn alloc_fns(
        &mut self,
        elem: QualType,
        op: OverloadedOperator,
        global: bool,
        loc: Loc,
    ) -> AllocFns {
        let name = DeclName::operator(op);
        if !global
            && let Some(rec) = self.ast.types.as_record(elem)
            && (self.ast.definition(rec).is_some() || self.require_complete(elem, loc))
            && let Some(def) = self.ast.definition(rec)
        {
            let found = self.lookup_in_class_dn(def, name, LookupKind::Ordinary);
            let fns: Vec<DeclId> = found
                .decls
                .iter()
                .copied()
                .filter(|&d| matches!(self.kind_of(d), DeclKind::CXXMethod | DeclKind::Function))
                .collect();
            if !found.decls.is_empty() {
                return AllocFns { fns, in_class: true };
            }
        }
        let tu = self.ast.tu;
        let fns = self
            .ast
            .lookup_local(tu, name)
            .iter()
            .filter(|&d| self.ast.decls.get(d).kind() == DeclKind::Function)
            .collect();
        AllocFns { fns, in_class: false }
    }

    /// The usual deallocation function among `fns` ([basic.stc.dynamic.
    /// deallocation]/5, [expr.delete]/10): the alignment parameter must
    /// match `aligned` when one that does exists; then a class-scope
    /// lookup prefers the unsized function, a global one the sized one
    /// when `sized`. Destroying operator delete is not supported yet.
    /// Returns the function and whether it takes the size.
    pub(crate) fn usual_delete(
        &mut self,
        fns: &[DeclId],
        in_class: bool,
        aligned: bool,
        sized: bool,
    ) -> Option<(DeclId, bool)> {
        let vp = self.ast.types.pointer_to(QualType::VOID);
        let size_t = self.size_type();
        let al = self.global_ops.align_val_t;
        let mut usual: Vec<(DeclId, bool, bool)> = Vec::new();
        for &f in fns {
            let Some((ps, variadic)) = self.fn_params(f) else { continue };
            if variadic || ps.is_empty() || !self.ast.types.same_type(ps[0], vp) {
                continue;
            }
            let rest = &ps[1..];
            let is = |t: QualType, w: Option<QualType>| {
                w.is_some_and(|w| self.ast.types.same_type(t, w))
            };
            let (has_size, has_align) = match rest {
                [] => (false, false),
                [a] if is(*a, Some(size_t)) => (true, false),
                [a] if is(*a, al) => (false, true),
                [a, b] if is(*a, Some(size_t)) && is(*b, al) => (true, true),
                _ => continue,
            };
            usual.push((f, has_size, has_align));
        }
        if usual.iter().any(|u| u.2 == aligned) {
            usual.retain(|u| u.2 == aligned);
        }
        let want_size = !in_class && sized;
        let pick = usual.iter().find(|u| u.1 == want_size).or_else(|| usual.first())?;
        Some((pick.0, pick.1))
    }

    /// `no suitable member 'operator delete' in 'X'` with a note per
    /// function found ([expr.delete]/10).
    pub(crate) fn no_suitable_delete(
        &mut self,
        op: OverloadedOperator,
        elem: QualType,
        fns: &[DeclId],
        loc: Loc,
    ) {
        let name = format!("operator {}", op.spelling());
        let mut msg = self
            .diag(DiagId::sema_no_suitable_delete_member, loc)
            .arg(format!("'{name}'"))
            .arg(self.ty_arg(elem));
        for &f in fns {
            let note = self
                .diag(DiagId::note_member_declared_named, self.ast.decls.get(f).loc())
                .arg(name.clone());
            msg = msg.with_note(note);
        }
        self.emit(msg);
    }
}
