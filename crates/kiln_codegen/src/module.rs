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
use kiln_diag::{Diag, DiagId};
use kiln_llvm::{
    Attr, AttrKind, AttrSetId, ComdatKind, FuncId, GlobalId, Linkage, Module, TypeId as LTy,
    UnnamedAddr, UwTableKind, Value, Visibility,
};
use kiln_source::fx::{FxHashMap, FxHashSet};
use kiln_source::{Loc, Span};

use crate::fir::{Op, V};
use crate::fold::Folds;
use crate::func::Fx;
use crate::{CodeModel, CodegenError, CodegenOptions, RelocModel, StackProtector, UnwindTables};

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
    /// Functions declared for an `optnone` declaration: neither they nor
    /// their call sites get the `-Os`/`-Oz` size attributes.
    pub optnone_fns: FxHashSet<FuncId>,
    /// First declaration → global variable.
    pub gvars: FxHashMap<DeclId, GlobalId>,
    /// Whether any visibility other than default can arise (a
    /// `-fvisibility*` flag or a visibility attribute in the TU): when
    /// not, every symbol is default and the model is skipped.
    pub vis_active: bool,
    /// Memoised visibility per (declaration, computation kind).
    pub lv_cache: std::cell::RefCell<FxHashMap<(DeclId, u8), crate::visibility::Lv>>,
    /// Discardable definitions not yet needed: first declaration → definition.
    pub deferred: FxHashMap<DeclId, DeclId>,
    /// Each class definition's index in completion order
    /// (`DeclArena::completed_records`; instantiations included).
    pub completion_index: FxHashMap<DeclId, u32>,
    /// While the top-level walk runs, the class-completion epoch at the
    /// end of the top-level declaration being emitted: a class completed
    /// later is incomplete to a reference emitted now (clang emits each
    /// top-level declaration as it is parsed); `u32::MAX` after the walk.
    pub walk_epoch: u32,
    /// Definitions to emit (clang `DeferredDeclsToEmit`).
    pub to_emit: Vec<Def>,
    /// First declarations of the end-of-TU instantiations not yet handed
    /// to `EmitGlobal` (§2a): while held, a reference only declares them,
    /// as clang's references to a pending instantiation without a body.
    pub held: FxHashSet<DeclId>,
    /// The variable definitions Sema instantiated at a reference while
    /// parsing, by (namespace-scope declaration, after it): handed to
    /// `EmitGlobal` right before (or after) that declaration, held until
    /// then (§2a, clang `PassToConsumerRAII`).
    pub ref_vars: FxHashMap<(DeclId, bool), Vec<DeclId>>,
    /// String literal globals by (array type, bytes).
    pub strings: FxHashMap<(LTy, Vec<u8>), GlobalId>,
    /// Named struct types by record.
    pub named: FxHashMap<DeclId, LTy>,
    /// Base-subobject (`.base`) struct types by record.
    pub named_base: FxHashMap<DeclId, LTy>,
    /// The `.constant` globals of frozen constant objects (clang
    /// `GetAddrOfUnnamedGlobalConstantDecl`), by value.
    pub const_objects: FxHashMap<kiln_ast::ConstValueId, GlobalId>,
    /// The constant `contract_violation` objects, per assertion and
    /// detection mode (spec/contracts.md §6.3).
    pub contract_objects: FxHashMap<(kiln_ast::StmtId, u16), Value>,
    /// Whether a checked contract assertion was emitted.
    pub contract_checked: bool,
    /// The outermost default argument being emitted that holds a
    /// source-location builtin (clang `SourceLocExprScopeGuard`).
    pub srcloc: Option<kiln_ast::ExprId>,
    /// Static locals already emitted.
    pub static_locals: FxHashMap<DeclId, GlobalId>,
    /// Whether each class definition asked about is consteval-only
    /// ([`Cg::is_consteval_only`]).
    pub consteval_only: FxHashMap<DeclId, bool>,
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
    /// The first declaration given each symbol name (clang `Manglings`'
    /// representative declaration, for `err_duplicate_mangled_name`).
    pub reps: FxHashMap<String, DeclId>,
    /// The symbols declared `extern_weak` for a use of a GNU `weakref`
    /// declaration that nothing has referenced strongly yet (clang
    /// `WeakRefReferences`): a later ordinary reference makes the symbol
    /// external ([`Cg::strong_ref`]).
    pub weakref_syms: FxHashSet<String>,
    /// `alias` / `ifunc` definitions ([`crate::alias`]).
    pub(crate) alias: crate::alias::AliasState,
    /// Whether a duplicate mangled name was diagnosed (clang hands no
    /// later declaration to code generation after an error).
    pub dup_reported: bool,
    /// Thunk functions by mangled name (WP-080).
    pub thunk_fns: FxHashMap<String, FuncId>,
    /// Structors whose base variant body was generated.
    pub sdone: FxHashSet<DeclId>,
    /// `_ZTV` globals by class (WP-080).
    pub vtables: FxHashMap<DeclId, GlobalId>,
    /// Vtable groups, construction groups, VTTs and subobject tables
    /// by class (one shared session).
    pub vts: kiln_abi::vtable::VTableSession<'a>,
    /// Vtables referenced but not yet defined (clang `DeferredVTables`).
    pub deferred_vtables: Vec<DeclId>,
    /// External vtables to emit `available_externally` once every
    /// deferred definition is emitted, if they then can be (clang
    /// `OpportunisticVTables`, `-O1` and up).
    pub opportunistic_vtables: Vec<DeclId>,
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
    /// The finished body of each variadic virtual method defined so far
    /// (and whether it uses placeholders), cloned into its
    /// return-adjusting thunks (clang `GenerateVarArgsThunk`).
    pub vararg_bodies: FxHashMap<FuncId, (crate::fir::Fir, bool)>,
    /// Warnings codegen reports itself (clang's `CGM.getDiags()`, e.g.
    /// `-Watomic-alignment`), in emission order.
    pub diags: Vec<Diag>,
    /// Per-function state.
    pub f: Fx,
    /// The `$_N` ids of unnamed classes, in first-mangle order.
    pub anon: mangle::AnonIds,
    /// Dynamic initialization of static-storage variables.
    pub gi: crate::global_init::GlobalInits,
    /// TBAA node caches (`!tbaa`, `!tbaa.struct`).
    pub tbaa: crate::tbaa::TbaaCache,
}

