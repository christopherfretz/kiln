//! Inheriting constructors ([class.inhctor.init], [namespace.udecl]/13;
//! spec/sema-class.md §4.10, WP-114): the constructor of the derived
//! class that a selected `ConstructorUsingShadow` stands for (clang
//! `findInheritingConstructor`), its definition on first odr-use (clang
//! `DefineInheritingConstructor`) and its exception specification (clang
//! `ComputeInheritingCtorExceptionSpec`).

use kiln_ast::decl::{CtorInit, CtorInitKind, DeclFlags, DeclKind, FnSpec};
use kiln_ast::expr::ConstructionKind;
use kiln_ast::names::DeclName;
use kiln_ast::types::ExcSpec;
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_source::Loc;

use crate::Sema;

/// A base subobject an inheriting constructor initializes by an
/// inherited constructor: the base class (first declaration), the
/// constructor called and whether it is a virtual base.
#[derive(Clone, Copy, Debug)]
pub(crate) struct InheritedBase {
    pub base: DeclId,
    pub ctor: DeclId,
    pub is_virtual: bool,
    /// The constructor is itself inherited through a virtual base
    /// ([class.inhctor.init]/2; clang `InheritedFromVBase`).
    pub from_vbase: bool,
}

impl Sema<'_, '_> {
    /// The inheriting constructor of the class of `shadow` for base
    /// constructor `bc` (the shadow's target or a specialization of
    /// it), declared on first use and cached (clang
    /// `findInheritingConstructor`): implicit, inline, named and
    /// accessed as `bc` ([namespace.udecl]/19), with `bc`'s parameter
    /// types and *explicit-specifier*, `constexpr` as `bc` when a
    /// defaulted default constructor could be, deleted when `bc` is.
    pub(crate) fn find_inheriting_ctor(&mut self, shadow: DeclId, bc: DeclId) -> DeclId {
        let Some(d) = self.ast.decls.get(shadow).ctx() else { return bc };
        let d = self.class_def(d);
        if let Some(&dc) = self.inh_ctors.get(&(d, bc)) {
            return dc;
        }
        let bty = self.ast.decls.get(bc).ty().unwrap_or(QualType::ERROR);
        let info =
            self.ast.types.proto(bty).map(kiln_ast::types::FuncProto::info).unwrap_or_default();
        let mut params = Vec::new();
        self.fn_param_types(bc, &mut params);
        let fty = self.ast.types.function(QualType::VOID, &params, info, ExcSpec::None);
        let bspec = self.fn_spec(bc);
        let mut spec = FnSpec::new().with_inline(true).with_explicit(bspec.is_explicit());
        // [dcl.constexpr]/3 before C++26 (P3533R2): no virtual bases.
        if !self.vbases_forbid_constexpr(self.defn_bits(d)) {
            spec = spec.with_constexpr_kind(bspec.constexpr_kind());
        }
        let loc = self.ast.decls.get(shadow).loc();
        let name = self.ast.decls.get(bc).name();
        let access = self.ast.decls.get(bc).misc().access();
        let h = DeclHead::new(d, name, loc).flags(DeclFlags::IMPLICIT).access(access);
        let dc = self.builder().append_implicit_member(h, DeclKind::CXXConstructor, fty, spec);
        let ps: Vec<DeclId> = params
            .iter()
            .enumerate()
            .map(|(i, &t)| {
                let i = u16::try_from(i).unwrap_or(u16::MAX);
                let h = DeclHead::new(dc, DeclName::EMPTY, loc).flags(DeclFlags::IMPLICIT);
                self.builder().param_head(h, t, i, 0)
            })
            .collect();
        if !ps.is_empty() {
            let _ = self.builder().set_params(dc, &ps);
        }
        let t = self.with_exc_spec(fty, ExcSpec::Deferred(dc));
        self.ast.decls.set_ty(dc, t);
        if bspec.is_deleted() {
            self.ast.decls.mark_function(dc, FnSpec::new().with_deleted(true));
        }
        let _ = self.ast.decls.set_inheriting_ctor(dc, shadow);
        let _ = self.ast.decls.set_inherited_ctor(dc, bc);
        self.inh_ctors.insert((d, bc), dc);
        self.inh_ctor_base.insert(dc, bc);
        if !bspec.is_deleted() && self.inheriting_ctor_should_delete(dc) {
            self.ast.decls.mark_function(dc, FnSpec::new().with_deleted(true));
        }
        dc
    }

    /// The base constructor inheriting constructor `dc` stands for.
    pub(crate) fn inherited_ctor_base(&self, dc: DeclId) -> Option<DeclId> {
        self.inh_ctor_base.get(&self.ast.first(dc)).copied()
    }

