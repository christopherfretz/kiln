//! Module-level emission (spec/codegen.md §2): the declaration maps,
//! `EmitGlobal`/`EmitDeferred` ordering, linkage, attributes, globals,
//! string literals and the module trailer.

use kiln_abi::call::{ArgKind, CallInfo, classify_call};
use kiln_abi::irtype::{IrType, IrTypes, record_kind_prefix};
use kiln_abi::mangle;
use kiln_abi::{Arch, Target};
use kiln_ast::decl::FriendTarget;
use kiln_ast::decl::{
    ConstexprKind, DeclFlags, DeclKind, DefinitionBits, TemplLinkKind, VarStorage,
};
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{BuiltinType, PrintOptions, TypeView, print_type};
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

    /// The return or some argument is passed by address (`sret`,
    /// `byval`, or a pointer to a temporary): clang
    /// `ConstructAttributeList`'s `AddPotentialArgAccess` cases.
    pub fn passes_indirect(&self) -> bool {
        self.sret() || self.info.args.iter().any(|a| matches!(a.kind, ArgKind::Indirect { .. }))
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
    /// First declarations of the end-of-TU instantiations not yet handed
    /// to `EmitGlobal` (§2a): while held, a reference only declares them,
    /// as clang's references to a pending instantiation without a body.
    pub held: FxHashSet<DeclId>,
    /// String literal globals by (array type, bytes).
    pub strings: FxHashMap<(LTy, Vec<u8>), GlobalId>,
    /// Named struct types by record.
    pub named: FxHashMap<DeclId, LTy>,
    /// Base-subobject (`.base`) struct types by record.
    pub named_base: FxHashMap<DeclId, LTy>,
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
    /// Dynamic classes explicitly instantiated by a definition, in
    /// order: their vtables are required ([temp.explicit]/10; Sema's
    /// `MarkVTableUsed(DefinitionRequired)`).
    pub explicit_vtables: Vec<DeclId>,
    /// Vtables already defined.
    pub vt_emitted: FxHashSet<DeclId>,
    /// `_ZTI` definitions by mangled name.
    pub rtti_globals: FxHashMap<String, GlobalId>,
    /// Bodies using placeholders, lowered once the module is complete.
    pub pending: Vec<(FuncId, DeclId, crate::fir::Fir)>,
    /// Warnings codegen reports itself (clang's `CGM.getDiags()`, e.g.
    /// `-Watomic-alignment`), in emission order.
    pub diags: Vec<kiln_diag::Diag>,
    /// Per-function state.
    pub f: Fx,
    /// The `$_N` ids of unnamed classes, in first-mangle order.
    pub anon: mangle::AnonIds,
    /// Dynamic initialization of static-storage variables.
    pub gi: crate::global_init::GlobalInits,
}

impl<'a> Cg<'a> {
    pub fn new(
        ast: &'a Ast,
        folds: &'a Folds,
        target: &'a Target,
        opts: &'a CodegenOptions,
        void_ptr: QualType,
    ) -> Self {
        // clang 20+'s AArch64 data layout at compat 22 (spec/codegen.md §6a).
        let spec = match target.triple.arch {
            Arch::AArch64 if opts.clang_compat >= 22 => kiln_llvm::AARCH64_LINUX_GNU_22,
            Arch::AArch64 => kiln_llvm::AARCH64_LINUX_GNU,
            Arch::X86_64 => kiln_llvm::X86_64_LINUX_GNU,
        };
        Cg {
            ast,
            folds,
            tys: IrTypes::new(ast, target).with_clang_compat(opts.clang_compat),
            target,
            opts,
            m: {
                let mut m = Module::with_target(&opts.source_filename, spec);
                // clang 21+'s `captures(none)` (spec/codegen.md §6a).
                if opts.clang_compat >= 22 {
                    m.ir_version = opts.llvm_major;
                }
                m
            },
            fns: FxHashMap::default(),
            nounwind: FxHashMap::default(),
            gvars: FxHashMap::default(),
            deferred: FxHashMap::default(),
            held: ast.templates.end_of_tu_defs().iter().map(|&d| ast.first(d)).collect(),
            to_emit: Vec::new(),
            strings: FxHashMap::default(),
            named: FxHashMap::default(),
            named_base: FxHashMap::default(),
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
            explicit_vtables: Vec::new(),
            vt_emitted: FxHashSet::default(),
            rtti_globals: FxHashMap::default(),
            pending: Vec::new(),
            diags: Vec::new(),
            f: Fx::empty(),
            gi: crate::global_init::GlobalInits::default(),
            anon: mangle::AnonIds::new(),
        }
    }

    pub fn is_x86(&self) -> bool {
        self.target.triple.arch == Arch::X86_64
    }

    /// The functions' frame-pointer kind (spec/codegen.md "Frame
    /// pointer").
    /// The kind is clang 22's `"non-leaf-no-reserve"` (module flag 4) for
    /// `non-leaf` at compat 22 with an LLVM 22 backend (LLVM 19 rejects the
    /// value; spec/codegen.md §6a, "Frame pointer").
    pub fn frame_pointer(&self) -> crate::FramePointer {
        let fp = crate::FramePointer::resolve(
            self.is_x86(),
            self.opts.optimize,
            self.opts.omit_frame_pointer,
            self.opts.omit_leaf_frame_pointer,
        );
        if fp == crate::FramePointer::NonLeaf
            && self.opts.clang_compat >= 22
            && self.opts.llvm_major >= 22
        {
            crate::FramePointer::NonLeafNoReserve
        } else {
            fp
        }
    }

    // ---- driving -------------------------------------------------------

    /// Emits the whole translation unit; also returns the warnings
    /// reported on the way (even when emission fails).
    pub fn run(mut self) -> (R<Module>, Vec<kiln_diag::Diag>) {
        let r = self.run_tu();
        (r.map(|()| self.m), self.diags)
    }

    fn run_tu(&mut self) -> R<()> {
        let tu = self.ast.tu;
        self.emit_members(tu)?;
        self.emit_late_explicit_insts()?;
        self.emit_key_function_vtables()?;
        self.emit_end_of_tu_instantiations()?;
        self.emit_deferred()?;
        self.resolve_placeholders()?;
        self.emit_global_init_trailer()?;
        self.trailer();
        Ok(())
    }

    fn emit_members(&mut self, ctx: DeclId) -> R<()> {
        let ast = self.ast;
        let members: Vec<DeclId> = ast.decls.members(ctx).collect();
        self.emit_explicit_member_insts(ctx)?;
        for d in members {
            if ast.templates.is_late_explicit_inst(ast.first(d)) {
                continue;
            }
            self.emit_member(d)?;
            self.emit_explicit_member_insts(d)?;
        }
        Ok(())
    }

