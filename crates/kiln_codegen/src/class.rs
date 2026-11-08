//! Classes (spec/codegen.md §7): constructor/destructor variants and
//! their definitions (clang `ItaniumCXXABI::emitCXXStructor`,
//! `EmitConstructorBody`, `EmitDestructorBody`), constructor and
//! destructor calls, member function calls and `this`.
//!
//! Polymorphic classes and virtual bases are WP-080: every path that would
//! need a vtable pointer or a VTT reports an error instead.

use kiln_abi::mangle::{self, Structor};
use kiln_ast::decl::{
    CtorInitKind, DeclKind, Defaulted, DefinitionBits, FnStorage, TagKind, TemplLinkKind,
};
use kiln_ast::expr::{ConstructionKind, ExprFlags, ExprKind, StorageDuration};
use kiln_ast::stmt::StmtKind;
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType, StmtId, TypeId};
use kiln_llvm::{BinOp, Flags, FuncId, IntPredicate, Linkage, TypeId as LTy, UnnamedAddr, Value};
use kiln_source::fx::FxHashSet;

use crate::cleanup::Cleanup;
use crate::fir::{Op, V};
use crate::func::{Addr, CallArg, min_align};
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
            DeclKind::CXXConstructor | DeclKind::CXXDestructor => n.ctx(),
            DeclKind::CXXMethod | DeclKind::CXXConversion => {
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
        let extern_c = mangle::is_extern_c(ast, first);
        let sig = self.sig_this(fty, extern_c, this);
        let ps = ast.pass_object_size_params(first);
        Ok(self.with_object_sizes(sig, fty, &ps, extern_c))
    }

    /// The definition of class `rd`.
    pub(crate) fn class_def(&self, rd: DeclId) -> Option<DeclId> {
        self.ast.definition(rd)
    }

    pub(crate) fn class_bits(&self, rd: DeclId) -> DefinitionBits {
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

    /// The class whose destructor variable `d` of type `q` needs at the
    /// end of its lifetime (clang `VarDecl::needsDestruction`):
    /// [`Cg::destructed_class`], unless `d` has constant destruction
    /// ([expr.const]/7, [`crate::fold::Folds::const_destroyed`]).
    pub(crate) fn var_destructed_class(&self, d: DeclId, q: QualType) -> Option<DeclId> {
        if self.folds.const_destroyed.contains(&self.ast.first(d)) || self.is_no_destroy(d) {
            return None;
        }
        self.destructed_class(q)
    }

    /// clang `VarDecl::isNoDestroy`: variable `d` has static or thread
    /// storage duration and its destructor is never registered
    /// (`[[clang::no_destroy]]`, or `-fc++-static-destructors=` excludes
    /// it and it is not `[[clang::always_destroy]]`; WP-805).
    pub(crate) fn is_no_destroy(&self, d: DeclId) -> bool {
        use kiln_ast::attr::AttrKind;
        let ast = self.ast;
        let Some(v) = ast.decls.var(d) else { return false };
        if crate::fold::is_local(ast, d) && !crate::fold::is_static_local(ast, d) {
            return false;
        }
        if self.has_fn_attr(d, AttrKind::NO_DESTROY) {
            return true;
        }
        if self.has_fn_attr(d, AttrKind::ALWAYS_DESTROY) {
            return false;
        }
        match self.opts.static_destructors {
            crate::StaticDestructors::All => false,
            crate::StaticDestructors::None => true,
            crate::StaticDestructors::ThreadLocal => v.spec.tls() == kiln_ast::decl::TlsKind::None,
        }
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
        // A default not instantiated yet ([temp.inst]/12) still counts.
        let has_default = |p: DeclId| {
            ast.decls.param(p).is_some_and(|x| {
                x.default_arg.is_some()
                    || x.flags & kiln_ast::decl::ParamData::DEFAULT_ARG_PENDING != 0
            })
        };
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
        if self.is_explicit_inst_declaration(first) {
            return None;
        }
        ast.redecls_of(first).find(|&r| {
            ast.decls.function(r).is_some_and(|f| {
                f.body.is_some()
                    && !matches!(f.templ.get(), Some((TemplLinkKind::Pattern, _)))
                    && !f.spec.is_immediate()
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

    /// The signature of variant `v` of `first` (`decl_sig`, plus the VTT
    /// parameter of a base-object structor of a class with virtual
    /// bases).
    pub(crate) fn structor_sig(&mut self, first: DeclId, v: Option<Structor>) -> R<Sig> {
        let mut sig = self.decl_sig(first)?;
        if self.inheriting_drops_params(first, v) {
            sig = self.without_params(sig);
        }
        let k = self.ast.decls.get(first).kind();
        if v == Some(Structor::Base)
            && matches!(k, DeclKind::CXXConstructor | DeclKind::CXXDestructor)
            && self.structor_has_vbases(first)
        {
            return Ok(self.with_vtt(sig));
        }
        Ok(sig)
    }

    /// Whether variant `v` of constructor `first` is the base-object
    /// variant of an inheriting constructor whose constructor-using-shadow
    /// constructs a virtual base: it takes no parameters but the VTT,
    /// since only the complete-object variant constructs that base (clang
    /// `inheritingCtorHasParams`, [class.inhctor.init]/2).
    pub(crate) fn inheriting_drops_params(&self, first: DeclId, v: Option<Structor>) -> bool {
        v == Some(Structor::Base)
            && self
                .ast
                .decls
                .inheriting_ctor_shadow(first)
                .is_some_and(|s| self.ast.shadow_constructed(s).1)
    }

    /// clang `EmitInheritedCXXConstructorCall`: the base subobject at
    /// `dest` constructed by the base-object variant of `e`'s constructor
    /// with the enclosing inheriting constructor's parameters forwarded
    /// (none when the constructor is inherited through a virtual base,
    /// which only the complete-object variant constructs).
    pub(crate) fn emit_inherited_ctor_init(&mut self, e: ExprId, dest: Addr) -> R<()> {
        let ast = self.ast;
        let ctor = ast.exprs.constructor(e);
        let (kind, from_vbase) = ast.exprs.inherited_ctor_info(e);
        let rd = ast.decls.get(ctor).ctx().expect("constructor class");
        let first = ast.first(ctor);
        let mut vals = Vec::new();
        let mut ptys = Vec::new();
        if let Some((v, t)) = self.f.inh_args.clone() {
            // Inlined at the call site: the call's own arguments.
            if !from_vbase {
                vals = v;
                ptys = t;
            }
        } else if !from_vbase {
            let outer = self.f.decl.and_then(|d| ast.decls.function(d)).map(|f| f.params);
            let params: Vec<DeclId> =
                outer.map(|l| ast.lists.iter(l).collect()).unwrap_or_default();
            for p in params {
                let q = ast.decls.get(p).ty().expect("param type");
                let Some(&a) = self.f.locals.get(&p) else {
                    return err(self.loc_of(e), "inherited constructor parameter not bound");
                };
                // EmitDelegateCallArg.
                vals.push(self.delegate_call_arg(a, q));
                ptys.push(q);
            }
        }
        if !from_vbase && self.inlines_inheriting_ctor(ctor) {
            let vtt = if self.structor_has_vbases(first) {
                Some(self.vtt_arg(rd, kind == ConstructionKind::VirtualBase, false)?)
            } else {
                None
            };
            return self.emit_inlined_inheriting_ctor(ctor, dest, Structor::Base, vals, ptys, vtt);
        }
        let fty = ast.decls.get(first).ty().expect("constructor type");
        let mut sig = self.call_sig_this(fty, false, &ptys, Some(rd));
        if from_vbase {
            sig = self.without_params(sig);
        }
        if self.structor_has_vbases(first) {
            sig = self.with_vtt(sig);
            let vtt = self.vtt_arg(rd, kind == ConstructionKind::VirtualBase, false)?;
            self.f.call_vtt = Some(vtt);
        }
        let (callee, fid) = self.structor_ref(ctor, Structor::Base)?;
        self.call_vals(&sig, callee, fid, Some(dest.v), &vals, &[], None, &[])?;
        Ok(())
    }

    /// Whether a call of constructor `ctor` is inlined at the call site:
    /// an inheriting constructor of a C-variadic base constructor, whose
    /// variadic arguments cannot be forwarded (clang
    /// `canEmitDelegateCallArgs` false →
    /// `EmitInlinedInheritingCXXConstructorCall`).
    pub(crate) fn inlines_inheriting_ctor(&self, ctor: DeclId) -> bool {
        let ast = self.ast;
        ast.decls.inheriting_ctor_shadow(ctor).is_some()
            && ast
                .decls
                .get(ctor)
                .ty()
                .and_then(|t| ast.types.proto(ast.types.canonical(t)))
                .is_some_and(kiln_ast::types::FuncProto::variadic)
    }

    /// clang `EmitInlinedInheritingCXXConstructorCall`: variant `v` of
    /// inheriting constructor `ctor` emitted in place on the object at
    /// `dest`, its `CXXInheritedCtorInitExpr`s forwarding the call's
    /// evaluated arguments `vals` (of types `tys`). `vtt` is the VTT
    /// argument of a base-object variant of a class with virtual bases,
    /// computed by the caller (`addImplicitConstructorArgs`); it becomes
    /// the inlined body's VTT parameter.
    pub(crate) fn emit_inlined_inheriting_ctor(
        &mut self,
        ctor: DeclId,
        dest: Addr,
        v: Structor,
        vals: Vec<CallArg>,
        tys: Vec<QualType>,
        vtt: Option<V>,
    ) -> R<()> {
        let ast = self.ast;
        let first = ast.first(ctor);
        let rd = ast.decls.get(ctor).ctx().expect("constructor class");
        let def = self.structor_def(first);
        let Some((def, body)) =
            def.and_then(|d| ast.decls.function(d).and_then(|f| f.body).map(|b| (d, b)))
        else {
            return err(Some(ast.decls.get(ctor).loc()), "inheriting constructor is not defined");
        };
        debug_assert_eq!(
            vtt.is_some(),
            v == Structor::Base && self.structor_has_vbases(first),
            "VTT argument of an inlined inheriting constructor"
        );
        let saved = (
            self.f.decl,
            self.f.this,
            self.f.this_class,
            self.f.closure.take(),
            self.f.structor,
            self.f.vtt.take(),
            self.f.inh_args.take(),
        );
        // The simplified prolog: `this` and the VTT are spilled and
        // reloaded as by a function prolog (`EmitParmDecl` +
        // `EmitInstanceFunctionProlog`).
        let a = self.f.fir.alloca(LTy::PTR, 8);
        let vtt_addr = vtt.map(|_| self.f.fir.alloca(LTy::PTR, 8));
        self.store(dest.v, a, 8, false);
        if let (Some(t), Some(va)) = (vtt, vtt_addr) {
            self.store(t, va, 8, false);
        }
        let this = self.load(LTy::PTR, a, 8, false);
        self.f.vtt = vtt_addr.map(|va| self.load(LTy::PTR, va, 8, false));
        self.f.decl = Some(def);
        self.f.this = Some(this);
        self.f.this_class = Some(rd);
        self.f.structor = Some(v);
        self.f.inh_args = Some((vals, tys));
        let r = self.emit_ctor_body(def, body);
        (
            self.f.decl,
            self.f.this,
            self.f.this_class,
            self.f.closure,
            self.f.structor,
            self.f.vtt,
            self.f.inh_args,
        ) = saved;
        r
    }

    /// Declares structor variant `v` of `first` (clang
    /// `getAddrOfCXXStructor` creating the declaration).
    pub(crate) fn declare_structor(&mut self, first: DeclId, v: Structor) -> R<(Value, FuncId)> {
        if let Some(&r) = self.sfns.get(&(first, v)) {
            return Ok(r);
        }
        let name = mangle::mangle_structor(self.ast, &mut self.anon, first, v);
        self.note_rep(&name, first);
        // Every variant of a structor with an asm label takes the label
        // (clang `getMangledName` of each `GlobalDecl`): they are one
        // function, the first variant defined ([`Cg::emit_structor_def`]).
        let shared = if self.labelled_structor(first) { self.m.get_function(&name) } else { None };
        let f = match shared {
            Some(f) if self.m.resolve(Value::func(f)) == Value::func(f) => f,
            _ => {
                let sig = self.structor_sig(first, Some(v))?;
                let f = self.declare_fn(&name, first, &sig)?;
                self.m.func_mut(f).unnamed_addr = UnnamedAddr::Global;
                f
            }
        };
        let r = (Value::func(f), f);
        self.sfns.insert((first, v), r);
        Ok(r)
    }

    /// Whether constructor or destructor `first` has an asm label.
    pub(crate) fn labelled_structor(&self, first: DeclId) -> bool {
        mangle::asm_label(self.ast, first).is_some()
    }

    /// The labelled structor function `f` for a definition of variant `v`
    /// of `first` (clang `GetOrCreateLLVMFunction` for a definition under
    /// the label every variant takes; spec/abi.md §1): `f` itself when
    /// its type is the variant's — an earlier variant's body stays and
    /// this one's is appended after it as unreachable blocks — else `f`
    /// retyped at the end of the module, dropping an earlier variant's
    /// body (the complete-object variant, without the VTT parameter of
    /// the base-object one of a class with virtual bases, replaces it).
    fn labelled_structor_def(
        &mut self,
        d: DeclId,
        first: DeclId,
        v: Structor,
        f: FuncId,
    ) -> R<FuncId> {
        if !self.labelled_structor(first) {
            return Ok(f);
        }
        self.check_duplicate_def(d, f);
        let sig = self.structor_sig(first, Some(v))?;
        if self.m.func(f).ty() != sig.fn_ty {
            if let Err(e) = self.m.redefine_function_to_end(f, sig.fn_ty) {
                return err(Some(self.ast.decls.get(d).loc()), format!("cannot define {e:?}"));
            }
            self.set_decl_attrs(f, first, &sig);
            self.m.func_mut(f).unnamed_addr = UnnamedAddr::Global;
        }
        Ok(f)
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
    /// than alias it. A `weak` one is not: its linkage is `weak` whatever
    /// the inline and instantiation rules say, so clang
    /// (`getCodegenToUse` → `COMDAT`) aliases it in the `C5`/`D5` comdat.
    fn complete_is_rauw(&self, def: DeclId) -> bool {
        self.discardable_fn(def) && !self.weak_structor(def)
    }

    /// Whether structor definition `def` is `weak`: a `weak` attribute
    /// on one of its declarations, and not internal (as
    /// [`Cg::set_def_linkage`] decides).
    fn weak_structor(&self, def: DeclId) -> bool {
        !self.fn_internal(def) && self.ast.is_weak_decl(def)
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
        let first_ref = self.c1_decl.insert(first);
        if self.structor_has_vbases(first) {
            // A separate complete-object function.
            let (val, f) = self.declare_structor(first, v)?;
            self.queue_structor(first, v);
            return Ok((V::C(val), Some(f)));
        }
        if self.complete_is_rauw(def) {
            // The complete-object declaration is later replaced by the
            // base variant, which takes its place in the module (clang
            // `applyReplacements` moves the base function to where the
            // complete-object declaration was created: here, at the end).
            let existed = self.sfns.contains_key(&(first, Structor::Base));
            let f = self.base_fn(first, false)?;
            if existed && first_ref {
                self.m.move_function_to_end(f);
            }
            self.queue_structor(first, v);
            return Ok((V::C(Value::func(f)), None));
        }
        // An alias defined later in the translation unit (only once it is
        // used, for a discardable `weak` one).
        if self.discardable_fn(def) {
            self.queue_structor(first, v);
        }
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
        if fd.spec.is_immediate() {
            return Ok(());
        }
        let first = ast.first(d);
        if self.is_explicit_inst_declaration(first) || self.held.contains(&first) {
            return Ok(());
        }
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
                // spec/modules.md §7: the module's object defines it.
                if !ast.modules.is_imported_named_unit_decl(d) {
                    self.emit_structor_def(d, v)?;
                }
                continue;
            }
            let visible = match v {
                Structor::Base | Structor::Deleting => self.sfns.contains_key(&(first, v)),
                Structor::Complete => self.c1_decl.contains(&first),
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
        match v {
            Structor::Base => {
                self.squeued.insert((first, v));
                // The declaration if it is referenced already (clang
                // `TryEmitBaseDestructorAsAlias` runs before
                // `codegenCXXStructor` creates it).
                let declared = self.sfns.get(&(first, v)).map(|e| e.1);
                if declared.is_some_and(|f| !self.m.func(f).is_declaration())
                    || self.sdone.contains(&first)
                {
                    return Ok(());
                }
                self.sdone.insert(first);
                if self.try_base_dtor_alias(d, first, declared)? {
                    return Ok(());
                }
                let (_, f) = self.declare_structor(first, v)?;
                let f = self.labelled_structor_def(d, first, v, f)?;
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
                    if !self.m.func(f).is_declaration() && !self.labelled_structor(first) {
                        return Ok(());
                    }
                    let f = self.labelled_structor_def(d, first, v, f)?;
                    let saved = std::mem::replace(&mut self.f, crate::func::Fx::empty());
                    let r = self.gen_code(d, f, Some(Structor::Complete));
                    self.f = saved;
                    r?;
                    return self.emit_thunks(d, Some(v));
                }
                if self.complete_is_rauw(d) || self.labelled_structor(first) {
                    // A labelled one is the base variant's function (clang
                    // `emitConstructorDestructorAlias` skips an alias whose
                    // name is defined).
                    let f = self.base_fn(first, true)?;
                    self.sfns.insert((first, v), (Value::func(f), f));
                    if let Some(&p) = self.ph_of.get(&(first, v)) {
                        self.phs[p as usize] = Some(Value::func(f));
                    }
                    return self.emit_thunks(d, Some(v));
                }
                let f = self.base_fn(first, true)?;
                // The base variant may itself be an alias
                // ([`Cg::try_base_dtor_alias`]).
                let target =
                    self.sfns.get(&(first, Structor::Base)).map_or(Value::func(f), |e| e.0);
                let name = mangle::mangle_structor(ast, &mut self.anon, first, v);
                let fty = self.m.func(f).ty();
                let a = match self.m.add_alias(&name, fty, target) {
                    Ok(a) => a,
                    Err(e) => {
                        return err(
                            Some(ast.decls.get(d).loc()),
                            format!("cannot define alias {name}: {e:?}"),
                        );
                    }
                };
                // The alias takes the base variant's linkage (`weak` for
                // a `weak` one, whose base variant may not be defined
                // yet; `weak_odr` for an explicit instantiation
                // definition; clang `emitConstructorDestructorAlias`).
                let linkage = if self.weak_structor(d) {
                    Linkage::Weak
                } else {
                    match self.m.func(f).linkage {
                        Linkage::WeakOdr => Linkage::WeakOdr,
                        _ => Linkage::External,
                    }
                };
                let visibility = self.m.func(f).visibility;
                {
                    let al = self.m.alias_mut(a);
                    al.linkage = linkage;
                    al.dso_local = true;
                    al.unnamed_addr = UnnamedAddr::Global;
                    al.visibility = visibility;
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
                if !self.m.func(f).is_declaration() && !self.labelled_structor(first)
                    || !self.d0_done.insert(first)
                {
                    return Ok(());
                }
                let f = self.labelled_structor_def(d, first, v, f)?;
                let saved = std::mem::replace(&mut self.f, crate::func::Fx::empty());
                let r = self.gen_code(d, f, Some(Structor::Deleting));
                self.f = saved;
                r?;
                self.emit_thunks(d, Some(v))
            }
        }
    }

    /// The linkage clang gives a definition of function `d`
    /// (`getFunctionLinkage`, as [`Cg::set_def_linkage`] sets it).
    fn fn_def_linkage(&self, d: DeclId) -> Linkage {
        if self.fn_internal(d) {
            Linkage::Internal
        } else if self.is_available_externally_fn(self.ast.first(d)) {
            Linkage::AvailableExternally
        } else if self.is_explicit_inst_definition(d) {
            Linkage::WeakOdr
        } else if self.fn_is_inline(d) {
            Linkage::LinkOnceOdr
        } else {
            Linkage::External
        }
    }

    /// clang `TryEmitBaseDestructorAsAlias` (`-mconstructor-aliases`, at
    /// -O1 and above; spec/codegen.md "Structor aliases"): the base-object
    /// destructor `f` of definition `d` is not emitted when its body is
    /// empty, its class has no virtual bases and no field needing
    /// destruction, and exactly one non-virtual base, at offset zero, has
    /// a non-trivial destructor. A discardable (`linkonce_odr`,
    /// `available_externally`, internal) one is replaced by that base's
    /// base-object destructor (unless that is an `available_externally`
    /// `always_inline` one);
    /// an external one becomes an alias of it when the base's is already
    /// defined and not weak. A `weak_odr` one (clang `COMDAT` structor
    /// codegen) is always emitted. `declared` is the base-object
    /// declaration if one exists. Returns whether the base-object variant
    /// is settled (no definition to emit).
    fn try_base_dtor_alias(
        &mut self,
        d: DeclId,
        first: DeclId,
        declared: Option<FuncId>,
    ) -> R<bool> {
        let ast = self.ast;
        if !self.opts.optimize()
            || ast.decls.get(d).kind() != DeclKind::CXXDestructor
            || self.labelled_structor(first)
        {
            return Ok(false);
        }
        let linkage = self.fn_def_linkage(d);
        if linkage == Linkage::WeakOdr {
            return Ok(false);
        }
        // A defaulted destructor's synthesized body is on its first
        // declaration.
        let body = ast.redecls_of(d).find_map(|r| ast.decls.function(r).and_then(|x| x.body));
        let trivial_body = body.is_some_and(|b| {
            ast.stmts.get(b).kind() == StmtKind::CompoundStmt
                && ast.lists.get(ast.stmts.compound_body(b)).is_empty()
        });
        if !trivial_body || self.structor_has_vbases(d) {
            return Ok(false);
        }
        let Some(rd) = ast.decls.get(d).ctx() else { return Ok(false) };
        let Some(def) = self.class_def(rd) else { return Ok(false) };
        let field_needs_dtor = ast.decls.members(def).any(|m| {
            ast.decls.get(m).kind() == DeclKind::Field
                && ast.decls.get(m).ty().is_some_and(|q| self.destructed_class(q).is_some())
        });
        if field_needs_dtor {
            return Ok(false);
        }
        let mut unique = None;
        for b in ast.decls.bases(def, &ast.lists) {
            if b.is_virtual {
                continue;
            }
            let Some(br) = ast.types.as_record(b.ty) else { continue };
            if !self.has_nontrivial_dtor(br) {
                continue;
            }
            if unique.is_some() {
                return Ok(false);
            }
            unique = Some(br);
        }
        let Some(base) = unique else { return Ok(false) };
        if self.base_offset(rd, base) != Some(0) {
            return Ok(false);
        }
        let Some(bdtor) = self.class_def(base).and_then(|b| ast.destructor(b)) else {
            return Ok(false);
        };
        let bfirst = ast.first(bdtor);
        let target_linkage = self.fn_def_linkage(bfirst);
        if matches!(
            linkage,
            Linkage::Internal | Linkage::LinkOnceOdr | Linkage::AvailableExternally
        ) {
            // clang keeps an `available_externally` `always_inline` target
            // unreferenced (libc++); the alias is then refused below.
            if target_linkage == Linkage::AvailableExternally && self.has_always_inline_attr(bfirst)
            {
                return Ok(false);
            }
            // `addReplacement`.
            let bf = self.base_fn(bfirst, true)?;
            let target = self.sfns.get(&(bfirst, Structor::Base)).map_or(Value::func(bf), |e| e.0);
            if let Some(f) = declared
                && let Err(e) = self.m.replace_function(f, target)
            {
                return err(Some(ast.decls.get(d).loc()), format!("cannot replace {e:?}"));
            }
            self.sfns.insert((first, Structor::Base), (target, bf));
            return Ok(true);
        }
        let bf = self.base_fn(bfirst, true)?;
        let target = self.sfns.get(&(bfirst, Structor::Base)).map_or(Value::func(bf), |e| e.0);
        // No alias of a declaration (`isDeclarationForLinker`) or of a
        // weak definition (different comdats in different TUs).
        let declaration = match self.m.resolve(target).get() {
            kiln_llvm::ValueRef::Func(g) => self.m.func(g).is_declaration(),
            _ => false,
        };
        if declaration
            || matches!(
                target_linkage,
                Linkage::LinkOnceOdr | Linkage::WeakOdr | Linkage::AvailableExternally
            )
        {
            return Ok(false);
        }
        let r = if let Some(f) = declared {
            let fty = self.m.func(f).ty();
            self.m.replace_function_with_alias(f, fty, target)
        } else {
            let name = mangle::mangle_structor(ast, &mut self.anon, first, Structor::Base);
            let fty = self.structor_sig(first, Some(Structor::Base))?.fn_ty;
            self.m.add_alias(&name, fty, target)
        };
        let a = match r {
            Ok(a) => a,
            Err(e) => return err(Some(ast.decls.get(d).loc()), format!("cannot alias {e:?}")),
        };
        // clang `SetCommonAttributes` → `setGVProperties(Alias, D)`.
        let visibility = self.decl_visibility(d, true);
        {
            // Destructors are always `unnamed_addr`; `SetCommonAttributes`.
            let al = self.m.alias_mut(a);
            al.linkage = Linkage::External;
            al.dso_local = true;
            al.unnamed_addr = UnnamedAddr::Global;
            al.visibility = visibility;
        }
        self.sfns.insert((first, Structor::Base), (Value::alias(a), declared.unwrap_or(bf)));
        Ok(true)
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
        // A function-try-block's handlers also cover the mem-initializers
        // ([except.pre]/4): the catch scope is entered before the prologue.
        let fn_try = ast.stmts.get(body).kind() == StmtKind::CXXTryStmt;
        if fn_try {
            self.enter_try(body)?;
        }
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
                self.record_ty(rd);
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
                    self.push_base_eh_dtor(base, off, false);
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
                    self.push_base_eh_dtor(base, off, true);
                }
                CtorInitKind::Delegating => {
                    self.flush_memcpy_run(d, &mut run)?;
                    let this = self.this_addr();
                    self.emit_agg_into(ci.init, this, false)?;
                    // clang `EmitDelegatingCXXConstructorCall`: once the
                    // target constructor returns the object is complete
                    // ([except.ctor]/4), so a throw destroys it.
                    if self.opts.exceptions && self.has_nontrivial_dtor(rd) {
                        let base = self.f.structor == Some(Structor::Base);
                        let c = Cleanup::DelegatingDtor { addr: this.v, record: rd, base };
                        self.push_cleanup_kind(c, false, true);
                    }
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
                    self.emit_field_init(ci.kind, field, ci.init, ci.loc)?;
                }
            }
        }
        self.flush_memcpy_run(d, &mut run)?;
        if !vptrs_done {
            self.init_vtable_ptrs(rd)?;
            // clang `EmitCtorPrologue`'s `FieldConstructionScope` takes
            // `LoadCXXThisAddress()` even with no member initializers, so
            // the class's IR type is created here (its name's uniquing
            // suffix, e.g. an explicitly instantiated `X<long>` before
            // `X<int>`; [temp.explicit]/10).
            self.record_ty(rd);
        }
        self.emit_stmt(if fn_try { ast.stmts.try_block(body) } else { body })?;
        self.end_scope(sc);
        if fn_try {
            self.exit_try(body, true)?;
        }
        Ok(())
    }

    /// clang `EmitBaseInitializer`'s `CallBaseDtor` EH cleanup: a base
    /// subobject at `off` (a virtual base when `virt`) is destroyed if a
    /// later initializer or the body throws ([except.ctor]/3).
    fn push_base_eh_dtor(&mut self, base: DeclId, off: u64, virt: bool) {
        if self.opts.exceptions && self.has_nontrivial_dtor(base) {
            self.push_cleanup_kind(Cleanup::BaseOfThis { base, offset: off, virt }, false, true);
        }
    }

    /// clang `pushEHDestroy`: an EH-only cleanup destroying the
    /// just-initialized member at `a` of type `q` ([except.ctor]/3).
    fn push_eh_destroy(&mut self, a: Addr, q: QualType) {
        if !self.opts.exceptions {
            return;
        }
        let Some(rd) = self.destructed_class(q) else { return };
        let c = if self.array_elem(q).is_some() {
            Cleanup::ArrayDtor { addr: a, record: rd }
        } else {
            Cleanup::Dtor { addr: a.v, record: rd, base: false }
        };
        self.push_cleanup_kind(c, false, true);
    }

    /// The byte range `[first, last)` of field `field` of `rd` if clang's
    /// `ConstructorMemcpyizer` may copy it with a memcpy. A field that is
    /// empty for layout has an empty range: clang `addMemcpyableField`
    /// joins it to the run but copies none of its bytes (an empty
    /// `[[no_unique_address]]` member shares its offset with a non-empty
    /// one).
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
        let start = bit / 8;
        if self.tys.is_empty_record_for_layout(q) {
            return Some((start, start));
        }
        let (s, _) = self.size_align(q);
        let size = match ast.types.as_record(q).and_then(|r| self.class_def(r)) {
            Some(r) if self.array_elem(q).is_none() => self.tys.layouter().record(r).data_size,
            _ => s,
        };
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
    /// normally. The memcpy spans from the lowest-offset non-empty field
    /// to the end of the highest-offset one (`addNextField`), and a run of
    /// only empty fields copies nothing.
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
            return self.emit_field_init(ci.kind, field, ci.init, ci.loc);
        }
        // clang `addInitialField` / `addNextField`.
        let mut first: Option<(DeclId, u64)> = None;
        let mut last: Option<(u64, u64)> = None;
        for &(f, s, e) in items.iter().filter(|&&(_, s, e)| e > s) {
            if let (Some((_, fs)), Some((ls, _))) = (first, last) {
                if s < fs {
                    first = Some((f, s));
                } else if s >= ls {
                    last = Some((s, e));
                }
            } else {
                first = Some((f, s));
                last = Some((s, e));
            }
        }
        // clang `ConstructorMemcpyizer::pushEHDestructors`, before the
        // memcpy.
        if self.opts.exceptions {
            for &(f, _, _) in &items {
                let q = ast.decls.get(f).ty().expect("field type");
                if self.destructed_class(q).is_some() {
                    let this = self.this_addr();
                    let a = self.field_lvalue_init(this, f, q)?.addr;
                    self.push_eh_destroy(a, q);
                }
            }
        }
        let (Some((first, start)), Some((_, end))) = (first, last) else {
            return Ok(());
        };
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
    /// clang `EmitMemberInitializer` (at the mem-initializer's location).
    fn emit_field_init(&mut self, kind: CtorInitKind, field: DeclId, init: ExprId, loc: kiln_source::Loc) -> R<()> {
        let dl = self.dbg_apply(loc);
        let r = self.emit_field_init_x(kind, field, init);
        self.dbg_restore(dl);
        r
    }

    fn emit_field_init_x(&mut self, kind: CtorInitKind, field: DeclId, init: ExprId) -> R<()> {
        let ast = self.ast;
        let this = self.this_addr();
        let lv = if kind == CtorInitKind::IndirectField {
            let chain: Vec<DeclId> = ast.lists.iter(ast.indirect_field_chain(field)).collect();
            let mut base = this;
            let mut btb = crate::tbaa::Tb::Type;
            let mut lv = None;
            for f in chain {
                let q = ast.decls.get(f).ty().expect("field type");
                let l = self.field_lvalue_init_tb(base, btb, f, q)?;
                base = l.addr;
                btb = l.tb;
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
            let st = self.store(p, lv.addr.v, lv.addr.align, false);
            self.tbaa_mark(st, fq, lv.tb);
            return Ok(());
        }
        let addr = lv.addr;
        if self.is_aggregate(fq) {
            let overlap = false;
            self.emit_agg_into(init, addr, overlap)?;
        } else {
            self.init_to_lvalue(init, lv, false)?;
        }
        self.push_eh_destroy(addr, fq);
        Ok(())
    }

    /// clang `EmitDestructorBody` for the base variant (the complete
    /// variant is an alias or replaced by it: no virtual bases).
    pub(crate) fn emit_dtor_body(&mut self, d: DeclId, body: StmtId) -> R<()> {
        let ast = self.ast;
        // A function-try-block is entered before anything else; its
        // handlers also cover the destruction of the members and bases
        // ([except.pre]/4).
        let fn_try = ast.stmts.get(body).kind() == StmtKind::CXXTryStmt;
        let rd = self.f.this_class.expect("destructor class");
        let complete_vbases =
            self.f.structor == Some(Structor::Complete) && self.structor_has_vbases(d);
        if complete_vbases && self.class_bits_of(rd).contains(DefinitionBits::ABSTRACT) {
            self.emit_trap_unreachable();
            return Ok(());
        }
        if fn_try {
            self.enter_try(body)?;
        }
        let sc = self.begin_scope(false);
        // The complete variant of a class with virtual bases and a
        // function-try-block acts like the base variant after entering
        // the virtual-base cleanups (clang does not delegate: that would
        // introduce two handler blocks).
        if complete_vbases {
            self.push_vbase_dtor_cleanups(rd);
        }
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
                self.push_cleanup(Cleanup::BaseOfThis { base: b, offset: off, virt: false });
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
        self.emit_stmt(if fn_try { ast.stmts.try_block(body) } else { body })?;
        self.end_scope(sc);
        if fn_try {
            self.exit_try(body, true)?;
        }
        Ok(())
    }

    /// clang `EmitDestructorBody` for the complete-object variant of a
    /// class with virtual bases: the base-object destructor (with the
    /// class's own VTT), then the virtual bases in reverse order
    /// (`EnterDtorCleanups(Dtor_Complete)`).
    pub(crate) fn emit_complete_dtor_body(&mut self) -> R<()> {
        let rd = self.f.this_class.expect("destructor class");
        // An abstract class's complete-object destructor can never be
        // called.
        if self.class_bits_of(rd).contains(DefinitionBits::ABSTRACT) {
            self.emit_trap_unreachable();
            return Ok(());
        }
        let sc = self.begin_scope(false);
        self.push_vbase_dtor_cleanups(rd);
        let this = self.this_value();
        self.call_dtor_of(rd, this, true, false);
        self.end_scope(sc);
        Ok(())
    }

    /// `EnterDtorCleanups(Dtor_Complete)`: the virtual bases of `rd`.
    fn push_vbase_dtor_cleanups(&mut self, rd: DeclId) {
        let def = self.class_def(rd).expect("class definition");
        let mut vbases = Vec::new();
        self.collect_vbases(def, &mut vbases, 0);
        for b in vbases {
            if !self.has_nontrivial_dtor(b) {
                continue;
            }
            let Some(off) = self.vbase_offset(rd, b) else { continue };
            self.push_cleanup(Cleanup::BaseOfThis { base: b, offset: off, virt: true });
        }
    }

    /// clang `CXXRecordDecl::vbases()` order: per direct base, its
    /// virtual bases, then itself if virtual; each class once.
    /// Each class is expanded once (`seen` holds the expanded classes
    /// and the collected ones, first declarations): a second expansion
    /// adds nothing, and re-expanding per path is exponential in a
    /// lattice of virtual diamonds.
    fn collect_vbases(&self, r: DeclId, out: &mut Vec<DeclId>, depth: usize) {
        let mut expanded = FxHashSet::default();
        let mut have = FxHashSet::default();
        self.collect_vbases_in(r, out, depth, &mut expanded, &mut have);
    }

    fn collect_vbases_in(
        &self,
        r: DeclId,
        out: &mut Vec<DeclId>,
        depth: usize,
        expanded: &mut FxHashSet<DeclId>,
        have: &mut FxHashSet<DeclId>,
    ) {
        let ast = self.ast;
        if depth > 1024 {
            return;
        }
        let bases: Vec<(QualType, bool)> =
            ast.decls.bases(r, &ast.lists).map(|b| (b.ty, b.is_virtual)).collect();
        for (bq, virt) in bases {
            let Some(b) = ast.types.as_record(bq) else { continue };
            let Some(bd) = self.class_def(b) else { continue };
            if self.class_bits(bd).contains(DefinitionBits::HAS_VIRTUAL_BASES)
                && expanded.insert(ast.first(bd))
            {
                self.collect_vbases_in(bd, out, depth + 1, expanded, have);
            }
            if virt && have.insert(ast.first(bd)) {
                out.push(bd);
            }
        }
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
        let Some(&od) = ast.decls.cold.dtor_delete.get(ast.first(d)) else {
            return err(loc, "virtual destructor without a deallocation function");
        };
        let ty = self.class_type(rd);
        if ast.is_destroying_operator_delete(od) {
            // EnterDtorCleanups with a destroying operator delete: it
            // destroys the object itself, so it alone is called, with
            // `this` converted to its first parameter and the size of
            // this class ([expr.delete]/7.1, [class.dtor]/16).
            let ptr = match ast.decls.cold.dtor_delete_this.get(ast.first(d)) {
                Some(&arg) => self.emit_scalar(arg)?,
                None => this.v,
            };
            return self.emit_delete_call(od, ptr, ty, loc);
        }
        // EnterDtorCleanups: the deallocation is a normal-and-EH cleanup
        // around the complete-object destructor call.
        self.push_cleanup(Cleanup::DtorDelete { op_delete: od, ptr: this.v, ty });
        let fty = ast.decls.get(ast.first(d)).ty().expect("destructor type");
        let (callee, fid) = self.structor_ref(d, Structor::Complete)?;
        let sig = self.call_sig_this(fty, false, &[], Some(rd));
        let r = self.call_vals(&sig, callee, fid, Some(this.v), &[], &[], None, &[]);
        self.pop_cleanup_block(false);
        r.map(|_| ())
    }

    /// `DestroyField`: destroys field `field` of `*this` (class `record`);
    /// `eh` as for [`Self::array_destroy_at`].
    pub(crate) fn destroy_field_of_this(&mut self, field: DeclId, record: DeclId, eh: bool) {
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
        self.destroy_object(lv.addr, q, eh);
    }

    /// clang `emitDestroy` with `destroyCXXObject`.
    pub(crate) fn destroy_object(&mut self, a: Addr, q: QualType, eh: bool) {
        let Some(rd) = self.destructed_class(q) else { return };
        if self.array_elem(q).is_some() {
            self.array_destroy_at(a, rd, eh);
        } else {
            self.call_dtor(rd, a.v, false);
        }
    }

    /// `emitArrayLength` + `emitArrayDestroy` for the constant-length
    /// array at `a` (IR type `a.ty`) of objects of class `rd`; `eh`
    /// destroys the rest if a destructor throws (clang
    /// `useEHCleanupForArray`, on the normal path only).
    pub(crate) fn array_destroy_at(&mut self, a: Addr, rd: DeclId, eh: bool) {
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
        self.array_destroy(begin, n, t, rd, eh);
    }

    /// Calls the destructor of `record` on the object at `addr` (`base`:
    /// the base-object variant). Errors go to `f.error` (cleanups cannot
    /// fail).
    pub(crate) fn call_dtor(&mut self, record: DeclId, addr: V, base: bool) {
        self.call_dtor_of(record, addr, base, false);
    }

    /// `call_dtor` where `virt` says the base-object variant destroys a
    /// virtual base of `*this` (its VTT argument, clang
    /// `GetVTTParameter(ForVirtualBase)`).
    pub(crate) fn call_dtor_of(&mut self, record: DeclId, addr: V, base: bool, virt: bool) {
        if let Err(e) = self.try_call_dtor(record, addr, base, (virt, false)) {
            self.f.error.get_or_insert(e);
        }
    }

    /// clang `CallDelegatingCtorDtor`: destroys `*this` after a delegating
    /// constructor's target returned; a base-object variant forwards the
    /// current function's VTT (`GetVTTParameter(Delegating)`).
    pub(crate) fn call_dtor_delegating(&mut self, record: DeclId, addr: V, base: bool) {
        if let Err(e) = self.try_call_dtor(record, addr, base, (false, true)) {
            self.f.error.get_or_insert(e);
        }
    }

    /// `vtt`: (virtual base, delegating) for `vtt_arg`.
    fn try_call_dtor(&mut self, record: DeclId, addr: V, base: bool, vtt: (bool, bool)) -> R<()> {
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
        let mut sig = self.call_sig_this(fty, false, &[], Some(def));
        if base && self.structor_has_vbases(first) {
            sig = self.with_vtt(sig);
            let vtt = self.vtt_arg(def, vtt.0, vtt.1)?;
            self.f.call_vtt = Some(vtt);
        }
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
            // clang `EmitExprWithCleanupsLValue`: the bound address
            // survives a cleanup block shared with a branch out of the
            // full-expression (a `co_await` suspension's destroy path),
            // `ForceCleanup({&V})` (WP-726).
            let sc = self.begin_scope(false);
            let mut r = self.emit_ref_binding(self.sub(e));
            match &mut r {
                Ok(v) => self.end_scope_reload(sc, &mut [v]),
                Err(_) => self.end_scope(sc),
            }
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
            StorageDuration::FullExpression => self.push_full_expr_cleanup(c, true, true),
            StorageDuration::Automatic => self.push_lifetime_extended_destroy(c),
            StorageDuration::Static | StorageDuration::Thread => {}
        }
    }

    /// clang `pushLifetimeExtendedDestroy`: the cleanup covers the rest
    /// of the full-expression as a deferred-deactivation cleanup
    /// (`pushDestroyAndDeferDeactivation`; its EH half destroys the
    /// temporary if a later part of the full-expression throws), and is
    /// pushed again after it (`pushCleanupAfterFullExprWithActiveFlag`).
    /// Inside a conditional branch the address is saved and both get
    /// their own `cleanup.cond` flag (`ConditionalCleanup`), the first
    /// one erased if its cleanup is never emitted.
    fn push_lifetime_extended_destroy(&mut self, c: Cleanup) {
        if self.f.fir.cur().is_none() {
            self.f.cl.lifetime_extended.push((c, None));
            return;
        }
        let in_cond = !self.f.conds.is_empty();
        let c = if in_cond { self.save_cleanup_in_cond(c) } else { c };
        let deact_flag = in_cond.then(|| self.create_cleanup_active_flag());
        let z = V::C(self.m.const_bool(false));
        let dominator =
            self.f.fir.push(Op::Bin { op: BinOp::Or, flags: Flags::NONE, l: z, r: z }, LTy::I1);
        self.push_cleanup(c.clone());
        let depth = self.f.cl.depth();
        self.f.cl.deferred.push((depth, dominator));
        if let Some(flag) = deact_flag {
            self.init_full_expr_cleanup_with_flag(flag);
            self.f.cl.stack[depth - 1].aux.push(flag.v);
        }
        let le_flag = in_cond.then(|| self.create_cleanup_active_flag());
        self.f.cl.lifetime_extended.push((c, le_flag));
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
                        self.emit_null_init(dest, q, size)?;
                    }
                }
                _ => {
                    if !self.class_bits(rd).contains(DefinitionBits::EMPTY) {
                        let def = self.class_def(rd).expect("class definition");
                        let nv = self.tys.layouter().record(def).nv_size;
                        // clang `EmitNullBaseClassInitialization`: the
                        // store range is addressed as a byte offset (0;
                        // Itanium has no vbptr gaps) from the subobject.
                        self.emit_null_base_init(dest, def, nv)?;
                    }
                }
            }
        }
        let fd = *ast.decls.function(ctor).expect("constructor");
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.construct_args(e)).collect();
        self.psabi_call_check(e, ctor, &args, true);
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
        self.construct_array_n(e, begin, elem, elem_align, nn, false)
    }

    /// The element loop of clang `EmitCXXAggrConstructorCall`: `num`
    /// objects of type `elem` from `begin`, each constructed by `e` in
    /// its own cleanup scope; a non-constant count is checked for zero
    /// first (`isempty`). With `zero_each` every element is
    /// zero-initialized before its constructor runs (an array new's
    /// value-initialization, clang's `zeroInitialize`).
    pub(crate) fn construct_array_n(
        &mut self,
        e: ExprId,
        begin: V,
        elem: LTy,
        elem_align: u64,
        num: V,
        zero_each: bool,
    ) -> R<()> {
        let ast = self.ast;
        let mut out = None;
        if self.cint(num).is_none() {
            let z = self.i64c(0);
            let empty = self.icmp(IntPredicate::Eq, num, z);
            let lp = self.f.fir.create_block();
            let o = self.f.fir.create_block();
            self.cond_br(empty, o, lp);
            self.emit_block(lp, false);
            out = Some(o);
        }
        let end = self.gep(elem, begin, vec![num], true);
        let entry = self.f.fir.cur().expect("insertion point");
        let body = self.f.fir.create_block();
        self.emit_block(body, false);
        let phi = self.f.fir.push(Op::Phi { ty: LTy::PTR, inc: vec![(begin, entry)] }, LTy::PTR);
        let sc = self.begin_scope(false);
        // The elements constructed so far are destroyed if a constructor
        // throws ([except.ctor]/3, `pushRegularPartialArrayCleanup`).
        let ctor_class = ast.decls.get(ast.exprs.constructor(e)).ctx().expect("constructor class");
        if self.opts.exceptions && self.has_nontrivial_dtor(ctor_class) {
            let c = Cleanup::PartialArray { begin, end: phi, elem, record: ctor_class };
            self.push_full_expr_cleanup(c, false, true);
        }
        let at = Addr { v: phi, ty: elem, align: elem_align };
        if zero_each && !self.class_bits(ctor_class).contains(DefinitionBits::EMPTY) {
            // clang `EmitNullInitialization` of the element.
            let size = self.alloc_size(elem);
            let mut eq = ast.exprs.get(e).ty();
            while let Some(x) = self.array_elem(eq) {
                eq = x;
            }
            self.emit_null_init(at, eq, size)?;
        }
        let r = self.construct_one(e, at, false);
        self.end_scope(sc);
        r?;
        let one = self.i64c(1);
        let next = self.gep(elem, phi, vec![one], true);
        let cur = self.f.fir.cur().expect("insertion point");
        self.f.fir.add_incoming(crate::fir::Fir::last_inst(phi), next, cur);
        let done = self.icmp(IntPredicate::Eq, next, end);
        let out = out.unwrap_or_else(|| self.f.fir.create_block());
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
            // clang `EmitAggregateCopy` ignores empty classes: an empty
            // base shares its address with another subobject ([intro.object]
            // /8), which a one-byte copy would clobber.
            if !self.class_bits(rd).contains(DefinitionBits::EMPTY) {
                let size = self.copy_size(rd, overlap);
                let c = self.memcpy(dest, src.addr.v, src.addr.align, size, false);
                let cq = self.class_type(rd);
                self.tbaa_struct_copy(c, cq);
            }
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
        // A base-object inheriting constructor that drops its parameters
        // is still called with the arguments (clang
        // `arrangeCXXConstructorCall` with `PassProtoArgs` false keeps the
        // argument list; only the callee's prototype omits them).
        let mut sig = self.call_sig_this(fty, false, &arg_tys, Some(rd));
        let needs_vtt = v == Structor::Base && self.structor_has_vbases(first);
        if needs_vtt {
            sig = self.with_vtt(sig);
        }
        let saved = self.f.dest_destructed;
        self.f.dest_destructed = false;
        let vals = self.emit_call_args(&sig, &args);
        self.f.dest_destructed = saved;
        let vals = vals?;
        if self.inlines_inheriting_ctor(ctor) {
            let vtt = if needs_vtt {
                let virt = info.kind == ConstructionKind::VirtualBase;
                let delegating = info.kind == ConstructionKind::Delegating;
                Some(self.vtt_arg(rd, virt, delegating)?)
            } else {
                None
            };
            return self.emit_inlined_inheriting_ctor(ctor, dest, v, vals, arg_tys, vtt);
        }
        if needs_vtt {
            let virt = info.kind == ConstructionKind::VirtualBase;
            let delegating = info.kind == ConstructionKind::Delegating;
            let vtt = self.vtt_arg(rd, virt, delegating)?;
            self.f.call_vtt = Some(vtt);
        }
        let (callee, fid) = self.structor_ref(ctor, v)?;
        self.call_vals(&sig, callee, fid, Some(dest.v), &vals, &args, None, &[])?;
        Ok(())
    }

    /// The bytes an aggregate copy of class `rd` writes (clang
    /// `EmitAggregateCopy`: the data size when the destination may
    /// overlap).
    /// clang `EmitAggregateCopy` of an object of class `rd`: copying an
    /// empty class is a no-op in C++ ("Ignore empty classes in C++").
    pub(crate) fn agg_copy_record(
        &mut self,
        rd: DeclId,
        dest: Addr,
        src: Addr,
        size: u64,
        volatile: bool,
    ) {
        if self.class_def(rd).is_some_and(|def| self.tys.layouter().is_empty(def)) {
            return;
        }
        let c = self.memcpy(dest, src.v, src.align, size, volatile);
        let q = self.class_type(rd);
        self.tbaa_struct_copy(c, q);
    }

    /// `EmitAggregateCopy` of an object of type `q` (see
    /// [`Self::agg_copy_record`]); the copy carries `!tbaa.struct`.
    pub(crate) fn agg_copy(&mut self, q: QualType, dest: Addr, src: Addr, size: u64, vol: bool) {
        if let TypeView::Record(rd) = self.ast.types.canonical_view(q) {
            self.agg_copy_record(rd, dest, src, size, vol);
        } else {
            let c = self.memcpy(dest, src.v, src.align, size, vol);
            self.tbaa_struct_copy(c, q);
        }
    }

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
        e: ExprId,
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
        self.psabi_call_check(e, md, args, self.this_class_of(md).is_some());
        if self.this_class_of(md).is_none() {
            // A static member function called through an object.
            self.emit_ignored(base)?;
            let f = self.get_fn(md)?;
            let mut sig = self.call_sig(fty, false, &arg_tys);
            let (extra, nr) = self.call_extra(md, fty, &mut sig);
            let r =
                self.call_with(&sig, V::C(Value::func(f)), Some(f), None, args, dest, &extra)?;
            if nr {
                self.finish_noreturn_call();
            }
            return Ok(r);
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
        let rd = ast.decls.get(md).ctx().expect("method class");
        let trivial = ast.decls.function(md).is_some_and(|f| f.spec.is_trivial())
            || (self.is_union(rd) && self.is_defaulted(md));
        let kind = ast.decls.get(md).kind();
        let trivial_assign = trivial
            && matches!(self.special_of(md), Some(Special::CopyAssign | Special::MoveAssign));
        // clang: an assignment operator call (`a = b`, `a += b`, ...)
        // evaluates its right operand before the object
        // ([expr.ass]/1, [over.match.oper]/2; `EvaluationOrder::
        // ForceRightToLeft`): the trivial assignment's source lvalue, or
        // the call arguments.
        let rtl = ast.exprs.get(e).kind() == ExprKind::CXXOperatorCallExpr
            && is_assignment_operator(ast.exprs.operator_call_op(e));
        let mut rtl_src = None;
        let mut rtl_vals = None;
        if rtl && trivial_assign {
            rtl_src = Some(self.emit_lvalue(args[0])?);
        }
        let crd = ast.decls.get(callee_md).ctx().expect("method class");
        let cfty = ast.decls.get(callee_md).ty().expect("method type");
        if rtl && !trivial {
            let sig = self.call_sig_this(cfty, false, &arg_tys, Some(crd));
            rtl_vals = Some(self.emit_call_args(&sig, args)?);
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
        if trivial {
            if kind == DeclKind::CXXDestructor {
                return Ok(None);
            }
            if trivial_assign {
                let src = match rtl_src {
                    Some(src) => src,
                    None => self.emit_lvalue(args[0])?,
                };
                // clang `EmitAggregateCopy`: empty classes are ignored.
                if !self.class_bits(rd).contains(DefinitionBits::EMPTY) {
                    let size = self.copy_size(rd, true);
                    let c = self.memcpy(this, src.addr.v, src.addr.align, size, false);
                    let cq = self.class_type(rd);
                    self.tbaa_struct_copy(c, cq);
                }
                return Ok(Some(this.v));
            }
        }
        if kind == DeclKind::CXXDestructor {
            let sig = self.call_sig_this(cfty, false, &[], Some(crd));
            if use_virtual {
                let fp = self.virtual_fn_ptr(md, Some(Structor::Complete), this.v, this.align)?;
                return self.call_vals(&sig, fp, None, Some(this.v), &[], &[], None, &[]);
            }
            let (callee, fid) = self.structor_ref(callee_md, Structor::Complete)?;
            return self.call_vals(&sig, callee, fid, Some(this.v), &[], &[], None, &[]);
        }
        let mut sig = self.call_sig_this(cfty, false, &arg_tys, Some(crd));
        if use_virtual {
            // The arguments come first; `prepareConcreteCallee` then
            // loads the slot.
            let vals = match rtl_vals {
                Some(v) => v,
                None => self.emit_call_args(&sig, args)?,
            };
            let (mut extra, nr) = self.call_extra(md, cfty, &mut sig);
            // The allocation attributes of the method named by the call
            // (clang `ConstructAttributeList` with the virtual callee's
            // `CGCalleeInfo`), then its `alloc_align` assumption.
            let mut ret_extra = Vec::new();
            self.alloc_call_attrs(md, &vals, &mut extra, &mut ret_extra);
            let fp = crate::expr::Callee::Virtual { md, this: this.v, align: this.align };
            let this = Some(this.v);
            let r =
                self.call_vals_to(&sig, fp, None, this, &vals, args, dest, &extra, &ret_extra)?;
            if let Some(p) = r {
                self.alloc_align_assume(md, &vals, p);
            }
            if nr {
                self.finish_noreturn_call();
            }
            return Ok(r);
        }
        let f = self.get_fn(callee_md)?;
        let (mut extra, nr) = self.call_extra(callee_md, cfty, &mut sig);
        let vals = match rtl_vals {
            Some(v) => v,
            None => self.emit_call_args(&sig, args)?,
        };
        let mut ret_extra = Vec::new();
        self.alloc_call_attrs(callee_md, &vals, &mut extra, &mut ret_extra);
        let callee = V::C(Value::func(f));
        let this = Some(this.v);
        let r =
            self.call_vals_ret(&sig, callee, Some(f), this, &vals, args, dest, &extra, &ret_extra)?;
        if let Some(p) = r {
            self.alloc_align_assume(callee_md, &vals, p);
        }
        if nr {
            self.finish_noreturn_call();
        }
        Ok(r)
    }

    /// The call-site attributes of a call of `d` (`noreturn` per
    /// `noreturn_call`) and whether `unreachable` follows it.
    fn noreturn_extra(&self, d: Option<DeclId>, fty: QualType) -> (Vec<kiln_llvm::Attr>, bool) {
        let (site, unreachable) = self.noreturn_call(d, fty);
        (
            if site {
                vec![kiln_llvm::Attr::Flag(kiln_llvm::AttrKind::NoReturn)]
            } else {
                Vec::new()
            },
            unreachable,
        )
    }

    /// [`Cg::noreturn_extra`] plus the `const` / `pure` attributes of
    /// method `d` (clang `ConstructAttributeList` on a member call's
    /// target decl, the method named even when the call is virtual),
    /// which also make the call `nounwind`.
    fn call_extra(&self, d: DeclId, fty: QualType, sig: &mut Sig) -> (Vec<kiln_llvm::Attr>, bool) {
        let (mut extra, nr) = self.noreturn_extra(Some(d), fty);
        sig.nothrow |= crate::builtins::const_pure_attrs(self.ast, d, sig, &mut extra);
        (extra, nr)
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

    /// `d` is an explicit object member function ([dcl.fct]/6): called
    /// like a non-member function, the object being its first argument.
    pub(crate) fn is_explicit_object_method(&self, d: DeclId) -> bool {
        matches!(self.ast.decls.get(d).kind(), DeclKind::CXXMethod | DeclKind::CXXConversion)
            && self.ast.decls.function(d).is_some_and(|f| f.spec.has_explicit_object())
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
        let dalign =
            self.class_def(derived).map_or(1, |d| u64::from(self.tys.layouter().record(d).align));
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
        self.tbaa_vptr(vt);
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
            self.bin(BinOp::Add, Flags::NONE, voff, c)
        };
        let adj = self.gep(LTy::I8, this.v, vec![off], true);
        let align = min_align(self.vbase_align(this.align, derived, vb), nv);
        let Some((_, end)) = blocks else { return Ok((adj, align)) };
        let from = self.f.fir.cur().expect("insertion point");
        self.emit_block(end, false);
        let orig = orig.expect("insertion point");
        let phi = self
            .f
            .fir
            .push(Op::Phi { ty: LTy::PTR, inc: vec![(adj, from), (null, orig)] }, LTy::PTR);
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

    /// clang `GetAddressOfDerivedClass` on a pointer: `p - off`
    /// (`getelementptr inbounds i8, p, -off`), null-checked when
    /// `null_check` and the offset is not zero, through clang's
    /// `cast.notnull` / `cast.null` / `cast.end` blocks.
    pub(crate) fn derived_ptr(&mut self, p: V, off: u64, null_check: bool) -> V {
        if off == 0 {
            return p;
        }
        let neg = |cg: &mut Self| {
            let o = cg.i64c(-i128::from(off));
            cg.gep(LTy::I8, p, vec![o], true)
        };
        if !null_check {
            return neg(self);
        }
        let cast_null = self.f.fir.create_block();
        let not_null = self.f.fir.create_block();
        let end = self.f.fir.create_block();
        let null = V::C(self.m.const_null(LTy::PTR));
        let is_null = self.icmp(IntPredicate::Eq, p, null);
        self.cond_br(is_null, cast_null, not_null);
        self.emit_block(not_null, false);
        let adj = neg(self);
        let from = self.f.fir.cur().expect("insertion point");
        self.emit_branch(end);
        self.emit_block(cast_null, false);
        self.emit_block(end, false);
        self.f
            .fir
            .push(Op::Phi { ty: LTy::PTR, inc: vec![(adj, from), (null, cast_null)] }, LTy::PTR)
    }

    /// The non-virtual offset of the base a base-to-derived cast `e` to
    /// class `derived` converts from (clang `GetNonVirtualBaseClassOffset`;
    /// sema rejects a path through a virtual base, [expr.static.cast]/2).
    pub(crate) fn derived_cast_offset(&mut self, e: ExprId, derived: DeclId) -> R<u64> {
        match self.base_path(e, derived)? {
            (None, off) => Ok(off),
            (Some(_), _) => err(self.loc_of(e), "base-to-derived cast from a virtual base"),
        }
    }

    /// `QualType::isPODType` (C++11: trivial and standard-layout classes,
    /// arrays of them, and every non-class type).
    pub(crate) fn is_pod(&self, q: QualType) -> bool {
        is_pod(self.ast, q)
    }
}

/// `QualType::isPODType` (C++11: trivial and standard-layout classes,
/// arrays of them, and every non-class type).
pub(crate) fn is_pod(ast: &kiln_ast::Ast, q: QualType) -> bool {
    let mut q = q;
    loop {
        q = match ast.types.canonical_view(q) {
            TypeView::ConstantArray(a) => a.elem,
            TypeView::IncompleteArray(e) | TypeView::VariableArray { elem: e, .. } => e,
            _ => break,
        };
    }
    match ast.types.as_record(q) {
        Some(r) => {
            let b = ast
                .definition(r)
                .and_then(|d| ast.decls.record(d))
                .map_or(DefinitionBits::empty(), |r| r.defn);
            b.contains(DefinitionBits::TRIVIALLY_COPYABLE)
                && b.contains(DefinitionBits::TRIVIAL_DEFAULT_CTOR)
                && b.contains(DefinitionBits::STANDARD_LAYOUT)
        }
        None => true,
    }
}

/// clang `CXXOperatorCallExpr::isAssignmentOp`.
fn is_assignment_operator(op: kiln_ast::names::OverloadedOperator) -> bool {
    use kiln_ast::names::OverloadedOperator as O;
    matches!(
        op,
        O::Equal
            | O::StarEqual
            | O::SlashEqual
            | O::PercentEqual
            | O::PlusEqual
            | O::MinusEqual
            | O::LessLessEqual
            | O::GreaterGreaterEqual
            | O::AmpEqual
            | O::CaretEqual
            | O::PipeEqual
    )
}