impl<'a> Cg<'a> {
    /// Whether `t` is consteval-only ([basic.types.general], P2996R13;
    /// sema's `is_consteval_only_type`, spec/reflection-repr.md §6):
    /// `std::meta::info`, or a pointer, reference, array, function or
    /// member pointer type or class reaching one through its parts, bases
    /// or non-static data members. Its objects exist only during constant
    /// evaluation, so a variable of such a type is never emitted. Classes
    /// are walked with a worklist and cached.
    pub(crate) fn is_consteval_only(&mut self, t: QualType) -> bool {
        let ast = self.ast;
        let mut seen: FxHashSet<DeclId> = FxHashSet::default();
        let mut records: Vec<DeclId> = Vec::new();
        let mut pending: Vec<DeclId> = Vec::new();
        let mut types = vec![t];
        let mut found = false;
        'walk: loop {
            while let Some(mut t) = types.pop() {
                // Bounded: a type nests finitely.
                loop {
                    match ast.types.canonical_view(t) {
                        TypeView::Builtin(_) => {
                            if ast.types.canonical(t).without_quals()
                                == BuiltinType::MetaInfo.qual_type()
                            {
                                found = true;
                                break 'walk;
                            }
                            break;
                        }
                        TypeView::Pointer(p)
                        | TypeView::LValueRef(p)
                        | TypeView::RValueRef(p)
                        | TypeView::IncompleteArray(p)
                        | TypeView::MemberPointer { pointee: p, .. } => t = p,
                        TypeView::ConstantArray(_) | TypeView::VariableArray { .. } => {
                            let Some(e) = ast.types.element(t) else { break };
                            t = e;
                        }
                        TypeView::Function(p) => {
                            types.push(p.ret());
                            types.extend_from_slice(ast.types.params(&p));
                            break;
                        }
                        TypeView::Record(r) => {
                            if let Some(def) = ast.definition(r) {
                                match self.consteval_only.get(&def) {
                                    Some(&true) => {
                                        found = true;
                                        break 'walk;
                                    }
                                    Some(&false) => {}
                                    None => {
                                        if seen.insert(def) {
                                            records.push(def);
                                            pending.push(def);
                                        }
                                    }
                                }
                            }
                            break;
                        }
                        _ => break,
                    }
                }
            }
            // The next class not yet walked: its bases and fields.
            let Some(def) = pending.pop() else { break };
            types.extend(ast.decls.bases(def, &ast.lists).map(|b| b.ty));
            types.extend(
                ast.members(def)
                    .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
                    .filter_map(|m| ast.decls.get(m).ty()),
            );
        }
        if !found {
            // Nothing reachable from `t` is consteval-only.
            for d in records {
                self.consteval_only.insert(d, false);
            }
        } else if let TypeView::Record(r) = ast.types.canonical_view(t)
            && let Some(def) = ast.definition(r)
        {
            self.consteval_only.insert(def, true);
        }
        found
    }

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
            optnone_fns: FxHashSet::default(),
            gvars: FxHashMap::default(),
            deferred: FxHashMap::default(),
            completion_index: FxHashMap::default(),
            walk_epoch: u32::MAX,
            held: ast
                .templates
                .end_of_tu_defs()
                .iter()
                .chain(ast.templates.ref_var_insts().iter().map(|(_, _, v)| v))
                .map(|&d| ast.first(d))
                .collect(),
            ref_vars: {
                let mut m: FxHashMap<(DeclId, bool), Vec<DeclId>> = FxHashMap::default();
                for &(anchor, after, v) in ast.templates.ref_var_insts() {
                    m.entry((anchor, after)).or_default().push(ast.first(v));
                }
                m
            },
            to_emit: Vec::new(),
            strings: FxHashMap::default(),
            named: FxHashMap::default(),
            named_base: FxHashMap::default(),
            const_objects: FxHashMap::default(),
            contract_objects: FxHashMap::default(),
            contract_checked: false,
            srcloc: None,
            static_locals: FxHashMap::default(),
            consteval_only: FxHashMap::default(),
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
            reps: FxHashMap::default(),
            weakref_syms: FxHashSet::default(),
            alias: crate::alias::AliasState::default(),
            dup_reported: false,
            thunk_fns: FxHashMap::default(),
            sdone: FxHashSet::default(),
            vtables: FxHashMap::default(),
            vts: kiln_abi::vtable::VTableSession::new(ast, target),
            deferred_vtables: Vec::new(),
            opportunistic_vtables: Vec::new(),
            explicit_vtables: Vec::new(),
            vt_emitted: FxHashSet::default(),
            rtti_globals: FxHashMap::default(),
            pending: Vec::new(),
            vararg_bodies: FxHashMap::default(),
            diags: Vec::new(),
            f: Fx::empty(),
            gi: crate::global_init::GlobalInits::default(),
            anon: mangle::AnonIds::with_reflection_mangling(opts.reflection_mangling),
            tbaa: crate::tbaa::TbaaCache::default(),
            vis_active: opts.visibility != Visibility::Default
                || opts.type_visibility != Visibility::Default
                || opts.inlines_hidden
                || ast.attrs.has_kind(kiln_ast::attr::AttrKind::VISIBILITY)
                || ast.attrs.has_kind(kiln_ast::attr::AttrKind::TYPE_VISIBILITY),
            lv_cache: std::cell::RefCell::default(),
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
            self.opts.optimize(),
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
    pub fn run(mut self) -> (R<Module>, Vec<Diag>) {
        let r = self.run_tu();
        (r.map(|()| self.m), self.diags)
    }

    fn run_tu(&mut self) -> R<()> {
        let tu = self.ast.tu;
        let completed = self.ast.decls.completed_records();
        self.completion_index.reserve(completed.len());
        for (i, &d) in (0u32..).zip(completed) {
            self.completion_index.entry(d).or_insert(i);
        }
        self.emit_members(tu)?;
        self.walk_epoch = u32::MAX;
        // A variable instantiated at a reference whose declaration the
        // walk did not reach (§2a).
        for (_, _, v) in self.ast.templates.ref_var_insts() {
            self.emit_instantiated_var(self.ast.first(*v))?;
        }
        self.emit_late_explicit_insts()?;
        self.emit_key_function_vtables()?;
        self.emit_end_of_tu_instantiations()?;
        self.emit_deferred()?;
        self.emit_vtables_opportunistically()?;
        self.resolve_placeholders()?;
        self.emit_global_init_trailer()?;
        self.emit_contract_defaults()?;
        self.check_aliases();
        self.trailer();
        Ok(())
    }

    /// Class definition `def` is incomplete to code emitted now: the
    /// top-level walk is emitting a declaration that ended before the
    /// class was completed (clang `isFuncTypeConvertible` /
    /// `isCompleteDefinition` at `HandleTopLevelDecl`).
    pub(crate) fn class_incomplete_now(&self, def: DeclId) -> bool {
        self.walk_epoch != u32::MAX
            && self.completion_index.get(&def).is_some_and(|&i| i >= self.walk_epoch)
    }

    /// A virtual member function (clang `SetFunctionAttributes` makes it
    /// `unnamed_addr`, also when its declaration is a placeholder).
    fn is_virtual_method(&self, first: DeclId) -> bool {
        let ast = self.ast;
        ast.decls.get(first).kind() == DeclKind::CXXMethod
            && ast
                .redecls_of(first)
                .any(|r| ast.decls.function(r).is_some_and(|x| x.spec.is_virtual()))
    }

    /// A parameter or the return type of function `first` is a class
    /// that is incomplete at this point of emission: never defined, or
    /// defined later in the translation unit (clang
    /// `isFuncTypeConvertible`).
    pub(crate) fn has_incomplete_class_in_sig(&self, first: DeclId) -> bool {
        let ast = self.ast;
        let Some(fty) = ast.decls.get(first).ty() else { return false };
        let Some(p) = ast.types.proto(fty) else { return false };
        let incomplete = |q: QualType| {
            ast.types.as_record(q).is_some_and(|rd| match ast.definition(rd) {
                None => true,
                Some(def) => self.class_incomplete_now(def),
            })
        };
        incomplete(p.ret()) || ast.types.params(p).iter().any(|&q| incomplete(q))
    }

    fn emit_members(&mut self, ctx: DeclId) -> R<()> {
        let ast = self.ast;
        let members: Vec<DeclId> = ast.decls.members(ctx).collect();
        self.emit_explicit_member_insts(ctx)?;
        let epochs = if ctx == ast.tu { self.member_epochs(&members) } else { Vec::new() };
        if let Some(&e) = epochs.first() {
            self.walk_epoch = e;
        }
        self.emit_ref_vars(ctx, true)?;
        for (i, &d) in members.iter().enumerate() {
            if let Some(&e) = epochs.get(i) {
                self.walk_epoch = e;
            }
            self.emit_ref_vars(d, false)?;
            if !ast.templates.is_late_explicit_inst(ast.first(d)) {
                self.emit_member(d)?;
                if !epochs.is_empty() {
                    // The explicit instantiation definitions after `d` are
                    // top-level declarations of their own, which the member
                    // list does not hold: what was complete when the next
                    // member ended is (at most a declaration later than clang).
                    self.walk_epoch = epochs.get(i + 1).copied().unwrap_or(u32::MAX);
                }
                self.emit_explicit_member_insts(d)?;
            }
            self.emit_ref_vars(d, true)?;
        }
        Ok(())
    }

    /// The variables Sema instantiated at a reference in (or after)
    /// namespace-scope declaration `anchor`, handed to `EmitGlobal`
    /// before it, or after it when `after` (§2a).
    fn emit_ref_vars(&mut self, anchor: DeclId, after: bool) -> R<()> {
        if self.ref_vars.is_empty() {
            return Ok(());
        }
        let Some(vars) = self.ref_vars.remove(&(anchor, after)) else { return Ok(()) };
        for v in vars {
            self.emit_instantiated_var(v)?;
        }
        Ok(())
    }

    /// The class-completion epoch at the end of the top-level declaration
    /// each member of the translation unit belongs to (the first recorded
    /// end at or after it, `DeclArena::top_level_ends`; `u32::MAX` past
    /// the last).
    fn member_epochs(&self, members: &[DeclId]) -> Vec<u32> {
        let mut at: FxHashMap<DeclId, u32> = FxHashMap::default();
        for &(last, e) in self.ast.decls.top_level_ends() {
            if let Some(l) = last {
                at.entry(l).or_insert(e);
            }
        }
        let mut out = vec![u32::MAX; members.len()];
        let mut cur = u32::MAX;
        for (i, d) in members.iter().enumerate().rev() {
            if let Some(&e) = at.get(d) {
                cur = e;
            }
            out[i] = cur;
        }
        out
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
            // An `alias` / `ifunc` declaration is a definition (clang
            // `EmitGlobal` -> `EmitAliasDefinition`).
            if self.emit_alias_definition(d)? {
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
        if ast.weakref_target(d).is_some() {
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
            // clang `EmitGlobal` names a deferred definition (its label
            // is the only name another definition can share).
            if mangle::asm_label(ast, first).is_some() {
                let name = mangle::mangle_decl(ast, &mut self.anon, first);
                self.note_rep(&name, first);
            } else if self.fn_internal(first) {
                // So it names an internal one: the `$_N` of an unnamed
                // class in its name (a closure in a template argument)
                // is allocated here, in clang's first-mangled order
                // ([`mangle::AnonIds`]). An external name has no `$_N`.
                let _ = mangle::mangle_decl(ast, &mut self.anon, first);
            }
            self.deferred.insert(first, d);
            self.alias_target_needed(first)?;
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
        // clang `EmitGlobal`: a weak reference produces no output by
        // itself; a use names its target ([`Cg::get_gvar`]).
        if ast.weakref_target(d).is_some() {
            return Ok(());
        }
        // [dcl.link]/8: `extern "C" int x;` declares, like `extern`.
        let ext = v.spec.storage() == VarStorage::Extern || ast.decls.is_single_line_linkage(d);
        // [temp.expl.spec]/14: `template<> int C<char>::n;` declares: uses
        // name the external symbol (a later one with an initializer, or
        // another unit, defines it).
        if ext && v.init.is_none() || ast.is_member_spec_declaration(d) {
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
            self.alias_target_needed(first)?;
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
                    self.held.insert(first);
                    self.emit_instantiated_var(first)?;
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

    /// Hands the definition Sema instantiated for held variable `first`
    /// to `EmitGlobal` (once; §2a).
    fn emit_instantiated_var(&mut self, first: DeclId) -> R<()> {
        let ast = self.ast;
        if !self.held.remove(&first) {
            return Ok(());
        }
        // The out-of-line definition Sema instantiated, with or without an
        // initializer (`template<class T> T A<T>::s;`).
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
        } else if ast.spec_info(first).is_some() {
            // A variable template specialization is its own definition
            // (also one used before its template's definition; one without
            // an initializer is default-initialized).
            self.emit_global_var(first)?;
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
                // A member excluded from its class's explicit
                // instantiation stays implicitly instantiated (clang
                // `InstantiateClassMembers` leaves its kind).
                let ctx = ast.decls.get(first).ctx();
                ast.excluded_from_explicit_instantiation(first)
                    || ctx.is_none_or(|c| match ast.spec_info(c) {
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
            // An instantiation kept `available_externally` under an
            // explicit instantiation declaration (a function template
            // specialization with a deduced return type at -O1) is
            // emitted on use, as an implicit one.
            if self.is_implicit_instantiation(r) || self.is_available_externally_fn(first) {
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
                let ctx = ast.decls.get(first).ctx();
                let in_class = ctx.is_some_and(|c| {
                    matches!(
                        ast.decls.get(c).kind(),
                        DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization
                    )
                });
                // An explicit specialization declared in a class template
                // is redeclared with each instantiated class, which the
                // walk does not reach either.
                let in_instantiated = || {
                    ctx.and_then(|c| ast.spec_info(c))
                        .is_some_and(|(_, s)| s.kind != SpecKind::ExplicitSpecialization)
                };
                in_class
                    && ast.spec_info(first).is_some_and(|(_, s)| {
                        s.kind == SpecKind::ImplicitInstantiation
                            || (s.kind == SpecKind::ExplicitSpecialization && in_instantiated())
                    })
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
    /// definition is provided), or is a GNU inline function
    /// ([`Self::is_gnu_inline_fn`]), and gets no definition here. An
    /// inline function clang would give `GVA_AvailableExternally` and keep
    /// (`shouldEmitFunction`) is the exception
    /// ([`Self::is_available_externally_fn`]).
    pub(crate) fn is_explicit_inst_declaration(&self, first: DeclId) -> bool {
        self.defined_elsewhere(first) && !self.keeps_available_externally(first)
    }

    /// clang `GVA_AvailableExternally` kept by `shouldEmitFunction`: an
    /// inline function under an explicit instantiation declaration, or a
    /// GNU inline function, emitted `available_externally` so the
    /// optimizer can inline it (spec/codegen.md "Explicit instantiation
    /// declarations", "GNU inline functions").
    pub(crate) fn is_available_externally_fn(&self, first: DeclId) -> bool {
        self.defined_elsewhere(first) && self.keeps_available_externally(first)
    }

    /// clang `GVA_AvailableExternally`: `first` is under an explicit
    /// instantiation declaration or is a GNU inline function, so the
    /// program's definition lives in another translation unit.
    fn defined_elsewhere(&self, first: DeclId) -> bool {
        self.explicit_inst_kind(first) == Some(SpecKind::ExplicitInstantiationDeclaration)
            || self.is_gnu_inline_fn(first)
    }

    /// clang `basicGVALinkageForFunction`'s GNU inline rule: in C++ an
    /// inline function with external linkage that carries `gnu_inline`
    /// (on any redeclaration; Sema keeps it only on an `inline`-specified
    /// one) has a definition that is not externally visible
    /// (`isInlineDefinitionExternallyVisible` is false in C++, with or
    /// without `extern`): another translation unit, typically the C
    /// library, provides the symbol (spec/codegen.md "GNU inline
    /// functions").
    fn is_gnu_inline_fn(&self, first: DeclId) -> bool {
        self.ast.decls.function(first).is_some()
            && self.has_fn_attr(first, kiln_ast::attr::AttrKind::GNU_INLINE)
            && !self.fn_internal(first)
    }

    /// clang `shouldEmitFunction` for a function under an explicit
    /// instantiation declaration or a GNU inline function: at -O0 only an
    /// inline `always_inline`
    /// one (the O0 pipeline's always-inliner inlines and deletes it, so
    /// the program never references its symbol: libstdc++'s
    /// `std::allocator<char>::allocate` is not exported); at -O1 and above
    /// any one whose definition is still instantiated ([temp.explicit]/12:
    /// an inline function or one with a deduced return type) and that
    /// has no `noinline`. Neither when the function is trivially
    /// recursive ([`Self::is_trivially_recursive`], clang's last check).
    fn keeps_available_externally(&self, first: DeclId) -> bool {
        (self.is_always_inline_fn(first) || self.keeps_inline_definition(first))
            && !self.is_trivially_recursive(first)
    }

    /// [`Self::keeps_available_externally`] for a function without
    /// `always_inline`: optimizing, and the definition is instantiated.
    fn keeps_inline_definition(&self, first: DeclId) -> bool {
        let ast = self.ast;
        let spec_any = |d: DeclId, p: &dyn Fn(kiln_ast::decl::FnSpec) -> bool| {
            ast.redecls_of(d).any(|r| ast.decls.function(r).is_some_and(|f| p(f.spec)))
        };
        // Sema marks an instantiation inline as its pattern is (clang
        // `InstantiateFunctionDefinition` copies `isInlined`).
        let inline = spec_any(first, &|s| {
            s.is_inline() || s.implicit_inline() || s.constexpr_kind() != ConstexprKind::None
        });
        self.opts.optimize()
            && (inline || spec_any(first, &kiln_ast::decl::FnSpec::return_deduced))
            && !self.has_fn_attr(first, kiln_ast::attr::AttrKind::NOINLINE)
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
    /// present), -O0 or `-fno-inline` gives `noinline` (at -O0, a `minsize`
    /// function), and above -O0 a `noinline` function keeps `noinline` and
    /// an `inline`-specified one
    /// (on a redeclaration or one of its template pattern's) gets
    /// `inlinehint`. [`Self::def_opt_attrs`] adds `naked`, `minsize` and
    /// `optsize`.
    pub(crate) fn def_inline_attrs(&self, d: DeclId) -> DefInline {
        use kiln_ast::attr::AttrKind as K;
        let noinline = self.has_fn_attr(d, K::NOINLINE);
        let optnone = self.has_fn_attr(d, K::OPTNONE);
        let always = self.has_fn_attr(d, K::ALWAYS_INLINE);
        let should_optnone = !self.opts.optimize() && !always && !self.has_fn_attr(d, K::MINSIZE);
        if optnone || should_optnone {
            DefInline::OptNone
        } else if self.has_fn_attr(d, K::NAKED) {
            DefInline::NoInline
        } else if always {
            if noinline { DefInline::NoInline } else { DefInline::Always }
        } else if !self.opts.optimize() || noinline || self.opts.no_inline {
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
    pub(crate) fn explicit_inst_kind(&self, first: DeclId) -> Option<SpecKind> {
        self.ast.governing_explicit_inst_kind(first)
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

    /// The IR type a value of type `q` is loaded and stored as
    /// (`convertTypeForLoadStore`): `iN` of a `_BitInt`'s ABI size where
    /// its in-memory type is a byte array, else [`Self::mty`].
    pub fn lsty(&mut self, q: QualType) -> LTy {
        let t = self.tys.convert_load_store(q);
        self.ir(&t)
    }

    /// `sizeof` and `alignof` of `q`.
    pub fn size_align(&mut self, q: QualType) -> (u64, u64) {
        let l = self.tys.layouter().type_layout(q);
        (l.size, l.align)
    }

    /// clang `EmitDeclInvariant`, the tail of `EmitCXXGlobalVarDeclInit`:
    /// after the dynamic initialization of variable `d` of type `ty` at
    /// global `g`, an object of constant storage (clang
    /// `isConstantStorage(Ctx, true, !NeedsDtor)`: `const`, or an array
    /// of `const`, with no `mutable` member and needing no destruction)
    /// never changes again, so `-O1` and above mark it with
    /// `llvm.invariant.start` ([dcl.type.cv]/4; spec/codegen.md
    /// "Invariant globals").
    pub(crate) fn emit_decl_invariant(&mut self, d: DeclId, ty: QualType, g: GlobalId) {
        let ast = self.ast;
        if !self.opts.optimize()
            || ast.types.is_reference(ty)
            || !ast.types.effective_quals(ty).is_const()
            || has_mutable(ast, ty)
            || self.var_destructed_class(d, ty).is_some()
        {
            return;
        }
        let f = kiln_llvm::intrinsics::invariant_start(&mut self.m);
        let fn_ty = self.m.func(f).ty();
        let width = self.size_align(ty).0;
        let size = V::C(self.m.const_int(LTy::I64, i128::from(width)));
        let callee = V::C(Value::func(f));
        let opts = Box::default();
        let args = vec![size, V::C(gval(g))];
        self.f.fir.push(Op::Call { fn_ty, callee, args, opts }, LTy::PTR);
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
    /// an incomplete type (whose alignment is 1, except an array of unknown
    /// bound, aligned as its element: `getTypeAlignInChars`). Classes use
    /// their non-virtual size and alignment.
    pub fn pointee_layout(&mut self, p: QualType) -> (Option<u64>, u64) {
        let ast = self.ast;
        match ast.types.canonical_view(p) {
            // getMinimumClassObjectSize / getClassPointerAlignment: a
            // final class is known to be a complete object.
            TypeView::Record(d) => self.this_layout(d),
            TypeView::IncompleteArray(e) => (None, self.size_align(e).1),
            TypeView::Function(_) => (None, 1),
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
            // A class completed after the top-level declaration being
            // emitted is incomplete.
            Some(def) if !self.class_incomplete_now(def) => {
                let fin = self.class_is_final(def);
                let eff = fin || self.dtor_is_final(def);
                let l = self.tys.layouter().record(def);
                let size = if eff { l.size } else { l.nv_size.max(1) };
                let align = if eff { l.align } else { l.nv_align };
                (Some(size), u64::from(align))
            }
            _ => (None, 1),
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
    /// `nonnull align A dereferenceable(N)` (no `nonnull` under
    /// `-fno-delete-null-pointer-checks`).
    fn ref_attrs(&mut self, q: QualType, out: &mut Vec<Attr>) {
        let Some(p) = self.referee(q) else { return };
        if !self.opts.null_pointer_is_valid {
            out.push(Attr::Flag(AttrKind::NonNull));
        }
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

    /// clang `hasBooleanRepresentation`: `bool`, or an enumeration whose
    /// underlying type is `bool` (an `i1` value in an `i8` object).
    pub fn has_bool_rep(&self, q: QualType) -> bool {
        kiln_abi::irtype::has_boolean_representation(self.ast, q)
    }

    /// [`Cg::has_bool_rep`], seeing through `_Atomic` (the memory form of
    /// an `_Atomic(bool)` is `i8` too).
    pub fn is_bool_value(&self, q: QualType) -> bool {
        match self.ast.types.canonical_view(q) {
            TypeView::Atomic(v) => self.has_bool_rep(v),
            _ => self.has_bool_rep(q),
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

    /// WP-803: `sig` (of function type `fty`, variadic extras included)
    /// with a hidden `size_t` parameter after each `pass_object_size`
    /// parameter `ps` (clang `appendParameterTypes` /
    /// `getExtParameterInfos`).
    pub(crate) fn with_object_sizes(
        &mut self,
        sig: Sig,
        fty: QualType,
        ps: &[(usize, u32, bool)],
        extern_c: bool,
    ) -> Sig {
        if ps.is_empty() {
            return sig;
        }
        let ast = self.ast;
        let p = *ast.types.proto(fty).expect("function prototype");
        let size_q = QualType::unqual(ast.types.target().size_t);
        let mut all = Vec::with_capacity(sig.params.len() + ps.len());
        for (i, &q) in sig.params.iter().enumerate() {
            all.push(q);
            if ps.iter().any(|o| o.0 == i) {
                all.push(size_q);
            }
        }
        let n = ast.types.params(&p).len() + ps.len();
        let t = sig.this.is_some();
        let info = self.classify(sig.ret, &all, n, !extern_c, t);
        let decl_info = self.classify(sig.ret, &all[..n], n, !extern_c, t);
        let fn_ty = self.fn_type(&decl_info, t, p.variadic());
        Sig { info, fn_ty, params: all, ..sig }
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
            // alignment), plus `noundef`; `dereferenceable_or_null(N)`
            // without `nonnull` under `-fno-delete-null-pointer-checks`.
            let (size, align) = self.this_layout(t);
            let mut s = vec![Attr::Flag(AttrKind::NoUndef)];
            if self.opts.null_pointer_is_valid {
                s.extend(size.map(Attr::DereferenceableOrNull));
            } else {
                s.push(Attr::Flag(AttrKind::NonNull));
                s.extend(size.map(Attr::Dereferenceable));
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

    /// The size attributes of `-Os` (`optsize`) and `-Oz` (`optsize
    /// minsize`) on a function or call site that is not `optnone` (clang
    /// `getTrivialDefaultFunctionAttributes`).
    pub(crate) fn size_attrs(&self, out: &mut Vec<Attr>) {
        if self.opts.opt_size > 0 {
            out.push(Attr::Flag(AttrKind::OptimizeForSize));
        }
        if self.opts.opt_size == 2 {
            out.push(Attr::Flag(AttrKind::MinSize));
        }
    }

    /// The call-site function attributes `-Os`/`-Oz` add to a call of
    /// `fid` (`None`: an indirect call) through clang `EmitCall`: the
    /// [`Self::size_attrs`] unless the callee is `optnone`.
    pub(crate) fn call_size_attrs(&self, fid: Option<FuncId>, out: &mut Vec<Attr>) {
        if !fid.is_some_and(|f| self.optnone_fns.contains(&f)) {
            self.size_attrs(out);
        }
    }

    /// The target-dependent function attributes (clang
    /// `getDefaultFunctionAttributes` on a function, not a call site),
    /// with the [`Self::size_attrs`]; an `optnone` function drops those
    /// again.
    pub fn target_attrs(&self, out: &mut Vec<Attr>) {
        self.target_attrs_of(out, None);
    }

    /// [`Self::target_attrs`] of function `d` (`None`: a helper without a
    /// declaration): its `target("...")` attribute adds to the
    /// `target-features` ([`Self::feature_map`]), and its
    /// `disable_tail_calls` attribute gives `"disable-tail-calls"="true"`
    /// on a declaration and a definition alike (clang
    /// `ConstructAttributeList`, not on a call site; WP-682).
    pub(crate) fn target_attrs_of(&self, out: &mut Vec<Attr>, d: Option<DeclId>) {
        self.size_attrs(out);
        if let Some(d) = d
            && self.has_fn_attr(d, kiln_ast::attr::AttrKind::DISABLE_TAIL_CALLS)
        {
            out.push(Attr::string("disable-tail-calls", "true"));
        }
        if let Some(fp) = self.frame_pointer().attr() {
            out.push(Attr::string("frame-pointer", fp));
        }
        if self.opts.null_pointer_is_valid {
            out.push(Attr::Flag(AttrKind::NullPointerIsValid));
        }
        out.push(Attr::string("no-trapping-math", "true"));
        out.push(Attr::string("stack-protector-buffer-size", "8"));
        let tune = self.opts.tune_cpu.as_deref();
        // `-march=native`: no `target-cpu`, the backend's `-mcpu=native`.
        let cpu = self.opts.target_cpu.as_deref();
        let native = cpu == Some("native");
        if self.is_x86() {
            // clang `getTrivialDefaultFunctionAttributes`: `-march=<cpu>`
            // names the CPU and drops the default `tune-cpu`. WP-630: so
            // does a `target("arch=<cpu>")`, and `tune=<cpu>` names the
            // tune CPU (clang `getFunctionFeatureMap`'s callers).
            let a = d.and_then(|d| self.target_attr_features(d)).unwrap_or_default();
            let mut tune = tune.or(if cpu.is_none() { Some("generic") } else { None });
            if a.cpu.is_some() {
                tune = None;
            }
            tune = a.tune.as_deref().or(tune);
            match a.cpu.as_deref().or(cpu) {
                Some("native") => {}
                c => out.push(Attr::string("target-cpu", c.unwrap_or("x86-64"))),
            }
            out.push(Attr::string("target-features", &self.features_attr(d)));
            if let Some(t) = tune {
                out.push(Attr::string("tune-cpu", t));
            }
        } else {
            if !native {
                out.push(Attr::string("target-cpu", "generic"));
            }
            out.push(Attr::string("target-features", &self.features_attr(d)));
            if let Some(t) = tune {
                out.push(Attr::string("tune-cpu", t));
            }
        }
    }

    /// The unwind-table and stack-protector attributes of a function
    /// definition (clang `SetLLVMFunctionAttributesForDefinition`):
    /// `uwtable` / `uwtable(sync)` per [`CodegenOptions::unwind_tables`],
    /// and `ssp` / `sspstrong` / `sspreq` per
    /// [`CodegenOptions::stack_protector`] (spec/codegen.md "Hardening
    /// and overflow options").
    pub(crate) fn def_attrs(&self, out: &mut Vec<Attr>) {
        match self.opts.unwind_tables {
            Some(UnwindTables::Async) => out.push(Attr::UwTable(UwTableKind::Async)),
            Some(UnwindTables::Sync) => out.push(Attr::UwTable(UwTableKind::Sync)),
            None => {}
        }
        let ssp = match self.opts.stack_protector {
            StackProtector::Off => return,
            StackProtector::On => AttrKind::StackProtect,
            StackProtector::Strong => AttrKind::StackProtectStrong,
            StackProtector::All => AttrKind::StackProtectReq,
        };
        out.push(Attr::Flag(ssp));
    }

    /// The `target-features` string of function `d`
    /// ([`Self::feature_map`]); clang lists the enabled features sorted,
    /// then the disabled ones sorted.
    fn features_attr(&self, d: Option<DeclId>) -> String {
        let f = self.feature_map(d);
        let on = f.iter().filter(|e| *e.1).map(|e| format!("+{}", e.0));
        let off = f.iter().filter(|e| !*e.1).map(|e| format!("-{}", e.0));
        on.chain(off).collect::<Vec<_>>().join(",")
    }

    /// `vscale_range(N/128,N/128)` under `-msve-vector-bits=N`, else
    /// `vscale_range(1,16)` with the `sve` target feature, on a function
    /// definition (clang `getVScaleRange` in `StartFunction`; not on the
    /// thunks and helpers clang builds without a `CodeGenFunction`).
    pub fn vscale_attr(&self, out: &mut Vec<Attr>) {
        if self.is_x86() {
            return;
        }
        if let Some(n) = self.opts.sve_vector_bits {
            out.push(Attr::VScaleRange(n / 128, n / 128));
        } else if self.opts.sve {
            out.push(Attr::VScaleRange(1, 16));
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
        if let Some(target) = self.weakref_symbol(first) {
            let f = if let Some(f) = self.m.get_function(&target) {
                f
            } else {
                let sig = self.decl_sig(first)?;
                let f = self.declare_fn(&target, first, &sig)?;
                self.m.func_mut(f).linkage = Linkage::ExternWeak;
                self.weakref_syms.insert(target);
                f
            };
            self.fns.insert(first, f);
            return Ok(f);
        }
        let name = mangle::mangle_decl(ast, &mut self.anon, first);
        self.note_rep(&name, first);
        let sig = self.decl_sig(first)?;
        // An existing function of that name and type is this one: a
        // runtime function codegen declared first (`__cxa_allocate_exception`
        // from a `throw`), or a separately declared redeclaration of the
        // same `extern "C"` function (clang `GetOrCreateLLVMFunction`
        // returns the entry unchanged).
        if self.m.get_function(&name).is_some() {
            self.strong_ref(&name, first);
        }
        let f = match self.m.get_function(&name) {
            Some(f) if self.m.func(f).ty() == sig.fn_ty => {
                let nothrow = sig.nothrow || ast.has_nothrow_attr(first);
                self.nounwind.entry(f).or_insert(nothrow);
                f
            }
            // clang `GetOrCreateLLVMFunction` returns an entry of another
            // type unchanged unless for a definition: two declarations
            // whose asm labels name one symbol are one function, called
            // with each declaration's own type; a definition retypes it
            // ([`Cg::fn_for_definition`]).
            Some(f) if self.m.resolve(Value::func(f)) == Value::func(f) => f,
            // clang `GetOrCreateLLVMFunction` of a type that cannot be
            // converted yet (a class parameter or return type incomplete
            // here): a `void ()` placeholder without attributes, kept
            // for later declarations and retyped by a definition; calls
            // use their own type.
            None if self.has_incomplete_class_in_sig(first) => {
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
                if self.is_virtual_method(first) {
                    self.m.func_mut(f).unnamed_addr = UnnamedAddr::Global;
                }
                let nothrow = sig.nothrow || ast.has_nothrow_attr(first);
                self.nounwind.insert(f, nothrow);
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

    /// The symbol a use of the GNU `weakref` declaration `first` refers
    /// to ([`kiln_ast::Ast::weakref_target`]).
    fn weakref_symbol(&self, first: DeclId) -> Option<String> {
        let lit = self.ast.weakref_target(first)?;
        Some(String::from_utf8_lossy(self.ast.strings.bytes(lit)).into_owned())
    }

    /// An ordinary reference (or definition) of symbol `name` by `first`:
    /// a symbol only `weakref` declarations referred to so far becomes an
    /// external reference unless `first` is itself `weak` (clang
    /// `GetOrCreateLLVMFunction` / `GetOrCreateLLVMGlobal` erasing it from
    /// `WeakRefReferences`).
    fn strong_ref(&mut self, name: &str, first: DeclId) {
        if !self.weakref_syms.remove(name) || self.ast.is_weak_decl(first) {
            return;
        }
        if let Some(f) = self.m.get_function(name) {
            self.m.func_mut(f).linkage = Linkage::External;
        } else if let Some(g) = self.m.get_global(name) {
            self.m.global_mut(g).linkage = Linkage::External;
        }
    }

    /// Declares the function `name` for declaration `first` with
    /// signature `sig` (the declaration attributes of
    /// `GetOrCreateLLVMFunction`).
    /// clang `setNonAliasAttributes` / `SetFunctionAttributes` /
    /// `EmitStaticVarDecl`: a GNU `section("name")` places the function
    /// or variable `d` in section `name`, declaration or definition.
    fn decl_section(&self, d: DeclId) -> Option<Box<str>> {
        let lit = self.ast.section_of(d)?;
        Some(String::from_utf8_lossy(self.ast.strings.bytes(lit)).into())
    }

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
        self.set_decl_attrs(f, first, sig);
        Ok(f)
    }

    /// The function `f` ([`Cg::get_fn`] of `d`) to define `d` into: a
    /// declaration of another type under the same name (an asm label
    /// shared with a declaration of another type) is replaced by one of
    /// the definition's type at the end of the module (clang
    /// `GetOrCreateLLVMFunction` with `IsForDefinition`).
    pub(crate) fn fn_for_definition(&mut self, d: DeclId, f: FuncId) -> R<FuncId> {
        let first = self.ast.first(d);
        self.check_duplicate_def(d, f);
        let sig = self.decl_sig(first)?;
        if self.m.func(f).ty() == sig.fn_ty || !self.m.func(f).is_declaration() {
            return Ok(f);
        }
        if let Err(e) = self.m.retype_function_to_end(f, sig.fn_ty) {
            return err(Some(self.ast.decls.get(d).loc()), format!("cannot define {e:?}"));
        }
        self.set_decl_attrs(f, first, &sig);
        Ok(f)
    }

    /// Records `first` as the representative declaration of symbol
    /// `name` unless one is recorded.
    pub(crate) fn note_rep(&mut self, name: &str, first: DeclId) {
        if !self.reps.contains_key(name) {
            self.reps.insert(name.to_owned(), first);
        }
    }

    /// clang `GetOrCreateLLVMFunction` with `IsForDefinition`: the
    /// definition `d` into the defined function `f` whose representative
    /// declaration is another entity is `err_duplicate_mangled_name`
    /// (once: clang emits no later declaration after the error). The
    /// body is still appended (its blocks unreachable), as clang does.
    pub(crate) fn check_duplicate_def(&mut self, d: DeclId, f: FuncId) {
        if self.dup_reported || self.m.func(f).is_declaration() {
            return;
        }
        let ast = self.ast;
        let name = self.m.func(f).name();
        let Some(&rep) = self.reps.get(name) else { return };
        if rep == ast.first(d) {
            return;
        }
        let prev = ast.definition(rep).unwrap_or(rep);
        let diag =
            Diag::new(DiagId::cg_duplicate_mangled_name, Span::empty(ast.decls.get(d).loc()))
                .arg(name)
                .note(DiagId::note_previous_definition, Span::empty(ast.decls.get(prev).loc()));
        self.diags.push(diag);
        self.dup_reported = true;
    }

    /// The declaration attributes of `f` for `first` with signature `sig`.
    pub(crate) fn set_decl_attrs(&mut self, f: FuncId, first: DeclId, sig: &Sig) {
        let ast = self.ast;
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
        // clang `ConstructAttributeList` under `-fno-plt`: a function not
        // yet defined is `nonlazybind` (kiln: none in the translation unit;
        // spec/codegen.md "Hardening and overflow options").
        if self.opts.no_plt && ast.definition(first).is_none() {
            fa.push(Attr::Flag(AttrKind::NonLazyBind));
        }
        self.target_attrs_of(&mut fa, Some(first));
        let optnone = self.has_fn_attr(first, kiln_ast::attr::AttrKind::OPTNONE);
        if optnone {
            strip_size_attrs(&mut fa);
        }
        let fa = self.m.attr_set(&fa);
        let (fa, ret) = self.alloc_fn_attrs(first, fa, ret);
        // clang `SetFunctionAttributes`: virtual member functions are
        // `unnamed_addr`.
        let virt = self.is_virtual_method(first);
        // clang `setGVProperties` on a declaration: only a known
        // visibility (spec/codegen.md "Symbol visibility").
        let vis = self.decl_visibility(first, false);
        // clang `SetFunctionAttributes`: a declaration of a `weak`
        // function is an `extern_weak` reference.
        let weak = ast.is_weak_decl(first) && ast.weakref_target(first).is_none();
        let section = self.decl_section(first);
        {
            let func = self.m.func_mut(f);
            func.visibility = vis;
            func.section = section;
            func.attrs.func = fa;
            func.attrs.ret = ret;
            func.attrs.params = params;
            if virt {
                func.unnamed_addr = UnnamedAddr::Global;
            }
            if weak && func.is_declaration() {
                func.linkage = Linkage::ExternWeak;
            }
        }
        self.nounwind.insert(f, nothrow);
        if optnone {
            self.optnone_fns.insert(f);
        }
    }

    /// Sets linkage and definition attributes of `f` defined by `d`.
    pub fn set_def_linkage(&mut self, f: FuncId, d: DeclId) {
        let internal = self.fn_internal(d);
        // clang `GetGVALinkageForFunction`: an explicit instantiation
        // definition is `GVA_StrongODR` (`weak_odr`), inline or not.
        let weak = !internal && self.is_explicit_inst_definition(d);
        let inline = self.fn_is_inline(d);
        // clang `getLLVMLinkageForDeclarator`: a `weak` definition is
        // `weak` (ahead of the inline and `available_externally` rules;
        // an inline one keeps its comdat).
        let weak_attr = !internal && self.ast.is_weak_decl(d);
        let mut name = self.m.func(f).name().to_owned();
        // clang `getCodegenToUse` → `COMDAT`: a weak (`weak_odr` or
        // `weak`) structor of a class without virtual bases puts its
        // base-object and deleting variants in the `C5`/`D5` comdat (the
        // complete-object variant is an alias).
        let structor_comdat = (weak || weak_attr)
            && matches!(
                self.ast.decls.get(d).kind(),
                DeclKind::CXXConstructor | DeclKind::CXXDestructor
            )
            && !self.structor_has_vbases(d);
        if structor_comdat {
            name = mangle::mangle_structor_comdat(self.ast, &mut self.anon, self.ast.first(d));
        }
        let comdat = if !internal && (inline || weak || structor_comdat) {
            Some(self.m.comdat(&name, ComdatKind::Any))
        } else {
            None
        };
        let available_externally = !internal && self.is_available_externally_fn(self.ast.first(d));
        let vis = if internal {
            Visibility::Default
        } else {
            self.decl_visibility(d, !available_externally)
        };
        let func = self.m.func_mut(f);
        func.visibility = vis;
        if internal {
            func.linkage = Linkage::Internal;
            func.dso_local = false;
        } else if weak_attr {
            func.linkage = Linkage::Weak;
            func.dso_local = true;
            func.comdat = comdat;
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
        let DefFnAttrs {
            decl,
            is_main,
            nounwind,
            mustprogress,
            vector_width,
            noreturn,
            inline,
            extra,
        } = f;
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
        self.def_attrs(&mut a);
        self.vscale_attr(&mut a);
        self.target_attrs_of(&mut a, decl);
        if inline == DefInline::OptNone {
            // `SetLLVMFunctionAttributesForDefinition` removes them.
            strip_size_attrs(&mut a);
        }
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
        // A namespace-scope array compound literal's object is defined at
        // its first use, its initializer emitted before the global, clang
        // `tryEmitGlobalCompoundLiteral` (WP-466).
        if ast.decls.compound_literal_of(first).is_some() {
            let init = ast.decls.var(first).and_then(|v| v.init);
            if init.is_some_and(|e| self.folds.get(e).is_some()) {
                self.emit_var_def(first)?;
                if let Some(&g) = self.gvars.get(&first) {
                    return Ok(g);
                }
            }
            self.to_emit.push(Def::Decl(first));
        }
        let Some(ty) = ast.decls.get(first).ty() else { return err(None, "variable without type") };
        let lt = self.mty(ty);
        // clang `GetWeakRefReference`: a use of a `weakref` variable is a
        // use of its target, declared `extern_weak` unless present.
        if let Some(target) = self.weakref_symbol(first) {
            if let Some(g) = self.m.get_global(&target) {
                self.gvars.insert(first, g);
                return Ok(g);
            }
            // `GetOrCreateLLVMGlobal` without a declaration: no alignment
            // and not `constant`.
            let g = self.m.add_global(&target, lt);
            self.m.global_mut(g).linkage = Linkage::ExternWeak;
            self.gvars.insert(first, g);
            self.weakref_syms.insert(target);
            return Ok(g);
        }
        let name = self.var_symbol(first);
        if self.weakref_syms.contains(&name)
            && let Some(g) = self.m.get_global(&name)
        {
            self.strong_ref(&name, first);
            self.gvars.insert(first, g);
            return Ok(g);
        }
        // clang `GetOrCreateLLVMGlobal` returns an existing global of the
        // name unchanged, whatever its type: variables whose asm labels
        // name one symbol share it.
        if let Some(g) = self.m.get_global(&name)
            && !name.is_empty()
            && mangle::asm_label(ast, first).is_some()
        {
            self.gvars.insert(first, g);
            return Ok(g);
        }
        // A symbol an alias declared by name (an aliasee defined after
        // the alias) is this variable's global.
        let g = match self.m.get_global(&name) {
            Some(g) if self.alias.targets.contains(&name) => g,
            _ => self.m.add_global(&name, lt),
        };
        // clang `GetOrCreateLLVMGlobal`: `getDeclAlign(D)`.
        let align = self.var_align(first);
        let constant = constant_storage(ast, ty);
        let vis = self.decl_visibility(first, false);
        // clang `GetOrCreateLLVMGlobal`: a declaration of a `weak`
        // variable is an `extern_weak` reference.
        let weak = ast.is_weak_decl(first);
        let section = self.decl_section(first);
        let gv = self.m.global_mut(g);
        gv.linkage = if weak { Linkage::ExternWeak } else { Linkage::External };
        gv.visibility = vis;
        gv.section = section;
        gv.align = Some(align);
        gv.is_constant = constant;
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
        // clang 19 `tryEmitGlobalCompoundLiteral` aligns a file-scope
        // compound literal to its type alignment (no x86-64 large-array
        // bump); clang 22 uses the variable rule.
        let literal_type_align =
            self.opts.clang_compat < 22 && self.ast.decls.compound_literal_of(d).is_some();
        kiln_abi::layout::decl_align(self.ast, d, self.target, literal_type_align)
    }

    /// The symbol of namespace-scope or static data member variable
    /// `first`: its mangled name, or `.compoundliteral` for the object of
    /// a namespace-scope array compound literal (uniqued by the module).
    fn var_symbol(&mut self, first: DeclId) -> String {
        if self.ast.decls.compound_literal_of(first).is_some() {
            return ".compoundliteral".to_owned();
        }
        mangle::mangle_decl(self.ast, &mut self.anon, first)
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
            let name = self.var_symbol(first);
            // A declaration of another variable with the same asm label
            // is this definition's global (clang `GetOrCreateLLVMGlobal`
            // with `IsForDefinition` replaces it by one of the
            // initializer's type at the end of the module).
            let shared = mangle::asm_label(ast, first)
                .or_else(|| self.weakref_syms.contains(&name).then(String::new))
                .and_then(|_| self.m.get_global(&name))
                .filter(|&g| self.m.global(g).initializer().is_none());
            self.weakref_syms.remove(&name);
            let g = match shared {
                Some(g) => {
                    self.m.retype_global_to_end(g, ity);
                    g
                }
                None => self.m.add_global(&name, ity),
            };
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
        // Set before the initialization function, whose guard copies it.
        let vis = if internal { Visibility::Default } else { self.decl_visibility(d, true) };
        let weak_attr = !internal && ast.is_weak_decl(d);
        let section = self.decl_section(d);
        let gv = self.m.global_mut(g);
        gv.is_constant = constant;
        gv.align = Some(align);
        gv.visibility = vis;
        gv.section = section;
        if internal {
            gv.linkage = Linkage::Internal;
            gv.dso_local = false;
        } else if weak_attr {
            // clang `getLLVMLinkageForDeclarator` (since clang 15,
            // D126324): plain `weak` for a constant too, since the
            // definition may be replaced at link time.
            gv.linkage = Linkage::Weak;
            gv.dso_local = true;
            gv.comdat = comdat;
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
        let vis = if inline_fn { self.decl_visibility(first, true) } else { Visibility::Default };
        let section = self.decl_section(d);
        let gv = self.m.global_mut(g);
        gv.is_constant = constant;
        gv.align = Some(align);
        gv.visibility = vis;
        gv.section = section;
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
        // clang: a template parameter object has the visibility of its
        // type (and value; not modelled: spec/codegen.md "Symbol
        // visibility"), not the global mode.
        let vis = if internal || !self.vis_active {
            Visibility::Default
        } else {
            self.type_visibility(ty)
        };
        let gv = self.m.global_mut(g);
        gv.is_constant = true;
        gv.visibility = vis;
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
        self.finish_dso_local();
        let fp = self.frame_pointer().module_flag();
        let direct_access = self.direct_access_external_data();
        let m = &mut self.m;
        // clang 19 emits an empty `!llvm.linker.options` when the language
        // has modules (C++20 and later); clang 20+ emits none.
        if self.opts.clang_compat < 22 && self.opts.cxx_modules {
            m.add_named_metadata("llvm.linker.options", &[]);
        }
        if matches!(self.opts.reloc, RelocModel::Pic(_))
            && self.opts.semantic_interposition == Some(true)
        {
            m.add_module_flag_i32(1, "SemanticInterposition", 1);
        }
        m.add_module_flag_i32(1, "wchar_size", 4);
        match self.opts.reloc {
            RelocModel::Static => {}
            RelocModel::Pic(l) => m.add_module_flag_i32(8, "PIC Level", i32::from(l)),
            RelocModel::Pie(l) => {
                m.add_module_flag_i32(8, "PIC Level", i32::from(l));
                m.add_module_flag_i32(7, "PIE Level", i32::from(l));
            }
        }
        // clang `CodeGenModule::Release`: the x86-64 large data threshold
        // is the driver's 65536 for `medium` and 0 for `large`.
        if let Some(cm) = self.opts.code_model {
            m.add_module_flag_i32(1, "Code Model", cm.flag_value());
            if self.target.triple.arch == Arch::X86_64
                && matches!(cm, CodeModel::Medium | CodeModel::Large)
            {
                let t = if cm == CodeModel::Medium { 65536 } else { 0 };
                m.add_module_flag_i64(1, "Large Data Threshold", t);
            }
        }
        if self.opts.no_plt {
            m.add_module_flag_i32(7, "RtLibUseGOT", 1);
        }
        if direct_access != (self.opts.reloc == RelocModel::Static) {
            m.add_module_flag_i32(7, "direct-access-external-data", i32::from(direct_access));
        }
        match self.opts.unwind_tables {
            Some(UnwindTables::Async) => m.add_module_flag_i32(7, "uwtable", 2),
            Some(UnwindTables::Sync) => m.add_module_flag_i32(7, "uwtable", 1),
            None => {}
        }
        if let Some(v) = fp {
            m.add_module_flag_i32(7, "frame-pointer", v);
        }
        m.add_ident(&self.opts.ident);
        self.tbaa_errno();
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

/// clang `QualType::isConstantStorage(Ctx, false, false)`, the constness
/// `GetOrCreateLLVMGlobal` gives a declaration: a reference (its slot is
/// never rebound), or a `const` object whose base element type is not a
/// class (one might be constructed or destroyed at run time).
fn constant_storage(ast: &Ast, ty: QualType) -> bool {
    if ast.types.is_reference(ty) {
        return true;
    }
    let mut t = ast.types.canonical(ty);
    while let Some(e) = ast.types.element(t) {
        t = ast.types.canonical(e);
    }
    ast.types.effective_quals(ty).is_const() && ast.types.as_record(t).is_none()
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
    /// The function (its `target("...")` attribute).
    pub(crate) decl: Option<DeclId>,
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

/// Removes `optsize` and `minsize` (an `optnone` function).
fn strip_size_attrs(a: &mut Vec<Attr>) {
    a.retain(|x| !matches!(x, Attr::Flag(AttrKind::OptimizeForSize | AttrKind::MinSize)));
}
