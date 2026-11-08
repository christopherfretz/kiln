//! Virtual tables (spec/codegen.md §7.4, WP-080): `_ZTV` vtable groups
//! (clang `ItaniumCXXABI::getAddrOfVTable` / `emitVTableDefinitions` /
//! `CodeGenVTables::createVTableInitializer`), `_ZTI`/`_ZTS` RTTI
//! (`ItaniumRTTIBuilder`), vtable-pointer initialization in structors
//! (`InitializeVTablePointers`) and virtual-call slot loads
//! (`getVirtualFunctionPointer`).

use std::rc::Rc;

use kiln_abi::call::ArgKind;
use kiln_abi::mangle::{self, Structor};
use kiln_ast::class::{SubobjectId, Thunk, VTableComponent, VTableLayout};
use kiln_ast::decl::{Access, ConstexprKind, DeclFlags, DeclKind, DefinitionBits, TagKind};
use kiln_ast::modules::UnitKind;
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_llvm::{
    Attr, AttrKind, CastOp, ComdatKind, FuncId, GepFlags, GlobalId, IntPredicate, Linkage, MdKindId,
    MetadataId, TypeId as LTy, UnnamedAddr, Value, ValueRef, Visibility,
};
use kiln_source::fx::{FxHashMap, FxHashSet};

use crate::fir::{Fir, Op, V};
use crate::func::min_align;
use crate::module::{Cg, R, err};

/// How a class's vtable is emitted (clang `getVTableLinkage` with
/// `isVTableExternal`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum VtLinkage {
    /// Defined in another translation unit (key function not defined
    /// here): an `external` declaration.
    External,
    /// Strong definition (key function defined here).
    Strong,
    /// `linkonce_odr` in a comdat.
    LinkOnce,
    /// `weak_odr` in a comdat (the class is explicitly instantiated,
    /// [temp.explicit]/10).
    Weak,
    /// `internal` (the class has internal linkage).
    Internal,
    /// `available_externally` (an external vtable emitted at `-O1` and
    /// up for devirtualization: clang `shouldEmitAvailableExternallyVTable`).
    AvailableExternally,
}

/// The state of a [`Cg::vmi_flags`] walk.
#[derive(Default)]
struct VmiWalk {
    nv: FxHashSet<DeclId>,
    vb: FxHashSet<DeclId>,
    expanded: FxHashSet<DeclId>,
    edges: FxHashMap<DeclId, u32>,
}

/// One vtable pointer to initialize (clang `VPtr`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct VPtr {
    /// The base subobject's class.
    pub class: DeclId,
    /// Its offset in the `VTableClass` object.
    pub offset: u64,
    /// The nearest virtual base on the path to it.
    pub nearest_vbase: Option<DeclId>,
    /// Its offset from that virtual base.
    pub offset_from_vbase: u64,
}

