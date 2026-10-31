//! Module-level emission (spec/codegen.md §2): the declaration maps,
//! `EmitGlobal`/`EmitDeferred` ordering, linkage, attributes, globals,
//! string literals and the module trailer.

use kiln_abi::call::{ArgKind, CallInfo, classify_call};
use kiln_abi::irtype::{IrType, IrTypes, record_kind_prefix};
use kiln_abi::mangle;
use kiln_abi::{Arch, Target};
use kiln_ast::decl::{
    ConstexprKind, DeclFlags, DeclKind, DefinitionBits, TemplLinkKind, VarStorage,
};
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{Ast, DeclId, QualType};
use kiln_llvm::{
    Attr, AttrKind, AttrSetId, ComdatKind, FuncId, GlobalId, Linkage, Module, TypeId as LTy,
    UnnamedAddr, UwTableKind, Value,
};
use kiln_source::Loc;
use kiln_source::fx::{FxHashMap, FxHashSet};

use crate::fold::Folds;
use crate::func::Fx;
use crate::{CodegenError, CodegenOptions};

/// Result of an emission step.
pub(crate) type R<T> = Result<T, CodegenError>;

/// An error at `loc`.
pub(crate) fn err<T>(loc: Option<Loc>, msg: impl Into<String>) -> R<T> {
    Err(CodegenError { loc, msg: msg.into() })
}

/// The lowered signature of a function type (clang `CGFunctionInfo` plus
/// the IR function type).
#[derive(Clone, Debug)]
pub(crate) struct Sig {
    /// ABI classification.
    pub info: CallInfo,
    /// The IR function type.
    pub fn_ty: LTy,
    /// The source return type.
    pub ret: QualType,
    /// The source parameter types.
    pub params: Vec<QualType>,
    /// The prototype is `noexcept`.
    pub nothrow: bool,
    /// The class of `*this` for an implicit object parameter, passed
    /// after the `sret` pointer and before `info.args` (clang's
    /// `arrangeCXXMethodType` prepends it; it is classified with the
    /// parameters but kept out of `info.args`/`params`).
    pub this: Option<DeclId>,
    /// A VTT parameter follows `this` (clang `ItaniumCXXABI::
    /// buildStructorSignature` for base-object structors of classes with
    /// virtual bases).
    pub vtt: bool,
}

impl Sig {
    /// The return is `sret`.
    pub fn sret(&self) -> bool {
        matches!(self.info.ret.kind, ArgKind::Indirect { .. })
    }
}

/// A definition to emit (clang `DeferredDeclsToEmit` entry).
#[derive(Clone, Copy, Debug)]
pub(crate) enum Def {
    /// A function or variable definition.
    Decl(DeclId),
    /// One variant of a constructor or destructor definition.
    Structor(DeclId, mangle::Structor),
}

/// Module emission state.
pub(crate) struct Cg<'a> {
    pub ast: &'a Ast,
    pub folds: &'a Folds,
    pub tys: IrTypes<'a>,
    pub target: &'a Target,
    pub opts: &'a CodegenOptions,
    pub m: Module,
    /// First declaration → function.
    pub fns: FxHashMap<DeclId, FuncId>,
    /// Function → its definition carries `nounwind`.
    pub nounwind: FxHashMap<FuncId, bool>,
    /// First declaration → global variable.
    pub gvars: FxHashMap<DeclId, GlobalId>,
    /// Discardable definitions not yet needed: first declaration → definition.
    pub deferred: FxHashMap<DeclId, DeclId>,
    /// Definitions to emit (clang `DeferredDeclsToEmit`).
    pub to_emit: Vec<Def>,
    /// String literal globals by (array type, bytes).
    pub strings: FxHashMap<(LTy, Vec<u8>), GlobalId>,
    /// Named struct types by record.
    pub named: FxHashMap<DeclId, LTy>,
    /// Base-subobject (`.base`) struct types by record.
    pub named_base: FxHashMap<DeclId, LTy>,
    /// Names already used for named struct types.
    pub struct_names: FxHashMap<String, u32>,
    /// The `.constant` globals of frozen constant objects (clang
    /// `GetAddrOfUnnamedGlobalConstantDecl`), by value.
    pub const_objects: FxHashMap<kiln_ast::ConstValueId, GlobalId>,
    /// The outermost default argument being emitted that holds a
    /// source-location builtin (clang `SourceLocExprScopeGuard`).
    pub srcloc: Option<kiln_ast::ExprId>,
    /// Static locals already emitted.
    pub static_locals: FxHashMap<DeclId, GlobalId>,
    /// Static locals emitted per (function, name), for discriminators.
    pub static_local_names: FxHashMap<(Option<DeclId>, String), u32>,
    /// `void *`, used to classify implicit object parameters.
    pub void_ptr: QualType,
    /// Constructor/destructor variants: (first declaration, variant) →
    /// the symbol (a function or an alias) and the function it names.
    pub sfns: FxHashMap<(DeclId, mangle::Structor), (Value, FuncId)>,
    /// Discardable structor variants not yet needed.
    pub sdeferred: FxHashMap<(DeclId, mangle::Structor), DeclId>,
    /// The [`crate::fir::V::P`] placeholders: their symbols once known.
    pub phs: Vec<Option<Value>>,
    /// Placeholder by (first declaration, variant).
    pub ph_of: FxHashMap<(DeclId, mangle::Structor), u32>,
    /// Structor variants whose definitions are queued or emitted.
    pub squeued: FxHashSet<(DeclId, mangle::Structor)>,
    /// Classes' structors whose complete variant was referenced.
    pub c1_decl: FxHashSet<DeclId>,
    /// Structors whose complete variant is defined (alias or RAUW).
    pub c1_done: FxHashSet<DeclId>,
    /// Deleting destructors already emitted (WP-080).
    pub d0_done: FxHashSet<DeclId>,
    /// Thunk functions by mangled name (WP-080).
    pub thunk_fns: FxHashMap<String, FuncId>,
    /// Structors whose base variant body was generated.
    pub sdone: FxHashSet<DeclId>,
    /// `_ZTV` globals by class (WP-080).
    pub vtables: FxHashMap<DeclId, GlobalId>,
    /// Vtable group layouts by class.
    pub vt_layouts: FxHashMap<DeclId, std::rc::Rc<kiln_ast::class::VTableLayout>>,
    /// Vtables referenced but not yet defined (clang `DeferredVTables`).
    pub deferred_vtables: Vec<DeclId>,
    /// Vtables already defined.
    pub vt_emitted: FxHashSet<DeclId>,
    /// `_ZTI` definitions by mangled name.
    pub rtti_globals: FxHashMap<String, GlobalId>,
    /// Bodies using placeholders, lowered once the module is complete.
    pub pending: Vec<(FuncId, DeclId, crate::fir::Fir)>,
    /// Per-function state.
    pub f: Fx,
}