    /// The member functions and classes explicitly instantiated by a
    /// definition right after `after` ([temp.explicit]/10).
    fn emit_explicit_member_insts(&mut self, after: DeclId) -> R<()> {
        let ast = self.ast;
        for &m in ast.templates.explicit_member_insts_after(after) {
            if ast.templates.is_late_explicit_inst(ast.first(m)) {
                continue;
            }
            let Some(def) = ast.definition(m) else { continue };
            match ast.decls.get(def).kind() {
                DeclKind::CXXConstructor | DeclKind::CXXDestructor => {
                    self.emit_global_structor(def)?;
                }
                DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization => {
                    self.emit_class_members(def)?;
                }
                _ => self.emit_global_fn(def)?,
            }
        }
        Ok(())
    }

    /// The functions and variables explicitly instantiated before their
    /// template was defined, instantiated at the end of the TU: clang
    /// `PerformPendingInstantiations` hands them to `HandleTopLevelDecl`
    /// after the last top-level declaration, before `Release`.
    fn emit_late_explicit_insts(&mut self) -> R<()> {
        let ast = self.ast;
        for &m in ast.templates.late_explicit_insts() {
            if ast.decls.get(m).kind() == DeclKind::Var {
                // A variable template specialization is its own definition.
                self.emit_global_var(m)?;
                continue;
            }
            let Some(def) = ast.definition(m) else { continue };
            match ast.decls.get(def).kind() {
                DeclKind::CXXConstructor | DeclKind::CXXDestructor => {
                    self.emit_global_structor(def)?;
                }
                _ => self.emit_global_fn(def)?,
            }
        }
        Ok(())
    }

