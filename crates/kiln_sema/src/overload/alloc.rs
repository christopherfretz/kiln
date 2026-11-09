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

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::{ExcSpec, FuncInfo};
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::lookup::LookupKind;

/// What `std::type_identity` names ([`Sema::lookup_std_type_identity`]).
#[derive(Clone, Copy)]
pub(crate) enum StdTypeIdentity {
    /// No `std::type_identity`.
    Absent,
    /// Not a class template with a single type parameter: the first
    /// declaration found.
    Malformed(DeclId),
    /// The class template.
    Template(DeclId),
}

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
    /// `(function, using-shadow)` for each class-scope function found
    /// through a using-declaration: access is checked on the shadow
    /// ([namespace.udecl]/19, clang's `FoundDecl`).
    pub via: Vec<(DeclId, DeclId)>,
}

impl AllocFns {
    /// The declaration lookup found for `f`: its using-shadow when it was
    /// found through a using-declaration, else `f`.
    pub(crate) fn found_decl(&self, f: DeclId) -> DeclId {
        self.via.iter().find(|&&(t, _)| t == f).map_or(f, |&(_, s)| s)
    }
}

/// A usual deallocation function (clang `UsualDeallocFnInfo`).
#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "clang's UsualDeallocFnInfo")]
pub(crate) struct UsualDealloc {
    /// The declaration found (a function template for a deduced one).
    pub found: DeclId,
    /// The function called.
    pub f: DeclId,
    /// A destroying operator delete ([expr.delete]/10.1).
    pub destroying: bool,
    /// Type-aware: takes `std::type_identity<T>` first (P2719).
    pub typed: bool,
    /// Takes the size.
    pub sized: bool,
    /// Takes the alignment.
    pub aligned: bool,
    /// A function template's specialization.
    pub template: bool,
}