impl Cg<'_> {
    // ---- layouts -------------------------------------------------------

    /// The vtable group layout of dynamic class `rd` (cached).
    pub(crate) fn vt_layout(&mut self, rd: DeclId) -> R<Rc<VTableLayout>> {
        match self.vts.vtable_layout(rd) {
            Ok(l) => Ok(l),
            Err(e) => err(
                Some(self.ast.decls.get(self.ast.first(rd)).loc()),
                format!("cannot lay out the vtable: {e:?}"),
            ),
        }
    }

    /// The class is dynamic (clang `isDynamicClass`).
    pub(crate) fn is_dynamic(&self, rd: DeclId) -> bool {
        self.class_def(rd)
            .and_then(|d| self.ast.decls.record(d))
            .is_some_and(|r| r.defn.is_dynamic())
    }

    /// The `SubobjectId` of the base subobject (`class`, `offset`) of
    /// `layout_class`.
    pub(crate) fn subobject_id(
        &mut self,
        layout_class: DeclId,
        class: DeclId,
        offset: u64,
    ) -> Option<SubobjectId> {
        let c = self.ast.first(class);
        self.vts.subobjects(layout_class).id(c, offset)
    }

    /// The (vtable index, index within that vtable) of the address point
    /// of subobject `sub` in group `l` (clang `getAddressPoint`).
    pub(crate) fn address_point_of(l: &VTableLayout, sub: SubobjectId) -> Option<(u32, u32)> {
        let i = l.address_points.binary_search_by_key(&sub, |&(s, _)| s).ok()?;
        let ap = l.address_points[i].1;
        // An address point follows at least the offset-to-top and RTTI
        // of its own vtable, and may equal the end of it (no virtual
        // functions): never the start of the next one.
        let k = l.vtable_starts.iter().rposition(|&st| st < ap)?;
        Some((u32::try_from(k).ok()?, ap - l.vtable_starts[k]))
    }

    /// The number of components of vtable `k` of group `l`.
    pub(crate) fn vtable_size(l: &VTableLayout, k: usize) -> u32 {
        let start = l.vtable_starts[k];
        let end = l
            .vtable_starts
            .get(k + 1)
            .copied()
            .unwrap_or_else(|| u32::try_from(l.components.len()).unwrap_or(u32::MAX));
        end - start
    }

    /// The IR type of a vtable group (`{ [N0 x ptr], [N1 x ptr], ... }`).
    pub(crate) fn vtable_ir_ty(&mut self, l: &VTableLayout) -> LTy {
        let tys: Vec<LTy> = (0..l.vtable_starts.len())
            .map(|k| {
                let n = Self::vtable_size(l, k);
                self.m.types.array(LTy::PTR, u64::from(n))
            })
            .collect();
        self.m.types.struct_literal(&tys, false)
    }

    // ---- linkage -------------------------------------------------------

    /// The key function of `rd` and its definition in this translation
    /// unit, if any. Template patterns and instantiations have none
    /// (Itanium C++ ABI 5.2.6; clang `computeKeyFunction`).
    fn key_function(&self, rd: DeclId) -> Option<(DeclId, Option<DeclId>)> {
        let ast = self.ast;
        let def = self.class_def(rd)?;
        if self.class_is_templated_or_instantiated(def) {
            return None;
        }
        let &k = ast.decls.cold.key_function.get(def)?;
        let body =
            ast.redecls_of(k).find(|&r| ast.decls.function(r).is_some_and(|f| f.body.is_some()));
        Some((k, body))
    }

    /// Whether class `def` is a template pattern, is dependent, or is a
    /// (member) class instantiated from a template (any specialization
    /// kind but an explicit specialization).
    fn class_is_templated_or_instantiated(&self, def: DeclId) -> bool {
        let ast = self.ast;
        let n = ast.decls.get(def);
        if n.has(DeclFlags::DEPENDENT_CTX) || ast.described_template(def).is_some() {
            return true;
        }
        let first = ast.first(def);
        ast.spec_info(first).is_some_and(|(_, s)| s.kind != SpecKind::ExplicitSpecialization)
            || ast
                .member_spec_info(first)
                .is_some_and(|(_, m)| m.kind != SpecKind::ExplicitSpecialization)
    }

    /// [`Self::class_explicit_inst_kind`] for the vtable and RTTI: a member
    /// class already defined when its enclosing class was explicitly
    /// instantiated by a definition keeps its implicit kind (Sema marks
    /// the member classes that instantiation defined; clang
    /// `InstantiateClassMembers`), so its vtable is emitted only if used.
    pub(crate) fn vtable_explicit_inst_kind(&self, first: DeclId) -> Option<SpecKind> {
        let ast = self.ast;
        match self.class_explicit_inst_kind(first) {
            Some(SpecKind::ExplicitInstantiationDefinition) => {
                let own = ast
                    .spec_info(first)
                    .map(|(_, s)| s.kind)
                    .or_else(|| ast.member_spec_info(first).map(|(_, m)| m.kind));
                (own == Some(SpecKind::ExplicitInstantiationDefinition))
                    .then_some(SpecKind::ExplicitInstantiationDefinition)
            }
            k => k,
        }
    }

    /// The explicit instantiation kind of class `first`, itself or as a
    /// member class of an explicitly instantiated class (the innermost
    /// non-implicit kind decides; `None` when implicit or explicitly
    /// specialized).
    pub(crate) fn class_explicit_inst_kind(&self, first: DeclId) -> Option<SpecKind> {
        let ast = self.ast;
        let mut c = Some(first);
        while let Some(d) = c {
            if !matches!(
                ast.decls.get(d).kind(),
                DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization
            ) {
                return None;
            }
            let kind = ast
                .spec_info(d)
                .map(|(_, s)| s.kind)
                .or_else(|| ast.member_spec_info(d).map(|(_, m)| m.kind));
            match kind {
                Some(
                    k @ (SpecKind::ExplicitInstantiationDeclaration
                    | SpecKind::ExplicitInstantiationDefinition),
                ) => return Some(k),
                Some(SpecKind::ImplicitInstantiation | SpecKind::Undeclared) => {}
                _ => return None,
            }
            c = ast.decls.get(d).ctx();
        }
        None
    }

    /// clang `getVTableLinkage` / `isVTableExternal` (at `-O0`).
    pub(crate) fn vtable_linkage(&self, rd: DeclId) -> VtLinkage {
        let first = self.ast.first(rd);
        if mangle::class_is_internal(self.ast, first) {
            return VtLinkage::Internal;
        }
        if self.is_implicit_instantiation(first) {
            return VtLinkage::LinkOnce;
        }
        // [temp.explicit]/14: `extern template` — the vtable is defined
        // by the explicit instantiation definition elsewhere.
        match self.vtable_explicit_inst_kind(first) {
            Some(SpecKind::ExplicitInstantiationDeclaration) => return VtLinkage::External,
            // Template instantiations have no key function (Itanium
            // 5.2.6); an explicit instantiation definition is `weak_odr`.
            Some(_) => return VtLinkage::Weak,
            None => {}
        }
        // clang `isVTableExternal`: the tables of a class attached to a
        // named module are emitted, strongly, only by the unit that
        // defines it (spec/modules.md §7).
        if let Some(def) = self.class_def(first)
            && self.ast.module_attachment(def).is_some()
            && !self.class_is_templated_or_instantiated(def)
        {
            return if self.ast.modules.owner(def).unit == 0 {
                VtLinkage::Strong
            } else {
                VtLinkage::External
            };
        }
        match self.key_function(rd) {
            None => VtLinkage::LinkOnce,
            Some((_, None)) => VtLinkage::External,
            // A key function defined in an imported unit's global module
            // fragment: that unit's object defines the tables (a
            // deviation from clang 22, which emits them in every importer
            // and not in the module unit; spec/modules.md §7).
            Some((_, Some(def))) if self.ast.modules.is_imported_named_unit_decl(def) => {
                VtLinkage::External
            }
            Some((_, Some(def))) => {
                if self.fn_is_inline(def) {
                    VtLinkage::LinkOnce
                } else {
                    VtLinkage::Strong
                }
            }
        }
    }

    // ---- vtable globals ------------------------------------------------

    /// clang `ItaniumCXXABI::getAddrOfVTable`: the `_ZTV` global of `rd`,
    /// declared on first use and queued for deferred emission.
    pub(crate) fn vtable_global(&mut self, rd: DeclId) -> R<GlobalId> {
        let first = self.ast.first(rd);
        if let Some(&g) = self.vtables.get(&first) {
            return Ok(g);
        }
        self.deferred_vtables.push(first);
        let l = self.vt_layout(first)?;
        let ty = self.vtable_ir_ty(&l);
        let name = mangle::mangle_class_special(self.ast, &mut self.anon, "_ZTV", first);
        let vis = self.class_visibility(first, false);
        let g = self.m.add_global(&name, ty);
        let gv = self.m.global_mut(g);
        gv.is_constant = true;
        gv.linkage = Linkage::External;
        gv.unnamed_addr = UnnamedAddr::Global;
        gv.align = Some(8);
        gv.visibility = vis;
        self.vtables.insert(first, g);
        Ok(g)
    }

    /// clang `EmitDeferredVTables`: an external vtable is emitted
    /// `available_externally` at `-O1` and up when it can be
    /// speculatively (`shouldEmitVTableAtEndOfTranslationUnit`), else
    /// queued for [`Self::emit_vtables_opportunistically`].
    pub(crate) fn emit_deferred_vtables(&mut self) -> R<()> {
        let cur = std::mem::take(&mut self.deferred_vtables);
        for rd in cur {
            if self.vtable_linkage(rd) != VtLinkage::External {
                self.emit_vtable_def(rd)?;
            } else if self.opts.optimize() {
                if self.can_speculatively_emit_vtable(rd)? {
                    self.emit_vtable_def_as(rd, VtLinkage::AvailableExternally)?;
                } else {
                    self.opportunistic_vtables.push(rd);
                }
            }
        }
        Ok(())
    }

    /// clang `EmitVTablesOpportunistically`, after every deferred
    /// definition: an inline virtual function emitted after the first
    /// attempt may now allow the `available_externally` copy.
    pub(crate) fn emit_vtables_opportunistically(&mut self) -> R<()> {
        for rd in std::mem::take(&mut self.opportunistic_vtables) {
            if self.can_speculatively_emit_vtable(rd)? {
                self.emit_vtable_def_as(rd, VtLinkage::AvailableExternally)?;
            }
        }
        Ok(())
    }

    /// clang `ItaniumCXXABI::canSpeculativelyEmitVTable`: the class
    /// itself and every dynamic virtual base can be emitted as a base.
    fn can_speculatively_emit_vtable(&mut self, rd: DeclId) -> R<bool> {
        if !self.can_speculatively_emit_vtable_as_base(rd)? {
            return Ok(false);
        }
        let Some(def) = self.class_def(rd) else { return Ok(false) };
        let vbases: Vec<DeclId> = {
            let l = self.tys.layouter().record(def);
            l.vbases.iter().map(|&(v, _)| v).collect()
        };
        for v in vbases {
            if self.is_dynamic(v) && !self.can_speculatively_emit_vtable_as_base(v)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// clang `canSpeculativelyEmitVTableAsBaseClass`: no inline virtual
    /// function of the vtable is still unemitted, and (for a class with
    /// virtual bases, whose VTT must be emittable too) the same holds
    /// for each dynamic non-virtual base; and the vtable is not hidden
    /// (clang `isVTableHidden`).
    fn can_speculatively_emit_vtable_as_base(&mut self, rd: DeclId) -> R<bool> {
        let ast = self.ast;
        let first = ast.first(rd);
        if self.is_vtable_hidden(first)? {
            return Ok(false);
        }
        if self.has_unused_virtual_inline_function(first)? {
            return Ok(false);
        }
        if !self.class_bits_of(first).contains(DefinitionBits::HAS_VIRTUAL_BASES) {
            return Ok(true);
        }
        let Some(def) = self.class_def(first) else { return Ok(false) };
        let bases: Vec<DeclId> = ast
            .decls
            .bases(def, &ast.lists)
            .filter(|b| !b.is_virtual)
            .filter_map(|b| ast.types.as_record(b.ty))
            .collect();
        for b in bases {
            if self.is_dynamic(b) && !self.can_speculatively_emit_vtable_as_base(b)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// clang `isVTableHidden`: `rd`'s `type_info` (its type visibility)
    /// or a function in a used slot that is not defined in this
    /// translation unit is hidden, so an `available_externally` copy
    /// could reference a symbol another DSO does not export.
    fn is_vtable_hidden(&mut self, rd: DeclId) -> R<bool> {
        if !self.vis_active {
            return Ok(false);
        }
        if self.class_visibility(rd, true) == Visibility::Hidden {
            return Ok(true);
        }
        let ast = self.ast;
        let l = self.vt_layout(rd)?;
        for c in &l.components {
            let (VTableComponent::Func(m)
            | VTableComponent::CompleteDtor(m)
            | VTableComponent::DeletingDtor(m)) = *c
            else {
                continue;
            };
            let defined = ast.redecls_of(m).any(|r| {
                ast.decls.function(r).is_some_and(|f| f.body.is_some() || f.spec.is_deleted())
            });
            if !defined && self.decl_visibility(m, true) == Visibility::Hidden {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// clang `hasAnyUnusedVirtualInlineFunction`: an inline function
    /// (deleted ones included, [dcl.fct.def.delete]/4) in a used slot
    /// of `rd`'s vtable whose symbol is not (yet) a definition here.
    fn has_unused_virtual_inline_function(&mut self, rd: DeclId) -> R<bool> {
        let ast = self.ast;
        let l = self.vt_layout(rd)?;
        for c in &l.components {
            let (m, v) = match *c {
                VTableComponent::Func(m) => (m, None),
                VTableComponent::CompleteDtor(m) => (m, Some(Structor::Complete)),
                VTableComponent::DeletingDtor(m) => (m, Some(Structor::Deleting)),
                _ => continue,
            };
            let inline = ast.redecls_of(m).any(|r| {
                ast.decls.function(r).is_some_and(|f| {
                    f.spec.is_inline()
                        || f.spec.implicit_inline()
                        || f.spec.constexpr_kind() != ConstexprKind::None
                        || f.spec.is_deleted()
                })
            });
            if !inline {
                continue;
            }
            let first = ast.first(m);
            let name = match v {
                None => mangle::mangle_decl(ast, &mut self.anon, first),
                Some(v) => mangle::mangle_structor(ast, &mut self.anon, first, v),
            };
            let defined = match self.m.lookup(&name).map(Value::get) {
                Some(ValueRef::Func(f)) => !self.m.func(f).is_declaration(),
                Some(ValueRef::Alias(_)) => true,
                _ => false,
            };
            if !defined {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Sema's `DefineUsedVTables` → `HandleVTable` for classes whose key
    /// function is defined in this translation unit, in the order the
    /// key functions are defined.
    pub(crate) fn emit_key_function_vtables(&mut self) -> R<()> {
        let ast = self.ast;
        let mut classes: Vec<(DeclId, DeclId)> = ast
            .decls
            .cold
            .key_function
            .iter()
            .filter_map(|(c, _)| {
                if self.class_def(c).is_some_and(|d| {
                    ast.module_attachment(d).is_some()
                        && !self.class_is_templated_or_instantiated(d)
                }) {
                    return None;
                }
                let (_, def) = self.key_function(c)?;
                let def = def?;
                Some((def, c))
            })
            .collect();
        // clang marks the vtable of every dynamic class defined in a named
        // module unit's purview used when the class is completed.
        classes.extend(self.module_dynamic_classes().into_iter().map(|c| (c, c)));
        classes.sort_unstable();
        for (_, c) in classes {
            if ast.decls.get(c).has(DeclFlags::INVALID) {
                continue;
            }
            if self.vtable_linkage(c) == VtLinkage::Strong {
                self.emit_vtable_def(c)?;
            }
        }
        for c in std::mem::take(&mut self.explicit_vtables) {
            self.emit_vtable_def(c)?;
        }
        Ok(())
    }

    /// The dynamic, non-templated classes (nested ones included) that
    /// this interface or partition unit defines in its purview (their
    /// vtables are used at their definition: `mark_module_vtable_used`).
    fn module_dynamic_classes(&self) -> Vec<DeclId> {
        let ast = self.ast;
        let mut out = Vec::new();
        let interface = ast.modules.units.first().is_some_and(|u| {
            matches!(
                u.kind,
                UnitKind::Interface
                    | UnitKind::PartitionInterface
                    | UnitKind::PartitionImplementation
            )
        });
        if !ast.modules.active || !interface {
            return out;
        }
        let mut stack = vec![ast.tu];
        while let Some(ctx) = stack.pop() {
            for d in ast.decls.members(ctx) {
                let n = ast.decls.get(d);
                if n.has(DeclFlags::INVALID) || n.has(DeclFlags::DEPENDENT_CTX) {
                    continue;
                }
                match n.kind() {
                    DeclKind::Namespace | DeclKind::Export => stack.push(d),
                    DeclKind::CXXRecord if ast.definition(d) == Some(d) => {
                        if self.class_is_templated_or_instantiated(d) {
                            continue;
                        }
                        stack.push(d);
                        if ast.modules.owner(d).unit == 0
                            && ast.module_attachment(d).is_some()
                            && self.is_dynamic(d)
                            && self.vtable_linkage(d) == VtLinkage::Strong
                        {
                            out.push(d);
                        }
                    }
                    _ => {}
                }
            }
        }
        out
    }

    /// clang `ItaniumCXXABI::emitVTableDefinitions` (idempotent).
    pub(crate) fn emit_vtable_def(&mut self, rd: DeclId) -> R<()> {
        let linkage = self.vtable_linkage(rd);
        self.emit_vtable_def_as(rd, linkage)
    }

    /// [`Self::emit_vtable_def`] with linkage `linkage` (clang
    /// `GenerateClassData`).
    pub(crate) fn emit_vtable_def_as(&mut self, rd: DeclId, linkage: VtLinkage) -> R<()> {
        let first = self.ast.first(rd);
        let g = self.vtable_global(first)?;
        if self.m.global(g).initializer().is_some() || !self.vt_emitted.insert(first) {
            return Ok(());
        }
        if self.class_bits_of(first).contains(DefinitionBits::HAS_VIRTUAL_BASES) {
            // clang `GenerateClassData`: the VTT (and its construction
            // vtables) before the vtable's own initializer.
            self.emit_vtt(first, linkage)?;
        }
        let l = self.vt_layout(first)?;
        let rd_ty = self.class_type(first);
        let rtti = self.rtti_descriptor(rd_ty)?;
        let init = self.vtable_init(&l, rtti, first)?;
        self.define_vtable_global(g, init, linkage, first)?;
        // clang `emitVTableDefinitions`: not on an `available_externally`
        // vtable (without whole-program vtables).
        if linkage != VtLinkage::AvailableExternally {
            self.vtable_type_metadata(g, &l, first);
        }
        Ok(())
    }

    /// clang `CodeGenModule::EmitVTableTypeMetadata` under LTO (WP-873,
    /// spec/codegen.md §5.3 "LTO"): for each address point of group `l` (laid
    /// out in `layout_class`), sorted by the base's `_ZTS` name and then
    /// offset, `!type !{i64 <byte offset>, <id of the base>}`, then one
    /// for every virtual function slot of the group (destructors and
    /// unused slots aside) with the id of the member function pointer
    /// type `<fn type> <base>::*` suffixed `.virtual`. An id is the
    /// mangled `_ZTS` name of an externally visible type, else a
    /// `distinct !{}` per type.
    fn vtable_type_metadata(&mut self, g: GlobalId, l: &VTableLayout, layout_class: DeclId) {
        if self.opts.lto.is_none() {
            return;
        }
        let ast = self.ast;
        let table = self.vts.subobjects(layout_class);
        let mut aps: Vec<(String, u32, DeclId)> = l
            .address_points
            .iter()
            .filter_map(|&(sub, idx)| {
                let &(so, _) = table.list.get(usize::from(sub.0))?;
                let name = mangle::mangle_class_special(ast, &mut self.anon, "_ZTS", so.class);
                Some((name, idx, so.class))
            })
            .collect();
        aps.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        aps.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
        let kind = self.m.md_kind("type");
        for (name, idx, base) in aps {
            let visible = !mangle::class_is_internal(ast, base);
            let id = self.type_md_id(name, visible);
            self.push_type_md(g, kind, idx, id);
            let class_ty = self.class_type(base).ty();
            for (i, c) in l.components.iter().enumerate() {
                let VTableComponent::Func(f) = *c else { continue };
                let Some(fty) = ast.decls.get(f).ty() else { continue };
                let mut name =
                    mangle::mangle_member_pointer_rtti(ast, &mut self.anon, fty, class_ty, "_ZTS");
                name.push_str(".virtual");
                let fn_visible = visible && mangle::type_externally_visible(ast, fty);
                let id = self.type_md_id(name, fn_visible);
                self.push_type_md(g, kind, i as u32, id);
            }
        }
    }

    /// A `!type` identifier ([`Self::vtable_type_metadata`]).
    fn type_md_id(&mut self, name: String, visible: bool) -> MetadataId {
        if visible {
            return self.m.md_string(name.as_bytes());
        }
        if let Some(&id) = self.type_md_ids.get(&name) {
            return id;
        }
        let id = self.m.md_distinct_tuple(&[]);
        self.type_md_ids.insert(name, id);
        id
    }

    /// `!type !{i64 <8 * slot>, id}` on vtable `g`.
    fn push_type_md(&mut self, g: GlobalId, kind: MdKindId, slot: u32, id: MetadataId) {
        let off = self.m.const_int(LTy::I64, 8 * i128::from(slot));
        let Ok(off) = self.m.md_const(off) else { return };
        let node = self.m.md_tuple(&[Some(off), Some(id)]);
        self.m.push_global_metadata(g, kind, node);
    }

    /// clang `GetAddrOfVTT`: the `_ZTT` global of `rd` (declared).
    pub(crate) fn vtt_global(&mut self, rd: DeclId) -> R<GlobalId> {
        let first = self.ast.first(rd);
        let name = mangle::mangle_class_special(self.ast, &mut self.anon, "_ZTT", first);
        if let Some(g) = self.m.get_global(&name) {
            return Ok(g);
        }
        // "This will also defer the definition of the VTT."
        self.vtable_global(first)?;
        let n = match self.vts.vtt(first) {
            Ok(v) => v.components.len(),
            Err(e) => {
                return err(
                    Some(self.ast.decls.get(first).loc()),
                    format!("cannot lay out the VTT: {e:?}"),
                );
            }
        };
        let ty = self.m.types.array(LTy::PTR, n as u64);
        let vis = self.class_visibility(first, false);
        let g = self.m.add_global(&name, ty);
        let gv = self.m.global_mut(g);
        gv.is_constant = true;
        gv.visibility = vis;
        gv.linkage = Linkage::External;
        gv.unnamed_addr = UnnamedAddr::Global;
        gv.align = Some(8);
        Ok(g)
    }

    /// clang `EmitVTTDefinition`: the construction vtables the VTT of
    /// `rd` points into, then the VTT itself.
    fn emit_vtt(&mut self, rd: DeclId, linkage: VtLinkage) -> R<()> {
        let ast = self.ast;
        let loc = Some(ast.decls.get(rd).loc());
        let vtt = self.vtt_global(rd)?;
        if self.m.global(vtt).initializer().is_some() {
            return Ok(());
        }
        let vtt_l = match self.vts.vtt(rd) {
            Ok(v) => v,
            Err(e) => return err(loc, format!("cannot lay out the VTT: {e:?}")),
        };
        let (comps, vts) = (&vtt_l.components, &vtt_l.vtables);
        let table = self.vts.subobjects(rd);
        let subs = &table.list;
        let mut groups: Vec<(Value, Rc<VTableLayout>)> = Vec::with_capacity(vts.len());
        for (k, &sub) in vts.iter().enumerate() {
            if k == 0 {
                let g = self.vtable_global(rd)?;
                groups.push((Value::global(g), self.vt_layout(rd)?));
                continue;
            }
            let Some(&(so, off)) = subs.get(usize::from(sub.0)) else {
                return err(loc, "VTT vtable of an unknown subobject");
            };
            let l = match self.vts.construction_vtable(rd, sub) {
                Ok(l) => l,
                Err(e) => return err(loc, format!("cannot lay out a construction vtable: {e:?}")),
            };
            let name = mangle::mangle_ctor_vtable(ast, &mut self.anon, rd, off as i64, so.class);
            let ty = self.vtable_ir_ty(&l);
            let g = match self.m.get_global(&name) {
                Some(g) => g,
                None => self.m.add_global(&name, ty),
            };
            {
                let gv = self.m.global_mut(g);
                gv.is_constant = true;
                gv.unnamed_addr = UnnamedAddr::Global;
                gv.align = Some(8);
            }
            let bty = self.class_type(so.class);
            let rtti = self.rtti_descriptor(bty)?;
            let init = self.vtable_init(&l, rtti, rd)?;
            self.define_vtable_global(g, init, linkage, rd)?;
            // clang `GenerateConstructionVTable`; clang 19 and 22 attach a
            // construction vtable's set twice.
            self.vtable_type_metadata(g, &l, rd);
            self.vtable_type_metadata(g, &l, rd);
            groups.push((Value::global(g), l));
        }
        let mut elems = Vec::with_capacity(comps.len());
        for c in comps {
            let Some(&(so, off)) = subs.get(usize::from(c.base.0)) else {
                return err(loc, "VTT entry of an unknown subobject");
            };
            let (g, l) = &groups[c.vtable_index as usize];
            let (g, l) = (*g, Rc::clone(l));
            elems.push(self.group_address_point(g, &l, rd, so.class, off)?);
        }
        let init = match self.m.const_array(LTy::PTR, &elems) {
            Ok(a) => a,
            Err(e) => return err(loc, format!("bad VTT: {e:?}")),
        };
        self.define_vtable_global(vtt, init, linkage, rd)
    }

    /// Sets the initializer, linkage and comdat of a vtable (or VTT)
    /// global.
    pub(crate) fn define_vtable_global(
        &mut self,
        g: GlobalId,
        init: Value,
        linkage: VtLinkage,
        rd: DeclId,
    ) -> R<()> {
        if let Err(e) = self.m.set_initializer(g, Some(init)) {
            return err(None, format!("bad vtable initializer: {e:?}"));
        }
        self.set_vague_linkage(g, linkage);
        // clang `setGVProperties(VTable, RD)`: the class's visibility; an
        // `available_externally` one keeps only an explicit one.
        let vis = match linkage {
            VtLinkage::Internal => Visibility::Default,
            VtLinkage::AvailableExternally => self.class_visibility(rd, false),
            _ => self.class_visibility(rd, true),
        };
        self.m.global_mut(g).visibility = vis;
        Ok(())
    }

    /// The record type of class `rd`.
    pub(crate) fn class_type(&self, rd: DeclId) -> QualType {
        let ast = self.ast;
        let def = self.class_def(rd).unwrap_or(rd);
        ast.decls.get(def).ty().unwrap_or(QualType::ERROR)
    }

    /// clang `createVTableInitializer` for group `l` of `layout_class`.
    pub(crate) fn vtable_init(
        &mut self,
        l: &VTableLayout,
        rtti: Value,
        layout_class: DeclId,
    ) -> R<Value> {
        let mut groups = Vec::with_capacity(l.vtable_starts.len());
        for k in 0..l.vtable_starts.len() {
            let start = l.vtable_starts[k] as usize;
            let n = Self::vtable_size(l, k) as usize;
            let mut elems = Vec::with_capacity(n);
            for i in start..start + n {
                let c = self.vtable_component(l, i, rtti, layout_class)?;
                elems.push(c);
            }
            match self.m.const_array(LTy::PTR, &elems) {
                Ok(a) => groups.push(a),
                Err(e) => return err(None, format!("bad vtable: {e:?}")),
            }
        }
        match self.m.const_anon_struct(&groups, false) {
            Ok(v) => Ok(v),
            Err(e) => err(None, format!("bad vtable: {e:?}")),
        }
    }

    /// An offset component (`inttoptr (i64 N to ptr)`, `null` for 0).
    fn offset_component(&mut self, o: i64) -> R<Value> {
        if o == 0 {
            return Ok(self.m.const_null(LTy::PTR));
        }
        let i = self.m.const_int(LTy::I64, i128::from(o));
        match self.m.const_cast(CastOp::IntToPtr, i, LTy::PTR) {
            Ok(v) => Ok(v),
            Err(e) => err(None, format!("bad vtable offset: {e:?}")),
        }
    }

    fn vtable_component(
        &mut self,
        l: &VTableLayout,
        i: usize,
        rtti: Value,
        layout_class: DeclId,
    ) -> R<Value> {
        let ast = self.ast;
        let (m, v) = match l.components[i] {
            VTableComponent::VCallOffset(o)
            | VTableComponent::VBaseOffset(o)
            | VTableComponent::OffsetToTop(o) => return self.offset_component(o),
            VTableComponent::Rtti(_) => return Ok(rtti),
            VTableComponent::UnusedFunc(_) => return Ok(self.m.const_null(LTy::PTR)),
            VTableComponent::Func(m) => (m, None),
            VTableComponent::CompleteDtor(m) => (m, Some(Structor::Complete)),
            VTableComponent::DeletingDtor(m) => (m, Some(Structor::Deleting)),
        };
        let fd = ast.decls.function(m);
        if fd.is_some_and(|f| f.spec.is_pure()) {
            return Ok(self.special_virtual_fn("__cxa_pure_virtual"));
        }
        if ast.redecls_of(m).any(|r| ast.decls.function(r).is_some_and(|f| f.spec.is_deleted())) {
            return Ok(self.special_virtual_fn("__cxa_deleted_virtual"));
        }
        let idx = u32::try_from(i).unwrap_or(u32::MAX);
        if let Ok(t) = l.thunks.binary_search_by_key(&idx, |&(c, _)| c) {
            let th = l.thunks[t].1;
            return self.thunk_for_vtable(m, v, th, layout_class);
        }
        match v {
            None => {
                let f = self.get_fn(m)?;
                Ok(Value::func(f))
            }
            Some(v) => self.structor_const(m, v),
        }
    }

    /// The constant symbol of structor variant `v` of `d` (for vtables).
    pub(crate) fn structor_const(&mut self, d: DeclId, v: Structor) -> R<Value> {
        let saved = self.f.uses_ph;
        let r = self.structor_ref(d, v)?;
        self.f.uses_ph = saved;
        match r.0 {
            V::C(c) => Ok(c),
            _ => err(
                Some(self.ast.decls.get(d).loc()),
                "a vtable references a structor alias defined later (unsupported order)",
            ),
        }
    }

    /// `__cxa_pure_virtual` / `__cxa_deleted_virtual` (`void()`,
    /// `unnamed_addr`).
    fn special_virtual_fn(&mut self, name: &str) -> Value {
        if let Some(f) = self.m.get_function(name) {
            return Value::func(f);
        }
        let ty = self.m.types.function(LTy::VOID, &[], false);
        match self.m.add_function(name, ty) {
            Ok(f) => {
                self.m.func_mut(f).unnamed_addr = UnnamedAddr::Global;
                Value::func(f)
            }
            Err(_) => self.m.const_null(LTy::PTR),
        }
    }

    /// clang `getVTableAddressPoint`: the address point of subobject
    /// (`class`, `offset`) in the vtable group of `vtable_class`
    /// (`getelementptr inbounds inrange(..)`).
    pub(crate) fn vtable_address_point(
        &mut self,
        vtable_class: DeclId,
        class: DeclId,
        offset: u64,
    ) -> R<Value> {
        let g = self.vtable_global(vtable_class)?;
        let l = self.vt_layout(vtable_class)?;
        self.group_address_point(Value::global(g), &l, vtable_class, class, offset)
    }

    /// The address point of subobject (`class`, `offset`) of
    /// `layout_class` in group `l` held by global `g`.
    pub(crate) fn group_address_point(
        &mut self,
        g: Value,
        l: &VTableLayout,
        layout_class: DeclId,
        class: DeclId,
        offset: u64,
    ) -> R<Value> {
        let loc = Some(self.ast.decls.get(class).loc());
        let Some(sub) = self.subobject_id(layout_class, class, offset) else {
            return err(loc, "vtable address point of an unknown subobject");
        };
        let Some((k, ap)) = Self::address_point_of(l, sub) else {
            return err(loc, "subobject without a vtable address point");
        };
        let size = Self::vtable_size(l, k as usize);
        let ty = self.vtable_ir_ty(l);
        let zero = self.m.const_int(LTy::I32, 0);
        let kv = self.m.const_int(LTy::I32, i128::from(k));
        let apv = self.m.const_int(LTy::I32, i128::from(ap));
        let inrange = (-8 * i64::from(ap), 8 * (i64::from(size) - i64::from(ap)));
        match self.m.const_gep(ty, g, &[zero, kv, apv], GepFlags::INBOUNDS, Some(inrange)) {
            Ok(v) => Ok(v),
            Err(e) => err(loc, format!("bad vtable address point: {e:?}")),
        }
    }

    // ---- vtable pointers -----------------------------------------------

    /// clang `getVTablePointers(VTableClass)`.
    pub(crate) fn vtable_pointers(&mut self, vtable_class: DeclId) -> Vec<VPtr> {
        let mut out = Vec::new();
        let mut seen = Vec::new();
        self.vptrs_rec(vtable_class, 0, None, 0, false, vtable_class, &mut seen, &mut out);
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn vptrs_rec(
        &mut self,
        class: DeclId,
        offset: u64,
        nearest_vbase: Option<DeclId>,
        offset_from_vbase: u64,
        nv_primary: bool,
        vtable_class: DeclId,
        seen: &mut Vec<DeclId>,
        out: &mut Vec<VPtr>,
    ) {
        let ast = self.ast;
        if !nv_primary {
            out.push(VPtr { class, offset, nearest_vbase, offset_from_vbase });
        }
        let Some(def) = self.class_def(class) else { return };
        let bases: Vec<(QualType, bool)> =
            ast.decls.bases(def, &ast.lists).map(|b| (b.ty, b.is_virtual)).collect();
        for (bq, virt) in bases {
            let Some(b) = ast.types.as_record(bq) else { continue };
            let b = ast.first(b);
            if !self.is_dynamic(b) {
                continue;
            }
            if virt {
                if seen.contains(&b) {
                    continue;
                }
                seen.push(b);
                let Some(vdef) = self.class_def(vtable_class) else { continue };
                let l = self.tys.layouter().record(vdef);
                let off = l.vbases.iter().find(|&&(x, _)| ast.first(x) == b).map_or(0, |&(_, o)| o);
                self.vptrs_rec(b, off, Some(b), 0, false, vtable_class, seen, out);
            } else {
                let l = self.tys.layouter().record(def);
                let bo = l.bases.iter().find(|&&(x, _)| ast.first(x) == b).map_or(0, |&(_, o)| o);
                let primary = l.primary_base.is_some_and(|(p, v)| !v && ast.first(p) == b);
                self.vptrs_rec(
                    b,
                    offset + bo,
                    nearest_vbase,
                    offset_from_vbase + bo,
                    primary,
                    vtable_class,
                    seen,
                    out,
                );
            }
        }
    }

    /// clang `InitializeVTablePointers(RD)` in a structor of `rd`.
    pub(crate) fn init_vtable_ptrs(&mut self, rd: DeclId) -> R<()> {
        if !self.is_dynamic(rd) {
            return Ok(());
        }
        let rd = self.ast.first(rd);
        for vp in self.vtable_pointers(rd) {
            self.init_vtable_ptr(rd, vp)?;
        }
        Ok(())
    }

    /// clang `InitializeVTablePointer`.
    fn init_vtable_ptr(&mut self, vtable_class: DeclId, vp: VPtr) -> R<()> {
        let needs_vtt = self.needs_vtt_param();
        let has_vbases = self.class_bits_of(vp.class).contains(DefinitionBits::HAS_VIRTUAL_BASES);
        let ap = if (has_vbases || vp.nearest_vbase.is_some()) && needs_vtt {
            self.vtt_address_point(vtable_class, vp)?
        } else {
            V::C(self.vtable_address_point(vtable_class, vp.class, vp.offset)?)
        };
        let this = self.this_addr();
        let field = if needs_vtt && vp.nearest_vbase.is_some() {
            // isVirtualOffsetNeededForVTableField.
            let vb = vp.nearest_vbase;
            self.base_address(this, vtable_class, vb, vp.offset_from_vbase, false)?
        } else {
            (self.byte_offset(this.v, vp.offset), min_align(this.align, vp.offset))
        };
        let st = self.store(ap, field.0, field.1, false);
        self.tbaa_vptr(st);
        Ok(())
    }

    /// The definition bits of class `rd`.
    pub(crate) fn class_bits_of(&self, rd: DeclId) -> DefinitionBits {
        self.class_def(rd)
            .and_then(|d| self.ast.decls.record(d))
            .map_or(DefinitionBits::empty(), |r| r.defn)
    }

    /// clang `CanSkipVTablePointerInitialization` for destructor `d` of
    /// `rd`.
    pub(crate) fn can_skip_vptr_init(&mut self, d: DeclId, rd: DeclId) -> bool {
        if !self.is_dynamic(rd) {
            return true;
        }
        let def = self.class_def(rd).unwrap_or(rd);
        if self.class_is_final(def) || self.dtor_final(def) {
            return true;
        }
        if !self.has_trivial_body(d) {
            return false;
        }
        self.fields_trivial_dtor_body(def)
    }

    fn dtor_final(&self, def: DeclId) -> bool {
        self.ast
            .destructor(def)
            .and_then(|d| self.ast.decls.function(d))
            .is_some_and(|f| f.spec.is_final())
    }

    /// `FunctionDecl::hasTrivialBody`: the body is `{}`.
    fn has_trivial_body(&self, d: DeclId) -> bool {
        let ast = self.ast;
        let body = ast.redecls_of(d).find_map(|r| ast.decls.function(r).and_then(|f| f.body));
        body.is_some_and(|b| self.is_empty_compound(b))
    }

    /// clang `HasTrivialDestructorBody(Context, BaseClassDecl,
    /// MostDerivedClassDecl)`.
    fn trivial_dtor_body(&mut self, class: DeclId, most_derived: DeclId) -> bool {
        let ast = self.ast;
        let Some(def) = self.class_def(class) else { return true };
        if self.class_bits_of(def).contains(DefinitionBits::TRIVIAL_DTOR) {
            return true;
        }
        let Some(dtor) = ast.destructor(def) else { return true };
        if !self.has_trivial_body(dtor) {
            return false;
        }
        if !self.fields_trivial_dtor_body(def) {
            return false;
        }
        let bases: Vec<(QualType, bool)> =
            ast.decls.bases(def, &ast.lists).map(|b| (b.ty, b.is_virtual)).collect();
        for (bq, virt) in &bases {
            if *virt {
                continue;
            }
            if let Some(b) = ast.types.as_record(*bq)
                && !self.trivial_dtor_body(b, most_derived)
            {
                return false;
            }
        }
        if ast.first(class) == ast.first(most_derived) {
            let vbases: Vec<DeclId> = {
                let l = self.tys.layouter().record(def);
                l.vbases.iter().map(|&(v, _)| v).collect()
            };
            for v in vbases {
                if !self.trivial_dtor_body(v, most_derived) {
                    return false;
                }
            }
        }
        true
    }

    /// Every field of `def` has a trivial destructor body
    /// (`FieldHasTrivialDestructorBody`).
    fn fields_trivial_dtor_body(&mut self, def: DeclId) -> bool {
        let ast = self.ast;
        let fields: Vec<DeclId> = ast
            .decls
            .members(def)
            .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
            .collect();
        for f in fields {
            let Some(mut q) = ast.decls.get(f).ty() else { continue };
            while let Some(e) = self.array_elem(q) {
                q = e;
            }
            let Some(r) = ast.types.as_record(q) else { continue };
            if !self.trivial_dtor_body(r, r) {
                return false;
            }
        }
        true
    }

    /// The statement is `{}`.
    fn is_empty_compound(&self, s: kiln_ast::StmtId) -> bool {
        let st = &self.ast.stmts;
        st.get(s).kind() == kiln_ast::stmt::StmtKind::CompoundStmt
            && self.ast.lists.iter::<kiln_ast::StmtId>(st.compound_body(s)).next().is_none()
    }

    // ---- VTTs (phase 3) ------------------------------------------------

    /// clang `CGCXXABI::NeedsVTTParameter(CurGD)`.
    pub(crate) fn needs_vtt_param(&self) -> bool {
        self.f.vtt.is_some()
    }

    /// The VTT entries of `rd` and the subobjects of the groups they
    /// point into (cached per class by the layouter's subobject ids).
    fn vtt_of(&mut self, rd: DeclId) -> R<Rc<kiln_abi::vtable::VttLayout>> {
        match self.vts.vtt(rd) {
            Ok(v) => Ok(v),
            Err(e) => {
                err(Some(self.ast.decls.get(rd).loc()), format!("cannot lay out the VTT: {e:?}"))
            }
        }
    }

    /// clang `getVTableAddressPointInStructorWithVTT`: the address point
    /// loaded from the VTT parameter (`getSecondaryVirtualPointerIndex`).
    fn vtt_address_point(&mut self, vtable_class: DeclId, vp: VPtr) -> R<V> {
        let loc = Some(self.ast.decls.get(vp.class).loc());
        let v = self.vtt_of(vtable_class)?;
        let comps = &v.components;
        let Some(sub) = self.subobject_id(vtable_class, vp.class, vp.offset) else {
            return err(loc, "vtable pointer of an unknown subobject");
        };
        let Some(i) = comps.iter().position(|c| c.vtable_index == 0 && c.base == sub) else {
            return err(loc, "no secondary virtual pointer index");
        };
        let mut vtt = self.f.vtt.expect("VTT parameter");
        if i != 0 {
            let o = self.i64c(i as i128);
            vtt = self.gep(LTy::PTR, vtt, vec![o], true);
        }
        Ok(self.load(LTy::PTR, vtt, 8, false))
    }

    /// clang `CodeGenFunction::GetVTTParameter`: the VTT argument of a
    /// call to a base-object structor of `base` (a virtual base when
    /// `virt`) from a structor of the current class.
    pub(crate) fn vtt_arg(&mut self, base: DeclId, virt: bool, delegating: bool) -> R<V> {
        let rd = self.f.this_class.expect("structor class");
        let loc = Some(self.ast.decls.get(base).loc());
        if delegating && let Some(vtt) = self.f.vtt {
            return Ok(vtt);
        }
        let same = self.class_def(base) == self.class_def(rd);
        let idx = if delegating || same {
            0
        } else {
            let off = if virt { self.vbase_offset(rd, base) } else { self.base_offset(rd, base) };
            let Some(off) = off else { return err(loc, "base class not in the layout") };
            let Some(sub) = self.subobject_id(rd, base, off) else {
                return err(loc, "VTT of an unknown subobject");
            };
            let v = self.vtt_of(rd)?;
            let (comps, vts) = (&v.components, &v.vtables);
            let Some(k) = vts.iter().position(|&s| s == sub) else {
                return err(loc, "no sub-VTT for the base");
            };
            let Some(i) = comps.iter().position(|c| c.vtable_index as usize == k) else {
                return err(loc, "empty sub-VTT");
            };
            i
        };
        if let Some(vtt) = self.f.vtt {
            let o = self.i64c(idx as i128);
            return Ok(self.gep(LTy::PTR, vtt, vec![o], true));
        }
        let g = self.vtt_global(rd)?;
        if idx == 0 {
            // A constant GEP of all-zero indices folds to the global.
            return Ok(V::C(Value::global(g)));
        }
        let ty = self.m.global(g).value_type();
        let zero = self.m.const_int(LTy::I64, 0);
        let iv = self.m.const_int(LTy::I64, idx as i128);
        match self.m.const_gep(ty, Value::global(g), &[zero, iv], GepFlags::INBOUNDS, None) {
            Ok(v) => Ok(V::C(v)),
            Err(e) => err(loc, format!("bad VTT address: {e:?}")),
        }
    }

    /// A thunk in a vtable slot (clang `maybeEmitThunk(ForVTable)`: at
    /// `-O0` only its declaration; the definition comes with the
    /// method's, `emit_thunks`).
    fn thunk_for_vtable(
        &mut self,
        m: DeclId,
        v: Option<Structor>,
        th: Thunk,
        _layout_class: DeclId,
    ) -> R<Value> {
        let f = self.thunk_fn(m, v, th)?;
        if self.opts.optimize()
            && self.m.func(f).is_declaration()
            && self.thunk_has_no_target(m, th)
        {
            // clang `maybeEmitThunk(ForVTable)` → `shouldEmitVTableThunk`:
            // above -O0 a vtable's thunk is defined `available_externally`
            // with it (a thunk of a method defined here is defined after
            // that method: `emit_thunks`); a variadic one is a `musttail`
            // forward (the -O1 pipeline's `EliminateAvailableExternally`
            // leaves its declaration). Not for an unprototyped
            // (incomplete parameter or return type) method or a
            // return-adjusting variadic one.
            let saved = std::mem::replace(&mut self.f, crate::func::Fx::empty());
            let r = self.gen_code_or_thunk(self.ast.first(m), f, v, Some(th));
            self.f = saved;
            r?;
        } else if self.opts.optimize()
            && self.m.func(f).is_declaration()
            && self.variadic_thunk_without_target(m, th)
        {
            // clang `maybeEmitThunk(ForVTable)` gives a return-adjusting
            // variadic thunk the attributes of a definition
            // (`SetLLVMFunctionAttributesForDefinition`: `uwtable`, the
            // `-Os`/`-Oz` size attributes, x86's `align 2`) and then
            // leaves it a declaration: it is a clone of the method
            // (`GenerateVarArgsThunk`, `emit_vararg_thunk_clone`), never
            // `available_externally`.
            let fa = self.m.func(f).attrs.func;
            let mut a = self.m.attrs_of(fa).to_vec();
            self.def_attrs(&mut a);
            self.size_attrs(&mut a);
            a.dedup();
            let fa = self.m.attr_set(&a);
            let x86 = self.is_x86();
            let func = self.m.func_mut(f);
            func.attrs.func = fa;
            if x86 {
                func.align = Some(2);
            }
        }
        Ok(Value::func(f))
    }

    /// The virtual method `m` has no definition here, is variadic, has a
    /// convertible type, and its thunk `th` adjusts the return.
    fn variadic_thunk_without_target(&self, m: DeclId, th: Thunk) -> bool {
        let ast = self.ast;
        if th.ret == (0, 0) {
            return false;
        }
        if ast.redecls_of(m).any(|r| ast.decls.function(r).is_some_and(|f| f.body.is_some())) {
            return false;
        }
        let first = ast.first(m);
        let Some(fty) = ast.decls.get(first).ty() else { return false };
        ast.types.proto(fty).is_some_and(kiln_ast::types::FuncProto::variadic)
            && !self.has_incomplete_class_in_sig(first)
    }

    /// The virtual method `m` has no definition here, has a convertible
    /// type (clang `isFuncTypeConvertible`: no incomplete class parameter
    /// or return type) and, if variadic, thunk `th` adjusts no return.
    fn thunk_has_no_target(&self, m: DeclId, th: Thunk) -> bool {
        let ast = self.ast;
        if ast.redecls_of(m).any(|r| ast.decls.function(r).is_some_and(|f| f.body.is_some())) {
            return false;
        }
        let Some(fty) = ast.decls.get(ast.first(m)).ty() else { return false };
        let Some(p) = ast.types.proto(fty).copied() else { return false };
        if p.variadic() && th.ret != (0, 0) {
            return false;
        }
        let complete =
            |q: QualType| ast.types.as_record(q).is_none_or(|r| self.class_def(r).is_some());
        complete(p.ret()) && ast.types.params(&p).iter().all(|&q| complete(q))
    }

    /// clang `GetAddrOfThunk`: the function of thunk `th` of variant `v`
    /// of method `m` (declared with `IsThunk` attributes: `this` is only
    /// `noundef`).
    fn thunk_fn(&mut self, m: DeclId, v: Option<Structor>, th: Thunk) -> R<FuncId> {
        let ast = self.ast;
        let first = ast.first(m);
        let name = mangle::mangle_thunk(
            ast,
            &mut self.anon,
            first,
            v.unwrap_or(Structor::Complete),
            th.this,
            th.ret,
        );
        if let Some(&f) = self.thunk_fns.get(&name) {
            return Ok(f);
        }
        if self.has_incomplete_class_in_sig(first) {
            // clang `GetAddrOfThunk` of a type `isFuncTypeConvertible`
            // rejects: `void ()` without attributes (spec/codegen.md §7z).
            let ty = self.m.types.function(LTy::VOID, &[], false);
            let f = match self.m.add_function(&name, ty) {
                Ok(f) => f,
                Err(e) => {
                    return err(
                        Some(ast.decls.get(first).loc()),
                        format!("cannot declare {name}: {e:?}"),
                    );
                }
            };
            self.m.func_mut(f).unnamed_addr = UnnamedAddr::Global;
            self.thunk_fns.insert(name, f);
            return Ok(f);
        }
        let sig = self.decl_sig(first)?;
        let f = self.declare_fn(&name, first, &sig)?;
        let (ret, params) = {
            let a = &self.m.func(f).attrs;
            (a.ret, a.params.clone())
        };
        let (ret, params) = self.thunk_attrs(&sig, ret, params);
        {
            let func = self.m.func_mut(f);
            func.unnamed_addr = UnnamedAddr::Global;
            func.attrs.ret = ret;
            func.attrs.params = params;
        }
        self.thunk_fns.insert(name, f);
        Ok(f)
    }

    /// clang `ConstructAttributeList(IsThunk)`: `this` is only `noundef`
    /// and a returned reference has no `nonnull`/`dereferenceable`/`align`.
    pub(crate) fn thunk_attrs(
        &mut self,
        sig: &crate::module::Sig,
        ret: kiln_llvm::AttrSetId,
        mut params: Vec<kiln_llvm::AttrSetId>,
    ) -> (kiln_llvm::AttrSetId, Vec<kiln_llvm::AttrSetId>) {
        let nu = self.m.attr_set(&[Attr::Flag(AttrKind::NoUndef)]);
        if let Some(p) = params.get_mut(usize::from(sig.sret())) {
            *p = nu;
        }
        let ret = if self.referee(sig.ret).is_some() {
            if sig.info.ret.noundef { nu } else { self.m.attr_set(&[]) }
        } else {
            ret
        };
        (ret, params)
    }

    /// clang `CodeGenVTables::EmitThunks`: after the definition `d` of a
    /// virtual method (variant `v` of a destructor) was emitted, defines
    /// its thunks.
    pub(crate) fn emit_thunks(&mut self, d: DeclId, v: Option<Structor>) -> R<()> {
        let ast = self.ast;
        if v == Some(Structor::Base) {
            return Ok(());
        }
        let first = ast.first(d);
        let virt = ast
            .redecls_of(first)
            .any(|r| ast.decls.function(r).is_some_and(|f| f.spec.is_virtual()));
        if !virt {
            return Ok(());
        }
        let Some(class) = ast.decls.get(first).ctx() else { return Ok(()) };
        let l = self.vt_layout(class)?;
        let ths: Vec<Thunk> = l
            .method_thunks
            .iter()
            .filter(|&&(m, _)| ast.first(m) == first)
            .map(|&(_, t)| t)
            .collect();
        let variadic = v.is_none() && self.is_variadic_virtual(d);
        for th in ths {
            let f = self.thunk_fn(d, v, th)?;
            if !self.m.func(f).is_declaration() {
                continue;
            }
            if variadic && th.ret != (0, 0) {
                self.emit_vararg_thunk_clone(d, f, th)?;
                continue;
            }
            let saved = std::mem::replace(&mut self.f, crate::func::Fx::empty());
            let r = self.gen_code_or_thunk(d, f, v, Some(th));
            self.f = saved;
            r?;
        }
        Ok(())
    }

    /// The method `d` is virtual and variadic.
    pub(crate) fn is_variadic_virtual(&self, d: DeclId) -> bool {
        let ast = self.ast;
        let first = ast.first(d);
        ast.decls
            .get(first)
            .ty()
            .and_then(|t| ast.types.proto(t))
            .is_some_and(kiln_ast::types::FuncProto::variadic)
            && ast
                .redecls_of(first)
                .any(|r| ast.decls.function(r).is_some_and(|f| f.spec.is_virtual()))
    }

    /// clang `GenerateVarArgsThunk`: the return-adjusting thunk `f` (`th`)
    /// of the variadic method `d` (just defined) is a clone of the
    /// method's body, appended to the module (`CloneFunction`) with the
    /// method's header, in which `this` is adjusted before its first
    /// store in the entry block and the returned value is adjusted before
    /// the first `ret` in layout order; then `setThunkProperties`.
    fn emit_vararg_thunk_clone(&mut self, d: DeclId, f: FuncId, th: Thunk) -> R<()> {
        let ast = self.ast;
        let loc = Some(ast.decls.get(d).loc());
        let method = self.get_fn(d)?;
        let Some((body, uses_ph)) =
            self.vararg_bodies.get(&method).and_then(|(b, ph)| Some((b.clone_body()?, *ph)))
        else {
            // A body with a `blockaddress` cannot be cloned here (or the
            // method was never defined, clang's `ErrorUnsupported`).
            return err(
                loc,
                "a return-adjusting thunk of this variadic virtual function is not supported",
            );
        };
        let first = ast.first(d);
        let sig = self.decl_sig(first)?;
        let rd = sig.this.expect("method class");
        let fn_ty = self.m.func(method).ty();
        // `CloneFunction` names the clone after the method (taking the
        // module's next unique suffix) before `takeName` renames it.
        let mname = self.m.func(method).name().to_owned();
        self.m.consume_unique_name(&mname);
        if let Err(e) = self.m.retype_function_to_end(f, fn_ty) {
            return err(loc, format!("cannot define a thunk of this function: {e:?}"));
        }
        let (attrs, unnamed_addr, align, personality, section, cc) = {
            let m = self.m.func(method);
            (m.attrs.clone(), m.unnamed_addr, m.align, m.personality, m.section.clone(), m.cc)
        };
        {
            let func = self.m.func_mut(f);
            func.attrs = attrs;
            func.unnamed_addr = unnamed_addr;
            func.align = align;
            func.personality = personality;
            func.section = section;
            func.cc = cc;
        }
        // setThunkProperties: the method's linkage, a comdat of its own.
        self.set_def_linkage(f, d);
        let nounwind = self.nounwind.get(&method).copied().unwrap_or(false);
        self.nounwind.insert(f, nounwind);

        let saved = std::mem::replace(&mut self.f, crate::func::Fx::empty());
        self.f.fir = body;
        self.f.decl = Some(d);
        self.f.func = Some(f);
        self.f.sig = Some(sig.clone());
        self.f.this_class = Some(rd);
        let r = self.vararg_thunk_adjust(&sig, rd, th);
        let fir = std::mem::replace(&mut self.f.fir, Fir::new(Vec::new()));
        self.f = saved;
        r?;
        if uses_ph {
            self.pending.push((f, d, fir));
        } else if let Err(e) = fir.lower(&mut self.m, f, &self.phs) {
            return err(loc, format!("IR lowering failed: {e:?}"));
        }
        Ok(())
    }

    /// The `this` and return adjustments of [`Cg::emit_vararg_thunk_clone`]
    /// on the cloned body in `self.f.fir`.
    fn vararg_thunk_adjust(&mut self, sig: &crate::module::Sig, rd: DeclId, th: Thunk) -> R<()> {
        let loc = Some(self.ast.decls.get(self.f.decl.expect("method")).loc());
        let this = V::Arg(u32::from(sig.sret()));
        let entry = self.f.fir.entry();
        let Some(store) = self.f.fir.first_store_of(entry, this) else {
            return err(loc, "no store of `this` in the entry block of a variadic method");
        };
        let (_, this_align) = self.this_layout(rd);
        let mark = self.f.fir.mark();
        self.f.fir.set_cur(Some(entry));
        let adj = self.type_adjustment(this, this_align, th.this, false);
        for i in mark..self.f.fir.mark() {
            if self.f.fir.inst_block(i).is_some() {
                self.f.fir.move_before(i, store);
            }
        }
        self.f.fir.set_store_val(store, adj);
        let ret = self.f.fir.layout().iter().find_map(|&b| {
            let t = self.f.fir.terminator(b)?;
            match self.f.fir.op(t) {
                Op::Ret { v } => Some((b, t, *v)),
                _ => None,
            }
        });
        if let Some((b, t, Some(rv))) = ret {
            self.f.fir.erase(t);
            self.f.fir.set_cur(Some(b));
            let p = self.return_adjustment(rv, sig.ret, th.ret);
            self.f.fir.push(Op::Ret { v: Some(p) }, LTy::VOID);
        }
        self.f.fir.set_cur(None);
        Ok(())
    }

    /// clang `EmitCallAndReturnForThunk`: the body of thunk `th` of
    /// variant `v` of method `d` (parameters `params`, spilled by the
    /// prolog).
    pub(crate) fn emit_thunk_body(
        &mut self,
        d: DeclId,
        v: Option<Structor>,
        th: Thunk,
        params: &[DeclId],
    ) -> R<()> {
        let ast = self.ast;
        let rd = self.f.this_class.expect("method class");
        let (_, this_align) = self.this_layout(rd);
        let this = self.this_value();
        let adj = self.type_adjustment(this, this_align, th.this, false);
        let first = ast.first(d);
        let fty = ast.decls.get(first).ty().expect("method type");
        if ast.types.proto(fty).is_some_and(kiln_ast::types::FuncProto::variadic) {
            return self.emit_musttail_thunk(d, v, th, params, adj);
        }
        let mut vals = Vec::with_capacity(params.len());
        let mut ptys = Vec::with_capacity(params.len());
        for &p in params {
            let q = ast.decls.get(p).ty().expect("param type");
            let a = self.f.locals[&p];
            // EmitDelegateCallArg.
            vals.push(self.delegate_call_arg(a, q));
            ptys.push(q);
        }
        let sig = self.call_sig_this(fty, false, &ptys, Some(rd));
        let (callee, fid) = if let Some(v) = v {
            self.structor_ref(d, v)?
        } else {
            let f = self.get_fn(d)?;
            (V::C(Value::func(f)), Some(f))
        };
        let rq = sig.ret;
        let agg = self.is_aggregate(rq);
        let dest = if sig.sret() || agg { self.f.ret_slot } else { None };
        let r = self.call_vals(&sig, callee, fid, Some(adj), &vals, &[], dest, &[])?;
        let call = self.f.calls.last().map(|c| c.0);
        let mut rv = r;
        if th.ret != (0, 0) {
            if let Some(p) = r {
                rv = Some(self.return_adjustment(p, rq, th.ret));
            }
        } else if let Some(c) = call {
            self.f.fir.set_tail(c);
        }
        // EmitReturnOfRValue.
        self.delegate_cx_return(rq, sig.sret());
        if let Some(x) = rv
            && !agg
            && let Some(slot) = self.f.ret_slot
        {
            self.store_scalar(x, slot, rq);
        }
        Ok(())
    }

    /// clang `EmitMustTailThunk` (from `EmitCallAndReturnForThunk` for a
    /// variadic method): the thunk's own arguments, `this` replaced by the
    /// adjusted `adj`, are forwarded to the method by a `musttail` call
    /// (which passes the variadic arguments on, `...`) whose result is
    /// returned; then a fresh, unreachable block takes the ordinary
    /// epilogue (clang's "FIXME: Don't emit unreachable code").
    fn emit_musttail_thunk(
        &mut self,
        d: DeclId,
        v: Option<Structor>,
        th: Thunk,
        params: &[DeclId],
        adj: V,
    ) -> R<()> {
        let ast = self.ast;
        // A return-adjusting variadic thunk is a clone of its method
        // (`emit_vararg_thunk_clone`); only one without a definition here
        // gets this far (clang `ErrorUnsupported`).
        if th.ret != (0, 0) {
            return err(
                Some(self.ast.decls.get(d).loc()),
                "a return-adjusting thunk of a variadic virtual function is not supported",
            );
        }
        let rd = self.f.this_class.expect("method class");
        let ptys: Vec<QualType> =
            params.iter().map(|&p| ast.decls.get(p).ty().expect("param type")).collect();
        let first = ast.first(d);
        let fty = ast.decls.get(first).ty().expect("method type");
        let sig = self.call_sig_this(fty, false, &ptys, Some(rd));
        let (callee, fid) = if let Some(v) = v {
            self.structor_ref(d, v)?
        } else {
            let f = self.get_fn(d)?;
            (V::C(Value::func(f)), Some(f))
        };
        let this_no = u32::from(sig.sret());
        let n = self.m.func(self.f.func.expect("thunk function")).param_types().len() as u32;
        let args: Vec<V> = (0..n).map(|i| if i == this_no { adj } else { V::Arg(i) }).collect();
        let rty = match &sig.info.ret.kind {
            ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. } => {
                let c = coerce.clone();
                self.ir(&c)
            }
            _ => LTy::VOID,
        };
        let (ret, pattrs) = self.sig_attrs(&sig, params.len(), false);
        let mut fa = Vec::new();
        if sig.nothrow {
            fa.push(Attr::Flag(AttrKind::NoUnwind));
        }
        self.call_size_attrs(fid, &mut fa);
        let func =
            if fa.is_empty() { kiln_llvm::AttrList::default().func } else { self.m.attr_set(&fa) };
        let opts = kiln_llvm::CallOpts {
            attrs: kiln_llvm::AttrList { func, ret, params: pattrs },
            cc: sig.llvm_cc(),
            ..kiln_llvm::CallOpts::default()
        };
        // The thunk has no cleanups or EH scope: always a plain `call`.
        let r =
            self.f.fir.push(Op::Call { fn_ty: sig.fn_ty, callee, args, opts: Box::new(opts) }, rty);
        self.f.fir.set_musttail(Fir::last_inst(r));
        let rv = (rty != LTy::VOID).then_some(r);
        self.f.fir.push(Op::Ret { v: rv }, LTy::VOID);
        self.f.fir.set_cur(None);
        let b = self.f.fir.create_block();
        self.emit_block(b, false);
        Ok(())
    }

    /// clang `PerformReturnAdjustment`: null-checked unless a reference.
    fn return_adjustment(&mut self, p: V, rq: QualType, adj: (i64, i64)) -> V {
        let ast = self.ast;
        let is_ref = self.referee(rq).is_some();
        let pointee = ast.types.pointee(rq).or_else(|| self.referee(rq));
        let align =
            pointee.and_then(|q| ast.types.as_record(q)).map_or(8, |c| self.this_layout(c).1);
        if is_ref {
            return self.type_adjustment(p, align, adj, true);
        }
        let notnull = self.f.fir.create_block();
        let null = self.f.fir.create_block();
        let end = self.f.fir.create_block();
        let z = V::C(self.m.const_null(LTy::PTR));
        // The IR builder's constant folder (a cloned variadic body can
        // return a constant).
        let isnull = match self.const_ptr_is_null(p) {
            Some(b) => V::C(self.m.const_bool(b)),
            None => self.icmp(IntPredicate::Eq, p, z),
        };
        self.cond_br(isnull, null, notnull);
        self.emit_block(notnull, false);
        let a = self.type_adjustment(p, align, adj, true);
        let afrom = self.f.fir.cur().expect("insertion point");
        // `Builder.CreateBr`, not `EmitBranch`: the insertion point stays,
        // so each block is placed after the one before it.
        self.br(end);
        self.emit_block(null, false);
        self.br(end);
        self.emit_block(end, false);
        self.f.fir.push(Op::Phi { ty: LTy::PTR, inc: vec![(a, afrom), (z, null)] }, LTy::PTR)
    }

    /// Whether the pointer constant `p` is null, where LLVM's constant
    /// folder knows: `null`, or a global that is not `extern_weak` (or an
    /// `inbounds` GEP of one).
    fn const_ptr_is_null(&self, p: V) -> Option<bool> {
        let V::C(c) = p else { return None };
        if matches!(self.m.const_of(c), Some(kiln_llvm::ConstData::Null(_))) {
            return Some(true);
        }
        let mut c = c;
        loop {
            return match c.get() {
                ValueRef::Const(_) => match self.m.const_of(c) {
                    Some(kiln_llvm::ConstData::Gep { flags, ops, .. })
                        if flags.0 & GepFlags::INBOUNDS.0 != 0 =>
                    {
                        c = ops[0];
                        continue;
                    }
                    _ => None,
                },
                ValueRef::Global(g) => {
                    let g = self.m.global(g);
                    (g.linkage != Linkage::ExternWeak && g.addrspace == 0).then_some(false)
                }
                ValueRef::Func(f) => {
                    (self.m.func(f).linkage != Linkage::ExternWeak).then_some(false)
                }
                _ => None,
            };
        }
    }

    /// clang (Itanium) `performTypeAdjustment` of the pointer `p` (to an
    /// object aligned `align`) by (non-virtual, virtual-offset offset).
    fn type_adjustment(&mut self, p: V, align: u64, (nv, virt): (i64, i64), ret: bool) -> V {
        if nv == 0 && virt == 0 {
            return p;
        }
        let mut v = p;
        if nv != 0 && !ret {
            let o = self.i64c(i128::from(nv));
            v = self.gep(LTy::I8, v, vec![o], true);
        }
        let mut r = v;
        if virt != 0 {
            let vt = self.load(LTy::PTR, v, min_align(align, nv.unsigned_abs()), false);
            self.tbaa_vptr(vt);
            let o = self.i64c(i128::from(virt));
            let op = self.gep(LTy::I8, vt, vec![o], true);
            let off = self.load(LTy::I64, op, 8, false);
            r = self.gep(LTy::I8, v, vec![off], true);
        }
        if nv != 0 && ret {
            let o = self.i64c(i128::from(nv));
            r = self.gep(LTy::I8, r, vec![o], true);
        }
        r
    }

    // ---- virtual calls -------------------------------------------------

    /// The vtable slot index of virtual method `md` (variant `v` of a
    /// destructor) from the address point of its class's vtable.
    pub(crate) fn method_vtable_index(&mut self, md: DeclId, v: Option<Structor>) -> R<u32> {
        let ast = self.ast;
        let first = ast.first(md);
        let Some(class) = ast.decls.get(first).ctx() else {
            return err(Some(ast.decls.get(md).loc()), "virtual function outside a class");
        };
        let l = self.vt_layout(class)?;
        let Ok(i) = l.method_indices.binary_search_by_key(&first, |&(m, _)| ast.first(m)) else {
            let pos = l.method_indices.iter().find(|&&(m, _)| ast.first(m) == first);
            return match pos {
                Some(&(_, i)) => Ok(i + u32::from(v == Some(Structor::Deleting))),
                None => err(Some(ast.decls.get(md).loc()), "virtual function without a slot"),
            };
        };
        Ok(l.method_indices[i].1 + u32::from(v == Some(Structor::Deleting)))
    }

    /// clang `getVirtualFunctionPointer`: loads the slot of `md` from the
    /// vtable of the object at `this` (alignment `align`).
    pub(crate) fn virtual_fn_ptr(
        &mut self,
        md: DeclId,
        v: Option<Structor>,
        this: V,
        align: u64,
    ) -> R<V> {
        let idx = self.method_vtable_index(md, v)?;
        let vt = self.load(LTy::PTR, this, align, false);
        self.tbaa_vptr(vt);
        let i = self.i64c(i128::from(idx));
        let slot = self.gep(LTy::PTR, vt, vec![i], true);
        Ok(self.load(LTy::PTR, slot, 8, false))
    }

    // ---- devirtualization ----------------------------------------------

    /// Methods `m` overrides directly (first declarations).
    pub(crate) fn overridden_methods(&self, m: DeclId) -> Vec<DeclId> {
        let ast = self.ast;
        let list =
            ast.decls.cold.overrides.get(ast.first(m)).copied().unwrap_or(kiln_ast::ListId::EMPTY);
        ast.lists.iter::<DeclId>(list).map(|o| ast.first(o)).collect()
    }

    /// clang `recursivelyOverrides(derived, base)`.
    fn recursively_overrides(&self, derived: DeclId, base: DeclId) -> bool {
        let base = self.ast.first(base);
        let mut stack = self.overridden_methods(derived);
        let mut seen = Vec::new();
        while let Some(o) = stack.pop() {
            if o == base {
                return true;
            }
            if seen.contains(&o) {
                continue;
            }
            seen.push(o);
            stack.extend(self.overridden_methods(o));
        }
        false
    }

    /// clang `getCorrespondingMethodDeclaredInClass(RD, false)`.
    fn corresponding_declared_in(&self, md: DeclId, rd: DeclId) -> Option<DeclId> {
        let ast = self.ast;
        let md = ast.first(md);
        let def = self.class_def(rd)?;
        if ast.decls.get(md).ctx().map(|c| ast.first(c)) == Some(ast.first(rd)) {
            return Some(md);
        }
        if ast.decls.get(md).kind() == DeclKind::CXXDestructor {
            let d = ast.destructor(def)?;
            return self.recursively_overrides(d, md).then(|| ast.first(d));
        }
        ast.decls
            .members(def)
            .filter(|&m| ast.decls.get(m).kind() == DeclKind::CXXMethod)
            .find(|&m| self.recursively_overrides(m, md))
            .map(|m| ast.first(m))
    }

    /// clang `CXXMethodDecl::getCorrespondingMethodInClass(RD, false)`.
    pub(crate) fn corresponding_method(&self, md: DeclId, rd: DeclId) -> Option<DeclId> {
        if let Some(m) = self.corresponding_declared_in(md, rd) {
            return Some(m);
        }
        let ast = self.ast;
        let def = self.class_def(rd)?;
        let mut finals: Vec<DeclId> = Vec::new();
        let bases: Vec<QualType> = ast.decls.bases(def, &ast.lists).map(|b| b.ty).collect();
        for bq in bases {
            let Some(b) = ast.types.as_record(bq) else { continue };
            let Some(d) = self.corresponding_method(md, b) else { continue };
            if finals.iter().any(|&o| o == d || self.recursively_overrides(o, d)) {
                continue;
            }
            finals.retain(|&o| !self.recursively_overrides(d, o));
            finals.push(d);
        }
        (finals.len() == 1).then(|| finals[0])
    }

    /// clang `Expr::IgnoreParenBaseCasts`.
    pub(crate) fn ignore_paren_base_casts(&self, mut e: ExprId) -> ExprId {
        use kiln_ast::expr::{CastKind, ExprKind};
        let ast = self.ast;
        loop {
            let k = ast.exprs.get(e).kind();
            if k == ExprKind::ParenExpr
                || (k.is_cast()
                    && matches!(
                        ast.exprs.cast_kind(e),
                        CastKind::DerivedToBase | CastKind::UncheckedDerivedToBase | CastKind::NoOp
                    ))
            {
                e = ast.exprs.operand(e);
            } else {
                return e;
            }
        }
    }

    /// clang `getCXXRecord(E)`: the class of `e`'s type or pointee type.
    pub(crate) fn expr_class(&self, e: ExprId) -> Option<DeclId> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        let q = ast.types.pointee(q).unwrap_or(q);
        ast.types.as_record(q).map(|r| ast.first(r))
    }

    /// clang `Expr::getBestDynamicClassType`.
    fn best_dynamic_class(&self, e: ExprId) -> Option<DeclId> {
        let e = self.ignore_paren_base_casts(e);
        let rd = self.expr_class(e)?;
        self.class_def(rd).map(|_| rd)
    }

    /// The class is effectively final (clang `isEffectivelyFinal`).
    pub(crate) fn effectively_final(&self, rd: DeclId) -> bool {
        self.class_def(rd).is_some_and(|d| self.class_is_final(d) || self.dtor_final(d))
    }

    fn method_final(&self, m: DeclId) -> bool {
        let ast = self.ast;
        ast.redecls_of(m).any(|r| ast.decls.function(r).is_some_and(|f| f.spec.is_final()))
    }

    fn method_pure(&self, m: DeclId) -> bool {
        let ast = self.ast;
        ast.redecls_of(m).any(|r| ast.decls.function(r).is_some_and(|f| f.spec.is_pure()))
    }

    /// clang `CXXMethodDecl::getDevirtualizedMethod(Base, false)` reduced
    /// to whether it is non-null.
    fn can_devirtualize(&self, md: DeclId, base: ExprId) -> bool {
        use kiln_ast::expr::ExprKind;
        let ast = self.ast;
        if self.method_final(md) {
            return !self.method_pure(md);
        }
        let b = self.ignore_paren_base_casts(base);
        let bn = ast.exprs.get(b);
        if bn.value_category() == kiln_ast::expr::ValueCategory::PRValue
            && ast.types.as_record(bn.ty()).is_some()
        {
            return true;
        }
        let Some(best) = self.best_dynamic_class(base) else { return false };
        let Some(dm) = self.corresponding_method(md, best) else { return false };
        if self.method_pure(dm) {
            return false;
        }
        if self.method_final(dm) || self.effectively_final(best) {
            return true;
        }
        match bn.kind() {
            ExprKind::DeclRefExpr => {
                let d = ast.exprs.decl(b);
                matches!(ast.decls.get(d).kind(), DeclKind::Var | DeclKind::ParmVar)
                    && ast.decls.get(d).ty().is_some_and(|q| ast.types.as_record(q).is_some())
            }
            ExprKind::MemberExpr => {
                let d = ast.exprs.decl(b);
                ast.decls.get(d).ty().is_some_and(|q| ast.types.as_record(q).is_some())
            }
            ExprKind::BinaryOperator if ast.exprs.binary_op(b).is_ptr_mem() => {
                let [_, r] = ast.exprs.operands(b);
                self.mp_parts(ast.exprs.get(r).ty())
                    .is_some_and(|(p, _)| ast.types.as_record(p).is_some())
            }
            _ => false,
        }
    }

    /// clang `EmitCXXMemberOrOperatorMemberCallExpr`'s devirtualization:
    /// the method to call directly and the (possibly stripped) object
    /// expression, or `None` for a virtual call.
    pub(crate) fn devirtualize(&self, md: DeclId, base: ExprId) -> Option<(DeclId, ExprId)> {
        if !self.can_devirtualize(md, base) {
            return None;
        }
        let ast = self.ast;
        let best = self.best_dynamic_class(base)?;
        let dm = self.corresponding_method(md, best)?;
        let dclass = ast.decls.get(dm).ctx().map(|c| ast.first(c));
        let ret = |m: DeclId| {
            ast.decls
                .get(m)
                .ty()
                .and_then(|t| ast.types.proto(t).copied())
                .map(|p| ast.types.canonical(p.ret()))
        };
        if ret(dm) != ret(md) {
            return None;
        }
        let inner = self.ignore_paren_base_casts(base);
        if self.expr_class(inner) == dclass {
            return Some((dm, inner));
        }
        if self.expr_class(base) != dclass {
            return None;
        }
        Some((dm, base))
    }

    // ---- RTTI ----------------------------------------------------------

    /// clang `GetAddrOfRTTIDescriptor(Ty, /*ForEH=*/false)`: the RTTI a
    /// vtable slot, `typeid` or `dynamic_cast` refers to; a null pointer
    /// under `-fno-rtti` (clang `shouldEmitRTTI`).
    pub(crate) fn rtti_descriptor(&mut self, q: QualType) -> R<Value> {
        if !self.opts.rtti {
            return Ok(self.m.const_null(LTy::PTR));
        }
        self.rtti(q)
    }

    /// clang `ItaniumRTTIBuilder::BuildTypeInfo`, also
    /// `GetAddrOfRTTIDescriptor(Ty, /*ForEH=*/true)`: exception handling
    /// (`throw`, handlers) needs a `type_info` even under `-fno-rtti`.
    pub(crate) fn rtti(&mut self, q: QualType) -> R<Value> {
        let ast = self.ast;
        let q = QualType::unqual(ast.types.canonical(q).ty());
        let name = mangle::mangle_rtti(ast, &mut self.anon, q, "_ZTI");
        if let Some(&g) = self.rtti_globals.get(&name) {
            return Ok(Value::global(g));
        }
        if let Some(rd) = ast.types.as_record(q) {
            // clang `ShouldUseExternalRTTIDescriptor`: under `-fno-rtti`
            // the key function's TU may not define it either, so it is
            // emitted here.
            if self.opts.rtti
                && self.is_dynamic(rd)
                && self.vtable_linkage(rd) == VtLinkage::External
            {
                return Ok(self.external_rtti(&name, Some(rd)));
            }
            return self.class_rtti(q, rd, &name);
        }
        if self.is_std_lib_rtti(q) {
            return Ok(self.external_rtti(&name, None));
        }
        let abi = match ast.types.canonical_view(q) {
            TypeView::Pointer(p) => return self.pointer_rtti(q, p, &name),
            TypeView::MemberPointer { pointee, class } => {
                return self.member_pointer_rtti(q, pointee, QualType::unqual(class), &name);
            }
            TypeView::Enum(_) => "_ZTVN10__cxxabiv116__enum_type_infoE",
            TypeView::Function(_) => "_ZTVN10__cxxabiv120__function_type_infoE",
            TypeView::ConstantArray(_) | TypeView::IncompleteArray(_) => {
                "_ZTVN10__cxxabiv117__array_type_infoE"
            }
            // clang `BuildTypeInfo`: vector and complex types are not
            // in the C++ runtime, so their `__fundamental_type_info` is
            // emitted where used, like a class's.
            TypeView::Vector { .. } | TypeView::Complex(_) => {
                "_ZTVN10__cxxabiv123__fundamental_type_infoE"
            }
            _ => return err(None, format!("RTTI for type `{name}` is not supported yet")),
        };
        let pre = self.reserve_rtti(&name);
        let linkage = self.type_rtti_linkage(q);
        let fields = vec![self.abi_vtable(abi)?, self.rtti_type_name(q, linkage)];
        self.finish_rtti(pre, &name, &fields, linkage, q)
    }

    /// clang `ContainsIncompleteClassType`.
    fn contains_incomplete_class(&self, q: QualType) -> bool {
        match self.ast.types.canonical_view(q) {
            TypeView::Record(rd) => self.class_def(rd).is_none(),
            TypeView::Pointer(p) => self.contains_incomplete_class(p),
            TypeView::MemberPointer { pointee, class } => {
                self.contains_incomplete_class(QualType::unqual(class))
                    || self.contains_incomplete_class(pointee)
            }
            _ => false,
        }
    }

    /// clang `getTypeInfoLinkage` for a non-class type `q`.
    fn type_rtti_linkage(&self, q: QualType) -> VtLinkage {
        if self.contains_incomplete_class(q) {
            return VtLinkage::Internal;
        }
        let ast = self.ast;
        let internal = |d: Option<DeclId>| d.is_some_and(|d| mangle::class_is_internal(ast, d));
        let mut t = q;
        loop {
            // (`pointee` also looks through member pointers.)
            if let TypeView::MemberPointer { class, .. } = ast.types.canonical_view(t)
                && internal(ast.types.as_record(QualType::unqual(class)))
            {
                return VtLinkage::Internal;
            }
            match ast.types.pointee(t) {
                Some(p) => t = p,
                None => break,
            }
        }
        if internal(ast.types.as_record(t).or_else(|| ast.types.as_enum(t))) {
            return VtLinkage::Internal;
        }
        VtLinkage::LinkOnce
    }

    /// clang `extractPBaseFlags`: the qualifier, incomplete-class (8) and
    /// noexcept (64) flags of a pointee, and its `type_info` (a noexcept
    /// function type's is that of the function type without `noexcept`).
    fn pointee_rtti(&mut self, pointee: QualType) -> R<(u32, Value)> {
        let ast = self.ast;
        let mut flags = u32::from(ast.types.effective_quals(pointee).bits());
        if self.contains_incomplete_class(pointee) {
            flags |= 8;
        }
        let nothrow = ast.types.proto(pointee).is_some_and(|p| ast.types.exc_spec(p).is_nothrow());
        if !nothrow {
            return Ok((flags, self.rtti(pointee)?));
        }
        flags |= 64;
        // The mangling of the function type without its exception
        // specification: drop the `Do` before `F`, after the method
        // qualifiers of an abominable function type (`KDoFvvRE`).
        let q = QualType::unqual(ast.types.canonical(pointee).ty());
        let with = mangle::mangle_rtti(ast, &mut self.anon, q, "_ZTI");
        let body = with.strip_prefix("_ZTI").unwrap_or("");
        let cv = body.len() - body.trim_start_matches(['r', 'V', 'K']).len();
        let name = match body[cv..].strip_prefix("Do") {
            Some(rest) => format!("_ZTI{}{rest}", &body[..cv]),
            None => return err(None, format!("unexpected noexcept function mangling `{with}`")),
        };
        if let Some(&g) = self.rtti_globals.get(&name) {
            return Ok((flags, Value::global(g)));
        }
        let pre = self.reserve_rtti(&name);
        let linkage = self.type_rtti_linkage(q);
        let mut fields = vec![self.abi_vtable("_ZTVN10__cxxabiv120__function_type_infoE")?];
        fields.push(self.rtti_name_global(&format!("_ZTS{}", &name[4..]), linkage, q));
        Ok((flags, self.finish_rtti(pre, &name, &fields, linkage, q)?))
    }

    /// clang `BuildPointerToMemberTypeInfo`: `{ vtable, name, i32 flags,
    /// pointee, class }`; flag 16 marks an incomplete class.
    fn member_pointer_rtti(
        &mut self,
        q: QualType,
        pointee: QualType,
        class: QualType,
        name: &str,
    ) -> R<Value> {
        let pre = self.reserve_rtti(name);
        let linkage = self.type_rtti_linkage(q);
        let mut fields =
            vec![self.abi_vtable("_ZTVN10__cxxabiv129__pointer_to_member_type_infoE")?];
        fields.push(self.rtti_type_name(q, linkage));
        let (mut flags, pti) = self.pointee_rtti(pointee)?;
        if self.contains_incomplete_class(class) {
            flags |= 16;
        }
        fields.push(self.m.const_int(LTy::I32, i128::from(flags)));
        fields.push(pti);
        fields.push(self.rtti(class)?);
        self.finish_rtti(pre, name, &fields, linkage, q)
    }

    /// clang `BuildPointerTypeInfo`: `{ vtable, name, i32 flags, pointee }`.
    fn pointer_rtti(&mut self, q: QualType, pointee: QualType, name: &str) -> R<Value> {
        let pre = self.reserve_rtti(name);
        let linkage = self.type_rtti_linkage(q);
        let mut fields = vec![self.abi_vtable("_ZTVN10__cxxabiv119__pointer_type_infoE")?];
        fields.push(self.rtti_type_name(q, linkage));
        // PTI_Const/Volatile/Restrict are the `Quals` bits.
        let (flags, pti) = self.pointee_rtti(pointee)?;
        fields.push(self.m.const_int(LTy::I32, i128::from(flags)));
        fields.push(pti);
        self.finish_rtti(pre, name, &fields, linkage, q)
    }

    /// clang `GetAddrOfTypeName`: the `_ZTS` string of `q`.
    fn rtti_type_name(&mut self, q: QualType, linkage: VtLinkage) -> Value {
        let ts_name = mangle::mangle_rtti(self.ast, &mut self.anon, q, "_ZTS");
        self.rtti_name_global(&ts_name, linkage, q)
    }

    /// The `_ZTS` global `ts_name` (its string is the mangling after the
    /// prefix).
    fn rtti_name_global(&mut self, ts_name: &str, linkage: VtLinkage, q: QualType) -> Value {
        let ts_name = ts_name.to_owned();
        if let Some(g) = self.m.get_global(&ts_name) {
            return Value::global(g);
        }
        let init = self.m.const_cstring(&ts_name.as_bytes()[4..]);
        let ty = self.m.const_type(init);
        let g = self.m.add_global(&ts_name, ty);
        let _ = self.m.set_initializer(g, Some(init));
        let vis = self.rtti_visibility(q, linkage);
        let gv = self.m.global_mut(g);
        gv.is_constant = true;
        gv.align = Some(1);
        gv.visibility = vis;
        self.set_vague_linkage(g, linkage);
        Value::global(g)
    }

    /// clang 22 `BuildTypeInfo` creates the `_ZTI` global before building
    /// its fields (so it prints before its `_ZTS` name, its `__cxxabiv1`
    /// vtable and its bases' and pointee's descriptors); clang 19 creates
    /// it after them (spec/codegen.md §6a). The reserved placeholder is
    /// retyped by [`Self::finish_rtti`].
    fn reserve_rtti(&mut self, name: &str) -> Option<GlobalId> {
        (self.opts.clang_compat >= 22).then(|| self.m.add_global(name, LTy::PTR))
    }

    /// The `_ZTI` global `name` (the `reserve_rtti` placeholder `pre`, or
    /// a new global) initialized with `fields`.
    fn finish_rtti(
        &mut self,
        pre: Option<GlobalId>,
        name: &str,
        fields: &[Value],
        linkage: VtLinkage,
        q: QualType,
    ) -> R<Value> {
        let init = match self.m.const_anon_struct(fields, false) {
            Ok(v) => v,
            Err(e) => return err(None, format!("bad type_info: {e:?}")),
        };
        let ty = self.m.const_type(init);
        let g = match pre {
            Some(g) => {
                self.m.set_global_value_type(g, ty);
                g
            }
            None => self.m.add_global(name, ty),
        };
        let _ = self.m.set_initializer(g, Some(init));
        let vis = self.rtti_visibility(q, linkage);
        let gv = self.m.global_mut(g);
        gv.is_constant = true;
        gv.align = Some(8);
        gv.visibility = vis;
        self.set_vague_linkage(g, linkage);
        self.rtti_globals.insert(name.to_owned(), g);
        Ok(Value::global(g))
    }

    /// clang `IsStandardLibraryRTTIDescriptor`: fundamental types and
    /// (const) pointers to them.
    fn is_std_lib_rtti(&self, q: QualType) -> bool {
        let ast = self.ast;
        let fund = |q: QualType| match ast.types.canonical_view(q) {
            TypeView::Builtin(b) => !matches!(b, BuiltinType::Dependent),
            _ => false,
        };
        if fund(q) {
            return true;
        }
        match ast.types.canonical_view(q) {
            TypeView::Pointer(p) => {
                let quals = ast.types.effective_quals(p);
                (quals.is_empty() || quals == kiln_ast::types::Quals::CONST) && fund(p)
            }
            _ => false,
        }
    }

    /// clang `BuildTypeInfo`: an RTTI definition of type `q` has `q`'s
    /// type visibility unless its linkage is local.
    fn rtti_visibility(&self, q: QualType, linkage: VtLinkage) -> Visibility {
        if linkage == VtLinkage::Internal { Visibility::Default } else { self.type_visibility(q) }
    }

    /// clang `GetAddrOfExternalRTTIDescriptor`: `external constant ptr`,
    /// with the class `rd`'s explicit visibility.
    fn external_rtti(&mut self, name: &str, rd: Option<DeclId>) -> Value {
        if let Some(g) = self.m.get_global(name) {
            return Value::global(g);
        }
        let vis = rd.map_or(Visibility::Default, |rd| self.class_visibility(rd, false));
        let g = self.m.add_global(name, LTy::PTR);
        let gv = self.m.global_mut(g);
        gv.is_constant = true;
        gv.linkage = Linkage::External;
        gv.visibility = vis;
        Value::global(g)
    }

    /// The `__cxxabiv1` `type_info` vtable `name` (`external global [0 x
    /// ptr]`) plus 2 pointers.
    fn abi_vtable(&mut self, name: &str) -> R<Value> {
        let g = self.m.get_global(name).unwrap_or_else(|| {
            let t = self.m.types.array(LTy::PTR, 0);
            let g = self.m.add_global(name, t);
            self.m.global_mut(g).linkage = Linkage::External;
            g
        });
        let two = self.m.const_int(LTy::I64, 2);
        match self.m.const_gep(LTy::PTR, Value::global(g), &[two], GepFlags::INBOUNDS, None) {
            Ok(v) => Ok(v),
            Err(e) => err(None, format!("bad type_info vtable: {e:?}")),
        }
    }

    /// clang `getTypeInfoLinkage` for class `rd`.
    fn rtti_linkage(&self, rd: DeclId) -> VtLinkage {
        if mangle::class_is_internal(self.ast, rd) || self.class_def(rd).is_none() {
            return VtLinkage::Internal;
        }
        // Without RTTI a `type_info` exists only for exception handling:
        // `linkonce_odr` in every TU that needs one.
        if self.opts.rtti && self.is_dynamic(rd) {
            return match self.vtable_linkage(rd) {
                VtLinkage::Strong => VtLinkage::Strong,
                VtLinkage::Weak => VtLinkage::Weak,
                VtLinkage::Internal => VtLinkage::Internal,
                _ => VtLinkage::LinkOnce,
            };
        }
        VtLinkage::LinkOnce
    }

    /// clang `ItaniumRTTIBuilder::BuildTypeInfo` for class type `q`.
    fn class_rtti(&mut self, q: QualType, rd: DeclId, name: &str) -> R<Value> {
        let pre = self.reserve_rtti(name);
        let ast = self.ast;
        let linkage = self.rtti_linkage(rd);
        let def = self.class_def(rd);
        let bases: Vec<(QualType, bool, bool)> = match def {
            Some(d) => {
                let tag = ast.decls.record(d).map(|r| r.tag);
                ast.decls
                    .bases(d, &ast.lists)
                    .map(|b| {
                        let public = match b.access {
                            Access::Public => true,
                            Access::None => tag != Some(TagKind::Class),
                            _ => false,
                        };
                        (b.ty, b.is_virtual, public)
                    })
                    .collect()
            }
            None => Vec::new(),
        };
        // BuildVTablePointer.
        let single = bases.len() == 1 && {
            let (bq, virt, public) = bases[0];
            let b = ast.types.as_record(bq);
            !virt
                && public
                && b.is_some_and(|b| {
                    self.class_bits_of(b).contains(DefinitionBits::EMPTY)
                        || self.is_dynamic(b) == self.is_dynamic(rd)
                })
        };
        let vt_name = if bases.is_empty() {
            "_ZTVN10__cxxabiv117__class_type_infoE"
        } else if single {
            "_ZTVN10__cxxabiv120__si_class_type_infoE"
        } else {
            "_ZTVN10__cxxabiv121__vmi_class_type_infoE"
        };
        let mut fields = vec![self.abi_vtable(vt_name)?];
        fields.push(self.rtti_type_name(q, linkage));
        if single {
            let b = self.rtti(bases[0].0)?;
            fields.push(b);
        } else if !bases.is_empty() {
            let flags = self.vmi_flags(rd);
            fields.push(self.m.const_int(LTy::I32, i128::from(flags)));
            fields.push(self.m.const_int(LTy::I32, bases.len() as i128));
            for (bq, virt, public) in bases {
                fields.push(self.rtti(bq)?);
                let Some(b) = ast.types.as_record(bq) else {
                    return err(None, "base of a non-class type");
                };
                let off: i64 = if virt {
                    let l = self.vt_layout(rd)?;
                    l.vbase_offset_offsets
                        .iter()
                        .find(|&&(v, _)| ast.first(v) == ast.first(b))
                        .map_or(0, |&(_, o)| o)
                } else {
                    i64::try_from(self.base_offset(rd, b).unwrap_or(0)).unwrap_or(0)
                };
                let mut of = off << 8;
                if virt {
                    of |= 1;
                }
                if public {
                    of |= 2;
                }
                fields.push(self.m.const_int(LTy::I64, i128::from(of)));
            }
        }
        self.finish_rtti(pre, name, &fields, linkage, q)
    }

    /// Linkage, `dso_local` and comdat of an RTTI or vtable definition.
    pub(crate) fn set_vague_linkage(&mut self, g: GlobalId, linkage: VtLinkage) {
        let name = self.m.global(g).name().to_owned();
        let comdat = matches!(linkage, VtLinkage::LinkOnce | VtLinkage::Weak)
            .then(|| self.m.comdat(&name, ComdatKind::Any));
        let gv = self.m.global_mut(g);
        match linkage {
            VtLinkage::Internal => {
                gv.linkage = Linkage::Internal;
                gv.dso_local = false;
            }
            VtLinkage::LinkOnce => {
                gv.linkage = Linkage::LinkOnceOdr;
                gv.dso_local = true;
                gv.comdat = comdat;
            }
            VtLinkage::Weak => {
                gv.linkage = Linkage::WeakOdr;
                gv.dso_local = true;
                gv.comdat = comdat;
            }
            VtLinkage::Strong | VtLinkage::External => {
                gv.linkage = Linkage::External;
                gv.dso_local = true;
            }
            VtLinkage::AvailableExternally => {
                gv.linkage = Linkage::AvailableExternally;
                gv.dso_local = false;
            }
        }
    }

    /// clang `ComputeVMIClassTypeInfoFlags`: 1 (non-diamond repeat) and
    /// 2 (diamond-shaped) over a walk of every base path. clang walks
    /// each path, which is exponential in the depth of a virtual
    /// lattice; kiln expands each class once: a second walk of a class's
    /// bases finds every class in them already seen, so it only adds
    /// 1 for each non-virtual and 2 for each virtual base edge under it
    /// (the class's memoised `edges`).
    fn vmi_flags(&self, rd: DeclId) -> u32 {
        let mut w = VmiWalk::default();
        let mut flags = 0;
        if let Some(def) = self.class_def(rd) {
            let bases: Vec<(QualType, bool)> =
                self.ast.decls.bases(def, &self.ast.lists).map(|b| (b.ty, b.is_virtual)).collect();
            for (bq, virt) in bases {
                flags |= self.vmi_base_flags(bq, virt, &mut w, 0);
            }
        }
        flags
    }

    fn vmi_base_flags(&self, bq: QualType, virt: bool, w: &mut VmiWalk, depth: usize) -> u32 {
        let ast = self.ast;
        let Some(b) = ast.types.as_record(bq).map(|b| ast.first(b)) else { return 0 };
        let mut flags = 0;
        if virt {
            if !w.vb.insert(b) {
                flags |= 2;
            } else if w.nv.contains(&b) {
                flags |= 1;
            }
        } else if !w.nv.insert(b) || w.vb.contains(&b) {
            flags |= 1;
        }
        if depth > 256 {
            return flags;
        }
        if !w.expanded.insert(b) {
            return flags | self.vmi_edges(b, w, depth);
        }
        if let Some(def) = self.class_def(b) {
            let bases: Vec<(QualType, bool)> =
                ast.decls.bases(def, &ast.lists).map(|x| (x.ty, x.is_virtual)).collect();
            for (q, v) in bases {
                flags |= self.vmi_base_flags(q, v, w, depth + 1);
            }
        }
        flags
    }

    /// 1 if a non-virtual and 2 if a virtual base edge lies under class
    /// `b` (within the walk's depth bound), memoised.
    fn vmi_edges(&self, b: DeclId, w: &mut VmiWalk, depth: usize) -> u32 {
        if let Some(&e) = w.edges.get(&b) {
            return e;
        }
        let ast = self.ast;
        let mut e = 0;
        if depth <= 256
            && let Some(def) = self.class_def(b)
        {
            let bases: Vec<(QualType, bool)> =
                ast.decls.bases(def, &ast.lists).map(|x| (x.ty, x.is_virtual)).collect();
            for (q, v) in bases {
                let Some(c) = ast.types.as_record(q).map(|c| ast.first(c)) else { continue };
                e |= if v { 2 } else { 1 };
                if depth < 256 {
                    e |= self.vmi_edges(c, w, depth + 1);
                }
            }
        }
        w.edges.insert(b, e);
        e
    }
}