impl<'a> Cg<'a> {
    pub fn new(
        ast: &'a Ast,
        folds: &'a Folds,
        target: &'a Target,
        opts: &'a CodegenOptions,
        void_ptr: QualType,
    ) -> Self {
        let spec = match target.triple.arch {
            Arch::AArch64 => kiln_llvm::AARCH64_LINUX_GNU,
            Arch::X86_64 => kiln_llvm::X86_64_LINUX_GNU,
        };
        Cg {
            ast,
            folds,
            tys: IrTypes::new(ast, target),
            target,
            opts,
            m: Module::with_target(&opts.source_filename, spec),
            fns: FxHashMap::default(),
            nounwind: FxHashMap::default(),
            gvars: FxHashMap::default(),
            deferred: FxHashMap::default(),
            to_emit: Vec::new(),
            strings: FxHashMap::default(),
            named: FxHashMap::default(),
            named_base: FxHashMap::default(),
            struct_names: FxHashMap::default(),
            const_objects: FxHashMap::default(),
            srcloc: None,
            static_locals: FxHashMap::default(),
            static_local_names: FxHashMap::default(),
            void_ptr,
            sfns: FxHashMap::default(),
            sdeferred: FxHashMap::default(),
            phs: Vec::new(),
            ph_of: FxHashMap::default(),
            squeued: FxHashSet::default(),
            c1_decl: FxHashSet::default(),
            c1_done: FxHashSet::default(),
            d0_done: FxHashSet::default(),
            thunk_fns: FxHashMap::default(),
            sdone: FxHashSet::default(),
            vtables: FxHashMap::default(),
            vt_layouts: FxHashMap::default(),
            deferred_vtables: Vec::new(),
            vt_emitted: FxHashSet::default(),
            rtti_globals: FxHashMap::default(),
            pending: Vec::new(),
            f: Fx::empty(),
        }
    }

    pub fn is_x86(&self) -> bool {
        self.target.triple.arch == Arch::X86_64
    }

    // ---- driving -------------------------------------------------------

    /// Emits the whole translation unit.
    pub fn run(mut self) -> R<Module> {
        let tu = self.ast.tu;
        self.emit_members(tu)?;
        self.emit_key_function_vtables()?;
        self.emit_deferred()?;
        self.resolve_placeholders()?;
        self.trailer();
        Ok(self.m)
    }

    fn emit_members(&mut self, ctx: DeclId) -> R<()> {
        let ast = self.ast;
        let members: Vec<DeclId> = ast.decls.members(ctx).collect();
        for d in members {
            let n = ast.decls.get(d);
            if n.has(DeclFlags::INVALID) {
                continue;
            }
            match n.kind() {
                DeclKind::Namespace | DeclKind::LinkageSpec | DeclKind::Export => {
                    self.emit_members(d)?;
                }
                DeclKind::Function | DeclKind::CXXMethod | DeclKind::CXXConversion => {
                    self.emit_global_fn(d)?;
                }
                DeclKind::CXXConstructor | DeclKind::CXXDestructor => {
                    self.emit_global_structor(d)?;
                }
                DeclKind::Var => self.emit_global_var(d)?,
                // clang `EmitTopLevelDecl(CXXRecord)`: static data members
                // and nested classes; inline member functions are emitted
                // when first used.
                DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization => {
                    self.emit_class_members(d)?;
                }
                DeclKind::FunctionTemplate | DeclKind::ClassTemplate | DeclKind::VarTemplate => {}
                _ => {}
            }
        }
        Ok(())
    }

    /// clang `EmitGlobal` for a function.
    fn emit_global_fn(&mut self, d: DeclId) -> R<()> {
        let ast = self.ast;
        let Some(fd) = ast.decls.function(d) else { return Ok(()) };
        if fd.body.is_none() || matches!(fd.templ.get(), Some((TemplLinkKind::Pattern, _))) {
            return Ok(());
        }
        if fd.spec.constexpr_kind() == ConstexprKind::Consteval {
            return Ok(());
        }
        let first = ast.first(d);
        if !self.discardable_fn(d) {
            return self.emit_fn_def(d);
        }
        if let Some(&f) = self.fns.get(&first) {
            if self.m.func(f).is_declaration() {
                self.to_emit.push(Def::Decl(d));
            }
        } else {
            self.deferred.insert(first, d);
        }
        Ok(())
    }

