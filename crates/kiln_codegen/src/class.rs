//! Classes (spec/codegen.md §7): constructor/destructor variants and
//! their definitions (clang `ItaniumCXXABI::emitCXXStructor`,
//! `EmitConstructorBody`, `EmitDestructorBody`), constructor and
//! destructor calls, member function calls and `this`.
//!
//! Polymorphic classes and virtual bases are WP-080: every path that would
//! need a vtable pointer or a VTT reports an error instead.

use kiln_abi::mangle::{self, Structor};
use kiln_ast::decl::{
    ConstexprKind, CtorInitKind, DeclKind, Defaulted, DefinitionBits, FnStorage, TagKind,
    TemplLinkKind,
};
use kiln_ast::expr::{ConstructionKind, ExprFlags, ExprKind, StorageDuration};
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType, StmtId, TypeId};
use kiln_llvm::{FuncId, IntPredicate, Linkage, TypeId as LTy, UnnamedAddr, Value};

use crate::cleanup::Cleanup;
use crate::fir::{Op, V};
use crate::func::{Addr, min_align};
use crate::module::{Cg, Def, R, Sig, err};

/// A special member function (clang `CXXSpecialMemberKind`, the subset
/// codegen distinguishes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Special {
    CopyCtor,
    MoveCtor,
    CopyAssign,
    MoveAssign,
}