    /// The bases inheriting constructor `dc` initializes by inherited
    /// constructor (clang `InheritedConstructorInfo`): the nominated base
    /// (with its own inheriting constructor when it inherited `bc`
    /// itself) and, when the shadow constructs a virtual base, that base
    /// (from its constructor directly).
    pub(crate) fn inherited_bases(&mut self, dc: DeclId) -> Vec<InheritedBase> {
        let Some(bc) = self.inherited_ctor_base(dc) else { return Vec::new() };
        let Some(shadow) = self.ast.decls.inheriting_ctor_shadow(dc) else { return Vec::new() };
        let Some(class) = self.ast.decls.get(dc).ctx() else { return Vec::new() };
        let ns = self.ast.decls.ctor_shadow_nominated(shadow);
        let nominated = self.ast.shadow_base_class(ns, shadow);
        let (cs, virt) = self.ast.shadow_constructed(shadow);
        let constructed = self.ast.shadow_base_class(cs, shadow);
        let mut entries: Vec<(DeclId, Option<DeclId>)> = Vec::new();
        if let Some(n) = nominated {
            entries.push((n, ns));
        }
        if virt
            && let Some(c) = constructed
            && Some(c) != nominated
        {
            entries.push((c, cs));
        }
        let mut out = Vec::new();
        let mut specs: Vec<(DeclId, bool)> = self
            .indexed_bases(class)
            .into_iter()
            .filter(|(_, b)| !b.is_virtual)
            .filter_map(|(_, b)| self.base_class(&b).map(|c| (self.ast.first(c), false)))
            .collect();
        specs.extend(
            self.virtual_bases(class).iter().map(|v| (self.ast.first(v.0), true)),
        );
        for (b, is_virtual) in specs {
            let Some(&(_, via)) = entries.iter().find(|e| e.0 == b) else { continue };
            let (ctor, from_vbase) = match via {
                Some(s) => (self.find_inheriting_ctor(s, bc), self.ast.shadow_constructed(s).1),
                None => (bc, false),
            };
            out.push(InheritedBase { base: b, ctor, is_virtual, from_vbase });
        }
        out
    }

    /// Defines inheriting constructor `dc` on its first odr-use at `loc`
    /// ([class.inhctor.init]/1): the inherited bases from their
    /// constructors with `dc`'s parameters forwarded
    /// (`CXXInheritedCtorInitExpr`), everything else as a defaulted
    /// default constructor would.
    pub(crate) fn define_inheriting_ctor(&mut self, dc: DeclId, loc: Loc) {
        let n = *self.ast.decls.get(dc);
        if self.ast.definition(dc).is_some()
            || n.flags().contains(DeclFlags::INVALID)
            || self.is_deleted_fn(dc)
            || self.synth.iter().any(|c| c.member == dc)
        {
            return;
        }
        let Some(class) = n.ctx() else { return };
        let class = self.class_def(class);
        let saved = self.push_synth(dc, super::special::SynthKind::Inheriting, loc);
        let at = n.loc();
        let mut written = Vec::new();
        for ib in self.inherited_bases(dc) {
            // clang marks it at the use (`CurrentLocation`), not the
            // `InitLoc` the initializer is built at.
            self.mark_function_referenced(ib.ctor, loc);
            let bty = self.class_type(ib.base);
            let kind = if ib.is_virtual {
                ConstructionKind::VirtualBase
            } else {
                ConstructionKind::NonVirtualBase
            };
            let init = self.builder().inherited_ctor_init(bty, ib.ctor, kind, ib.from_vbase, at);
            written.push(CtorInit {
                target: bty.ty().index(),
                kind: if ib.is_virtual { CtorInitKind::VirtualBase } else { CtorInitKind::Base },
                written_index: CtorInit::IMPLICIT,
                init,
                loc: at,
            });
        }
        let failed = self.complete_ctor_inits(
            dc,
            class,
            &written,
            super::special::ImplicitInit::Default,
            false,
        );
        if failed {
            self.ast.decls.set_flags(dc, DeclFlags::INVALID);
        } else {
            let body = self.builder().compound(&[], at, at);
            let _ = self.builder().set_body(dc, body);
        }
        self.pop_synth(saved);
    }

    /// Whether class `r` inherits a default constructor (clang
    /// `HasInheritedDefaultConstructor`): it then has an implicit default
    /// constructor even with user-declared constructors, which hides the
    /// inherited one.
    pub(crate) fn inherits_default_ctor(&self, r: DeclId) -> bool {
        self.ast.decls.members(r).any(|m| {
            self.kind_of(m) == DeclKind::ConstructorUsingShadow
                && self.ast.using_target(m).is_some_and(|t| {
                    self.kind_of(t) == DeclKind::CXXConstructor
                        && self.special_kind_of(t) == Some(super::SpecialKind::DefaultCtor)
                })
        })
    }
}
