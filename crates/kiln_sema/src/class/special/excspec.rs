//! The implicit exception specifications of special members
//! ([except.spec]/7-8, spec/sema-class.md §4.5): an implicitly declared
//! or defaulted-on-first-declaration special member, and a destructor
//! declared without a *noexcept-specifier*, carry `ExcSpec::Deferred`
//! until first used; clang's `ResolveExceptionSpec` /
//! `ComputeDefaultedSpecialMemberExceptionSpec` then compute it from the
//! subobject members it calls.
//!
//! The result is printed the way clang 19 builds it: `noexcept` when no
//! call may throw, `noexcept(false)` otherwise.

use kiln_ast::decl::{BaseSpec, DeclFlags, DefinitionBits};
use kiln_ast::types::{ExcSpec, NoexceptValue, Quals};
use kiln_ast::{DeclId, QualType};
use kiln_source::Loc;

use super::{Selected, SpecialKind};
use crate::Sema;

/// The recursion bound of nested subobject resolution.
const MAX_DEPTH: usize = 256;

impl Sema<'_, '_> {
    /// Resolves the deferred exception specification of special member
    /// `f` (and its redeclarations); a no-op for any other function.
    /// A specification still to be instantiated from a class template
    /// member's pattern is instantiated, with `at` as its point of
    /// instantiation.
    pub(crate) fn resolve_exception_spec(&mut self, f: DeclId, at: Loc) {
        let _ = self.resolve_exc(f, 0, at);
    }

    /// Whether `f` is non-throwing, resolving it first if deferred.
    fn resolve_exc(&mut self, f: DeclId, depth: usize, at: Loc) -> bool {
        let first = self.ast.first(f);
        if self.uninstantiated_exc.contains_key(&first) {
            self.instantiate_exception_spec(first, at);
        }
        let exc = self.exc_of(first);
        if !matches!(exc, ExcSpec::Deferred(_)) {
            return exc.is_nothrow();
        }
        if depth > MAX_DEPTH || self.unparsed_exc.contains(&first) {
            return false;
        }
        let Some(k) = self.special_kind_of(first) else { return false };
        let Some(ctx) = self.ast.decls.get(first).ctx() else { return false };
        let class = self.class_def(ctx);
        let cflags = self.ast.decls.get(class).flags();
        if cflags.contains(DeclFlags::INVALID) || cflags.contains(DeclFlags::DEPENDENT_CTX) {
            return false;
        }
        let nothrow = self.implicit_nothrow(first, class, k, depth, at);
        let spec = if nothrow {
            ExcSpec::BasicNoexcept
        } else {
            ExcSpec::NoexceptExpr { expr: self.noexcept_false(), value: NoexceptValue::False }
        };
        let redecls: Vec<DeclId> = self.ast.redecls_of(first).collect();
        for d in redecls {
            let Some(ty) = self.ast.decls.get(d).ty() else { continue };
            let t = self.with_exc_spec(ty, spec);
            self.ast.decls.set_ty(d, t);
        }
        nothrow
    }

    fn exc_of(&self, f: DeclId) -> ExcSpec {
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        self.ast.types.proto(ty).map_or(ExcSpec::None, |p| self.ast.types.exc_spec(p))
    }

    /// The shared `false` of `noexcept(false)`.
    fn noexcept_false(&mut self) -> kiln_ast::ExprId {
        if let Some(e) = self.special.noexcept_false {
            return e;
        }
        let e = self.builder().bool_literal(QualType::BOOL, false, Loc::INVALID);
        self.special.noexcept_false = Some(e);
        e
    }

    /// [except.spec]/7-8 for member `f` of kind `k` of class `class`:
    /// clang's `SpecialMemberExceptionSpecInfo` visits the potentially
    /// constructed bases for a constructor (all bases otherwise) and every
    /// field; a default constructor looks at default member initializers
    /// instead of the members' default constructors. Destructors of
    /// subobjects do not affect a constructor (clang 19).
    fn implicit_nothrow(
        &mut self,
        f: DeclId,
        class: DeclId,
        k: SpecialKind,
        depth: usize,
        at: Loc,
    ) -> bool {
        use SpecialKind as K;
        let const_arg = matches!(k, K::CopyCtor | K::CopyAssign) && self.copy_param_is_const_ref(f);
        let bases: Vec<BaseSpec> = self.ast.decls.bases(class, &self.ast.lists).copied().collect();
        let mut subs: Vec<BaseSpec> = bases.iter().filter(|b| !b.is_virtual).copied().collect();
        if !k.is_ctor() || !self.defn_bits(class).contains(DefinitionBits::ABSTRACT) {
            subs.extend(self.virtual_bases(class).into_iter().map(|x| x.1));
        }
        for b in subs {
            let Some(bc) = self.base_class(&b) else { continue };
            let rhs = if const_arg { Quals::CONST } else { Quals::NONE };
            if !self.subobject_nothrow(bc, k, rhs, Quals::NONE, depth, at) {
                return false;
            }
        }
        for fld in self.special_fields(class) {
            if self.ast.decls.get(fld).has(DeclFlags::INVALID) {
                continue;
            }
            if k == K::DefaultCtor && self.field_has_nsdmi(fld) {
                let init = self.ast.decls.field(fld).and_then(|x| x.init);
                if init.is_some_and(|e| self.can_throw(e)) {
                    return false;
                }
                continue;
            }
            let fty = self.ast.decls.get(fld).ty().unwrap_or(QualType::ERROR);
            let elem = self.base_element(fty);
            if self.ast.types.is_reference(elem) {
                continue;
            }
            let Some(rec) = self.element_class(elem) else { continue };
            let quals = self.ast.types.effective_quals(elem) & Quals::CV;
            let rhs = match k {
                K::DefaultCtor | K::Dtor => Quals::NONE,
                _ if const_arg && !self.field_is_mutable(fld) => quals | Quals::CONST,
                _ => quals,
            };
            let lhs = if k.is_assign() { quals } else { Quals::NONE };
            if !self.subobject_nothrow(rec, k, rhs, lhs, depth, at) {
                return false;
            }
        }
        true
    }

    /// Whether the member of kind `k` selected for a subobject of class
    /// `rec` is non-throwing (true when none is selected: the enclosing
    /// member is then deleted).
    fn subobject_nothrow(
        &mut self,
        rec: DeclId,
        k: SpecialKind,
        rhs: Quals,
        lhs: Quals,
        depth: usize,
        at: Loc,
    ) -> bool {
        match self.select_special(rec, k, rhs, lhs) {
            Selected::Found(m) | Selected::Deleted(m) => self.resolve_exc(m, depth + 1, at),
            _ => true,
        }
    }
}
