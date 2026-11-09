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

use super::define::{SynthContext, SynthKind};
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

    /// Whether `f` is non-throwing when that is known without resolving
    /// or instantiating its exception specification (the Oven's P3068
    /// check, `kiln_oven::SemaBridge::is_nothrow`).
    pub(crate) fn known_nothrow(&self, f: DeclId) -> Option<bool> {
        let first = self.ast.first(f);
        if self.uninstantiated_exc.contains_key(&first) {
            return None;
        }
        let exc = self.exc_of(first);
        (!matches!(exc, ExcSpec::Deferred(_))).then(|| exc.is_nothrow())
    }

    /// Whether `f` is non-throwing, resolving its exception specification
    /// first (at `at`) if needed.
    pub(crate) fn resolve_nothrow(&mut self, f: DeclId, at: Loc) -> bool {
        self.resolve_exc(f, 0, at)
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
        if self.cmp_kind(first).is_some() {
            return self.resolve_cmp_exc(first);
        }
        // An inheriting constructor: its inherited constructors, then
        // as a default constructor (clang
        // `ComputeInheritingCtorExceptionSpec`).
        let inherited = if self.inherited_ctor_base(first).is_some() {
            self.inherited_bases(first)
        } else {
            Vec::new()
        };
        let k = if inherited.is_empty() {
            let Some(k) = self.special_kind_of(first) else { return false };
            k
        } else {
            SpecialKind::DefaultCtor
        };
        let Some(ctx) = self.ast.decls.get(first).ctx() else { return false };
        let class = self.class_def(ctx);
        let cflags = self.ast.decls.get(class).flags();
        if cflags.contains(DeclFlags::INVALID) || cflags.contains(DeclFlags::DEPENDENT_CTX) {
            return false;
        }
        // clang `EvaluateImplicitExceptionSpec`: under an
        // exception-specification context at the use (a nested one at the
        // subobject that calls it).
        let inst_depth = self.inst.stack.len();
        self.synth.push(SynthContext {
            member: first,
            kind: SynthKind::ExceptionSpec,
            loc: at,
            inst_depth,
        });
        let nothrow = self.implicit_nothrow(first, class, k, &inherited, depth);
        if self.synth_noted.get() == self.synth.len() {
            self.synth_noted.set(0);
        }
        self.synth.pop();
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
    pub(crate) fn noexcept_false(&mut self) -> kiln_ast::ExprId {
        if let Some(e) = self.special.noexcept_false {
            return e;
        }
        let e = self.builder().bool_literal(QualType::BOOL, false, Loc::INVALID);
        self.special.noexcept_false = Some(e);
        e
    }

    /// [except.spec]/7-8 for member `f` of kind `k` of class `class`:
    /// whether every function it calls on a subobject is non-throwing
    /// (see [`Self::visit_implicit_callees`]).
    fn implicit_nothrow(
        &mut self,
        f: DeclId,
        class: DeclId,
        k: SpecialKind,
        inherited: &[crate::class::inhctor::InheritedBase],
        depth: usize,
    ) -> bool {
        self.visit_implicit_callees(f, class, k, inherited, &mut |s, c| match c {
            Callee::Member(m, at) => s.resolve_exc(m, depth + 1, at),
            Callee::ThrowingInit => false,
        })
    }

    /// The exceptions the implicit (or defaulted-on-first-declaration)
    /// destructor or assignment operator `f` allows, as clang's
    /// `ImplicitExceptionSpecification` accumulates them before C++17:
    /// the union of the dynamic lists of the subobject members it calls,
    /// [`ImplicitThrows::Any`] once one of them may throw anything, and
    /// [`ImplicitThrows::Unknown`] when a callee's specification is
    /// dependent or not yet computed. Used by the overrider check
    /// ([except.spec]/5 in C++14), which kiln's `noexcept`/`noexcept(false)`
    /// resolution cannot answer for a base with a type list.
    pub(crate) fn implicit_thrown_types(&mut self, f: DeclId) -> ImplicitThrows {
        let first = self.ast.first(f);
        let Some(k) = self.special_kind_of(first) else { return ImplicitThrows::Unknown };
        if !matches!(k, SpecialKind::Dtor | SpecialKind::CopyAssign | SpecialKind::MoveAssign) {
            return ImplicitThrows::Unknown;
        }
        let Some(ctx) = self.ast.decls.get(first).ctx() else { return ImplicitThrows::Unknown };
        let class = self.class_def(ctx);
        let cflags = self.ast.decls.get(class).flags();
        if cflags.contains(DeclFlags::INVALID) || cflags.contains(DeclFlags::DEPENDENT_CTX) {
            return ImplicitThrows::Unknown;
        }
        let mut out = ImplicitThrows::List(Vec::new());
        self.visit_implicit_callees(first, class, k, &[], &mut |s, c| {
            let Callee::Member(m, at) = c else {
                out = ImplicitThrows::Any;
                return false;
            };
            s.resolve_exc(m, 1, at);
            match s.exc_of(s.ast.first(m)) {
                ExcSpec::Dynamic(l) => {
                    if let ImplicitThrows::List(v) = &mut out {
                        for &t in s.ast.types.type_list_types(l) {
                            if !v.iter().any(|&x| s.ast.types.same_type(x, t)) {
                                v.push(t);
                            }
                        }
                    }
                    true
                }
                ExcSpec::Deferred(_)
                | ExcSpec::DependentNoexcept(_)
                | ExcSpec::NoexceptExpr { value: NoexceptValue::Dependent(_), .. } => {
                    out = ImplicitThrows::Unknown;
                    false
                }
                e if e.is_nothrow() => true,
                _ => {
                    out = ImplicitThrows::Any;
                    false
                }
            }
        });
        out
    }

    /// The functions special member `f` of kind `k` of class `class`
    /// calls on its subobjects, in order, each passed to `visit` with
    /// the subobject's location until `visit` returns false (the result
    /// is then false): clang's `SpecialMemberExceptionSpecInfo` visits
    /// the potentially constructed bases for a constructor (all bases
    /// otherwise) and every field; a default constructor looks at
    /// default member initializers instead of the members' default
    /// constructors (a potentially-throwing one is
    /// [`Callee::ThrowingInit`]). Destructors of subobjects do not
    /// affect a constructor (clang 19). A subobject with no selected
    /// member is skipped (the enclosing member is then deleted).
    fn visit_implicit_callees(
        &mut self,
        f: DeclId,
        class: DeclId,
        k: SpecialKind,
        inherited: &[crate::class::inhctor::InheritedBase],
        visit: &mut dyn FnMut(&mut Self, Callee) -> bool,
    ) -> bool {
        use SpecialKind as K;
        if k == K::DefaultCtor {
            self.exc_spec_needs_unparsed_inits(f, class);
        }
        let const_arg = matches!(k, K::CopyCtor | K::CopyAssign) && self.copy_param_is_const_ref(f);
        let bases: Vec<BaseSpec> = self.ast.decls.bases(class, &self.ast.lists).copied().collect();
        let mut subs: Vec<BaseSpec> = bases.iter().filter(|b| !b.is_virtual).copied().collect();
        if !k.is_ctor() || !self.defn_bits(class).contains(DefinitionBits::ABSTRACT) {
            subs.extend(self.virtual_bases(class).into_iter().map(|x| x.1));
        }
        for b in subs {
            let Some(bc) = self.base_class(&b) else { continue };
            let first = self.ast.first(bc);
            // clang `getSubobjectLoc`.
            let at = b.loc;
            if let Some(ib) = inherited.iter().find(|ib| ib.base == first) {
                if !visit(self, Callee::Member(ib.ctor, at)) {
                    return false;
                }
                continue;
            }
            let rhs = if const_arg { Quals::CONST } else { Quals::NONE };
            if !self.visit_exc_subobject(bc, k, rhs, Quals::NONE, at, visit) {
                return false;
            }
        }
        for fld in self.special_fields(class) {
            if k == K::DefaultCtor && self.inst.deferred_nsdmis.contains_key(&fld) {
                // clang `BuildCXXDefaultInitExpr` instantiates it, from
                // the constructor, under the exception-specification
                // context.
                let poi = self.ast.decls.get(f).loc();
                self.instantiate_nsdmi(fld, poi);
            }
            let at = self.ast.decls.get(fld).loc();
            if self.ast.decls.get(fld).has(DeclFlags::INVALID) {
                continue;
            }
            if k == K::DefaultCtor && self.field_has_nsdmi(fld) {
                let init = self.ast.decls.field(fld).and_then(|x| x.init);
                if init.is_none() {
                    continue;
                }
                if init.is_some_and(|e| self.can_throw(e)) && !visit(self, Callee::ThrowingInit) {
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
            if !self.visit_exc_subobject(rec, k, rhs, lhs, at, visit) {
                return false;
            }
        }
        true
    }

    /// Default member initializers of `class` needed by the implicit
    /// exception specification of default constructor `f` before they
    /// are parsed ([class.mem.general]/8; clang's `BuildCXXDefaultInitExpr`
    /// from `SpecialMemberExceptionSpecInfo`): each diagnosed at `f`, under
    /// one exception-specification context note at the use. The
    /// fields become invalid, so the constructor's definition skips them
    /// quietly (no cascade from the definition or its constant
    /// evaluation). Reported before the bases are visited, under the
    /// context [`Sema::resolve_exception_spec`] pushed. A specialization's
    /// initializer awaiting lazy instantiation was parsed in its pattern.
    fn exc_spec_needs_unparsed_inits(&mut self, f: DeclId, class: DeclId) {
        let unparsed: Vec<DeclId> = self
            .special_fields_iter(class)
            .filter(|&fld| {
                !self.ast.decls.get(fld).has(DeclFlags::INVALID)
                    && self.field_has_nsdmi(fld)
                    && self.ast.decls.field(fld).and_then(|x| x.init).is_none()
                    && !self.inst.deferred_nsdmis.contains_key(&fld)
            })
            .collect();
        if unparsed.is_empty() {
            return;
        }
        let loc = self.ast.decls.get(f).loc();
        for fld in unparsed {
            self.default_member_init_not_yet_parsed(fld, loc);
        }
    }

    /// `visit` on the member of kind `k` selected for a subobject of
    /// class `rec` (true, not visited, when none is selected).
    fn visit_exc_subobject(
        &mut self,
        rec: DeclId,
        k: SpecialKind,
        rhs: Quals,
        lhs: Quals,
        at: Loc,
        visit: &mut dyn FnMut(&mut Self, Callee) -> bool,
    ) -> bool {
        match self.select_special(rec, k, rhs, lhs) {
            Selected::Found(m) | Selected::Deleted(m) => visit(self, Callee::Member(m, at)),
            _ => true,
        }
    }
}

/// A function an implicit special member calls on a subobject
/// ([`Sema::visit_implicit_callees`]).
#[derive(Clone, Copy)]
enum Callee {
    /// The selected special member of a base or member subobject (or an
    /// inherited constructor), with the subobject's location.
    Member(DeclId, Loc),
    /// A potentially-throwing default member initializer.
    ThrowingInit,
}

/// What an implicit special member may throw before C++17
/// ([`Sema::implicit_thrown_types`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum ImplicitThrows {
    /// A callee's specification is dependent or not computed.
    Unknown,
    /// Any exception.
    Any,
    /// The listed types (none: non-throwing).
    List(Vec<QualType>),
}