    fn emit_member(&mut self, d: DeclId) -> R<()> {
        let ast = self.ast;
        {
            let n = ast.decls.get(d);
            // A templated declaration ([temp.pre]/8: an out-of-line
            // member of a class template) is emitted only as instantiated.
            if n.has(DeclFlags::INVALID) || n.has(DeclFlags::DEPENDENT_CTX) || self.is_templated(d)
            {
                return Ok(());
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
                    if !ast.templates.is_moved_class_inst(ast.first(d)) {
                        self.emit_class_members(d)?;
                    }
                    if ast.modules.active && ast.decls.get(d).kind() == DeclKind::CXXRecord {
                        self.emit_module_class_defs(d, 0)?;
                    }
                }
                DeclKind::FunctionTemplate | DeclKind::ClassTemplate | DeclKind::VarTemplate => {}
                DeclKind::FileScopeAsm => self.emit_file_scope_asm(d),
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
        if fd.spec.is_immediate() {
            return Ok(());
        }
        let first = ast.first(d);
        if self.is_explicit_inst_declaration(first) || self.held.contains(&first) {
            return Ok(());
        }
        if !self.discardable_fn(d) {
            // A definition owned by an imported named-module unit (its
            // purview or its global module fragment) belongs to that
            // unit's object file (spec/modules.md §7).
            if ast.modules.is_imported_named_unit_decl(d) {
                return Ok(());
            }
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
        // [temp.explicit]/10: an explicit instantiation definition of a
        // class (or of a member class) defines every member that is not
        // a template, used or not (clang `InstantiateClassMembers` hands
        // each definition to `HandleTopLevelDecl`).
        let explicit = self.class_explicit_inst_kind(ast.first(d))
            == Some(SpecKind::ExplicitInstantiationDefinition);
        if explicit && self.is_dynamic(d) && self.vtable_explicit_inst_kind(ast.first(d)).is_some()
        {
            self.explicit_vtables.push(ast.first(d));
        }
        let members: Vec<DeclId> = ast.decls.members(d).collect();
        for m in members {
            let n = ast.decls.get(m);
            if n.has(DeclFlags::INVALID) {
                continue;
            }
            // An explicitly specialized member is an ordinary definition,
            // emitted where it appears (clang `InstantiateClassMembers`
            // skips `TSK_ExplicitSpecialization`).
            if explicit && self.is_member_explicit_spec(m) {
                continue;
            }
            match n.kind() {
                DeclKind::Function | DeclKind::CXXMethod | DeclKind::CXXConversion if explicit => {
                    if let Some(def) = ast.definition(m) {
                        self.emit_global_fn(def)?;
                    }
                }
                DeclKind::CXXConstructor | DeclKind::CXXDestructor if explicit => {
                    if let Some(def) = ast.definition(m) {
                        self.emit_global_structor(def)?;
                    }
                }
                // Any out-of-class redeclaration of a static data member
                // is its definition ([class.static.data]/2).
                DeclKind::Var if explicit && !self.var_is_inline(m) => {
                    if let Some(def) = ast.redecls_of(m).find(|&r| r != ast.first(m)) {
                        self.emit_global_var(def)?;
                    }
                }
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

    /// The in-class function definitions of namespace-scope class `d`
    /// (and of its nested classes, in member order) that are not inline
    /// because the class is attached to a named module ([class.mfct]/1,
    /// [class.friend]/7; spec/modules.md §7): emitted (strong) whether
    /// used or not, right after the class, as clang's
    /// `HandleInlineFunctionDefinition` / `EmitDeferredDecls` do.
    fn emit_module_class_defs(&mut self, d: DeclId, depth: usize) -> R<()> {
        let ast = self.ast;
        let n = ast.decls.get(d);
        if depth > 256
            || n.has(DeclFlags::INVALID)
            || n.has(DeclFlags::DEPENDENT_CTX)
            || ast.templ_link(d).get().is_some()
            || ast.definition(d) != Some(d)
        {
            return Ok(());
        }
        let members: Vec<DeclId> = ast.decls.members(d).collect();
        for m in members {
            match ast.decls.get(m).kind() {
                DeclKind::CXXMethod | DeclKind::CXXConversion if self.module_non_inline_def(m) => {
                    self.emit_global_fn(m)?;
                }
                DeclKind::CXXConstructor | DeclKind::CXXDestructor
                    if self.module_non_inline_def(m) =>
                {
                    self.emit_global_structor(m)?;
                }
                DeclKind::Friend => {
                    if let FriendTarget::Decl(f) = ast.decls.friend_target(m)
                        && ast.decls.get(f).kind() == DeclKind::Function
                        && self.module_non_inline_def(f)
                    {
                        self.emit_global_fn(f)?;
                    }
                }
                DeclKind::CXXRecord => self.emit_module_class_defs(m, depth + 1)?,
                _ => {}
            }
        }
        Ok(())
    }

    /// Whether `m` is an in-class function definition that is not inline
    /// because its class is attached to a named module ([class.mfct]/1).
    fn module_non_inline_def(&self, m: DeclId) -> bool {
        let ast = self.ast;
        ast.modules.active
            && ast.decls.function(m).is_some_and(|f| f.body.is_some() && f.templ.get().is_none())
            && !ast.decls.get(m).has(DeclFlags::IMPLICIT)
            && ast.module_attachment(m).is_some()
            && !self.discardable_fn(m)
    }

    /// clang `EmitGlobal` for a variable.
    fn emit_global_var(&mut self, d: DeclId) -> R<()> {
        let ast = self.ast;
        let Some(v) = ast.decls.var(d) else { return Ok(()) };
        // [dcl.link]/8: `extern "C" int x;` declares, like `extern`.
        let ext = v.spec.storage() == VarStorage::Extern || ast.decls.is_single_line_linkage(d);
        if ext && v.init.is_none() {
            return Ok(());
        }
        if matches!(v.templ.get(), Some((TemplLinkKind::Pattern, _))) {
            return Ok(());
        }
        let first = ast.first(d);
        // [temp.explicit]/14: an explicit instantiation declaration
        // provides no definition (references name the external symbol).
        // A held end-of-TU instantiation is handed over later (§2a).
        if self.is_explicit_inst_declaration(first) || self.held.contains(&first) {
            return Ok(());
        }
        if !self.discardable_var(d) || self.var_must_emit(d) {
            if !self.discardable_var(d) && ast.modules.is_imported_named_unit_decl(d) {
                return Ok(());
            }
            return self.emit_var_def(d);
        }
        self.note_deferred_init(first, d);
        if self.gvars.contains_key(&first) {
            self.to_emit.push(Def::Decl(d));
        } else {
            self.deferred.insert(first, d);
        }
        Ok(())
    }

    /// The definitions Sema instantiated at the end of the translation
    /// unit, handed to `EmitGlobal` in instantiation order (clang's
    /// `PerformPendingInstantiations` calling `HandleTopLevelDecl` for
    /// each, after the TU's declarations and the key-function vtables
    /// of `DefineUsedVTables`, before `EmitDeferred`; §2a).
    fn emit_end_of_tu_instantiations(&mut self) -> R<()> {
        let ast = self.ast;
        for &d in ast.templates.end_of_tu_defs() {
            let first = ast.first(d);
            if !self.held.remove(&first) {
                continue;
            }
            match ast.decls.get(first).kind() {
                DeclKind::Var => {
                    // The out-of-line definition Sema instantiated, with or
                    // without an initializer (`template<class T> T A<T>::s;`).
                    let def = self.instantiated_var_def(first).or_else(|| {
                        ast.redecls_of(first).filter(|&r| r != first).find(|&r| {
                            let n = ast.decls.get(r);
                            !n.has(DeclFlags::INVALID)
                                && !n.has(DeclFlags::DEPENDENT_CTX)
                                && ast.decls.var(r).is_some_and(|v| {
                                    !matches!(v.templ.get(), Some((TemplLinkKind::Pattern, _)))
                                })
                        })
                    });
                    if let Some(def) = def {
                        self.emit_global_var(def)?;
                    } else if ast.spec_info(first).is_some()
                        && ast.decls.var(first).is_some_and(|v| v.init.is_some())
                    {
                        // A variable template specialization used before
                        // its template's definition is its own definition.
                        self.emit_global_var(first)?;
                    }
                }
                DeclKind::CXXConstructor | DeclKind::CXXDestructor => {
                    if let Some(def) = ast.definition(first) {
                        self.emit_global_structor(def)?;
                    }
                }
                _ => {
                    if let Some(def) = ast.definition(first) {
                        self.emit_global_fn(def)?;
                    }
                }
            }
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

    /// Whether `d` is a templated entity ([temp.pre]/8): the pattern of a
    /// template or a partial specialization, or declared in one (an
    /// out-of-line member definition `template<class T> void A<T>::g()
    /// {}` is a member of `A`'s pattern). Templated entities are never
    /// emitted; only their instantiations are (clang `isDependentContext`
    /// in `EmitTopLevelDecl`).
    pub(crate) fn is_templated(&self, d: DeclId) -> bool {
        let ast = self.ast;
        let mut c = Some(d);
        while let Some(x) = c {
            let n = ast.decls.get(x);
            if kiln_ast::templates::is_template_kind(n.kind()) {
                return true;
            }
            if matches!(
                ast.templ_link(x).get(),
                Some((
                    TemplLinkKind::Pattern
                        | TemplLinkKind::PartialSpecialization
                        | TemplLinkKind::DependentFriend,
                    _
                ))
            ) {
                return true;
            }
            if n.kind() == DeclKind::TranslationUnit {
                return false;
            }
            c = n.ctx();
        }
        false
    }

    /// Whether `d` is (a member of) an implicit template instantiation,
    /// which has vague (`linkonce_odr`) linkage.
    pub(crate) fn is_implicit_instantiation(&self, d: DeclId) -> bool {
        let ast = self.ast;
        let first = ast.first(d);
        match ast.templ_link(first).get() {
            Some((TemplLinkKind::MemberOfSpecialization, i)) => {
                // An explicitly specialized member is an ordinary
                // function (WP-106).
                if ast
                    .templates
                    .member_spec(i)
                    .is_some_and(|m| m.kind == SpecKind::ExplicitSpecialization)
                {
                    return false;
                }
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
        if self.is_explicit_inst_declaration(first) || self.held.contains(&first) {
            return None;
        }
        ast.redecls_of(first).find(|&r| {
            let Some(f) = ast.decls.function(r) else { return false };
            if f.body.is_none() || matches!(f.templ.get(), Some((TemplLinkKind::Pattern, _))) {
                return false;
            }
            if f.spec.is_immediate() {
                return false;
            }
            let in_class = ast.decls.lexical_ctx(r).is_some_and(|c| {
                matches!(
                    ast.decls.get(c).kind(),
                    DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization
                )
            });
            if self.is_implicit_instantiation(r) {
                return true;
            }
            // An in-class definition attached to a named module is not
            // inline ([class.mfct]/1): an importer only declares it.
            in_class && (self.fn_is_inline(r) || !ast.modules.is_imported_named_unit_decl(r))
        })
    }

    /// The definition of static data member `first` of an implicitly
    /// instantiated class template specialization, or of an implicit
    /// specialization of a member variable template, which no walk of the
    /// translation unit reaches ([temp.inst]/5,7: instantiated when used).
    fn instantiated_var_def(&self, first: DeclId) -> Option<DeclId> {
        let ast = self.ast;
        let instantiated = match ast.templ_link(first).get() {
            Some((TemplLinkKind::MemberOfSpecialization, _)) => true,
            Some((TemplLinkKind::Specialization, _)) => {
                ast.decls.get(first).ctx().is_some_and(|c| {
                    matches!(
                        ast.decls.get(c).kind(),
                        DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization
                    )
                }) && ast
                    .spec_info(first)
                    .is_some_and(|(_, s)| s.kind == SpecKind::ImplicitInstantiation)
            }
            _ => false,
        };
        if !instantiated || self.is_explicit_inst_declaration(first) || self.held.contains(&first) {
            return None;
        }
        ast.redecls_of(first).find(|&r| {
            let n = ast.decls.get(r);
            !n.has(DeclFlags::INVALID)
                && !n.has(DeclFlags::DEPENDENT_CTX)
                && ast.decls.var(r).is_some_and(|v| {
                    v.init.is_some() && !matches!(v.templ.get(), Some((TemplLinkKind::Pattern, _)))
                })
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
        mangle::is_internal_symbol(self.ast, d)
    }

    pub(crate) fn discardable_fn(&self, d: DeclId) -> bool {
        self.fn_internal(d) || (self.fn_is_inline(d) && !self.is_explicit_inst_definition(d))
    }

    pub(crate) fn var_is_inline(&self, d: DeclId) -> bool {
        let ast = self.ast;
        ast.redecls_of(d).any(|r| ast.decls.var(r).is_some_and(|v| v.spec.is_inline()))
    }

    pub(crate) fn discardable_var(&self, d: DeclId) -> bool {
        if mangle::is_internal_symbol(self.ast, d) {
            return true;
        }
        // An explicit instantiation definition is emitted, inline or not.
        match self.var_instantiation_linkage(self.ast.first(d)) {
            Some(Linkage::WeakOdr) => false,
            inst => self.var_is_inline(d) || inst == Some(Linkage::LinkOnceOdr),
        }
    }

    /// Whether member `m` of a class template specialization is an
    /// explicit specialization (`template<> void X<int>::f() {}`).
    fn is_member_explicit_spec(&self, m: DeclId) -> bool {
        let ast = self.ast;
        ast.member_spec_info(ast.first(m))
            .is_some_and(|(_, x)| x.kind == SpecKind::ExplicitSpecialization)
    }

    /// Whether variable or function `first` is under an explicit
    /// instantiation declaration (`extern template`), itself or through
    /// its enclosing class specialization ([temp.explicit]/14: no
    /// definition is provided; at -O0 clang emits none, even for inline
    /// members, which would be `available_externally`). An inline
    /// function carrying `always_inline` is the exception
    /// ([`Self::is_available_externally_fn`]): clang emits it
    /// `available_externally` at every level and the always-inliner of the
    /// O0 pipeline inlines and deletes it, so the program never references
    /// its symbol (libstdc++'s `std::allocator<char>::allocate` is not
    /// exported; spec/codegen.md "Explicit instantiation declarations").
    pub(crate) fn is_explicit_inst_declaration(&self, first: DeclId) -> bool {
        self.explicit_inst_kind(first) == Some(SpecKind::ExplicitInstantiationDeclaration)
            && !self.is_always_inline_fn(first)
    }

    /// clang `GVA_AvailableExternally` emitted at -O0 (`shouldEmitFunction`
    /// keeps it only with `AlwaysInlineAttr`): an inline `always_inline`
    /// function under an explicit instantiation declaration.
    pub(crate) fn is_available_externally_fn(&self, first: DeclId) -> bool {
        self.explicit_inst_kind(first) == Some(SpecKind::ExplicitInstantiationDeclaration)
            && self.is_always_inline_fn(first)
    }

    /// An inline function with the `always_inline` attribute (on any
    /// redeclaration; an instantiation carries its pattern's attributes).
    fn is_always_inline_fn(&self, first: DeclId) -> bool {
        self.ast.decls.function(first).is_some()
            && self.fn_is_inline(first)
            && self.has_always_inline_attr(first)
    }

    /// Whether a redeclaration of function `d` carries `always_inline`
    /// and none carries `noinline` or `optnone` (clang
    /// `SetLLVMFunctionAttributesForDefinition`: `noinline` wins, and
    /// `optnone` overrides `always_inline`; the definition gets
    /// `alwaysinline` instead of `noinline optnone`).
    pub(crate) fn has_always_inline_attr(&self, d: DeclId) -> bool {
        use kiln_ast::attr::AttrKind as K;
        self.has_fn_attr(d, K::ALWAYS_INLINE)
            && !self.has_fn_attr(d, K::NOINLINE)
            && !self.has_fn_attr(d, K::OPTNONE)
    }

    /// The `inline` part of a definition's attributes (clang
    /// `SetLLVMFunctionAttributesForDefinition`). `optnone` on a
    /// redeclaration, or -O0 without `minsize` / `always_inline`, gives
    /// `noinline optnone` (an `optnone` attribute overrides `always_inline`
    /// and `minsize`); otherwise `naked` gives `noinline`, `always_inline`
    /// gives `alwaysinline` (`noinline` alone when `noinline` is also
    /// present), -O0 gives `noinline` (a `minsize` function), and above -O0
    /// a `noinline` function keeps `noinline` and an `inline`-specified one
    /// (on a redeclaration or one of its template pattern's) gets
    /// `inlinehint`. [`Self::def_opt_attrs`] adds `naked`, `minsize` and
    /// `optsize`.
    pub(crate) fn def_inline_attrs(&self, d: DeclId) -> DefInline {
        use kiln_ast::attr::AttrKind as K;
        let noinline = self.has_fn_attr(d, K::NOINLINE);
        let optnone = self.has_fn_attr(d, K::OPTNONE);
        let always = self.has_fn_attr(d, K::ALWAYS_INLINE);
        let should_optnone = !self.opts.optimize && !always && !self.has_fn_attr(d, K::MINSIZE);
        if optnone || should_optnone {
            DefInline::OptNone
        } else if self.has_fn_attr(d, K::NAKED) {
            DefInline::NoInline
        } else if always {
            if noinline { DefInline::NoInline } else { DefInline::Always }
        } else if !self.opts.optimize || noinline {
            DefInline::NoInline
        } else if self.inline_specified(d) {
            DefInline::Hint
        } else {
            DefInline::Plain
        }
    }

    /// The `naked`, `optsize` and `minsize` attributes of definition `d`
    /// whose inlining attributes are `inline` (clang
    /// `SetLLVMFunctionAttributesForDefinition`): `naked` from the
    /// attribute; without an `optnone` attribute, `cold` adds `optsize`
    /// unless the definition is `optnone` by -O0, and `minsize` adds
    /// `minsize`.
    pub(crate) fn def_opt_attrs(&self, d: DeclId, inline: DefInline, out: &mut Vec<Attr>) {
        use kiln_ast::attr::AttrKind as K;
        if self.has_fn_attr(d, K::NAKED) {
            out.push(Attr::Flag(AttrKind::Naked));
        }
        if self.has_fn_attr(d, K::OPTNONE) {
            return;
        }
        if self.has_fn_attr(d, K::COLD) && inline != DefInline::OptNone {
            out.push(Attr::Flag(AttrKind::OptimizeForSize));
        }
        if self.has_fn_attr(d, K::MINSIZE) {
            out.push(Attr::Flag(AttrKind::MinSize));
        }
    }

    /// clang `isInlineSpecified` on a redeclaration of function `d` or of
    /// its template instantiation pattern (an implicit `inline` — a
    /// class-member definition, `constexpr` — does not count).
    fn inline_specified(&self, d: DeclId) -> bool {
        let ast = self.ast;
        let spec = |f: DeclId| {
            ast.redecls_of(f).any(|r| ast.decls.function(r).is_some_and(|x| x.spec.is_inline()))
        };
        let first = ast.first(d);
        let explicit_spec = ast.spec_info(first).map(|(_, s)| s.kind)
            == Some(SpecKind::ExplicitSpecialization)
            || self.is_member_explicit_spec(first);
        spec(d) || (!explicit_spec && ast.instantiated_from(first).is_some_and(spec))
    }

    /// Whether a redeclaration of `d` carries an attribute of kind `k`.
    pub(crate) fn has_fn_attr(&self, d: DeclId, k: kiln_ast::attr::AttrKind) -> bool {
        let ast = self.ast;
        ast.redecls_of(d).any(|r| {
            ast.lists
                .iter::<kiln_ast::AttrId>(ast.attrs.of_decl(r))
                .any(|a| ast.attrs.get(a).kind == k)
        })
    }

    /// Whether variable or function `first` is under an explicit
    /// instantiation definition, itself or through its enclosing class
    /// specialization ([temp.explicit]/10; clang `GVA_StrongODR`: the
    /// definition is emitted `weak_odr` in a comdat, even when unused).
    pub(crate) fn is_explicit_inst_definition(&self, first: DeclId) -> bool {
        self.explicit_inst_kind(first) == Some(SpecKind::ExplicitInstantiationDefinition)
    }

    /// The explicit instantiation kind that governs variable or function
    /// `first`: its own, else that of the innermost explicitly
    /// instantiated or specialized enclosing class (clang propagates the
    /// class's kind to its members in `InstantiateClassMembers`).
    /// `None` when implicit or an explicit specialization.
    fn explicit_inst_kind(&self, first: DeclId) -> Option<SpecKind> {
        let ast = self.ast;
        let first = ast.first(first);
        let explicit = |k: SpecKind| match k {
            SpecKind::ExplicitInstantiationDeclaration
            | SpecKind::ExplicitInstantiationDefinition => Some(Some(k)),
            SpecKind::ExplicitSpecialization => Some(None),
            _ => None,
        };
        match ast.templ_link(first).get() {
            Some((TemplLinkKind::MemberOfSpecialization, i)) => {
                if let Some(k) = ast.templates.member_spec(i).and_then(|m| explicit(m.kind)) {
                    return k;
                }
                // The member's own kind is implicit: the innermost
                // explicitly instantiated enclosing class decides, unless
                // the member (or a member class on the way) is excluded
                // from explicit instantiation.
                if ast.excluded_from_explicit_instantiation(first) {
                    return None;
                }
                let mut ctx = ast.decls.get(first).ctx();
                while let Some(c) = ctx {
                    if !matches!(
                        ast.decls.get(c).kind(),
                        DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization
                    ) {
                        break;
                    }
                    if ast.member_spec_info(c).is_some()
                        && ast.excluded_from_explicit_instantiation(c)
                    {
                        return None;
                    }
                    if let Some(k) = ast.spec_info(c).and_then(|(_, s)| explicit(s.kind)) {
                        return k;
                    }
                    if let Some(k) = ast.member_spec_info(c).and_then(|(_, m)| explicit(m.kind)) {
                        return k;
                    }
                    ctx = ast.decls.get(c).ctx();
                }
                None
            }
            Some((TemplLinkKind::Specialization, _)) => {
                ast.spec_info(first).and_then(|(_, s)| explicit(s.kind)).flatten()
            }
            _ => None,
        }
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
            IrType::ScalableVector(e, n) => {
                let e = self.ir(e);
                self.m.types.scalable_vector(e, u32::try_from(*n).unwrap_or(u32::MAX))
            }
            IrType::TargetExt(name) => self.m.types.target_ext(name),
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
        // clang `addRecordTypeName`: an unnamed class (without a typedef
        // name for linkage) is plain `anon`; LLVM uniquifies a repeated
        // name with one module-wide counter.
        let leaf = if mangle::decl_name(self.ast, first).is_empty() {
            "anon".to_owned()
        } else {
            self.qualified_name(first)
        };
        let name = format!("{}.{leaf}", record_kind_prefix(self.ast, first));
        let t = self.m.types.named_struct(&name);
        self.named.insert(first, t);
        let rec = self.tys.record(first).clone();
        let fields: Vec<LTy> = rec.members.iter().map(|x| self.ir(x)).collect();
        // clang `ComputeRecordLayout`: the base-subobject type is created
        // before the complete type's body is filled in.
        if let Some(bm) = &rec.base_members {
            // `addRecordTypeName(D, BaseTy, ".base")` spells the record's
            // own name, uniquified on its own (an empty non-POD class
            // consumes a counter value for an unused `.base`).
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
        let is_fn = |k: DeclKind| {
            matches!(
                k,
                DeclKind::Function
                    | DeclKind::CXXMethod
                    | DeclKind::CXXConstructor
                    | DeclKind::CXXDestructor
                    | DeclKind::CXXConversion
            )
        };
        let leaf = || {
            let s = mangle::decl_name(ast, d);
            if s.is_empty() { "anon".to_owned() } else { s }
        };
        // clang `printQualifiedName` on a function-local declaration
        // prints only its name.
        let mut ctx = ast.decls.get(d).ctx();
        while let Some(c) = ctx {
            match ast.decls.get(c).kind() {
                DeclKind::LinkageSpec | DeclKind::Export => ctx = ast.decls.get(c).ctx(),
                k if is_fn(k) => return leaf(),
                _ => break,
            }
        }
        let mut parts = vec![leaf()];
        let mut cur = ast.decls.get(d).ctx();
        while let Some(x) = cur {
            let n = ast.decls.get(x);
            match n.kind() {
                DeclKind::TranslationUnit => break,
                DeclKind::LinkageSpec | DeclKind::Export => {}
                DeclKind::Namespace => {
                    let s = mangle::decl_name(ast, x);
                    parts.push(if s.is_empty() { "(anonymous namespace)".to_owned() } else { s });
                }
                k if is_fn(k) => {
                    let mut s = mangle::decl_name(ast, x);
                    s.push('(');
                    if let Some(p) = n.ty().and_then(|t| ast.types.proto(t).copied()) {
                        let ps = ast.types.params(&p);
                        for (i, &q) in ps.iter().enumerate() {
                            if i > 0 {
                                s.push_str(", ");
                            }
                            s.push_str(&print_type(&ast.types, ast, q, PrintOptions::SUGARED));
                        }
                        if p.variadic() {
                            s.push_str(if ps.is_empty() { "..." } else { ", ..." });
                        }
                    }
                    s.push(')');
                    parts.push(s);
                }
                _ => {
                    let mut s = mangle::decl_name(ast, x);
                    // An enclosing class template specialization prints
                    // with its arguments (`A<B>::In`).
                    if !s.is_empty()
                        && let Some(l) = ast.spec_print_args(x)
                    {
                        kiln_ast::types::write_arg_list(&ast.types, ast, l, &mut s);
                    }
                    parts.push(if s.is_empty() {
                        format!("(anonymous {})", record_kind_prefix(ast, x))
                    } else {
                        s
                    });
                }
            }
            // A namespace reopened through an inline namespace set prints
            // under its lexical parent, as clang's `printQualifiedName`
            // does (`struct.A::R::U`, spec/sema.md §4.6).
            cur = match n.kind() {
                DeclKind::Namespace => ast.decls.lexical_ctx(x).or(n.ctx()),
                _ => n.ctx(),
            };
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

    /// [`Cg::is_bool`], seeing through `_Atomic` (the memory form of an
    /// `_Atomic(bool)` is `i8` too).
    pub fn is_bool_value(&self, q: QualType) -> bool {
        match self.ast.types.canonical_view(q) {
            TypeView::Atomic(v) => self.is_bool(v),
            _ => self.is_bool(q),
        }
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

    /// The signature of a runtime library call `ret(params...)` that
    /// cannot throw and has no target declaration (clang
    /// `arrangeFreeFunctionCall` of a `noexcept` prototype; the `_Complex`
    /// multiplication and division routines).
    pub(crate) fn libcall_sig(&mut self, ret: QualType, params: &[QualType]) -> Sig {
        let info = self.classify(ret, params, params.len(), true, false);
        let fn_ty = self.fn_type(&info, false, false);
        Sig { info, fn_ty, ret, params: params.to_vec(), nothrow: true, this: None, vtt: false }
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

    /// `sig` without its parameters (only `this`, and the VTT if any,
    /// remain): clang `inheritingCtorHasParams` false.
    pub(crate) fn without_params(&mut self, mut sig: Sig) -> Sig {
        let vtt = sig.vtt;
        sig.params.clear();
        sig.info.args.clear();
        sig.fn_ty = self.fn_type(&sig.info, sig.this.is_some(), false);
        sig.vtt = false;
        if vtt { self.with_vtt(sig) } else { sig }
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
                // clang 21+: the callee owns an indirect argument the caller
                // does not destroy (spec/codegen.md §6a; LLVM 21+ syntax).
                ArgKind::Indirect { byval: false, .. }
                    if self.opts.clang_compat >= 22
                        && self.opts.llvm_major >= 21
                        && self.destructed_class(sig.params[i]).is_none() =>
                {
                    s.push(Attr::Flag(AttrKind::DeadOnReturn));
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
        if let Some(fp) = self.frame_pointer().attr() {
            out.push(Attr::string("frame-pointer", fp));
        }
        out.push(Attr::string("no-trapping-math", "true"));
        out.push(Attr::string("stack-protector-buffer-size", "8"));
        if self.is_x86() {
            out.push(Attr::string("target-cpu", "x86-64"));
            out.push(Attr::string("target-features", "+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87"));
            out.push(Attr::string("tune-cpu", "generic"));
        } else {
            out.push(Attr::string("target-cpu", "generic"));
            // clang 19 sorts the `+` features and appends the explicit
            // `-mno-outline-atomics` after `-fmv`.
            out.push(Attr::string(
                "target-features",
                if self.opts.outline_atomics {
                    "+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"
                } else {
                    "+fp-armv8,+neon,+v8a,-fmv,-outline-atomics"
                },
            ));
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
        let name = mangle::mangle_decl(ast, &mut self.anon, first);
        let sig = self.decl_sig(first)?;
        // An existing function of that name and type is this one: a
        // runtime function codegen declared first (`__cxa_allocate_exception`
        // from a `throw`), or a separately declared redeclaration of the
        // same `extern "C"` function (clang `GetOrCreateLLVMFunction`
        // returns the entry unchanged).
        let f = match self.m.get_function(&name) {
            Some(f) if self.m.func(f).ty() == sig.fn_ty => {
                let nothrow = sig.nothrow || ast.has_nothrow_attr(first);
                self.nounwind.entry(f).or_insert(nothrow);
                f
            }
            _ => self.declare_fn(&name, first, &sig)?,
        };
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
        // clang `ConstructAttributeList`: a non-throwing type or a
        // `NoThrowAttr` (a recognized C library function).
        let mut cp = Vec::new();
        let const_pure = crate::builtins::const_pure_attrs(ast, first, sig, &mut cp);
        let nothrow = sig.nothrow || const_pure || ast.has_nothrow_attr(first);
        if nothrow {
            fa.push(Attr::Flag(AttrKind::NoUnwind));
        }
        fa.extend(cp);
        if ast.is_noreturn_fn(first) {
            fa.push(Attr::Flag(AttrKind::NoReturn));
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
        self.nounwind.insert(f, nothrow);
        Ok(f)
    }

    /// Sets linkage and definition attributes of `f` defined by `d`.
    pub fn set_def_linkage(&mut self, f: FuncId, d: DeclId) {
        let internal = self.fn_internal(d);
        // clang `GetGVALinkageForFunction`: an explicit instantiation
        // definition is `GVA_StrongODR` (`weak_odr`), inline or not.
        let weak = !internal && self.is_explicit_inst_definition(d);
        let inline = self.fn_is_inline(d);
        let mut name = self.m.func(f).name().to_owned();
        // clang `getCodegenToUse` → `COMDAT`: a weak structor of a class
        // without virtual bases puts its base-object and deleting
        // variants in the `C5`/`D5` comdat (the complete-object variant
        // is an alias).
        if weak
            && matches!(
                self.ast.decls.get(d).kind(),
                DeclKind::CXXConstructor | DeclKind::CXXDestructor
            )
            && !self.structor_has_vbases(d)
        {
            name = mangle::mangle_structor_comdat(self.ast, &mut self.anon, self.ast.first(d));
        }
        let comdat = if !internal && (inline || weak) {
            Some(self.m.comdat(&name, ComdatKind::Any))
        } else {
            None
        };
        let available_externally = !internal && self.is_available_externally_fn(self.ast.first(d));
        let func = self.m.func_mut(f);
        if internal {
            func.linkage = Linkage::Internal;
            func.dso_local = false;
        } else if available_externally {
            // No comdat and not `dso_local` (clang `setDSOLocal` skips
            // `available_externally`).
            func.linkage = Linkage::AvailableExternally;
            func.dso_local = false;
        } else if weak {
            func.linkage = Linkage::WeakOdr;
            func.dso_local = true;
            func.comdat = comdat;
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
    pub fn def_fn_attrs(&mut self, f: DefFnAttrs) -> AttrSetId {
        let DefFnAttrs { is_main, nounwind, mustprogress, vector_width, noreturn, inline, extra } =
            f;
        let mut a = Vec::new();
        if inline == DefInline::Always {
            a.push(Attr::Flag(AttrKind::AlwaysInline));
        }
        if inline == DefInline::Hint {
            a.push(Attr::Flag(AttrKind::InlineHint));
        }
        if mustprogress {
            a.push(Attr::Flag(AttrKind::MustProgress));
        }
        if inline == DefInline::NoInline || inline == DefInline::OptNone {
            a.push(Attr::Flag(AttrKind::NoInline));
        }
        if is_main {
            a.push(Attr::Flag(AttrKind::NoRecurse));
        }
        if noreturn {
            a.push(Attr::Flag(AttrKind::NoReturn));
        }
        if nounwind {
            a.push(Attr::Flag(AttrKind::NoUnwind));
        }
        if inline == DefInline::OptNone {
            a.push(Attr::Flag(AttrKind::OptimizeNone));
        }
        a.extend(extra);
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
        } else if let Some(def) = self.instantiated_var_def(first) {
            self.to_emit.push(Def::Decl(def));
        }
        let Some(ty) = ast.decls.get(first).ty() else { return err(None, "variable without type") };
        let lt = self.mty(ty);
        let name = mangle::mangle_decl(ast, &mut self.anon, first);
        let g = self.m.add_global(&name, lt);
        // clang `GetOrCreateLLVMGlobal`: `getDeclAlign(D)`.
        let align = self.var_align(first);
        let gv = self.m.global_mut(g);
        gv.linkage = Linkage::External;
        gv.align = Some(align);
        self.gvars.insert(first, g);
        self.mark_tls(first, g, false);
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
        if !ast.types.effective_quals(ty).is_const() || has_mutable(ast, ty) {
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

    /// The alignment of the storage of variable `d` (clang
    /// `getDeclAlign(D)`): its `alignas`/`aligned` attribute, else
    /// [`Self::global_align`] of its type ([dcl.align]/5).
    pub fn var_align(&mut self, d: DeclId) -> u64 {
        kiln_abi::layout::decl_align(self.ast, d, self.target, false)
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
        let mut needs_init = false;
        let init = match any_init.filter(|&i| !ast.is_trivial_default_construction(i)) {
            None => self.null_const(ty)?,
            Some(e) => {
                if let Some(c) = self.folds.get(e) {
                    self.const_value(c, ty, Some(e), true)?
                } else {
                    // Dynamic initialization ([basic.start.dynamic]): zero
                    // until the initialization function runs.
                    needs_init = true;
                    self.null_const(ty)?
                }
            }
        };
        let is_ref = ast.types.is_reference(ty);
        let needs_dtor = !is_ref && self.var_destructed_class(first, ty).is_some();
        let ity = self.m.const_type(init);
        let g = if let Some(&g) = self.gvars.get(&first) {
            // A declaration created by an earlier use has the memory type;
            // a union initialized through a non-largest member (or a
            // struct holding one) has its initializer's type instead.
            if self.m.global(g).value_type() != ity {
                if self.m.global(g).initializer().is_some() {
                    return err(loc, "global redeclared with a different IR type");
                }
                self.m.retype_global_to_end(g, ity);
            }
            g
        } else {
            let name = mangle::mangle_decl(ast, &mut self.anon, first);
            let g = self.m.add_global(&name, ity);
            self.gvars.insert(first, g);
            self.mark_tls(first, g, false);
            g
        };
        let internal = mangle::is_internal_symbol(ast, d);
        let instantiated = self.var_instantiation_linkage(first);
        let inline = self.var_is_inline(d) || instantiated.is_some();
        let quals = ast.types.effective_quals(ty);
        let constant =
            ((quals.is_const() && !has_mutable(ast, ty)) || is_ref) && !needs_init && !needs_dtor;
        let align = self.var_align(d);
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
            gv.linkage = instantiated.unwrap_or(Linkage::LinkOnceOdr);
            gv.dso_local = true;
            gv.comdat = comdat;
        } else {
            gv.linkage = Linkage::External;
            gv.dso_local = true;
        }
        if needs_init || needs_dtor {
            self.emit_var_init_fn(d, g, needs_init)?;
        }
        Ok(())
    }

    /// The linkage of the definition of variable `first` when it is a
    /// template instantiation (clang `getLVForDecl` /
    /// `GetLLVMLinkageVarDefinition`): `weak_odr` for an explicit
    /// instantiation definition ([temp.explicit]), `linkonce_odr` for an
    /// implicit instantiation; `None` for an explicit specialization or a
    /// non-templated variable.
    pub(crate) fn var_instantiation_linkage(&self, first: DeclId) -> Option<Linkage> {
        let ast = self.ast;
        let explicit = |k: SpecKind| k == SpecKind::ExplicitInstantiationDefinition;
        match ast.templ_link(first).get() {
            Some((TemplLinkKind::MemberOfSpecialization, i)) => {
                let m = ast.templates.member_spec(i)?;
                if m.kind == SpecKind::ExplicitSpecialization {
                    return None;
                }
                if explicit(m.kind)
                    || self.explicit_inst_kind(first)
                        == Some(SpecKind::ExplicitInstantiationDefinition)
                {
                    Some(Linkage::WeakOdr)
                } else {
                    Some(Linkage::LinkOnceOdr)
                }
            }
            Some((TemplLinkKind::Specialization, _)) => match ast.spec_info(first)?.1.kind {
                SpecKind::ExplicitSpecialization => None,
                k if explicit(k) => Some(Linkage::WeakOdr),
                _ => Some(Linkage::LinkOnceOdr),
            },
            _ => None,
        }
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
        let name = mangle::mangle_static_local(ast, &mut self.anon, first, index);
        let mt = self.mty(ty);
        let g = self.m.add_global(&name, mt);
        self.gi.local_index.insert(first, index);
        let align = self.var_align(d);
        let init_expr = v.init.filter(|&e| !ast.is_trivial_default_construction(e));
        let dynamic = self.static_needs_init(d, init_expr);
        let constant = (ast.types.effective_quals(ty).is_const() && !has_mutable(ast, ty)
            || ast.types.is_reference(ty))
            && !dynamic;
        let encl = mangle::enclosing_function(ast, first);
        let inline_fn = encl.is_some_and(|f| {
            (self.fn_is_inline(f) || self.is_explicit_inst_definition(f)) && !self.fn_internal(f)
        });
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
        self.mark_tls(first, g, true);
        self.static_locals.insert(first, g);
        // clang `AddInitializerToStaticVarDecl`.
        // A dynamic initializer runs, guarded, at the declaration
        // statement ([stmt.dcl]/3; `static_local_init`).
        let init = match init_expr.and_then(|e| self.folds.get(e).map(|c| (e, c))) {
            None => self.null_const(ty)?,
            Some((e, c)) => self.const_value(c, ty, Some(e), true)?,
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

    /// The global of template parameter object `d` ([temp.param]/8):
    /// a `linkonce_odr` constant in its own comdat holding its value, or
    /// an `internal` one when its value names an entity without external
    /// linkage (clang `GetAddrOfTemplateParamObject`).
    pub fn param_object_global(&mut self, d: DeclId) -> R<GlobalId> {
        if let Some(&g) = self.gvars.get(&d) {
            return Ok(g);
        }
        let ast = self.ast;
        let loc = Some(ast.decls.get(d).loc());
        let Some(v) = ast.templates.param_object_value(d) else {
            return err(loc, "template parameter object without a value");
        };
        let ty = ast.decls.get(d).ty().unwrap_or(QualType::INT);
        let init = self.const_value(v, ty, None, true)?;
        let name = mangle::mangle_param_object(ast, &mut self.anon, d);
        let ity = self.m.const_type(init);
        let g = self.m.add_global(&name, ity);
        self.gvars.insert(d, g);
        if let Err(e) = self.m.set_initializer(g, Some(init)) {
            return err(loc, format!("bad initializer: {e:?}"));
        }
        // An object whose type or value names an entity without external
        // linkage is `internal`; clang 19 keeps its comdat, clang 22 drops
        // it. Neither sets an alignment.
        let internal = mangle::is_internal_symbol(ast, d);
        let comdat = (!internal || self.opts.clang_compat < 22)
            .then(|| self.m.comdat(&name, ComdatKind::Any));
        let gv = self.m.global_mut(g);
        gv.is_constant = true;
        gv.linkage = if internal { Linkage::Internal } else { Linkage::LinkOnceOdr };
        gv.dso_local = !internal;
        gv.comdat = comdat;
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
        self.string_global_named(bytes, elem_size, n_elems, ".str")
    }

    /// [`Self::string_global`] naming a new global `name` (clang's
    /// `GetAddrOfConstantStringFromLiteral(S, Name)`; an equal constant
    /// already emitted is reused whatever its name).
    pub fn string_global_named(
        &mut self,
        bytes: &[u8],
        elem_size: u64,
        n_elems: u64,
        name: &str,
    ) -> GlobalId {
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
        let g = self.private_const(name, init, elem_size);
        self.strings.insert((at, bytes.to_vec()), g);
        g
    }

    // ---- module trailer ------------------------------------------------

    fn trailer(&mut self) {
        let fp = self.frame_pointer().module_flag();
        let m = &mut self.m;
        // clang 20+ emits no empty `!llvm.linker.options`.
        if self.opts.clang_compat < 22 {
            m.add_named_metadata("llvm.linker.options", &[]);
        }
        m.add_module_flag_i32(1, "wchar_size", 4);
        m.add_module_flag_i32(8, "PIC Level", 2);
        m.add_module_flag_i32(7, "PIE Level", 2);
        m.add_module_flag_i32(7, "uwtable", 2);
        if let Some(v) = fp {
            m.add_module_flag_i32(7, "frame-pointer", v);
        }
        m.add_ident(&self.opts.ident);
    }
}

/// Whether a record type has a `mutable` member (never in M2, which has no
/// `mutable`), which keeps a const global out of read-only memory.
/// Whether `ty` (or its array element type) is a class with a `mutable`
/// member, directly or in a subobject: a `const` object of such a type
/// is not emitted `constant` (clang `isTypeConstant`, `hasMutableFields`).
pub(crate) fn has_mutable(ast: &Ast, ty: QualType) -> bool {
    let mut t = ast.types.canonical(ty);
    while let Some(e) = ast.types.element(t) {
        t = ast.types.canonical(e);
    }
    ast.types.as_record(t).is_some_and(|r| {
        let def = ast.definition(r).unwrap_or(r);
        ast.decls.record(def).is_some_and(|x| x.defn.contains(DefinitionBits::HAS_MUTABLE_FIELDS))
    })
}

/// `Value` of a global.
pub(crate) fn gval(g: GlobalId) -> Value {
    Value::global(g)
}

/// The inlining attributes of a definition ([`Cg::def_inline_attrs`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DefInline {
    /// `noinline optnone`: the default at -O0, or an `optnone` attribute.
    OptNone,
    /// `noinline` alone: `always_inline` together with `noinline`,
    /// `naked`, `minsize` at -O0, or `noinline` above -O0.
    NoInline,
    /// `alwaysinline`.
    Always,
    /// `inlinehint`: an `inline`-specified function above -O0.
    Hint,
    /// Nothing: any other function above -O0.
    Plain,
}

/// The inputs of [`Cg::def_fn_attrs`]: the flags that decide a
/// definition's function attributes.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "independent attribute flags")]
pub(crate) struct DefFnAttrs {
    pub(crate) is_main: bool,
    pub(crate) nounwind: bool,
    pub(crate) mustprogress: bool,
    pub(crate) vector_width: u64,
    pub(crate) noreturn: bool,
    /// `noinline optnone`, `noinline`, `alwaysinline`, `inlinehint` or
    /// nothing ([`Cg::def_inline_attrs`]).
    pub(crate) inline: DefInline,
    /// `const` / `pure` attributes (`crate::builtins::const_pure_attrs`).
    pub(crate) extra: Vec<Attr>,
}
