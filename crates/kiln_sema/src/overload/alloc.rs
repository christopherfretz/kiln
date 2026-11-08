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
    /// The function templates found (`template<class... A> static void*
    /// operator new(size_t, A&&...)`, or a global one; [temp.over]).
    pub templates: Vec<DeclId>,
    /// Found in the scope of the allocated class.
    pub in_class: bool,
    /// The class they were found in, the naming class of their access
    /// check ([class.access]/4, [expr.new]/19, [expr.delete]/12).
    pub naming: Option<DeclId>,
}

impl Sema<'_, '_> {
    /// The functions named `operator op` for an object of type `elem`:
    /// the class's own (with those of its bases) unless `global`, else
    /// the global ones. Function templates, class-scope or global, are
    /// kept apart for [`Sema::find_operator_new_template`].
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
            let templates: Vec<DeclId> = found
                .decls
                .iter()
                .copied()
                .filter(|&d| self.kind_of(d) == DeclKind::FunctionTemplate)
                .collect();
            if !found.decls.is_empty() {
                return AllocFns { fns, templates, in_class: true, naming: Some(def) };
            }
        }
        let tu = self.ast.tu;
        let found: Vec<DeclId> = self.ast.lookup_local(tu, name).iter().collect();
        let fns = found
            .iter()
            .copied()
            .filter(|&d| self.ast.decls.get(d).kind() == DeclKind::Function)
            .collect();
        // Global allocation function templates (LLVM's `template <...>
        // void *operator new(size_t, BumpPtrAllocatorImpl<...> &)`).
        let templates = found
            .into_iter()
            .filter(|&d| self.ast.decls.get(d).kind() == DeclKind::FunctionTemplate)
            .collect();
        AllocFns { fns, templates, in_class: false, naming: None }
    }

    /// The usual deallocation function among `fns` ([basic.stc.dynamic.
    /// deallocation]/5, [expr.delete]/10): the alignment parameter must
    /// match `aligned` when one that does exists; then a class-scope
    /// lookup prefers the unsized function, a global one the sized one
    /// when `sized` (and `-fsized-deallocation`). With `destroying` (a delete-expression or a deleting
    /// destructor, not a new-expression's cleanup), a destroying operator
    /// delete `operator delete(C*, std::destroying_delete_t, ...)` is
    /// usual too, and when one is found only those are considered
    /// ([expr.delete]/10.1). Returns the function and whether it takes
    /// the size.
    #[allow(
        clippy::fn_params_excessive_bools,
        reason = "clang's resolveDeallocationOverload flags"
    )]
    pub(crate) fn usual_delete(
        &mut self,
        fns: &[DeclId],
        in_class: bool,
        aligned: bool,
        sized: bool,
        destroying: bool,
    ) -> Option<(DeclId, bool)> {
        let vp = self.ast.types.pointer_to(QualType::VOID);
        let size_t = self.size_type();
        let al = self.global_ops.align_val_t;
        let mut usual: Vec<(DeclId, bool, bool)> = Vec::new();
        let mut any_destroying = false;
        for &f in fns {
            let Some((ps, variadic)) = self.fn_params(f) else { continue };
            if variadic || ps.is_empty() {
                continue;
            }
            let is_destroying = self.ast.is_destroying_operator_delete(f);
            if is_destroying && !destroying {
                continue;
            }
            if !is_destroying && !self.ast.types.same_type(ps[0], vp) {
                continue;
            }
            if any_destroying && !is_destroying {
                continue;
            }
            let rest = &ps[if is_destroying { 2 } else { 1 }..];
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
            if is_destroying && !any_destroying {
                usual.clear();
                any_destroying = true;
            }
            usual.push((f, has_size, has_align));
        }
        if usual.iter().any(|u| u.2 == aligned) {
            usual.retain(|u| u.2 == aligned);
        }
        // `-fno-sized-deallocation`: a global sized function is never
        // preferred, even when declared (clang 19).
        let want_size = !in_class && sized && self.sized_deallocation;
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