/// The implicit deallocation arguments a delete-expression, deleting
/// destructor or new-expression cleanup prefers (clang
/// `ImplicitDeallocationParameters`).
#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "clang's ImplicitDeallocationParameters")]
pub(crate) struct DeallocWant {
    /// The type deallocated.
    pub ty: QualType,
    /// Type-aware functions are considered and preferred.
    pub typed: bool,
    /// The type is over-aligned.
    pub aligned: bool,
    /// The size can be passed and a sized function is preferred.
    pub sized: bool,
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
        // An array of class type looks in the element class
        // ([expr.new]/12 "the allocated type is a class type T or array
        // thereof"; clang `getBaseElementType`).
        let elem = self.base_element(elem);
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
            let via = found
                .decls
                .iter()
                .zip(&found.found)
                .filter(|(t, s)| t != s)
                .map(|(&t, &s)| (t, s))
                .collect();
            if !found.decls.is_empty() {
                return AllocFns { fns, templates, in_class: true, naming: Some(def), via };
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
        AllocFns { fns, templates, in_class: false, naming: None, via: Vec::new() }
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

    /// clang `UsualDeallocFnInfo`: `found` (a function, or a function
    /// template whose type-aware usual specialization for `ty` is
    /// deduced, clang `BuildTypeAwareUsualDelete`) as a usual
    /// deallocation function for an object of type `ty`
    /// ([basic.stc.dynamic.deallocation]/3, P2719): its `std::
    /// type_identity<ty>`, destroying, size and alignment parameters.
    /// `None` when it is not a usual deallocation function, or a
    /// type-aware one for another type.
    fn usual_dealloc_info(
        &mut self,
        found: DeclId,
        ty: QualType,
        loc: Loc,
    ) -> Option<UsualDealloc> {
        let template = self.kind_of(found) == DeclKind::FunctionTemplate;
        let size_t = self.size_type();
        let al = self.global_ops.align_val_t;
        let is_al = |s: &Self, t: QualType| al.is_some_and(|a| s.ast.types.same_unqualified(t, a));
        let f = if template {
            let pat = self.templated_function(found)?;
            if !self.is_type_aware_alloc_fn(found) {
                return None;
            }
            let (ps, variadic) = self.fn_params(pat)?;
            if variadic || ps.len() != 4 || ps[1..].iter().any(|&p| self.is_dependent_type(p)) {
                return None;
            }
            let ti = self.std_type_identity(ty, loc)?;
            let mut want = vec![ti];
            want.extend_from_slice(&ps[1..]);
            let info = FuncInfo::default();
            let expected = self.ast.types.function(QualType::VOID, &want, info, ExcSpec::None);
            self.deduce_decl(found, None, expected, loc).ok()?
        } else {
            found
        };
        let (ps, variadic) = self.fn_params(f)?;
        if variadic || ps.is_empty() {
            return None;
        }
        let typed = self.is_type_aware_alloc_fn(f);
        let mut i = 0;
        if typed {
            let ti = self.std_type_identity(ty, loc)?;
            if !self.ast.types.same_type(ps[0], ti) {
                return None;
            }
            i = 1;
        }
        let destroying = !typed && self.ast.is_destroying_operator_delete(f);
        let vp = self.ast.types.pointer_to(QualType::VOID);
        if !destroying && !ps.get(i).is_some_and(|&p| self.ast.types.same_type(p, vp)) {
            return None;
        }
        i += 1 + usize::from(destroying);
        let sized = ps.get(i).is_some_and(|&p| self.ast.types.same_unqualified(p, size_t));
        i += usize::from(sized);
        let aligned = ps.get(i).is_some_and(|&p| is_al(self, p));
        i += usize::from(aligned);
        // A type-aware usual deallocation function takes both
        // (`operator delete(std::type_identity<T>, void*, std::size_t,
        // std::align_val_t)`).
        if i != ps.len() || typed && !(sized && aligned) {
            return None;
        }
        Some(UsualDealloc { found, f, destroying, typed, sized, aligned, template })
    }

    /// clang `resolveDeallocationOverload`: the best usual deallocation
    /// functions among `found` for `want` ([expr.delete]/10, P2719): a
    /// destroying one first, then one whose type-awareness, alignment
    /// and size parameters match `want` in that order, then a
    /// type-aware non-template over a template, then the more
    /// specialized template. Several when they tie. Type-aware ones are
    /// skipped unless `want.typed`, destroying ones unless `destroying`.
    pub(crate) fn resolve_usual_dealloc(
        &mut self,
        found: &[DeclId],
        want: DeallocWant,
        destroying: bool,
        loc: Loc,
    ) -> Vec<UsualDealloc> {
        let mut infos: Vec<UsualDealloc> = Vec::new();
        for &d in found {
            if self.kind_of(d) != DeclKind::FunctionTemplate
                && self.ast.decls.get(d).has(DeclFlags::INVALID)
            {
                continue;
            }
            // A type-aware one is not looked at for an untyped want (its
            // `std::type_identity<T>` is never built, clang
            // `UsualDeallocFnInfo` under `PassTypeIdentity::No`).
            if !want.typed && self.is_type_aware_alloc_fn(d) {
                continue;
            }
            // With a malformed `std::type_identity`, clang marks every
            // type-aware declaration invalid; a namespace-scope one is
            // then not found at all, so is skipped without the malformed
            // diagnostic a class member's use gives.
            if self.is_type_aware_alloc_fn(d)
                && matches!(self.lookup_std_type_identity(), StdTypeIdentity::Malformed(_))
                && !self.ast.decls.get(d).ctx().is_some_and(|c| self.kind_of(c) == DeclKind::CXXRecord)
            {
                continue;
            }
            let Some(u) = self.usual_dealloc_info(d, want.ty, loc) else { continue };
            if (u.typed && !want.typed) || (u.destroying && !destroying) {
                continue;
            }
            infos.push(u);
        }
        let key = |u: &UsualDealloc| {
            (
                u.destroying,
                u.typed == want.typed,
                u.aligned == want.aligned,
                u.sized == want.sized,
                !(u.typed && u.template),
            )
        };
        let Some(best) = infos.iter().map(key).max() else { return infos };
        infos.retain(|u| key(u) == best);
        // Type-aware templates: the more specialized one ([temp.func.order]).
        if infos.len() > 1 && infos.iter().all(|u| u.typed && u.template) {
            let n = 4 + usize::from(infos[0].destroying);
            let mut keep: Vec<UsualDealloc> = Vec::new();
            for (i, u) in infos.iter().enumerate() {
                let beaten = infos
                    .iter()
                    .enumerate()
                    .any(|(j, v)| i != j && self.more_specialized(v.found, u.found, n));
                if !beaten {
                    keep.push(*u);
                }
            }
            infos = keep;
        }
        infos
    }

    /// What ordinary lookup of `std::type_identity` finds (clang
    /// `LookupStdClassTemplate`, P2719): nothing, a usable class
    /// template (one whose only required parameter is a type parameter),
    /// or something else, the declaration a diagnostic names.
    pub(crate) fn lookup_std_type_identity(&self) -> StdTypeIdentity {
        let std = self.intern("std");
        let ns = self.lookup_qualified(self.ast.tu, std, LookupKind::Nested);
        let Some(ns) = ns.decls.iter().copied().find(|&d| self.kind_of(d) == DeclKind::Namespace)
        else {
            return StdTypeIdentity::Absent;
        };
        let n = self.intern("type_identity");
        let found = self.lookup_qualified(ns, n, LookupKind::Ordinary);
        let Some(&first) = found.decls.first() else { return StdTypeIdentity::Absent };
        let single = found.decls.iter().all(|&d| self.ast.first(d) == self.ast.first(first));
        if !single || self.kind_of(first) != DeclKind::ClassTemplate {
            return StdTypeIdentity::Malformed(first);
        }
        // `getMinRequiredArguments() == 1` and a type parameter first.
        let params = self.template_params(first);
        let required = params
            .iter()
            .take_while(|&&p| {
                !self.ast.template_parm(p).is_some_and(|x| x.pack)
                    && self.template_default(p).is_none()
            })
            .count();
        let type_first = params.first().is_some_and(|&p| {
            self.kind_of(p) == DeclKind::TemplateTypeParm
                && !self.ast.template_parm(p).is_some_and(|x| x.pack)
        });
        if required != 1 || !type_first {
            return StdTypeIdentity::Malformed(first);
        }
        StdTypeIdentity::Template(first)
    }

    /// `std::type_identity<T>` for the unqualified `ty` (P2719, clang
    /// `tryBuildStdTypeIdentity`), not required to be complete: the
    /// new-expression requires that at its start, a delete-expression of
    /// the selected deallocation function. `None` without a usable
    /// template; a malformed one is diagnosed at its declaration on every
    /// use, as clang does not remember it (`err_malformed_std_class_
    /// template`).
    pub(crate) fn std_type_identity(&mut self, ty: QualType, loc: Loc) -> Option<QualType> {
        use crate::templ::actions::TemplateIdRef;
        use kiln_ast::types::TemplateArg;
        let tmpl = match self.lookup_std_type_identity() {
            StdTypeIdentity::Absent => return None,
            StdTypeIdentity::Malformed(d) => {
                let at = self.ast.decls.get(d).loc();
                let msg = self
                    .diag(DiagId::sema_malformed_std_class_template, at)
                    .arg("type_identity")
                    .allow_duplicate();
                self.emit(msg);
                return None;
            }
            StdTypeIdentity::Template(t) => t,
        };
        let name = self.template_name_for(tmpl);
        let ty = self.ast.types.unqualified(ty);
        let Ok(TemplateIdRef::Type(t)) =
            self.act_template_id(name, &[TemplateArg::ty(ty)], loc, loc)
        else {
            return None;
        };
        Some(t)
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
