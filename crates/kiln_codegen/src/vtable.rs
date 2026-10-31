//! Virtual tables (spec/codegen.md §7.4, WP-080): `_ZTV` vtable groups
//! (clang `ItaniumCXXABI::getAddrOfVTable` / `emitVTableDefinitions` /
//! `CodeGenVTables::createVTableInitializer`), `_ZTI`/`_ZTS` RTTI
//! (`ItaniumRTTIBuilder`), vtable-pointer initialization in structors
//! (`InitializeVTablePointers`) and virtual-call slot loads
//! (`getVirtualFunctionPointer`).

use std::rc::Rc;

use kiln_abi::mangle::{self, Structor};
use kiln_ast::class::{SubobjectId, VTableComponent, VTableLayout};
use kiln_ast::decl::{Access, DeclKind, DefinitionBits, TagKind};
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_llvm::{
    CastOp, ComdatKind, GepFlags, GlobalId, Linkage, TypeId as LTy, UnnamedAddr, Value,
};

use crate::fir::V;
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
    /// `internal` (the class has internal linkage).
    Internal,
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
        let first = self.ast.first(rd);
        if let Some(l) = self.vt_layouts.get(&first) {
            return Ok(Rc::clone(l));
        }
        let l = match kiln_abi::vtable::vtable_layout(self.ast, first, self.target) {
            Ok(l) => Rc::new(l),
            Err(e) => {
                return err(
                    Some(self.ast.decls.get(first).loc()),
                    format!("cannot lay out the vtable: {e:?}"),
                );
            }
        };
        self.vt_layouts.insert(first, Rc::clone(&l));
        Ok(l)
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
        let ast = self.ast;
        let c = ast.first(class);
        let subs = self.tys.layouter().subobjects(layout_class);
        subs.iter()
            .position(|(s, o)| ast.first(s.class) == c && *o == offset)
            .and_then(|i| u16::try_from(i).ok())
            .map(SubobjectId)
    }

    /// The (vtable index, index within that vtable) of the address point
    /// of subobject `sub` in group `l` (clang `getAddressPoint`).
    pub(crate) fn address_point_of(l: &VTableLayout, sub: SubobjectId) -> Option<(u32, u32)> {
        let i = l.address_points.binary_search_by_key(&sub, |&(s, _)| s).ok()?;
        let ap = l.address_points[i].1;
        let k = l.vtable_starts.iter().rposition(|&st| st <= ap)?;
        Some((u32::try_from(k).ok()?, ap - l.vtable_starts[k]))
    }

    /// The number of components of vtable `k` of group `l`.
    pub(crate) fn vtable_size(l: &VTableLayout, k: usize) -> u32 {
        let start = l.vtable_starts[k];
        let end = l.vtable_starts.get(k + 1).copied().unwrap_or_else(|| {
            u32::try_from(l.components.len()).unwrap_or(u32::MAX)
        });
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
    /// unit, if any.
    fn key_function(&self, rd: DeclId) -> Option<(DeclId, Option<DeclId>)> {
        let ast = self.ast;
        let def = self.class_def(rd)?;
        let &k = ast.decls.cold.key_function.get(def)?;
        let body = ast.redecls_of(k).find(|&r| ast.decls.function(r).is_some_and(|f| f.body.is_some()));
        Some((k, body))
    }

    /// clang `getVTableLinkage` / `isVTableExternal` (at `-O0`).
    pub(crate) fn vtable_linkage(&self, rd: DeclId) -> VtLinkage {
        let first = self.ast.first(rd);
        if mangle::has_internal_linkage(self.ast, first) {
            return VtLinkage::Internal;
        }
        if self.is_implicit_instantiation(first) {
            return VtLinkage::LinkOnce;
        }
        match self.key_function(rd) {
            None => VtLinkage::LinkOnce,
            Some((_, None)) => VtLinkage::External,
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
        let name = mangle::mangle_class_special(self.ast, "_ZTV", first);
        let g = self.m.add_global(&name, ty);
        let gv = self.m.global_mut(g);
        gv.is_constant = true;
        gv.linkage = Linkage::External;
        gv.unnamed_addr = UnnamedAddr::Global;
        gv.align = Some(8);
        self.vtables.insert(first, g);
        Ok(g)
    }

    /// clang `EmitDeferredVTables`.
    pub(crate) fn emit_deferred_vtables(&mut self) -> R<()> {
        let cur = std::mem::take(&mut self.deferred_vtables);
        for rd in cur {
            if self.vtable_linkage(rd) != VtLinkage::External {
                self.emit_vtable_def(rd)?;
            }
        }
        Ok(())
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
                let (_, def) = self.key_function(c)?;
                let def = def?;
                Some((def, c))
            })
            .collect();
        classes.sort_unstable();
        for (_, c) in classes {
            if ast.decls.get(c).has(kiln_ast::decl::DeclFlags::INVALID) {
                continue;
            }
            if self.vtable_linkage(c) == VtLinkage::Strong {
                self.emit_vtable_def(c)?;
            }
        }
        Ok(())
    }

    /// clang `ItaniumCXXABI::emitVTableDefinitions` (idempotent).
    pub(crate) fn emit_vtable_def(&mut self, rd: DeclId) -> R<()> {
        let first = self.ast.first(rd);
        let g = self.vtable_global(first)?;
        if self.m.global(g).initializer().is_some() || !self.vt_emitted.insert(first) {
            return Ok(());
        }
        let l = self.vt_layout(first)?;
        let linkage = self.vtable_linkage(first);
        let rd_ty = self.class_type(first);
        let rtti = self.rtti(rd_ty)?;
        let init = self.vtable_init(&l, rtti, first)?;
        self.define_vtable_global(g, init, linkage)
    }

    /// Sets the initializer, linkage and comdat of a vtable (or VTT)
    /// global.
    pub(crate) fn define_vtable_global(
        &mut self,
        g: GlobalId,
        init: Value,
        linkage: VtLinkage,
    ) -> R<()> {
        if let Err(e) = self.m.set_initializer(g, Some(init)) {
            return err(None, format!("bad vtable initializer: {e:?}"));
        }
        self.set_vague_linkage(g, linkage);
        Ok(())
    }

    /// The record type of class `rd`.
    pub(crate) fn class_type(&self, rd: DeclId) -> QualType {
        let ast = self.ast;
        let def = self.class_def(rd).unwrap_or(rd);
        ast.decls.get(def).ty().map_or(QualType::ERROR, |t| t)
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
                let off =
                    l.vbases.iter().find(|&&(x, _)| ast.first(x) == b).map_or(0, |&(_, o)| o);
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
            let vb = vp.nearest_vbase.expect("checked");
            let p = self.vbase_ptr(this, vtable_class, vb)?;
            let a = min_align(this.align, 0);
            (self.byte_offset(p, vp.offset_from_vbase), a)
        } else {
            (self.byte_offset(this.v, vp.offset), min_align(this.align, vp.offset))
        };
        self.store(ap, field.0, field.1, false);
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
        let fields: Vec<DeclId> =
            ast.decls.members(def).filter(|&m| ast.decls.get(m).kind() == DeclKind::Field).collect();
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
        false
    }

    /// The address point loaded from the VTT parameter.
    fn vtt_address_point(&mut self, _vtable_class: DeclId, vp: VPtr) -> R<V> {
        err(Some(self.ast.decls.get(vp.class).loc()), "VTT parameters are not supported yet (WP-080)")
    }

    /// The address of virtual base `vb` of the object at `this`.
    fn vbase_ptr(&mut self, _this: crate::func::Addr, _rd: DeclId, vb: DeclId) -> R<V> {
        err(Some(self.ast.decls.get(vb).loc()), "virtual base access is not supported yet (WP-080)")
    }

    /// A thunk in a vtable slot.
    fn thunk_for_vtable(
        &mut self,
        m: DeclId,
        _v: Option<Structor>,
        _th: kiln_ast::class::Thunk,
        _layout_class: DeclId,
    ) -> R<Value> {
        err(Some(self.ast.decls.get(m).loc()), "thunks are not supported yet (WP-080)")
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
        let i = self.i64c(i128::from(idx));
        let slot = self.gep(LTy::PTR, vt, vec![i], true);
        Ok(self.load(LTy::PTR, slot, 8, false))
    }

    // ---- devirtualization ----------------------------------------------

    /// Methods `m` overrides directly (first declarations).
    pub(crate) fn overridden_methods(&self, m: DeclId) -> Vec<DeclId> {
        let ast = self.ast;
        let list = ast
            .decls
            .cold
            .overrides
            .get(ast.first(m))
            .copied()
            .unwrap_or(kiln_ast::ListId::EMPTY);
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
            if k == ExprKind::ParenExpr {
                e = ast.exprs.operand(e);
            } else if k.is_cast()
                && matches!(
                    ast.exprs.cast_kind(e),
                    CastKind::DerivedToBase | CastKind::UncheckedDerivedToBase | CastKind::NoOp
                )
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

    /// clang `GetAddrOfRTTIDescriptor(Ty)`.
    pub(crate) fn rtti(&mut self, q: QualType) -> R<Value> {
        let ast = self.ast;
        let q = QualType::unqual(ast.types.canonical(q).ty());
        let name = mangle::mangle_rtti(ast, q, "_ZTI");
        if let Some(&g) = self.rtti_globals.get(&name) {
            return Ok(Value::global(g));
        }
        if let Some(rd) = ast.types.as_record(q) {
            if self.is_dynamic(rd) && self.vtable_linkage(rd) == VtLinkage::External {
                return Ok(self.external_rtti(&name));
            }
            return self.class_rtti(q, rd, &name);
        }
        if self.is_std_lib_rtti(q) {
            return Ok(self.external_rtti(&name));
        }
        err(None, format!("RTTI for type `{name}` is not supported yet"))
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

    /// clang `GetAddrOfExternalRTTIDescriptor`: `external constant ptr`.
    fn external_rtti(&mut self, name: &str) -> Value {
        if let Some(g) = self.m.get_global(name) {
            return Value::global(g);
        }
        let g = self.m.add_global(name, LTy::PTR);
        let gv = self.m.global_mut(g);
        gv.is_constant = true;
        gv.linkage = Linkage::External;
        Value::global(g)
    }

    /// The `__cxxabiv1` type_info vtable `name` (`external global [0 x
    /// ptr]`) plus 2 pointers.
    fn abi_vtable(&mut self, name: &str) -> R<Value> {
        let g = match self.m.get_global(name) {
            Some(g) => g,
            None => {
                let t = self.m.types.array(LTy::PTR, 0);
                let g = self.m.add_global(name, t);
                self.m.global_mut(g).linkage = Linkage::External;
                g
            }
        };
        let two = self.m.const_int(LTy::I64, 2);
        match self.m.const_gep(LTy::PTR, Value::global(g), &[two], GepFlags::INBOUNDS, None) {
            Ok(v) => Ok(v),
            Err(e) => err(None, format!("bad type_info vtable: {e:?}")),
        }
    }

    /// clang `getTypeInfoLinkage` for class `rd`.
    fn rtti_linkage(&self, rd: DeclId) -> VtLinkage {
        if mangle::has_internal_linkage(self.ast, self.ast.first(rd)) {
            return VtLinkage::Internal;
        }
        if self.is_dynamic(rd) {
            return match self.vtable_linkage(rd) {
                VtLinkage::Strong => VtLinkage::Strong,
                VtLinkage::Internal => VtLinkage::Internal,
                _ => VtLinkage::LinkOnce,
            };
        }
        VtLinkage::LinkOnce
    }

    /// clang `ItaniumRTTIBuilder::BuildTypeInfo` for class type `q`.
    fn class_rtti(&mut self, q: QualType, rd: DeclId, name: &str) -> R<Value> {
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
        // GetAddrOfTypeName.
        let ts_name = mangle::mangle_rtti(ast, q, "_ZTS");
        let ts = {
            let init = self.m.const_cstring(ts_name[4..].as_bytes());
            let ty = self.m.const_type(init);
            let g = self.m.add_global(&ts_name, ty);
            let _ = self.m.set_initializer(g, Some(init));
            let gv = self.m.global_mut(g);
            gv.is_constant = true;
            gv.align = Some(1);
            self.set_vague_linkage(g, linkage);
            g
        };
        fields.push(Value::global(ts));
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
        let init = match self.m.const_anon_struct(&fields, false) {
            Ok(v) => v,
            Err(e) => return err(None, format!("bad type_info: {e:?}")),
        };
        let ty = self.m.const_type(init);
        let g = self.m.add_global(name, ty);
        let _ = self.m.set_initializer(g, Some(init));
        let gv = self.m.global_mut(g);
        gv.is_constant = true;
        gv.align = Some(8);
        self.set_vague_linkage(g, linkage);
        self.rtti_globals.insert(name.to_owned(), g);
        Ok(Value::global(g))
    }

    /// Linkage, `dso_local` and comdat of an RTTI or vtable definition.
    pub(crate) fn set_vague_linkage(&mut self, g: GlobalId, linkage: VtLinkage) {
        let name = self.m.global(g).name().to_owned();
        let comdat =
            (linkage == VtLinkage::LinkOnce).then(|| self.m.comdat(&name, ComdatKind::Any));
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
            VtLinkage::Strong | VtLinkage::External => {
                gv.linkage = Linkage::External;
                gv.dso_local = true;
            }
        }
    }

    /// clang `ComputeVMIClassTypeInfoFlags`.
    fn vmi_flags(&self, rd: DeclId) -> u32 {
        let mut nv: Vec<DeclId> = Vec::new();
        let mut vb: Vec<DeclId> = Vec::new();
        let mut flags = 0;
        if let Some(def) = self.class_def(rd) {
            let bases: Vec<(QualType, bool)> =
                self.ast.decls.bases(def, &self.ast.lists).map(|b| (b.ty, b.is_virtual)).collect();
            for (bq, virt) in bases {
                flags |= self.vmi_base_flags(bq, virt, &mut nv, &mut vb, 0);
            }
        }
        flags
    }

    fn vmi_base_flags(
        &self,
        bq: QualType,
        virt: bool,
        nv: &mut Vec<DeclId>,
        vb: &mut Vec<DeclId>,
        depth: usize,
    ) -> u32 {
        let ast = self.ast;
        let Some(b) = ast.types.as_record(bq).map(|b| ast.first(b)) else { return 0 };
        let mut flags = 0;
        if virt {
            if vb.contains(&b) {
                flags |= 2;
            } else {
                vb.push(b);
                if nv.contains(&b) {
                    flags |= 1;
                }
            }
        } else if nv.contains(&b) {
            flags |= 1;
        } else {
            nv.push(b);
            if vb.contains(&b) {
                flags |= 1;
            }
        }
        if depth > 256 {
            return flags;
        }
        if let Some(def) = self.class_def(b) {
            let bases: Vec<(QualType, bool)> =
                ast.decls.bases(def, &ast.lists).map(|x| (x.ty, x.is_virtual)).collect();
            for (q, v) in bases {
                flags |= self.vmi_base_flags(q, v, nv, vb, depth + 1);
            }
        }
        flags
    }
}