    /// The static data members and nested classes of class `d`.
    fn emit_class_members(&mut self, d: DeclId) -> R<()> {
        let ast = self.ast;
        if ast.decls.get(d).has(DeclFlags::INVALID) {
            return Ok(());
        }
        let members: Vec<DeclId> = ast.decls.members(d).collect();
        for m in members {
            let n = ast.decls.get(m);
            if n.has(DeclFlags::INVALID) {
                continue;
            }
            match n.kind() {
                // An in-class static data member declaration is a
                // definition only when inline ([class.static.data]/3,4).
                DeclKind::Var if self.var_is_inline(m) => self.emit_global_var(m)?,
                DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization => {
                    self.emit_class_members(m)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// clang `EmitGlobal` for a variable.
    fn emit_global_var(&mut self, d: DeclId) -> R<()> {
        let ast = self.ast;
        let Some(v) = ast.decls.var(d) else { return Ok(()) };
        if v.spec.storage() == VarStorage::Extern && v.init.is_none() {
            return Ok(());
        }
        if matches!(v.templ.get(), Some((TemplLinkKind::Pattern, _))) {
            return Ok(());
        }
        let first = ast.first(d);
        if !self.discardable_var(d) {
            return self.emit_var_def(d);
        }
        if self.gvars.contains_key(&first) {
            self.to_emit.push(Def::Decl(d));
        } else {
            self.deferred.insert(first, d);
        }
        Ok(())
    }

    /// clang `EmitDeferred`.
    fn emit_deferred(&mut self) -> R<()> {
        if !self.deferred_vtables.is_empty() {
            self.emit_deferred_vtables()?;
        }
        let cur = std::mem::take(&mut self.to_emit);
        for def in cur {
            let d = match def {
                Def::Decl(d) => d,
                Def::Structor(d, v) => {
                    self.emit_structor_def(d, v)?;
                    if !self.to_emit.is_empty() || !self.deferred_vtables.is_empty() {
                        self.emit_deferred()?;
                    }
                    continue;
                }
            };
            let first = self.ast.first(d);
            if self.ast.decls.get(d).kind() == DeclKind::Var {
                let still_decl = self
                    .gvars
                    .get(&first)
                    .is_none_or(|&g| self.m.global(g).initializer().is_none());
                if still_decl {
                    self.emit_var_def(d)?;
                }
            } else {
                let still_decl =
                    self.fns.get(&first).is_none_or(|&f| self.m.func(f).is_declaration());
                if still_decl {
                    self.emit_fn_def(d)?;
                }
            }
            if !self.to_emit.is_empty() || !self.deferred_vtables.is_empty() {
                self.emit_deferred()?;
            }
        }
        Ok(())
    }

    // ---- linkage -------------------------------------------------------

    /// Whether `d` is (a member of) an implicit template instantiation,
    /// which has vague (`linkonce_odr`) linkage.
    pub(crate) fn is_implicit_instantiation(&self, d: DeclId) -> bool {
        let ast = self.ast;
        let first = ast.first(d);
        match ast.templ_link(first).get() {
            Some((TemplLinkKind::MemberOfSpecialization, _)) => {
                let ctx = ast.decls.get(first).ctx();
                ctx.is_none_or(|c| match ast.spec_info(c) {
                    Some((_, s)) => s.kind == SpecKind::ImplicitInstantiation,
                    None => true,
                })
            }
            Some((TemplLinkKind::Specialization, _)) => {
                ast.spec_info(first).is_some_and(|(_, s)| s.kind == SpecKind::ImplicitInstantiation)
            }
            _ => false,
        }
    }

    /// The definition of a function whose definition is emitted on first
    /// use but is not reached by the top-level walk (a member function
    /// defined in its class, an implicit instantiation).
    pub(crate) fn inline_def(&self, first: DeclId) -> Option<DeclId> {
        let ast = self.ast;
        ast.redecls_of(first).find(|&r| {
            let Some(f) = ast.decls.function(r) else { return false };
            if f.body.is_none() || matches!(f.templ.get(), Some((TemplLinkKind::Pattern, _))) {
                return false;
            }
            if f.spec.constexpr_kind() == ConstexprKind::Consteval {
                return false;
            }
            let in_class = ast.decls.lexical_ctx(r).is_some_and(|c| {
                matches!(
                    ast.decls.get(c).kind(),
                    DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization
                )
            });
            in_class || self.is_implicit_instantiation(r)
        })
    }

    pub fn fn_is_inline(&self, d: DeclId) -> bool {
        let ast = self.ast;
        self.is_implicit_instantiation(d)
            || ast.redecls_of(d).any(|r| {
                ast.decls.function(r).is_some_and(|f| {
                    f.spec.is_inline()
                        || f.spec.implicit_inline()
                        || f.spec.constexpr_kind() != ConstexprKind::None
                })
            })
    }

    pub fn fn_internal(&self, d: DeclId) -> bool {
        mangle::has_internal_linkage(self.ast, d)
    }

    pub(crate) fn discardable_fn(&self, d: DeclId) -> bool {
        self.fn_internal(d) || self.fn_is_inline(d)
    }

    fn var_is_inline(&self, d: DeclId) -> bool {
        let ast = self.ast;
        ast.redecls_of(d).any(|r| ast.decls.var(r).is_some_and(|v| v.spec.is_inline()))
    }

    fn discardable_var(&self, d: DeclId) -> bool {
        mangle::has_internal_linkage(self.ast, d) || self.var_is_inline(d)
    }

    // ---- types ---------------------------------------------------------

    /// The IR type of `t`.
    pub fn ir(&mut self, t: &IrType) -> LTy {
        match t {
            IrType::Void => LTy::VOID,
            IrType::Int(1) => LTy::I1,
            IrType::Int(8) => LTy::I8,
            IrType::Int(16) => LTy::I16,
            IrType::Int(32) => LTy::I32,
            IrType::Int(64) => LTy::I64,
            IrType::Int(128) => LTy::I128,
            IrType::Int(n) => self.m.types.int(*n),
            IrType::Ptr => LTy::PTR,
            IrType::Half => LTy::HALF,
            IrType::BFloat => LTy::BFLOAT,
            IrType::Float => LTy::FLOAT,
            IrType::Double => LTy::DOUBLE,
            IrType::X86Fp80 => LTy::X86_FP80,
            IrType::Fp128 => LTy::FP128,
            IrType::Array(e, n) => {
                let e = self.ir(e);
                self.m.types.array(e, *n)
            }
            IrType::Vector(e, n) => {
                let e = self.ir(e);
                self.m.types.vector(e, u32::try_from(*n).unwrap_or(u32::MAX))
            }
            IrType::Record(d) => self.record_ty(*d),
            IrType::RecordBase(d) => {
                let first = self.ast.first(*d);
                self.record_ty(first);
                self.named_base.get(&first).copied().unwrap_or_else(|| self.record_ty(first))
            }
            IrType::Struct(fs, packed) => {
                let fs: Vec<LTy> = fs.iter().map(|x| self.ir(x)).collect();
                self.m.types.struct_literal(&fs, *packed)
            }
        }
    }

    /// The named struct type of record `d`.
    pub fn record_ty(&mut self, d: DeclId) -> LTy {
        let first = self.ast.first(d);
        if let Some(&t) = self.named.get(&first) {
            return t;
        }
        let base =
            format!("{}.{}", record_kind_prefix(self.ast, first), self.qualified_name(first));
        let n = self.struct_names.entry(base.clone()).or_insert(0);
        let name = if *n == 0 { base } else { format!("{base}.{n}") };
        *n += 1;
        let t = self.m.types.named_struct(&name);
        self.named.insert(first, t);
        let rec = self.tys.record(first).clone();
        let fields: Vec<LTy> = rec.members.iter().map(|x| self.ir(x)).collect();
        // clang `ComputeRecordLayout`: the base-subobject type is created
        // before the complete type's body is filled in.
        if let Some(bm) = &rec.base_members {
            let bt = self.m.types.named_struct(&format!("{name}.base"));
            self.named_base.insert(first, bt);
            let bf: Vec<LTy> = bm.iter().map(|x| self.ir(x)).collect();
            let _ = self.m.types.set_body(bt, &bf, rec.packed);
        }
        let _ = self.m.types.set_body(t, &fields, rec.packed);
        t
    }

    fn qualified_name(&self, d: DeclId) -> String {
        let ast = self.ast;
        let mut parts = Vec::new();
        let mut cur = Some(d);
        while let Some(x) = cur {
            let n = ast.decls.get(x);
            match n.kind() {
                DeclKind::TranslationUnit => break,
                DeclKind::LinkageSpec | DeclKind::Export => {}
                DeclKind::Namespace => {
                    let s = mangle::decl_name(ast, x);
                    parts.push(if s.is_empty() { "(anonymous namespace)".to_owned() } else { s });
                }
                _ => {
                    let s = mangle::decl_name(ast, x);
                    parts.push(if s.is_empty() { "anon".to_owned() } else { s });
                }
            }
            cur = n.ctx();
        }
        parts.reverse();
        parts.join("::")
    }

    /// The IR type of a value of type `q` (`ConvertType`).
    pub fn lty(&mut self, q: QualType) -> LTy {
        let t = self.tys.convert(q);
        self.ir(&t)
    }

    /// The in-memory IR type of `q` (`ConvertTypeForMem`).
    pub fn mty(&mut self, q: QualType) -> LTy {
        let t = self.tys.convert_mem(q);
        self.ir(&t)
    }

    /// `sizeof` and `alignof` of `q`.
    pub fn size_align(&mut self, q: QualType) -> (u64, u64) {
        let l = self.tys.layouter().type_layout(q);
        (l.size, l.align)
    }

    /// The referee of a reference type.
    pub fn referee(&self, q: QualType) -> Option<QualType> {
        match self.ast.types.canonical_view(q) {
            TypeView::LValueRef(p) | TypeView::RValueRef(p) => Some(p),
            _ => None,
        }
    }

    /// clang `getMinimumObjectSize` and `getNaturalTypeAlignment(T,
    /// forPointeeType)` of an object reached through a pointer or
    /// reference: `(dereferenceable bytes, alignment)`, the size `None` for
    /// an incomplete type (whose alignment is 1). Classes use their
    /// non-virtual size and alignment.
    pub fn pointee_layout(&mut self, p: QualType) -> (Option<u64>, u64) {
        let ast = self.ast;
        match ast.types.canonical_view(p) {
            // getMinimumClassObjectSize / getClassPointerAlignment: a
            // final class is known to be a complete object.
            TypeView::Record(d) => self.this_layout(d),
            TypeView::IncompleteArray(_) | TypeView::Function(_) => (None, 1),
            _ if ast.types.is_void(p) => (None, 1),
            _ => {
                let (s, a) = self.size_align(p);
                (Some(s), a)
            }
        }
    }

    /// The `dereferenceable` size and alignment of `this` pointing to an
    /// object of class `rd` (clang `getMinimumClassObjectSize` and
    /// `getClassPointerAlignment`).
    pub fn this_layout(&mut self, rd: DeclId) -> (Option<u64>, u64) {
        match self.ast.definition(rd) {
            Some(def) => {
                let fin = self.class_is_final(def);
                let eff = fin || self.dtor_is_final(def);
                let l = self.tys.layouter().record(def);
                let size = if eff { l.size } else { l.nv_size.max(1) };
                let align = if eff { l.align } else { l.nv_align };
                (Some(size), u64::from(align))
            }
            None => (None, 1),
        }
    }

    /// The class has the `final` class-virt-specifier (clang
    /// `hasAttr<FinalAttr>`).
    pub fn class_is_final(&self, def: DeclId) -> bool {
        self.ast.decls.record(def).is_some_and(|r| r.defn.contains(DefinitionBits::FINAL))
    }

    /// The destructor is declared `final` (the rest of clang
    /// `isEffectivelyFinal`).
    fn dtor_is_final(&self, def: DeclId) -> bool {
        self.ast
            .destructor(def)
            .and_then(|d| self.ast.decls.function(d))
            .is_some_and(|f| f.spec.is_final())
    }

    /// clang `ConstructAttributeList` for a reference parameter or return:
    /// `nonnull align A dereferenceable(N)`.
    fn ref_attrs(&mut self, q: QualType, out: &mut Vec<Attr>) {
        let Some(p) = self.referee(q) else { return };
        out.push(Attr::Flag(AttrKind::NonNull));
        if self.ast.types.is_function(p) {
            return;
        }
        let (size, align) = self.pointee_layout(p);
        if let Some(n) = size.filter(|&n| n > 0) {
            out.push(Attr::Dereferenceable(n));
        }
        out.push(Attr::Align(align));
    }

    pub fn is_bool(&self, q: QualType) -> bool {
        matches!(self.ast.types.canonical_view(q), TypeView::Builtin(BuiltinType::Bool))
    }

    pub fn is_signed(&self, q: QualType) -> bool {
        kiln_abi::irtype::is_signed_or_enum(self.ast, q, self.target)
    }

    // ---- signatures ----------------------------------------------------

    /// The lowered signature of function type `fty`.
    pub fn sig(&mut self, fty: QualType, extern_c: bool) -> Sig {
        self.sig_this(fty, extern_c, None)
    }

    /// The lowered signature of function type `fty` with an implicit
    /// object parameter of type `this*` when `this` is set.
    pub fn sig_this(&mut self, fty: QualType, extern_c: bool, this: Option<DeclId>) -> Sig {
        let ast = self.ast;
        let p = *ast.types.proto(fty).expect("function prototype");
        let params: Vec<QualType> = ast.types.params(&p).to_vec();
        let ret = p.ret();
        let nothrow = ast.types.exc_spec(&p).is_nothrow();
        let variadic = p.variadic();
        let info = self.classify(ret, &params, params.len(), !extern_c, this.is_some());
        let fn_ty = self.fn_type(&info, this.is_some(), variadic);
        Sig { info, fn_ty, ret, params, nothrow, this, vtt: false }
    }

    /// clang `arrangeLLVMFunctionInfo`: classifies `this` (when present)
    /// with the parameters, then splits it off.
    fn classify(
        &mut self,
        ret: QualType,
        params: &[QualType],
        n_required: usize,
        strict_return: bool,
        this: bool,
    ) -> CallInfo {
        if !this {
            return classify_call(&mut self.tys, ret, params, n_required, strict_return);
        }
        let mut all = Vec::with_capacity(params.len() + 1);
        all.push(self.void_ptr);
        all.extend_from_slice(params);
        let mut info = classify_call(&mut self.tys, ret, &all, n_required + 1, strict_return);
        info.args.remove(0);
        info
    }

    /// The signature of a call with the given argument types (variadic
    /// extras appended).
    pub fn call_sig(&mut self, fty: QualType, extern_c: bool, args: &[QualType]) -> Sig {
        self.call_sig_this(fty, extern_c, args, None)
    }

    /// [`Cg::call_sig`] with an implicit object argument.
    pub fn call_sig_this(
        &mut self,
        fty: QualType,
        extern_c: bool,
        args: &[QualType],
        this: Option<DeclId>,
    ) -> Sig {
        let ast = self.ast;
        let p = *ast.types.proto(fty).expect("function prototype");
        let params: Vec<QualType> = ast.types.params(&p).to_vec();
        let ret = p.ret();
        let mut all = params.clone();
        all.extend_from_slice(&args[params.len().min(args.len())..]);
        let t = this.is_some();
        let info = self.classify(ret, &all, params.len(), !extern_c, t);
        let decl_info = self.classify(ret, &params, params.len(), !extern_c, t);
        let fn_ty = self.fn_type(&decl_info, t, p.variadic());
        let nothrow = ast.types.exc_spec(&p).is_nothrow();
        Sig { info, fn_ty, ret, params: all, nothrow, this, vtt: false }
    }

    /// `sig` with a VTT parameter after `this`.
    pub(crate) fn with_vtt(&mut self, mut sig: Sig) -> Sig {
        if sig.vtt {
            return sig;
        }
        let Some((ret, ps, variadic)) = self.m.types.function_info(sig.fn_ty) else {
            return sig;
        };
        let mut ps = ps.to_vec();
        let at = usize::from(sig.sret()) + 1;
        ps.insert(at, LTy::PTR);
        sig.fn_ty = self.m.types.function(ret, &ps, variadic);
        sig.vtt = true;
        sig
    }

    fn fn_type(&mut self, info: &CallInfo, this: bool, variadic: bool) -> LTy {
        let mut ps = Vec::new();
        let rt = match &info.ret.kind {
            ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. } => self.ir(coerce),
            ArgKind::Indirect { .. } => {
                ps.push(LTy::PTR);
                LTy::VOID
            }
            ArgKind::Ignore => LTy::VOID,
        };
        if this {
            ps.push(LTy::PTR);
        }
        for a in &info.args {
            match &a.kind {
                ArgKind::Direct { coerce: IrType::Struct(fs, _), flatten: true, .. } => {
                    for f in fs.clone() {
                        let t = self.ir(&f);
                        ps.push(t);
                    }
                }
                ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. } => {
                    let t = self.ir(coerce);
                    ps.push(t);
                }
                ArgKind::Indirect { .. } => ps.push(LTy::PTR),
                ArgKind::Ignore => {}
            }
        }
        self.m.types.function(rt, &ps, variadic)
    }

    /// The return and parameter attributes of `sig` (`ConstructAttributeList`).
    pub fn sig_attrs(
        &mut self,
        sig: &Sig,
        n_args: usize,
        def: bool,
    ) -> (AttrSetId, Vec<AttrSetId>) {
        let mut ret = Vec::new();
        let mut params = Vec::new();
        match &sig.info.ret.kind {
            ArgKind::Indirect { align, .. } => {
                let t = self.mty(sig.ret);
                let mut a = vec![Attr::Flag(AttrKind::DeadOnUnwind)];
                if def {
                    a.push(Attr::Flag(AttrKind::NoAlias));
                }
                a.push(Attr::Flag(AttrKind::Writable));
                a.push(Attr::SRet(t));
                a.push(Attr::Align(*align));
                params.push(self.m.attr_set(&a));
            }
            ArgKind::Extend { signed, .. } => {
                if sig.info.ret.noundef {
                    ret.push(Attr::Flag(AttrKind::NoUndef));
                }
                ret.push(Attr::Flag(if *signed { AttrKind::SExt } else { AttrKind::ZExt }));
            }
            ArgKind::Direct { .. } => {
                if sig.info.ret.noundef {
                    ret.push(Attr::Flag(AttrKind::NoUndef));
                }
                self.ref_attrs(sig.ret, &mut ret);
            }
            ArgKind::Ignore => {}
        }
        if let Some(t) = sig.this {
            // ConstructAttributeList: `nonnull dereferenceable(N) align A`
            // on `this` (the minimum object size and class pointer
            // alignment), plus `noundef`.
            let (size, align) = self.this_layout(t);
            let mut s = vec![Attr::Flag(AttrKind::NoUndef), Attr::Flag(AttrKind::NonNull)];
            if let Some(n) = size {
                s.push(Attr::Dereferenceable(n));
            }
            s.push(Attr::Align(align));
            params.push(self.m.attr_set(&s));
        }
        if sig.vtt {
            params.push(self.m.attr_set(&[Attr::Flag(AttrKind::NoUndef)]));
        }
        for (i, a) in sig.info.args.iter().take(n_args).enumerate() {
            let mut s = Vec::new();
            if a.noundef {
                s.push(Attr::Flag(AttrKind::NoUndef));
            }
            self.ref_attrs(sig.params[i], &mut s);
            match &a.kind {
                ArgKind::Extend { signed, .. } => {
                    s.push(Attr::Flag(if *signed { AttrKind::SExt } else { AttrKind::ZExt }));
                }
                ArgKind::Indirect { byval: true, align } => {
                    let t = self.mty(sig.params[i]);
                    s.push(Attr::ByVal(t));
                    s.push(Attr::Align(*align));
                }
                ArgKind::Direct { align: Some(n), .. } => s.push(Attr::AlignStack(*n)),
                ArgKind::Direct { coerce: IrType::Struct(fs, _), flatten: true, .. } => {
                    for _ in 1..fs.len() {
                        params.push(self.m.attr_set(&s));
                    }
                }
                ArgKind::Ignore => continue,
                _ => {}
            }
            params.push(self.m.attr_set(&s));
        }
        let ret = self.m.attr_set(&ret);
        (ret, params)
    }

    /// The target-dependent function attributes.
    pub fn target_attrs(&self, out: &mut Vec<Attr>) {
        if self.is_x86() {
            out.push(Attr::string("frame-pointer", "all"));
        } else {
            out.push(Attr::string("frame-pointer", "non-leaf"));
        }
        out.push(Attr::string("no-trapping-math", "true"));
        out.push(Attr::string("stack-protector-buffer-size", "8"));
        if self.is_x86() {
            out.push(Attr::string("target-cpu", "x86-64"));
            out.push(Attr::string("target-features", "+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87"));
            out.push(Attr::string("tune-cpu", "generic"));
        } else {
            out.push(Attr::string("target-cpu", "generic"));
            out.push(Attr::string("target-features", "+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"));
        }
    }

    // ---- functions -----------------------------------------------------

    /// The (possibly declaration-only) function for `d` (clang
    /// `GetOrCreateLLVMFunction`).
    pub fn get_fn(&mut self, d: DeclId) -> R<FuncId> {
        let ast = self.ast;
        let first = ast.first(d);
        if let Some(&f) = self.fns.get(&first) {
            return Ok(f);
        }
        let name = mangle::mangle_decl(ast, first);
        let sig = self.decl_sig(first)?;
        let f = self.declare_fn(&name, first, &sig)?;
        self.fns.insert(first, f);
        if let Some(def) = self.deferred.remove(&first) {
            self.to_emit.push(Def::Decl(def));
        } else if let Some(def) = self.inline_def(first) {
            self.to_emit.push(Def::Decl(def));
        }
        Ok(f)
    }

    /// Declares the function `name` for declaration `first` with
    /// signature `sig` (the declaration attributes of
    /// `GetOrCreateLLVMFunction`).
    pub(crate) fn declare_fn(&mut self, name: &str, first: DeclId, sig: &Sig) -> R<FuncId> {
        let ast = self.ast;
        let f = match self.m.add_function(name, sig.fn_ty) {
            Ok(f) => f,
            Err(e) => {
                return err(
                    Some(ast.decls.get(first).loc()),
                    format!("cannot declare {name}: {e:?}"),
                );
            }
        };
        let (ret, params) = self.sig_attrs(sig, sig.params.len(), false);
        let mut fa = Vec::new();
        if sig.nothrow {
            fa.push(Attr::Flag(AttrKind::NoUnwind));
        }
        self.target_attrs(&mut fa);
        let fa = self.m.attr_set(&fa);
        let (fa, ret) = self.alloc_fn_attrs(first, fa, ret);
        // clang `SetFunctionAttributes`: virtual member functions are
        // `unnamed_addr`.
        let virt = ast.decls.get(first).kind() == DeclKind::CXXMethod
            && ast
                .redecls_of(first)
                .any(|r| ast.decls.function(r).is_some_and(|x| x.spec.is_virtual()));
        {
            let func = self.m.func_mut(f);
            func.attrs.func = fa;
            func.attrs.ret = ret;
            func.attrs.params = params;
            if virt {
                func.unnamed_addr = UnnamedAddr::Global;
            }
        }
        self.nounwind.insert(f, sig.nothrow);
        Ok(f)
    }

    /// Sets linkage and definition attributes of `f` defined by `d`.
    pub fn set_def_linkage(&mut self, f: FuncId, d: DeclId) {
        let internal = self.fn_internal(d);
        let inline = self.fn_is_inline(d);
        let name = self.m.func(f).name().to_owned();
        let comdat =
            if !internal && inline { Some(self.m.comdat(&name, ComdatKind::Any)) } else { None };
        let func = self.m.func_mut(f);
        if internal {
            func.linkage = Linkage::Internal;
            func.dso_local = false;
        } else if inline {
            func.linkage = Linkage::LinkOnceOdr;
            func.dso_local = true;
            func.comdat = comdat;
        } else {
            func.linkage = Linkage::External;
            func.dso_local = true;
        }
    }

    /// The function attributes of a definition.
    pub fn def_fn_attrs(
        &mut self,
        is_main: bool,
        nounwind: bool,
        mustprogress: bool,
        vector_width: u64,
    ) -> AttrSetId {
        let mut a = Vec::new();
        if mustprogress {
            a.push(Attr::Flag(AttrKind::MustProgress));
        }
        a.push(Attr::Flag(AttrKind::NoInline));
        if is_main {
            a.push(Attr::Flag(AttrKind::NoRecurse));
        }
        if nounwind {
            a.push(Attr::Flag(AttrKind::NoUnwind));
        }
        a.push(Attr::Flag(AttrKind::OptimizeNone));
        a.push(Attr::UwTable(UwTableKind::Async));
        self.target_attrs(&mut a);
        if self.is_x86() {
            a.push(Attr::string("min-legal-vector-width", &vector_width.to_string()));
        }
        self.m.attr_set(&a)
    }

    // ---- globals -------------------------------------------------------

    /// The global for variable `d`, declaring it if needed.
    pub fn get_gvar(&mut self, d: DeclId) -> R<GlobalId> {
        let ast = self.ast;
        let first = ast.first(d);
        if let Some(&g) = self.gvars.get(&first) {
            return Ok(g);
        }
        if let Some(&g) = self.static_locals.get(&first) {
            return Ok(g);
        }
        if let Some(def) = self.deferred.remove(&first) {
            // Emit it now if it is a discardable definition (its position
            // is the point of first use, as clang's GetOrCreateLLVMGlobal
            // followed by the deferred definition reusing it).
            self.to_emit.push(Def::Decl(def));
        }
        let Some(ty) = ast.decls.get(first).ty() else { return err(None, "variable without type") };
        let lt = self.mty(ty);
        let name = mangle::mangle_decl(ast, first);
        let g = self.m.add_global(&name, lt);
        let (_, align) = self.size_align(ty);
        let gv = self.m.global_mut(g);
        gv.linkage = Linkage::External;
        gv.align = Some(align);
        self.gvars.insert(first, g);
        self.available_externally(first, g, ty)?;
        Ok(g)
    }

    /// clang `GetOrCreateLLVMGlobal`: a `const` static data member with
    /// an in-class initializer and no definition in this translation
    /// unit exposes its value as an `available_externally` constant.
    fn available_externally(&mut self, first: DeclId, g: GlobalId, ty: QualType) -> R<()> {
        let ast = self.ast;
        if !ast.decls.get(first).ctx().is_some_and(|c| {
            ast.decls.get(c).kind().payload() == kiln_ast::decl::PayloadClass::Record
        }) {
            return Ok(());
        }
        if ast.redecls_of(first).count() > 1 || self.var_is_inline(first) {
            return Ok(());
        }
        if !ast.types.effective_quals(ty).is_const() || has_mutable(ty) {
            return Ok(());
        }
        if mangle::has_internal_linkage(ast, first) {
            return Ok(());
        }
        let Some(init) = ast.decls.var(first).and_then(|v| v.init) else { return Ok(()) };
        let Some(c) = self.folds.get(init) else { return Ok(()) };
        let c = self.const_value(c, ty, Some(init), true)?;
        if self.m.const_type(c) != self.m.global(g).value_type() {
            return Ok(());
        }
        if let Err(e) = self.m.set_initializer(g, Some(c)) {
            return err(Some(ast.decls.get(first).loc()), format!("bad initializer: {e:?}"));
        }
        let gv = self.m.global_mut(g);
        gv.is_constant = true;
        gv.linkage = Linkage::AvailableExternally;
        Ok(())
    }

    /// The alignment of global variable of type `ty`.
    pub fn global_align(&mut self, ty: QualType) -> u64 {
        let (size, align) = self.size_align(ty);
        if self.is_x86() && self.ast.types.is_array(ty) && size >= 16 {
            align.max(16)
        } else {
            align
        }
    }

    /// clang `EmitGlobalVarDefinition`.
    fn emit_var_def(&mut self, d: DeclId) -> R<()> {
        let ast = self.ast;
        let first = ast.first(d);
        let v = ast.decls.var(d).expect("var");
        let ty = ast.decls.get(d).ty().expect("var type");
        let loc = Some(ast.decls.get(d).loc());
        // A trivial default constructor does nothing; the static object
        // is zero-initialized ([basic.start.static]/2).
        // clang `getAnyInitializer`: an out-of-line definition of a static
        // data member takes the in-class initializer.
        let any_init = v
            .init
            .or_else(|| ast.redecls_of(first).find_map(|r| ast.decls.var(r).and_then(|x| x.init)));
        let init = match any_init.filter(|&i| !ast.is_trivial_default_construction(i)) {
            None => {
                let t = self.mty(ty);
                self.m.const_zero(t)
            }
            Some(e) => match self.folds.get(e) {
                Some(c) => self.const_value(c, ty, Some(e), true)?,
                None => {
                    return err(
                        loc,
                        "dynamic initialization of a global is not supported yet (M3)",
                    );
                }
            },
        };
        let ity = self.m.const_type(init);
        let g = if let Some(&g) = self.gvars.get(&first) {
            if self.m.global(g).value_type() != ity {
                return err(loc, "global redeclared with a different IR type");
            }
            g
        } else {
            let name = mangle::mangle_decl(ast, first);
            let g = self.m.add_global(&name, ity);
            self.gvars.insert(first, g);
            g
        };
        let internal = mangle::has_internal_linkage(ast, d);
        let inline = self.var_is_inline(d);
        let quals = ast.types.effective_quals(ty);
        let constant = quals.is_const() && !has_mutable(ty);
        let align = self.global_align(ty);
        let name = self.m.global(g).name().to_owned();
        let comdat =
            if inline && !internal { Some(self.m.comdat(&name, ComdatKind::Any)) } else { None };
        if let Err(e) = self.m.set_initializer(g, Some(init)) {
            return err(loc, format!("bad initializer: {e:?}"));
        }
        let gv = self.m.global_mut(g);
        gv.is_constant = constant;
        gv.align = Some(align);
        if internal {
            gv.linkage = Linkage::Internal;
            gv.dso_local = false;
        } else if inline {
            gv.linkage = Linkage::LinkOnceOdr;
            gv.dso_local = true;
            gv.comdat = comdat;
        } else {
            gv.linkage = Linkage::External;
            gv.dso_local = true;
        }
        Ok(())
    }

    /// A function-local `static` variable (clang `EmitStaticVarDecl`).
    pub fn static_local(&mut self, d: DeclId) -> R<GlobalId> {
        let ast = self.ast;
        let first = ast.first(d);
        if let Some(&g) = self.static_locals.get(&first) {
            return Ok(g);
        }
        let v = ast.decls.var(d).expect("var");
        let ty = ast.decls.get(d).ty().expect("var type");
        let loc = Some(ast.decls.get(d).loc());
        // clang `CreateStaticVarDecl`: the global exists (with the memory
        // type) before its initializer is emitted, so constants the
        // initializer creates follow it and a self-reference resolves.
        // The discriminator counts earlier static locals of the same name in
        // the same function (emission order is declaration order).
        let key = (mangle::enclosing_function(ast, first), mangle::decl_name(ast, first));
        let index = {
            let n = self.static_local_names.entry(key).or_insert(0);
            *n += 1;
            *n - 1
        };
        let name = mangle::mangle_static_local(ast, first, index);
        let mt = self.mty(ty);
        let g = self.m.add_global(&name, mt);
        let align = self.global_align(ty);
        let constant = ast.types.effective_quals(ty).is_const();
        let encl = mangle::enclosing_function(ast, first);
        let inline_fn = encl.is_some_and(|f| self.fn_is_inline(f) && !self.fn_internal(f));
        let comdat = if inline_fn { Some(self.m.comdat(&name, ComdatKind::Any)) } else { None };
        let gv = self.m.global_mut(g);
        gv.is_constant = constant;
        gv.align = Some(align);
        if inline_fn {
            gv.linkage = Linkage::LinkOnceOdr;
            gv.dso_local = true;
            gv.comdat = comdat;
        } else {
            gv.linkage = Linkage::Internal;
        }
        self.static_locals.insert(first, g);
        // clang `AddInitializerToStaticVarDecl`.
        let init = match v.init {
            None => self.m.const_zero(mt),
            Some(e) => match self.folds.get(e) {
                Some(c) => self.const_value(c, ty, Some(e), true)?,
                None => {
                    return err(
                        loc,
                        "dynamic initialization of a static local is not supported yet (M3)",
                    );
                }
            },
        };
        let ity = self.m.const_type(init);
        if ity != mt {
            self.m.set_global_value_type(g, ity);
        }
        if let Err(e) = self.m.set_initializer(g, Some(init)) {
            return err(loc, format!("bad initializer: {e:?}"));
        }
        Ok(g)
    }

    /// A private `unnamed_addr` constant named `base`, uniqued `.N` by the
    /// module's one counter as LLVM's `ValueSymbolTable::makeUniqueName`
    /// (`.str.1`, `.constant.2`, `.str.3`).
    pub fn private_const(&mut self, base: &str, init: Value, align: u64) -> GlobalId {
        let ty = self.m.const_type(init);
        let g = self.m.add_global(base, ty);
        let _ = self.m.set_initializer(g, Some(init));
        let gv = self.m.global_mut(g);
        gv.linkage = Linkage::Private;
        gv.unnamed_addr = UnnamedAddr::Global;
        gv.is_constant = true;
        gv.align = Some(align);
        g
    }

    /// The global holding a string literal of array type `arr` (clang
    /// `GetAddrOfConstantStringFromLiteral`).
    pub fn string_global(&mut self, bytes: &[u8], elem_size: u64, n_elems: u64) -> GlobalId {
        let et = match elem_size {
            1 => LTy::I8,
            2 => LTy::I16,
            _ => LTy::I32,
        };
        let at = self.m.types.array(et, n_elems);
        if let Some(&g) = self.strings.get(&(at, bytes.to_vec())) {
            return g;
        }
        let init = if elem_size == 1 {
            self.m.const_bytes(bytes)
        } else {
            let mut es = Vec::new();
            for c in bytes.chunks(elem_size as usize) {
                let mut v: u64 = 0;
                for (i, b) in c.iter().enumerate() {
                    v |= u64::from(*b) << (8 * i);
                }
                es.push(self.m.const_int(et, i128::from(v)));
            }
            self.m.const_array(et, &es).expect("string array")
        };
        let g = self.private_const(".str", init, elem_size);
        self.strings.insert((at, bytes.to_vec()), g);
        g
    }

    // ---- module trailer ------------------------------------------------

    fn trailer(&mut self) {
        let m = &mut self.m;
        m.add_named_metadata("llvm.linker.options", &[]);
        m.add_module_flag_i32(1, "wchar_size", 4);
        m.add_module_flag_i32(8, "PIC Level", 2);
        m.add_module_flag_i32(7, "PIE Level", 2);
        m.add_module_flag_i32(7, "uwtable", 2);
        m.add_module_flag_i32(
            7,
            "frame-pointer",
            if self.target.triple.arch == Arch::X86_64 { 2 } else { 1 },
        );
        m.add_ident(&self.opts.ident);
    }
}

/// Whether a record type has a `mutable` member (never in M2, which has no
/// `mutable`), which keeps a const global out of read-only memory.
fn has_mutable(_ty: QualType) -> bool {
    false
}

/// `Value` of a global.
pub(crate) fn gval(g: GlobalId) -> Value {
    Value::global(g)
}