impl Cg<'_> {
    // ---- signatures ----------------------------------------------------

    /// The class of the implicit object parameter of function `d`, if it
    /// has one (constructors, destructors, conversion functions and
    /// implicit-object member functions).
    pub(crate) fn this_class_of(&self, d: DeclId) -> Option<DeclId> {
        let ast = self.ast;
        let n = ast.decls.get(d);
        match n.kind() {
            DeclKind::CXXConstructor | DeclKind::CXXDestructor | DeclKind::CXXConversion => n.ctx(),
            DeclKind::CXXMethod => {
                let f = ast.decls.function(d)?;
                if f.spec.storage() == FnStorage::Static || f.spec.has_explicit_object() {
                    None
                } else {
                    n.ctx()
                }
            }
            _ => None,
        }
    }

    /// The declaration signature of function `first` (clang
    /// `arrangeGlobalDeclaration`; constructors return `void` on both
    /// targets).
    pub(crate) fn decl_sig(&mut self, first: DeclId) -> R<Sig> {
        let ast = self.ast;
        let Some(fty) = ast.decls.get(first).ty() else {
            return err(Some(ast.decls.get(first).loc()), "function without a type");
        };
        let this = self.this_class_of(first);
        Ok(self.sig_this(fty, mangle::is_extern_c(ast, first), this))
    }

    /// The definition of class `rd`.
    pub(crate) fn class_def(&self, rd: DeclId) -> Option<DeclId> {
        self.ast.definition(rd)
    }

    fn class_bits(&self, rd: DeclId) -> DefinitionBits {
        self.class_def(rd)
            .and_then(|d| self.ast.decls.record(d))
            .map_or(DefinitionBits::empty(), |r| r.defn)
    }

    pub(crate) fn is_union(&self, rd: DeclId) -> bool {
        self.class_def(rd)
            .and_then(|d| self.ast.decls.record(d))
            .is_some_and(|r| r.tag == TagKind::Union)
    }

    /// The class has a non-trivial destructor.
    pub(crate) fn has_nontrivial_dtor(&self, rd: DeclId) -> bool {
        self.class_def(rd).is_some() && !self.class_bits(rd).contains(DefinitionBits::TRIVIAL_DTOR)
    }

    /// The class of type `q` if its objects need destruction
    /// (`QualType::isDestructedType` == `DK_cxx_destructor`), looking
    /// through arrays.
    pub(crate) fn destructed_class(&self, q: QualType) -> Option<DeclId> {
        let ast = self.ast;
        let mut q = q;
        while let Some(e) = self.array_elem(q) {
            q = e;
        }
        let rd = ast.types.as_record(q)?;
        self.has_nontrivial_dtor(rd).then_some(rd)
    }

    /// Which copy/move special member `f` is (clang
    /// `isCopyOrMoveConstructor` / `isCopyAssignmentOperator` /
    /// `isMoveAssignmentOperator`).
    pub(crate) fn special_of(&self, f: DeclId) -> Option<Special> {
        let ast = self.ast;
        let n = ast.decls.get(f);
        let kind = n.kind();
        if !matches!(kind, DeclKind::CXXConstructor | DeclKind::CXXMethod) {
            return None;
        }
        let class = ast.first(n.ctx()?);
        let fd = ast.decls.function(f)?;
        if matches!(fd.templ.get(), Some((TemplLinkKind::Pattern, _))) {
            return None;
        }
        let params: Vec<DeclId> = ast.lists.iter(fd.params).collect();
        let pty = |p: DeclId| ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        let has_default = |p: DeclId| ast.decls.param(p).is_some_and(|x| x.default_arg.is_some());
        let is_class = |t: QualType| ast.types.as_record(t).is_some_and(|r| ast.first(r) == class);
        if kind == DeclKind::CXXConstructor {
            let &p0 = params.first()?;
            if !params[1..].iter().all(|&p| has_default(p)) {
                return None;
            }
            return match ast.types.canonical_view(pty(p0)) {
                TypeView::LValueRef(t) if is_class(t) => Some(Special::CopyCtor),
                TypeView::RValueRef(t) if is_class(t) => Some(Special::MoveCtor),
                _ => None,
            };
        }
        if params.len() != 1 || fd.spec.storage() == FnStorage::Static {
            return None;
        }
        if !mangle::decl_name(ast, f).starts_with("operator=")
            || mangle::decl_name(ast, f) != "operator="
        {
            return None;
        }
        match ast.types.canonical_view(pty(params[0])) {
            TypeView::LValueRef(t) if is_class(t) => Some(Special::CopyAssign),
            TypeView::RValueRef(t) if is_class(t) => Some(Special::MoveAssign),
            _ if is_class(pty(params[0])) => Some(Special::CopyAssign),
            _ => None,
        }
    }

    /// clang `isMemcpyEquivalentSpecialMember`.
    pub(crate) fn memcpy_equivalent(&self, f: DeclId) -> bool {
        if self.special_of(f).is_none() {
            return false;
        }
        let Some(fd) = self.ast.decls.function(f) else { return false };
        if fd.spec.is_trivial() {
            return true;
        }
        let class = self.ast.decls.get(f).ctx();
        class.is_some_and(|c| self.is_union(c)) && self.is_defaulted(f)
    }

    fn is_defaulted(&self, f: DeclId) -> bool {
        self.ast.redecls_of(f).any(|r| {
            self.ast.decls.function(r).is_some_and(|x| x.spec.defaulted() != Defaulted::No)
        })
    }

    /// The element type of array type `q`.
    pub(crate) fn array_elem(&self, q: QualType) -> Option<QualType> {
        match self.ast.types.canonical_view(q) {
            TypeView::ConstantArray(a) => Some(a.elem),
            TypeView::IncompleteArray(e) | TypeView::VariableArray { elem: e, .. } => Some(e),
            _ => None,
        }
    }

    // ---- structor variants ---------------------------------------------

    /// The definition of structor `first` to emit, if any.
    fn structor_def(&self, first: DeclId) -> Option<DeclId> {
        let ast = self.ast;
        ast.redecls_of(first).find(|&r| {
            ast.decls.function(r).is_some_and(|f| {
                f.body.is_some()
                    && !matches!(f.templ.get(), Some((TemplLinkKind::Pattern, _)))
                    && f.spec.constexpr_kind() != ConstexprKind::Consteval
            })
        })
    }

    /// The class of structor `d` has virtual bases (its complete and base
    /// variants differ, clang `getCodegenToUse` → `Emit`).
    pub(crate) fn structor_has_vbases(&self, d: DeclId) -> bool {
        self.ast
            .decls
            .get(d)
            .ctx()
            .is_some_and(|c| self.class_bits(c).contains(DefinitionBits::HAS_VIRTUAL_BASES))
    }

    /// Base-object structors of classes with virtual bases take a VTT
    /// parameter, which is not supported yet.
    fn check_structor(&self, d: DeclId, v: Structor) -> R<()> {
        if v == Structor::Base && self.structor_has_vbases(d) {
            return err(
                Some(self.ast.decls.get(d).loc()),
                "base-object constructors and destructors of classes with virtual bases (VTT parameter) are not supported yet (WP-080)",
            );
        }
        Ok(())
    }

    /// Declares structor variant `v` of `first` (clang
    /// `getAddrOfCXXStructor` creating the declaration).
    pub(crate) fn declare_structor(&mut self, first: DeclId, v: Structor) -> R<(Value, FuncId)> {
        if let Some(&r) = self.sfns.get(&(first, v)) {
            return Ok(r);
        }
        let name = mangle::mangle_structor(self.ast, first, v);
        let sig = self.decl_sig(first)?;
        let f = self.declare_fn(&name, first, &sig)?;
        self.m.func_mut(f).unnamed_addr = UnnamedAddr::Global;
        let r = (Value::func(f), f);
        self.sfns.insert((first, v), r);
        Ok(r)
    }

    /// Queues the definition of variant `v` of `first` for emission if
    /// it is discardable and known (clang `GetOrCreateLLVMFunction`
    /// moving a deferred declaration to `DeferredDeclsToEmit`).
    fn queue_structor(&mut self, first: DeclId, v: Structor) {
        if self.squeued.contains(&(first, v)) {
            return;
        }
        let def = match self.sdeferred.remove(&(first, v)) {
            Some(d) => Some(d),
            None => self.inline_def(first),
        };
        if let Some(d) = def {
            self.squeued.insert((first, v));
            self.to_emit.push(Def::Structor(d, v));
        }
    }

    /// The base-object variant of `first`, declared if needed (and its
    /// definition queued when `queue`).
    fn base_fn(&mut self, first: DeclId, queue: bool) -> R<FuncId> {
        let (_, f) = self.declare_structor(first, Structor::Base)?;
        if queue {
            self.queue_structor(first, Structor::Base);
        }
        Ok(f)
    }

    /// Whether the complete-object variant of `first` is discardable,
    /// which makes clang replace it by the base variant (RAUW) rather
    /// than alias it.
    fn complete_is_rauw(&self, def: DeclId) -> bool {
        self.discardable_fn(def)
    }

    /// The callee for variant `v` of structor `d` (clang
    /// `getAddrOfCXXStructor`): the symbol and, when it is known to be the
    /// function itself, its id (call-site `nounwind` bookkeeping).
    pub(crate) fn structor_ref(&mut self, d: DeclId, v: Structor) -> R<(V, Option<FuncId>)> {
        let ast = self.ast;
        let first = ast.first(d);
        if v == Structor::Deleting {
            let (val, f) = self.declare_structor(first, v)?;
            self.queue_structor(first, v);
            return Ok((V::C(val), Some(f)));
        }
        self.check_structor(first, v)?;
        if let Some(&(val, f)) = self.sfns.get(&(first, v)) {
            if v == Structor::Base {
                // Declared by a RAUW'd complete-object reference: a use
                // of the base variant itself queues its definition.
                self.queue_structor(first, v);
            }
            let fid = (val == Value::func(f)
                && (v == Structor::Base || self.structor_has_vbases(first)))
            .then_some(f);
            return Ok((V::C(val), fid));
        }
        if v == Structor::Base {
            let f = self.base_fn(first, true)?;
            return Ok((V::C(Value::func(f)), Some(f)));
        }
        let Some(def) = self.structor_def(first) else {
            let (val, _) = self.declare_structor(first, v)?;
            return Ok((V::C(val), None));
        };
        self.c1_decl.insert(first);
        if self.structor_has_vbases(first) {
            // A separate complete-object function.
            let (val, f) = self.declare_structor(first, v)?;
            self.queue_structor(first, v);
            return Ok((V::C(val), Some(f)));
        }
        if self.complete_is_rauw(def) {
            // The complete-object declaration is later replaced by the
            // base variant, which takes its place in the module.
            let f = self.base_fn(first, false)?;
            self.queue_structor(first, v);
            return Ok((V::C(Value::func(f)), None));
        }
        // An alias defined later in the translation unit.
        let p = if let Some(&p) = self.ph_of.get(&(first, v)) {
            p
        } else {
            let p = u32::try_from(self.phs.len()).expect("placeholder count");
            self.phs.push(None);
            self.ph_of.insert((first, v), p);
            p
        };
        self.f.uses_ph = true;
        Ok((V::P(p), None))
    }

    /// clang `EmitCXXConstructors` / `EmitCXXDestructors` for the
    /// top-level declaration `d`.
    pub(crate) fn emit_global_structor(&mut self, d: DeclId) -> R<()> {
        let ast = self.ast;
        let Some(fd) = ast.decls.function(d) else { return Ok(()) };
        if fd.body.is_none() || matches!(fd.templ.get(), Some((TemplLinkKind::Pattern, _))) {
            return Ok(());
        }
        if fd.spec.constexpr_kind() == ConstexprKind::Consteval {
            return Ok(());
        }
        let first = ast.first(d);
        let is_ctor = ast.decls.get(d).kind() == DeclKind::CXXConstructor;
        let abstract_ = ast
            .decls
            .get(d)
            .ctx()
            .is_some_and(|c| self.class_bits(c).contains(DefinitionBits::ABSTRACT));
        let virtual_dtor = !is_ctor && fd.spec.is_virtual();
        for v in [Structor::Base, Structor::Complete, Structor::Deleting] {
            if is_ctor && v == Structor::Complete && abstract_ {
                continue;
            }
            if v == Structor::Deleting && !virtual_dtor {
                continue;
            }
            if !self.discardable_fn(d) {
                self.check_structor(d, v)?;
                self.emit_structor_def(d, v)?;
                continue;
            }
            let visible = match v {
                Structor::Base | Structor::Deleting => self.sfns.contains_key(&(first, v)),
                _ => self.c1_decl.contains(&first),
            };
            if visible {
                if self.squeued.insert((first, v)) {
                    self.to_emit.push(Def::Structor(d, v));
                }
            } else {
                self.sdeferred.insert((first, v), d);
            }
        }
        Ok(())
    }

    /// clang `emitCXXStructor` for variant `v` of definition `d`
    /// (idempotent).
    pub(crate) fn emit_structor_def(&mut self, d: DeclId, v: Structor) -> R<()> {
        let ast = self.ast;
        let first = ast.first(d);
        self.check_structor(d, v)?;
        match v {
            Structor::Base => {
                self.squeued.insert((first, v));
                let (_, f) = self.declare_structor(first, v)?;
                if !self.m.func(f).is_declaration() || self.sdone.contains(&first) {
                    return Ok(());
                }
                self.sdone.insert(first);
                let saved = std::mem::replace(&mut self.f, crate::func::Fx::empty());
                let r = self.gen_code(d, f, Some(Structor::Base));
                self.f = saved;
                r
            }
            Structor::Complete => {
                if !self.c1_done.insert(first) {
                    return Ok(());
                }
                self.squeued.insert((first, v));
                if self.structor_has_vbases(d) {
                    let (_, f) = self.declare_structor(first, v)?;
                    if !self.m.func(f).is_declaration() {
                        return Ok(());
                    }
                    let saved = std::mem::replace(&mut self.f, crate::func::Fx::empty());
                    let r = self.gen_code(d, f, Some(Structor::Complete));
                    self.f = saved;
                    r?;
                    return self.emit_thunks(d, Some(v));
                }
                if self.complete_is_rauw(d) {
                    let f = self.base_fn(first, true)?;
                    self.sfns.insert((first, v), (Value::func(f), f));
                    if let Some(&p) = self.ph_of.get(&(first, v)) {
                        self.phs[p as usize] = Some(Value::func(f));
                    }
                    return self.emit_thunks(d, Some(v));
                }
                let f = self.base_fn(first, true)?;
                let name = mangle::mangle_structor(ast, first, v);
                let fty = self.m.func(f).ty();
                let a = match self.m.add_alias(&name, fty, Value::func(f)) {
                    Ok(a) => a,
                    Err(e) => {
                        return err(
                            Some(ast.decls.get(d).loc()),
                            format!("cannot define alias {name}: {e:?}"),
                        );
                    }
                };
                {
                    let al = self.m.alias_mut(a);
                    al.linkage = Linkage::External;
                    al.dso_local = true;
                    al.unnamed_addr = UnnamedAddr::Global;
                }
                let val = Value::alias(a);
                self.sfns.insert((first, v), (val, f));
                if let Some(&p) = self.ph_of.get(&(first, v)) {
                    self.phs[p as usize] = Some(val);
                }
                self.emit_thunks(d, Some(v))
            }
            Structor::Deleting => {
                self.squeued.insert((first, v));
                let (_, f) = self.declare_structor(first, v)?;
                if !self.m.func(f).is_declaration() || !self.d0_done.insert(first) {
                    return Ok(());
                }
                let saved = std::mem::replace(&mut self.f, crate::func::Fx::empty());
                let r = self.gen_code(d, f, Some(Structor::Deleting));
                self.f = saved;
                r?;
                self.emit_thunks(d, Some(v))
            }
        }
    }

    // ---- constructor and destructor bodies -----------------------------

    /// `this` as an address of its class (clang `LoadCXXThisAddress`).
    pub(crate) fn this_addr(&mut self) -> Addr {
        let rd = self.f.this_class.expect("member function");
        let ty = self.record_ty(rd);
        let (_, align) = self.this_layout(rd);
        Addr { v: self.this_value(), ty, align }
    }

    /// `CXXThisValue`.
    pub(crate) fn this_value(&self) -> V {
        self.f.this.expect("`this` in a member function")
    }

    /// `p` advanced by `offset` bytes (clang `CreateConstInBoundsByteGEP`).
    pub(crate) fn byte_offset(&mut self, p: V, offset: u64) -> V {
        if offset == 0 {
            return p;
        }
        let o = self.i64c(i128::from(offset));
        self.gep(LTy::I8, p, vec![o], true)
    }

    /// The byte offset of direct non-virtual base `base` in `rd`.
    pub(crate) fn base_offset(&mut self, rd: DeclId, base: DeclId) -> Option<u64> {
        let ast = self.ast;
        let def = self.class_def(rd)?;
        let b = ast.first(base);
        let l = self.tys.layouter().record(def);
        l.bases.iter().find(|&&(x, _)| ast.first(x) == b).map(|&(_, o)| o)
    }

    /// clang `getOverlapForBaseInit`.
    pub(crate) fn base_may_overlap(&mut self, rd: DeclId, base: DeclId, off: u64) -> bool {
        let Some(def) = self.class_def(rd) else { return true };
        let nv = self.tys.layouter().record(def).nv_size;
        let bsize = self.class_def(base).map_or(0, |b| self.tys.layouter().record(b).size);
        off + bsize > nv
    }

    /// clang `EmitConstructorBody` (the base and complete variants share
    /// it: no virtual bases).
    pub(crate) fn emit_ctor_body(&mut self, d: DeclId, body: StmtId) -> R<()> {
        let ast = self.ast;
        let sc = self.begin_scope(false);
        let inits: Vec<kiln_ast::decl::CtorInit> = ast.ctor_inits(d).copied().collect();
        let memcpyize = self.is_defaulted(d)
            && matches!(self.special_of(d), Some(Special::CopyCtor | Special::MoveCtor));
        let rd = self.f.this_class.expect("constructor class");
        let mut run: Vec<(DeclId, u64, u64)> = Vec::new();
        // InitializeVTablePointers runs after the base initializers and
        // before the member initializers (not in delegating constructors).
        let delegating = inits.iter().any(|ci| ci.kind == CtorInitKind::Delegating);
        let mut vptrs_done = delegating;
        for ci in &inits {
            if !vptrs_done && matches!(ci.kind, CtorInitKind::Field | CtorInitKind::IndirectField) {
                self.flush_memcpy_run(d, &mut run)?;
                self.init_vtable_ptrs(rd)?;
                vptrs_done = true;
            }
            match ci.kind {
                CtorInitKind::Base => {
                    self.flush_memcpy_run(d, &mut run)?;
                    let t = QualType::unqual(TypeId::new(ci.target));
                    let Some(base) = ast.types.as_record(t) else {
                        return err(Some(ci.loc), "base initializer of a non-class type");
                    };
                    let Some(off) = self.base_offset(rd, base) else {
                        return err(Some(ci.loc), "base class not in the layout");
                    };
                    let this = self.this_addr();
                    let v = self.byte_offset(this.v, off);
                    let ty = self.record_ty(base);
                    let dest = Addr { v, ty, align: min_align(this.align, off) };
                    let overlap = self.base_may_overlap(rd, base, off);
                    self.emit_agg_into(ci.init, dest, overlap)?;
                    if self.opts.exceptions && self.has_nontrivial_dtor(base) {
                        self.f.eh_only = true;
                    }
                }
                CtorInitKind::VirtualBase => {
                    // clang `EmitBaseInitializer`: only the complete
                    // variant of a non-abstract class constructs them.
                    if self.f.structor == Some(Structor::Base)
                        || self.class_bits(rd).contains(DefinitionBits::ABSTRACT)
                    {
                        continue;
                    }
                    self.flush_memcpy_run(d, &mut run)?;
                    let t = QualType::unqual(TypeId::new(ci.target));
                    let Some(base) = ast.types.as_record(t) else {
                        return err(Some(ci.loc), "base initializer of a non-class type");
                    };
                    let Some(off) = self.vbase_offset(rd, base) else {
                        return err(Some(ci.loc), "virtual base not in the layout");
                    };
                    let this = self.this_addr();
                    let v = self.byte_offset(this.v, off);
                    let ty = self.record_ty(base);
                    let dest = Addr { v, ty, align: min_align(this.align, off) };
                    self.emit_agg_into(ci.init, dest, true)?;
                    if self.opts.exceptions && self.has_nontrivial_dtor(base) {
                        self.f.eh_only = true;
                    }
                }
                CtorInitKind::Delegating => {
                    self.flush_memcpy_run(d, &mut run)?;
                    let this = self.this_addr();
                    self.emit_agg_into(ci.init, this, false)?;
                }
                CtorInitKind::Field | CtorInitKind::IndirectField => {
                    let field = DeclId::new(ci.target);
                    if memcpyize
                        && ci.kind == CtorInitKind::Field
                        && let Some(span) = self.memcpyable_field(rd, field, ci.init)
                    {
                        run.push((field, span.0, span.1));
                        continue;
                    }
                    self.flush_memcpy_run(d, &mut run)?;
                    self.emit_field_init(ci.kind, field, ci.init)?;
                }
            }
        }
        self.flush_memcpy_run(d, &mut run)?;
        if !vptrs_done {
            self.init_vtable_ptrs(rd)?;
        }
        self.emit_stmt(body)?;
        self.end_scope(sc);
        Ok(())
    }

    /// The byte range `[first, last)` of field `field` of `rd` if clang's
    /// `ConstructorMemcpyizer` may copy it with a memcpy.
    fn memcpyable_field(&mut self, rd: DeclId, field: DeclId, init: ExprId) -> Option<(u64, u64)> {
        let ast = self.ast;
        let q = ast.decls.get(field).ty()?;
        if ast.types.effective_quals(q).is_volatile() {
            return None;
        }
        let init_ok = {
            let mut e = init;
            while matches!(ast.exprs.get(e).kind(), ExprKind::ExprWithCleanups) {
                e = self.sub(e);
            }
            let n = ast.exprs.get(e);
            if matches!(n.kind(), ExprKind::CXXConstructExpr) {
                let c = ast.exprs.constructor(e);
                ast.decls.function(c).is_some_and(|f| f.spec.is_trivial())
                    && matches!(self.special_of(c), Some(Special::CopyCtor | Special::MoveCtor))
            } else {
                false
            }
        };
        if !init_ok && self.referee(q).is_none() && !self.trivially_copyable(q) {
            return None;
        }
        let def = self.class_def(rd)?;
        let idx = self.field_number(def, field)?;
        let bit = self.tys.layouter().record(def).fields[idx];
        if self.tys.record(def).bitfields.contains_key(&field) {
            return None;
        }
        let (s, _) = self.size_align(q);
        let size = match ast.types.as_record(q).and_then(|r| self.class_def(r)) {
            Some(r) if self.array_elem(q).is_none() => self.tys.layouter().record(r).data_size,
            _ => s,
        };
        let start = bit / 8;
        Some((start, start + size))
    }

    /// The declaration-order index of field `field` in `def`.
    pub(crate) fn field_number(&self, def: DeclId, field: DeclId) -> Option<usize> {
        let ast = self.ast;
        ast.decls
            .members(def)
            .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
            .position(|m| m == field)
    }

    /// `QualType::isTriviallyCopyableType` for a field.
    fn trivially_copyable(&self, q: QualType) -> bool {
        let ast = self.ast;
        let mut q = q;
        while let Some(e) = self.array_elem(q) {
            q = e;
        }
        match ast.types.as_record(q) {
            Some(r) => self.class_bits(r).contains(DefinitionBits::TRIVIALLY_COPYABLE),
            None => true,
        }
    }

    /// clang `ConstructorMemcpyizer::emitMemcpy`: a run of two or more
    /// memcpyable fields is one memcpy; a single one is initialized
    /// normally.
    fn flush_memcpy_run(&mut self, ctor: DeclId, run: &mut Vec<(DeclId, u64, u64)>) -> R<()> {
        let ast = self.ast;
        if run.is_empty() {
            return Ok(());
        }
        let items = std::mem::take(run);
        if items.len() == 1 {
            let (field, _, _) = items[0];
            let ci = ast
                .ctor_inits(ctor)
                .find(|c| c.kind == CtorInitKind::Field && DeclId::new(c.target) == field)
                .copied()
                .expect("field initializer");
            return self.emit_field_init(ci.kind, field, ci.init);
        }
        let start = items[0].1;
        let end = items.last().expect("run").2;
        let first = items[0].0;
        let fq = ast.decls.get(first).ty().expect("field type");
        // clang `emitMemcpy`: the destination and the source are the first
        // field's lvalues (struct GEPs), destination first.
        let this = self.this_addr();
        let dst = self.field_lvalue_init(this, first, fq)?.addr;
        let fd = ast.decls.function(ctor).expect("constructor");
        let p0: DeclId = ast.lists.iter(fd.params).next().expect("copy parameter");
        let slot = *self.f.locals.get(&p0).expect("parameter slot");
        let src_obj = self.load(LTy::PTR, slot.v, slot.align, false);
        let pq = ast.decls.get(p0).ty().expect("param type");
        let src_q = self.referee(pq).unwrap_or(pq);
        let (_, src_align0) = self.pointee_layout(src_q);
        let rd = ast.decls.get(first).ctx().expect("field parent");
        let rty = self.record_ty(rd);
        let src_base = Addr { v: src_obj, ty: rty, align: src_align0 };
        let src = self.field_lvalue_init(src_base, first, fq)?.addr;
        let dst = Addr { v: dst.v, ty: LTy::I8, align: dst.align };
        self.memcpy(dst, src.v, src.align, end - start, false);
        Ok(())
    }

    /// clang `EmitMemberInitializer` / `EmitInitializerForField`.
    fn emit_field_init(&mut self, kind: CtorInitKind, field: DeclId, init: ExprId) -> R<()> {
        let ast = self.ast;
        let this = self.this_addr();
        let lv = if kind == CtorInitKind::IndirectField {
            let chain: Vec<DeclId> = ast.lists.iter(ast.indirect_field_chain(field)).collect();
            let mut base = this;
            let mut lv = None;
            for f in chain {
                let q = ast.decls.get(f).ty().expect("field type");
                let l = self.field_lvalue_init(base, f, q)?;
                base = l.addr;
                lv = Some(l);
            }
            lv.expect("indirect field chain")
        } else {
            let q = ast.decls.get(field).ty().expect("field type");
            self.field_lvalue_init(this, field, q)?
        };
        let fq = lv.q;
        if self.referee(fq).is_some() {
            let p = self.emit_ref_binding(init)?;
            self.store(p, lv.addr.v, lv.addr.align, false);
            return Ok(());
        }
        if self.is_aggregate(fq) {
            let overlap = false;
            return self.emit_agg_into(init, lv.addr, overlap);
        }
        self.init_to_lvalue(init, lv, false)
    }

    /// clang `EmitDestructorBody` for the base variant (the complete
    /// variant is an alias or replaced by it: no virtual bases).
    pub(crate) fn emit_dtor_body(&mut self, d: DeclId, body: StmtId) -> R<()> {
        let ast = self.ast;
        let sc = self.begin_scope(false);
        let rd = self.f.this_class.expect("destructor class");
        if !self.is_union(rd) {
            let def = self.class_def(rd).expect("class definition");
            let bases: Vec<(QualType, bool)> =
                ast.decls.bases(def, &ast.lists).map(|b| (b.ty, b.is_virtual)).collect();
            for (bq, virt) in bases {
                if virt {
                    continue;
                }
                let Some(b) = ast.types.as_record(bq) else { continue };
                if !self.has_nontrivial_dtor(b) {
                    continue;
                }
                let Some(off) = self.base_offset(rd, b) else { continue };
                self.push_cleanup(Cleanup::BaseOfThis { base: b, offset: off });
            }
            let fields: Vec<DeclId> = ast
                .decls
                .members(def)
                .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
                .collect();
            for f in fields {
                let Some(q) = ast.decls.get(f).ty() else { continue };
                // Members of anonymous unions are never destroyed.
                if ast.types.as_record(q).is_some_and(|r| {
                    self.is_union(r)
                        && matches!(
                            ast.names.view(ast.decls.get(f).name()),
                            kiln_ast::names::DeclNameView::Empty
                        )
                }) {
                    continue;
                }
                if self.destructed_class(q).is_some() {
                    self.push_cleanup(Cleanup::Field { field: f, record: rd });
                }
            }
        }
        if !self.can_skip_vptr_init(d, rd) {
            self.init_vtable_ptrs(rd)?;
        }
        self.emit_stmt(body)?;
        self.end_scope(sc);
        Ok(())
    }

    /// clang `EmitDestructorBody` for the deleting variant: the
    /// complete-object destructor, then the class's deallocation function
    /// (`CallDtorDelete`).
    pub(crate) fn emit_deleting_dtor_body(&mut self, d: DeclId) -> R<()> {
        let ast = self.ast;
        let rd = self.f.this_class.expect("destructor class");
        let loc = Some(ast.decls.get(d).loc());
        let this = self.this_addr();
        // EmitDestructorBody: an abstract class's D0 can never be called.
        if self.class_bits_of(rd).contains(DefinitionBits::ABSTRACT) {
            self.emit_trap_unreachable();
            return Ok(());
        }
        let fty = ast.decls.get(d).ty().expect("destructor type");
        let (callee, fid) = self.structor_ref(d, Structor::Complete)?;
        let sig = self.call_sig_this(fty, false, &[], Some(rd));
        self.call_vals(&sig, callee, fid, Some(this.v), &[], &[], None, &[])?;
        let Some(&od) = ast.decls.cold.dtor_delete.get(ast.first(d)) else {
            return err(loc, "virtual destructor without a deallocation function");
        };
        let ty = self.class_type(rd);
        self.emit_delete_call(od, this.v, ty, loc)
    }

    /// `DestroyField`: destroys field `field` of `*this` (class `record`).
    pub(crate) fn destroy_field_of_this(&mut self, field: DeclId, record: DeclId) {
        let ast = self.ast;
        let _ = record;
        let this = self.this_addr();
        let q = ast.decls.get(field).ty().expect("field type");
        let lv = match self.field_lvalue(this, field, q) {
            Ok(lv) => lv,
            Err(e) => {
                self.f.error.get_or_insert(e);
                return;
            }
        };
        self.destroy_object(lv.addr, q);
    }

    /// clang `emitDestroy` with `destroyCXXObject`.
    pub(crate) fn destroy_object(&mut self, a: Addr, q: QualType) {
        let Some(rd) = self.destructed_class(q) else { return };
        if self.array_elem(q).is_some() {
            self.array_destroy_at(a, rd);
        } else {
            self.call_dtor(rd, a.v, false);
        }
    }

    /// `emitArrayLength` + `emitArrayDestroy` for the constant-length
    /// array at `a` (IR type `a.ty`) of objects of class `rd`.
    pub(crate) fn array_destroy_at(&mut self, a: Addr, rd: DeclId) {
        let mut idx = vec![self.icn(LTy::I32, 0)];
        let mut t = a.ty;
        let mut n = 1u64;
        while let Some((e, len)) = self.m.types.array_info(t) {
            idx.push(self.icn(LTy::I32, 0));
            n *= len;
            t = e;
        }
        if n == 0 {
            return;
        }
        let begin = self.gep(a.ty, a.v, idx, true);
        self.array_destroy(begin, n, t, rd);
    }

    /// Calls the destructor of `record` on the object at `addr` (`base`:
    /// the base-object variant). Errors go to `f.error` (cleanups cannot
    /// fail).
    pub(crate) fn call_dtor(&mut self, record: DeclId, addr: V, base: bool) {
        if let Err(e) = self.try_call_dtor(record, addr, base) {
            self.f.error.get_or_insert(e);
        }
    }

    fn try_call_dtor(&mut self, record: DeclId, addr: V, base: bool) -> R<()> {
        let ast = self.ast;
        let Some(def) = self.class_def(record) else { return Ok(()) };
        if !self.has_nontrivial_dtor(def) {
            return Ok(());
        }
        let Some(dtor) = ast.destructor(def) else {
            return err(Some(ast.decls.get(def).loc()), "destructor not declared");
        };
        let v = if base { Structor::Base } else { Structor::Complete };
        let (callee, fid) = self.structor_ref(dtor, v)?;
        let first = ast.first(dtor);
        let fty = ast.decls.get(first).ty().expect("destructor type");
        let sig = self.call_sig_this(fty, false, &[], Some(def));
        self.call_vals(&sig, callee, fid, Some(addr), &[], &[], None, &[])?;
        Ok(())
    }

    // ---- aggregate destinations ----------------------------------------

    /// `emit_agg` into a slot that is externally destructed
    /// (`AggValueSlot::IsDestructed`) with the given overlap.
    pub(crate) fn emit_agg_into(&mut self, e: ExprId, dest: Addr, overlap: bool) -> R<()> {
        let saved = (self.f.dest_destructed, self.f.overlap);
        self.f.dest_destructed = true;
        self.f.overlap = overlap;
        let r = self.emit_agg(e, dest, false);
        self.f.dest_destructed = saved.0;
        self.f.overlap = saved.1;
        r
    }

    /// The return-slot initialization of `return e;`.
    pub(crate) fn emit_agg_slot(
        &mut self,
        e: ExprId,
        dest: Addr,
        overlap: bool,
        _zeroed: bool,
    ) -> R<()> {
        self.emit_agg_into(e, dest, overlap)
    }

    /// Binds a reference to `e` (clang `EmitReferenceBindingToExpr`): the
    /// address of the glvalue, or of a materialized temporary.
    pub(crate) fn emit_ref_binding(&mut self, e: ExprId) -> R<V> {
        let ast = self.ast;
        if ast.exprs.get(e).kind() == ExprKind::ExprWithCleanups {
            let sc = self.begin_scope(false);
            let r = self.emit_ref_binding(self.sub(e));
            self.end_scope(sc);
            return r;
        }
        if self.is_glvalue(e) {
            return Ok(self.emit_lvalue(e)?.addr.v);
        }
        err(self.loc_of(e), "binding a reference to a prvalue is not supported yet")
    }

    /// clang `pushTemporaryCleanup` after materializing the temporary
    /// `m` at `a`.
    pub(crate) fn push_temporary_cleanup(&mut self, m: ExprId, a: Addr) {
        let ast = self.ast;
        let q = ast.exprs.get(m).ty();
        let Some(rd) = self.destructed_class(q) else { return };
        let c = if self.array_elem(q).is_some() {
            Cleanup::ArrayDtor { addr: a, record: rd }
        } else {
            Cleanup::Dtor { addr: a.v, record: rd, base: false }
        };
        match ast.exprs.temporary_storage(m) {
            StorageDuration::FullExpression => self.push_cleanup(c),
            StorageDuration::Automatic => self.f.cl.lifetime_extended.push((c, None)),
            StorageDuration::Static | StorageDuration::Thread => {}
        }
    }

    // ---- constructor calls ---------------------------------------------

    /// clang `EmitCXXConstructExpr` into `dest`.
    pub(crate) fn emit_construct(&mut self, e: ExprId, dest: Addr, zeroed: bool) -> R<()> {
        let ast = self.ast;
        let ctor = ast.exprs.constructor(e);
        let info = ast.exprs.construct_info(e);
        let q = ast.exprs.get(e).ty();
        let overlap = std::mem::take(&mut self.f.overlap);
        let rd = ast.decls.get(ctor).ctx().expect("constructor class");
        if info.zeroing && !zeroed {
            match info.kind {
                ConstructionKind::Complete | ConstructionKind::Delegating => {
                    if !self.class_bits(rd).contains(DefinitionBits::EMPTY) {
                        let size = self.size_align(q).0;
                        self.memset(dest, 0, size, false);
                    }
                }
                _ => {
                    if !self.class_bits(rd).contains(DefinitionBits::EMPTY) {
                        let def = self.class_def(rd).expect("class definition");
                        let nv = self.tys.layouter().record(def).nv_size;
                        self.memset(dest, 0, nv, false);
                    }
                }
            }
        }
        let fd = *ast.decls.function(ctor).expect("constructor");
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.construct_args(e)).collect();
        if fd.spec.is_trivial() && args.is_empty() {
            return Ok(());
        }
        if self.array_elem(q).is_some() {
            return self.construct_array(e, dest);
        }
        self.construct_one(e, dest, overlap)
    }

    /// clang `EmitCXXAggrConstructorCall` for a constant-length array: a
    /// loop over the flattened elements, each constructed in its own
    /// cleanup scope. Without exceptions no partial-destroy cleanup.
    fn construct_array(&mut self, e: ExprId, dest: Addr) -> R<()> {
        let ast = self.ast;
        let mut q = ast.exprs.get(e).ty();
        let mut n = 1u64;
        loop {
            match ast.types.canonical_view(q) {
                TypeView::ConstantArray(a) => {
                    n = n.saturating_mul(a.size);
                    q = a.elem;
                }
                TypeView::IncompleteArray(_) | TypeView::VariableArray { .. } => {
                    return err(
                        self.loc_of(e),
                        "construction of variable-length arrays is not supported yet",
                    );
                }
                _ => break,
            }
        }
        if n == 0 {
            return Ok(());
        }
        let elem = self.mty(q);
        let begin = if self.m.types.array_info(dest.ty).is_some() {
            let mut idx = vec![self.icn(LTy::I32, 0)];
            let mut t = dest.ty;
            while let Some((el, _)) = self.m.types.array_info(t) {
                idx.push(self.icn(LTy::I32, 0));
                t = el;
            }
            self.gep(dest.ty, dest.v, idx, true)
        } else {
            dest.v
        };
        let elem_size = self.alloc_size(elem);
        let elem_align = min_align(dest.align, elem_size);
        let nn = self.i64c(i128::from(n));
        let end = self.gep(elem, begin, vec![nn], true);
        let entry = self.f.fir.cur().expect("insertion point");
        let body = self.f.fir.create_block();
        self.emit_block(body, false);
        let phi = self.f.fir.push(Op::Phi { ty: LTy::PTR, inc: vec![(begin, entry)] }, LTy::PTR);
        let sc = self.begin_scope(false);
        let r = self.construct_one(e, Addr { v: phi, ty: elem, align: elem_align }, false);
        self.end_scope(sc);
        r?;
        let one = self.i64c(1);
        let next = self.gep(elem, phi, vec![one], true);
        let cur = self.f.fir.cur().expect("insertion point");
        self.f.fir.add_incoming(crate::fir::Fir::last_inst(phi), next, cur);
        let done = self.icmp(IntPredicate::Eq, next, end);
        let out = self.f.fir.create_block();
        self.cond_br(done, out, body);
        self.emit_block(out, false);
        Ok(())
    }

    /// One complete-or-base object construction (clang
    /// `EmitCXXConstructorCall`).
    fn construct_one(&mut self, e: ExprId, dest: Addr, overlap: bool) -> R<()> {
        let ast = self.ast;
        let ctor = ast.exprs.constructor(e);
        let info = ast.exprs.construct_info(e);
        let rd = ast.decls.get(ctor).ctx().expect("constructor class");
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.construct_args(e)).collect();
        if self.memcpy_equivalent(ctor) {
            let src = self.emit_lvalue(args[0])?;
            let size = self.copy_size(rd, overlap);
            self.memcpy(dest, src.addr.v, src.addr.align, size, false);
            return Ok(());
        }
        let v = match info.kind {
            ConstructionKind::Complete => Structor::Complete,
            ConstructionKind::NonVirtualBase | ConstructionKind::VirtualBase => Structor::Base,
            ConstructionKind::Delegating => self.f.structor.unwrap_or(Structor::Complete),
        };
        let first = ast.first(ctor);
        let fty = ast.decls.get(first).ty().expect("constructor type");
        let arg_tys: Vec<QualType> = args.iter().map(|&a| ast.exprs.get(a).ty()).collect();
        let sig = self.call_sig_this(fty, false, &arg_tys, Some(rd));
        let saved = self.f.dest_destructed;
        self.f.dest_destructed = false;
        let vals = self.emit_call_args(&sig, &args);
        self.f.dest_destructed = saved;
        let vals = vals?;
        let (callee, fid) = self.structor_ref(ctor, v)?;
        self.call_vals(&sig, callee, fid, Some(dest.v), &vals, &args, None, &[])?;
        Ok(())
    }

    /// The bytes an aggregate copy of class `rd` writes (clang
    /// `EmitAggregateCopy`: the data size when the destination may
    /// overlap).
    pub(crate) fn copy_size(&mut self, rd: DeclId, overlap: bool) -> u64 {
        let Some(def) = self.class_def(rd) else { return 0 };
        let l = self.tys.layouter().record(def);
        if overlap { l.data_size } else { l.size }
    }

    // ---- member calls --------------------------------------------------

    /// clang `EmitCXXMemberCallExpr` (non-virtual).
    pub(crate) fn emit_member_call(&mut self, e: ExprId, dest: Option<Addr>) -> R<Option<V>> {
        let ast = self.ast;
        let mut callee_e = ast.exprs.callee(e);
        while ast.exprs.get(callee_e).kind() == ExprKind::ParenExpr {
            callee_e = ast.exprs.operand(callee_e);
        }
        if ast.exprs.get(callee_e).kind() == ExprKind::BinaryOperator
            && ast.exprs.binary_op(callee_e).is_ptr_mem()
        {
            return self.mp_call(e, callee_e, dest);
        }
        if ast.exprs.get(callee_e).kind() != ExprKind::MemberExpr {
            return err(self.loc_of(e), "calls through member pointers are not supported yet");
        }
        let md = ast.exprs.decl(callee_e);
        let base = ast.exprs.operand(callee_e);
        let arrow = ast.exprs.get(callee_e).has(ExprFlags::ARROW);
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.call_args(e)).collect();
        let qualified = ast.exprs.decl_ref_extra(callee_e).is_some_and(|x| x.qualifier.is_some());
        self.emit_method_call(e, md, base, arrow, &args, dest, qualified)
    }

    /// A call of member function `md` on object `base` (`arrow`: a
    /// pointer to it) with arguments `args`: a `CXXMemberCallExpr`, or a
    /// `CXXOperatorCallExpr` whose callee is a member (clang
    /// `EmitCXXOperatorMemberCallExpr`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_method_call(
        &mut self,
        _e: ExprId,
        md: DeclId,
        base: ExprId,
        arrow: bool,
        args: &[ExprId],
        dest: Option<Addr>,
        qualified: bool,
    ) -> R<Option<V>> {
        let ast = self.ast;
        let arg_tys: Vec<QualType> = args.iter().map(|&a| ast.exprs.get(a).ty()).collect();
        let fty = ast.decls.get(md).ty().expect("method type");
        if self.this_class_of(md).is_none() {
            // A static member function called through an object.
            self.emit_ignored(base)?;
            let f = self.get_fn(md)?;
            let sig = self.call_sig(fty, false, &arg_tys);
            return self.call_with(&sig, V::C(Value::func(f)), Some(f), None, args, dest, &[]);
        }
        // clang `EmitCXXMemberOrOperatorMemberCallExpr`: devirtualization.
        let is_virtual = ast.decls.function(md).is_some_and(|f| f.spec.is_virtual());
        let mut callee_md = md;
        let mut base = base;
        let mut use_virtual = is_virtual && !qualified;
        if use_virtual && let Some((dm, b)) = self.devirtualize(md, base) {
            callee_md = dm;
            base = b;
            use_virtual = false;
        }
        let this = if arrow {
            let p = self.emit_scalar(base)?;
            let pq = ast.types.pointee(ast.exprs.get(base).ty()).expect("pointee");
            let (_, align) = self.pointee_layout(pq);
            let ty = self.mty(pq);
            Addr { v: p, ty, align }
        } else {
            self.emit_lvalue(base)?.addr
        };
        let rd = ast.decls.get(md).ctx().expect("method class");
        let trivial = ast.decls.function(md).is_some_and(|f| f.spec.is_trivial())
            || (self.is_union(rd) && self.is_defaulted(md));
        let kind = ast.decls.get(md).kind();
        if trivial {
            if kind == DeclKind::CXXDestructor {
                return Ok(None);
            }
            if matches!(self.special_of(md), Some(Special::CopyAssign | Special::MoveAssign)) {
                let src = self.emit_lvalue(args[0])?;
                let size = self.copy_size(rd, true);
                self.memcpy(this, src.addr.v, src.addr.align, size, false);
                return Ok(Some(this.v));
            }
        }
        let crd = ast.decls.get(callee_md).ctx().expect("method class");
        let cfty = ast.decls.get(callee_md).ty().expect("method type");
        if kind == DeclKind::CXXDestructor {
            let sig = self.call_sig_this(cfty, false, &[], Some(crd));
            if use_virtual {
                let fp = self.virtual_fn_ptr(md, Some(Structor::Complete), this.v, this.align)?;
                return self.call_vals(&sig, fp, None, Some(this.v), &[], &[], None, &[]);
            }
            let (callee, fid) = self.structor_ref(callee_md, Structor::Complete)?;
            return self.call_vals(&sig, callee, fid, Some(this.v), &[], &[], None, &[]);
        }
        let sig = self.call_sig_this(cfty, false, &arg_tys, Some(crd));
        if use_virtual {
            // The arguments come first; `prepareConcreteCallee` then
            // loads the slot.
            let vals = self.emit_call_args(&sig, args)?;
            let fp = self.virtual_fn_ptr(md, None, this.v, this.align)?;
            return self.call_vals(&sig, fp, None, Some(this.v), &vals, args, dest, &[]);
        }
        let f = self.get_fn(callee_md)?;
        self.call_with(&sig, V::C(Value::func(f)), Some(f), Some(this.v), args, dest, &[])
    }

    // ---- expressions ---------------------------------------------------

    /// The initializer of the field of a `CXXDefaultInitExpr`.
    pub(crate) fn default_member_init(&self, e: ExprId) -> Option<ExprId> {
        let f = self.ast.exprs.default_init_field(e);
        self.ast.decls.field(f).and_then(|x| x.init)
    }

    /// A static member function.
    pub(crate) fn is_static_method(&self, d: DeclId) -> bool {
        self.ast.decls.get(d).kind() == DeclKind::CXXMethod
            && self.ast.decls.function(d).is_some_and(|f| f.spec.storage() == FnStorage::Static)
    }

    /// `e` is `this` (through parentheses).
    pub(crate) fn is_this_expr(&self, e: ExprId) -> bool {
        let ast = self.ast;
        let mut e = e;
        while ast.exprs.get(e).kind() == ExprKind::ParenExpr {
            e = ast.exprs.operand(e);
        }
        ast.exprs.get(e).kind() == ExprKind::CXXThisExpr
    }

    /// The byte offset of the base a derived-to-base cast `e` converts
    /// to, from class `derived` (clang `computeNonVirtualBaseClassOffset`).
    /// Paths with a virtual step are an error here (member pointers).
    pub(crate) fn base_path_offset(&mut self, e: ExprId, derived: DeclId) -> R<u64> {
        match self.base_path(e, derived)? {
            (None, off) => Ok(off),
            (Some(_), _) => err(self.loc_of(e), "conversion through a virtual base"),
        }
    }

    /// The path of derived-to-base cast `e` from `derived` as clang's
    /// `GetAddressOfBaseClass` splits it: the last virtual base on the
    /// path, if any, and the non-virtual offset from it (or from
    /// `derived`).
    pub(crate) fn base_path(&mut self, e: ExprId, derived: DeclId) -> R<(Option<DeclId>, u64)> {
        let ast = self.ast;
        let Some(path) = ast.exprs.cast_path(e) else { return Ok((None, 0)) };
        let specs: Vec<(QualType, bool)> = ast
            .lists
            .get(path)
            .iter()
            .map(|&i| {
                let b = ast.decls.base_spec(i);
                (b.ty, b.is_virtual)
            })
            .collect();
        // clang's `BuildBasePathArray` drops the steps before the last
        // virtual one: a virtual base is a base of every class below it.
        let start = specs.iter().rposition(|&(_, v)| v).unwrap_or(0);
        let mut cur = derived;
        let mut off = 0;
        let mut vbase = None;
        for (bq, virt) in specs.into_iter().skip(start) {
            let Some(b) = ast.types.as_record(bq) else {
                return err(self.loc_of(e), "base of a non-class type");
            };
            if virt {
                vbase = Some(b);
                cur = b;
                continue;
            }
            let Some(o) = self.base_offset(cur, b) else {
                return err(self.loc_of(e), "base class not in the layout");
            };
            off += o;
            cur = b;
        }
        if let Some(vb) = vbase
            && self.class_def(derived).is_some_and(|d| self.class_is_final(d))
        {
            let Some(o) = self.vbase_offset(derived, vb) else {
                return err(self.loc_of(e), "virtual base not in the layout");
            };
            return Ok((None, off + o));
        }
        Ok((vbase, off))
    }

    /// The byte offset of virtual base `vb` in the complete `rd`.
    pub(crate) fn vbase_offset(&mut self, rd: DeclId, vb: DeclId) -> Option<u64> {
        let ast = self.ast;
        let def = self.class_def(rd)?;
        let b = ast.first(vb);
        let l = self.tys.layouter().record(def);
        l.vbases.iter().find(|&&(x, _)| ast.first(x) == b).map(|&(_, o)| o)
    }

    /// clang `getVBaseAlignment`.
    pub(crate) fn vbase_align(&mut self, actual: u64, derived: DeclId, vb: DeclId) -> u64 {
        let Some(vdef) = self.class_def(vb) else { return 1 };
        let expected = u64::from(self.tys.layouter().record(vdef).nv_align);
        let dalign = self.class_def(derived).map_or(1, |d| u64::from(self.tys.layouter().record(d).align));
        if actual >= dalign { expected } else { actual.min(expected) }
    }

    /// clang `ItaniumCXXABI::GetVirtualBaseClassOffset`: the offset of
    /// virtual base `vb` of the `derived` object at `this`, loaded from
    /// its vtable.
    pub(crate) fn vbase_offset_value(&mut self, this: Addr, derived: DeclId, vb: DeclId) -> R<V> {
        let l = self.vt_layout(derived)?;
        let b = self.ast.first(vb);
        let Some(&(_, oo)) = l.vbase_offset_offsets.iter().find(|&&(x, _)| self.ast.first(x) == b)
        else {
            return err(Some(self.ast.decls.get(vb).loc()), "no vbase offset for the virtual base");
        };
        let vt = self.load(LTy::PTR, this.v, this.align, false);
        let o = self.i64c(i128::from(oo));
        let p = self.gep(LTy::I8, vt, vec![o], false);
        Ok(self.load(LTy::I64, p, 8, false))
    }

    /// clang `GetAddressOfBaseClass` with the path split by `base_path`:
    /// the address of the base and its alignment.
    pub(crate) fn base_address(
        &mut self,
        this: Addr,
        derived: DeclId,
        vbase: Option<DeclId>,
        nv: u64,
        null_check: bool,
    ) -> R<(V, u64)> {
        let Some(vb) = vbase else {
            return Ok((self.base_ptr(this.v, nv, null_check), min_align(this.align, nv)));
        };
        let blocks = null_check.then(|| {
            let not_null = self.f.fir.create_block();
            let end = self.f.fir.create_block();
            (not_null, end)
        });
        let null = V::C(self.m.const_null(LTy::PTR));
        let mut orig = None;
        if let Some((not_null, end)) = blocks {
            let is_null = self.icmp(IntPredicate::Eq, this.v, null);
            orig = self.f.fir.cur();
            self.cond_br(is_null, end, not_null);
            self.emit_block(not_null, false);
        }
        let voff = self.vbase_offset_value(this, derived, vb)?;
        let off = if nv == 0 {
            voff
        } else {
            let c = self.i64c(i128::from(nv));
            self.bin(kiln_llvm::BinOp::Add, kiln_llvm::Flags::NONE, voff, c)
        };
        let adj = self.gep(LTy::I8, this.v, vec![off], true);
        let align = min_align(self.vbase_align(this.align, derived, vb), nv);
        let Some((_, end)) = blocks else { return Ok((adj, align)) };
        let from = self.f.fir.cur().expect("insertion point");
        self.emit_block(end, false);
        let orig = orig.expect("insertion point");
        let phi = self.f.fir.push(Op::Phi { ty: LTy::PTR, inc: vec![(adj, from), (null, orig)] }, LTy::PTR);
        Ok((phi, align))
    }

    /// clang `GetAddressOfBaseClass` on a pointer: `p + off`, null-checked
    /// when `null_check` and the offset is not zero.
    pub(crate) fn base_ptr(&mut self, p: V, off: u64, null_check: bool) -> V {
        if off == 0 {
            return p;
        }
        if !null_check {
            return self.byte_offset(p, off);
        }
        let not_null = self.f.fir.create_block();
        let end = self.f.fir.create_block();
        let null = V::C(self.m.const_null(LTy::PTR));
        let is_null = self.icmp(IntPredicate::Eq, p, null);
        let orig = self.f.fir.cur().expect("insertion point");
        self.cond_br(is_null, end, not_null);
        self.emit_block(not_null, false);
        let adj = self.byte_offset(p, off);
        let from = self.f.fir.cur().expect("insertion point");
        self.emit_block(end, false);
        self.f.fir.push(Op::Phi { ty: LTy::PTR, inc: vec![(adj, from), (null, orig)] }, LTy::PTR)
    }

    /// `QualType::isPODType` (C++11: trivial and standard-layout classes,
    /// arrays of them, and every non-class type).
    pub(crate) fn is_pod(&self, q: QualType) -> bool {
        let mut q = q;
        while let Some(e) = self.array_elem(q) {
            q = e;
        }
        match self.ast.types.as_record(q) {
            Some(r) => {
                let b = self.class_bits(r);
                b.contains(DefinitionBits::TRIVIALLY_COPYABLE)
                    && b.contains(DefinitionBits::TRIVIAL_DEFAULT_CTOR)
                    && b.contains(DefinitionBits::STANDARD_LAYOUT)
            }
            None => true,
        }
    }
}
