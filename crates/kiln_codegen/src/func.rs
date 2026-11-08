//! Function bodies (spec/codegen.md §5): clang's `StartFunction`,
//! `FinishFunction`, the return block and the instruction helpers
//! (with `IRBuilder`'s constant folding).

use kiln_abi::call::ArgKind;
use kiln_abi::irtype::IrType;
use kiln_abi::mangle;
use kiln_ast::decl::DeclKind;
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, QualType};
use kiln_llvm::{
    Attr, AttrKind, AttrList, BinOp, CallOpts, CastOp, ConstData, Flags, FloatPredicate, FuncId,
    GepFlags, IntPredicate, IntValue, MemOpts, TypeId as LTy, Value,
};
use kiln_oven::apfloat::{self, FloatKind};
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;

use crate::CodegenError;
use crate::cleanup::{Cleanup, Cleanups, JumpDest};
use crate::debuginfo::func::VarSlot;
use crate::fir::{Blk, Fir, Op, V};
use crate::module::{Cg, R, Sig, err};
use crate::tbaa::Tb;

/// An address with its element type and alignment (clang `Address`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Addr {
    pub v: V,
    pub ty: LTy,
    pub align: u64,
}

/// A `break`/`continue` target pair (clang `BreakContinue`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Jump {
    pub brk: JumpDest,
    pub cont: Option<JumpDest>,
}

/// The active `switch`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SwitchCtx {
    pub inst: u32,
    pub cond: V,
    pub cond_ty: LTy,
    pub default: Blk,
    /// The head of the `sw.caserange` test chain (clang
    /// `CaseRangeBlock`), the switch's final default destination.
    pub range_head: Blk,
}

pub(crate) use crate::loop_md::LoopFrame;

/// A parameter's value after clang `EmitFunctionProlog`.
enum ParamVal {
    /// A scalar value, spilled to a `.addr` alloca by `EmitParmDecl`.
    Direct(V),
    /// The parameter's storage; `spill` for a non-`byval` indirect
    /// argument, whose pointer `EmitParmDecl` stores to an
    /// `.indirect_addr` alloca.
    Indirect(Addr, bool),
}

/// An evaluated call argument (clang `CallArg`).
#[derive(Clone, Copy, Debug)]
pub(crate) enum CallArg {
    /// A scalar value.
    Scalar(V),
    /// An aggregate in memory (`agg.tmp`).
    Agg(Addr),
    /// A `_Complex` value `(real, imag)`, stored to a temporary when the
    /// call is lowered (clang `CallArg::copyInto`).
    Complex(V, V),
}

/// How the function returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RetKind {
    Void,
    /// Direct or extended with the coerce type equal to the value type.
    Scalar,
    /// Coerced from the `retval` memory to the given type.
    Coerced,
    Sret,
}

/// Per-function state.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent per-function flags (clang's CodeGenFunction fields)"
)]
pub(crate) struct Fx {
    pub fir: Fir,
    pub decl: Option<DeclId>,
    pub func: Option<FuncId>,
    pub locals: FxHashMap<DeclId, Addr>,
    /// WP-803: each `pass_object_size(N)` parameter's `N` and the slot of
    /// its hidden `size_t` argument (clang `SizeArguments`).
    pub object_sizes: FxHashMap<DeclId, (u32, Addr)>,
    pub ret_block: Option<Blk>,
    pub ret_slot: Option<Addr>,
    pub ret_kind: RetKind,
    pub ret_coerce: Option<IrType>,
    pub sig: Option<Sig>,
    pub is_main: bool,
    /// A `naked` function (clang `EmitFunctionProlog` /
    /// `EmitFunctionEpilog`): no parameter prologue, and the epilogue is
    /// `unreachable` instead of a return.
    pub naked: bool,
    /// A thunk (clang `generateThunk`): its call of the target has no
    /// target declaration (`CGCallee::forDirect`), so the callee's
    /// `no_builtin` adds no call-site attribute (WP-808).
    pub thunk: bool,
    pub jumps: Vec<Jump>,
    pub labels: FxHashMap<DeclId, JumpDest>,
    /// The labels of the enclosing lexical scopes with cleanups (clang
    /// `LexicalScope::Labels`), innermost last; `None` for a lexical scope
    /// entered without cleanups.
    pub lex_labels: Vec<Vec<DeclId>>,
    /// The normal-cleanup stack.
    pub cl: Cleanups,
    /// `ReturnBlock` as a jump destination.
    pub ret_dest: Option<JumpDest>,
    /// `CXXThisValue`: `this` loaded once in the prolog.
    pub this: Option<V>,
    /// The class of `this` in a member function.
    pub this_class: Option<DeclId>,
    /// While an aggregate's initializer list is emitted: the object and
    /// its class, which `this` is in a default member initializer used
    /// there ([class.mem.general]/9 as [expr.prim.this]/2; clang
    /// `FieldConstructionScope` / `CXXDefaultInitExprScope`).
    pub default_init_this: Option<(V, DeclId)>,
    /// In a lambda's function call operator: the closure object's
    /// address (clang `CXXABIThisValue`, which `this` no longer is once
    /// `this` is captured) and the lambda.
    pub closure: Option<(V, kiln_ast::LambdaId)>,
    /// In a lambda's call operator with an explicit object parameter:
    /// that parameter. The closure object is re-derived from it at each
    /// capture access (clang `EmitLValueForLambdaField`), so the `V` of
    /// [`Fx::closure`] is unused.
    pub closure_obj: Option<DeclId>,
    /// The constructor/destructor variant being emitted.
    pub structor: Option<mangle::Structor>,
    /// The VTT parameter, loaded in the prolog (clang
    /// `CXXStructorImplicitParamValue`).
    pub vtt: Option<V>,
    /// The VTT argument of the structor call being emitted
    /// (consumed by `call_vals` for a signature with `vtt`).
    pub call_vtt: Option<V>,
    /// The NRVO flags of NRVO variables with non-trivial destructors.
    pub nrvo_flags: FxHashMap<DeclId, V>,
    /// The body refers to a [`V::P`] placeholder.
    pub uses_ph: bool,
    /// The first error raised where an `R` cannot be returned (cleanup
    /// emission); `gen_code` reports it after the body.
    pub error: Option<CodegenError>,
    /// The aggregate slot being initialized may overlap other data
    /// (clang `AggValueSlot::Overlap_t`), consumed by the next emission.
    pub overlap: bool,
    /// The aggregate slot an `_Atomic` assignment evaluates its right
    /// side into is volatile (clang `AggValueSlot::isVolatile`), consumed
    /// by the atomic load filling it (WP-165).
    pub slot_volatile: bool,
    /// The aggregate slot being initialized is destructed by its owner
    /// (clang `AggValueSlot::IsDestructed`): a bound temporary written
    /// there needs no cleanup.
    pub dest_destructed: bool,
    /// clang `CallArgList::CleanupsToDeactivate`: the EH cleanup of each
    /// callee-destroyed argument evaluated for a call not yet emitted,
    /// as `(argument address, cleanup depth, placeholder)` (WP-460;
    /// spec/codegen.md §7x).
    pub arg_cleanups: Vec<(V, crate::cleanup::Depth, V)>,
    /// clang `CalleeDestructedParamCleanups`: each by-value parameter
    /// this function destroys, as `(address, cleanup depth)`.
    pub param_cleanups: Vec<(V, crate::cleanup::Depth)>,
    pub sw: Option<SwitchCtx>,
    pub loops: Vec<LoopFrame>,
    pub mustprogress: bool,
    /// clang `LargestVectorWidth`: the widest vector in the signature or
    /// any call's IR arguments and result (x86 `min-legal-vector-width`).
    pub vector_width: u64,
    /// The call expression being emitted, for diagnostics of the call
    /// lowering itself (WP-810).
    pub call_loc: Option<Loc>,
    /// The `(real, imag)` value of the last call returning `_Complex`
    /// (`call_vals_to` returns no scalar for it; spec/codegen.md §3.12).
    pub cx_ret: Option<(V, V)>,
    /// Calls: (instruction, callee, call-site nounwind).
    pub calls: Vec<(u32, Option<FuncId>, bool)>,
    /// The loop statement about to be emitted and the `AttributedStmt`
    /// whose loop hints apply to it (clang `EmitStmt`'s `Attrs`).
    pub loop_hints: Option<(kiln_ast::StmtId, kiln_ast::StmtId)>,
    /// The call-site attributes of the enclosing `[[clang::nomerge]]`,
    /// `[[clang::noinline]]` and `[[clang::always_inline]]` statements
    /// (WP-813).
    pub stmt_calls: crate::class_b::StmtCalls,
    /// The function is `flatten` (WP-813).
    pub flatten: bool,
    /// The callee of the direct call being emitted, for its call-site
    /// class B attributes (WP-813).
    pub call_decl: Option<DeclId>,
    /// The returned call of the enclosing `[[clang::musttail]]` return
    /// statement (clang `MustTailCall`; WP-809).
    pub musttail: Option<kiln_ast::ExprId>,
    /// Set right before the musttail call's `call_vals_to` (its
    /// arguments evaluated): that call is emitted `musttail` + `ret`.
    pub musttail_armed: bool,
    /// The active conditional evaluations (clang `ConditionalEvaluation`),
    /// innermost last: each is the block the condition started in.
    pub conds: Vec<Blk>,
    /// The bound common expressions of the enclosing GNU binary
    /// conditionals (clang `OpaqueValueMapping`).
    pub opaque: FxHashMap<kiln_ast::ExprId, crate::expr::Opaque>,
    /// The indices of the enclosing `ArrayInitLoopExpr`s, innermost last
    /// (clang `ArrayInitIndex`).
    pub array_index: Vec<V>,
    /// The evaluated arguments (and their types) of the C-variadic
    /// inheriting constructor being inlined at its call site (clang
    /// `CXXInheritedCtorInitExprArgs`), forwarded by its
    /// `CXXInheritedCtorInitExpr`.
    pub inh_args: Option<(Vec<CallArg>, Vec<QualType>)>,
    /// The evaluated element counts of the variable length array bounds
    /// emitted so far, by size expression (clang `VLASizeMap`, WP-183).
    pub vla_sizes: FxHashMap<kiln_ast::ExprId, V>,
    /// A stack save was emitted in the current cleanup scope (clang
    /// `DidCallStackSave`).
    pub did_stack_save: bool,
    /// The coroutine being emitted (clang `CurCoro`, spec/codegen.md
    /// §7o).
    pub coro: Option<Box<crate::coro::CoroData>>,
    /// In an `await_suspend` wrapper: the frame pointer parameter, the
    /// value of `__builtin_coro_frame()` (clang
    /// `CurAwaitSuspendWrapper.FramePtr`).
    pub await_frame: Option<V>,
    /// Emit lifetime markers in this function even without optimization
    /// (set in a coroutine, spec/codegen.md §7p); at -O1+ every function
    /// has them ([`Cg::lifetime_markers_on`]).
    pub lifetime_markers: bool,
    /// The variables a jump bypasses, which get no lifetime markers
    /// (clang `Bypasses`).
    pub bypasses: Option<Box<crate::lifetime::Bypasses>>,
    /// The statement helpers outlined from this function so far
    /// (`crate::outline`, spec/landed/codegen/WP-641.md): the next one's
    /// number.
    pub outlined: u32,
    /// The FP semantics of the scope being emitted (WP-806, `crate::fp`).
    pub fp: kiln_ast::fp::FpOptions,
    /// The pragma override [`Fx::fp`] is the command line's with.
    pub fp_ovr: kiln_ast::fp::FpOverride,
    /// The `*-fp-math` attributes an FP scope turned off (`crate::fp`).
    pub fp_attrs_off: u8,
    /// clang's per-function `SanOpts` (WP-675, `crate::ubsan`).
    pub san: kiln_ast::sanitize::SanitizerSet,
    /// The trap block of each `UBSan` handler at `-O1` and up (clang
    /// `TrapBBs`, `crate::ubsan`).
    pub ub_traps: Vec<(u8, Blk)>,
}

impl Fx {
    pub fn empty() -> Fx {
        Fx {
            fir: Fir::new(Vec::new()),
            decl: None,
            func: None,
            locals: FxHashMap::default(),
            object_sizes: FxHashMap::default(),
            ret_block: None,
            ret_slot: None,
            ret_kind: RetKind::Void,
            ret_coerce: None,
            sig: None,
            is_main: false,
            naked: false,
            thunk: false,
            jumps: Vec::new(),
            labels: FxHashMap::default(),
            lex_labels: Vec::new(),
            cl: Cleanups::new(),
            ret_dest: None,
            this: None,
            this_class: None,
            default_init_this: None,
            closure: None,
            closure_obj: None,
            structor: None,
            vtt: None,
            call_vtt: None,
            nrvo_flags: FxHashMap::default(),
            uses_ph: false,
            error: None,
            overlap: false,
            slot_volatile: false,
            dest_destructed: false,
            arg_cleanups: Vec::new(),
            param_cleanups: Vec::new(),
            sw: None,
            loops: Vec::new(),
            mustprogress: true,
            vector_width: 0,
            call_loc: None,
            cx_ret: None,
            calls: Vec::new(),
            loop_hints: None,
            stmt_calls: crate::class_b::StmtCalls::default(),
            flatten: false,
            call_decl: None,
            musttail: None,
            musttail_armed: false,
            conds: Vec::new(),
            opaque: FxHashMap::default(),
            array_index: Vec::new(),
            inh_args: None,
            vla_sizes: FxHashMap::default(),
            did_stack_save: false,
            coro: None,
            await_frame: None,
            lifetime_markers: false,
            bypasses: None,
            outlined: 0,
            fp: kiln_ast::fp::FpOptions::default(),
            fp_ovr: kiln_ast::fp::FpOverride::NONE,
            fp_attrs_off: 0,
            san: kiln_ast::sanitize::SanitizerSet::NONE,
            ub_traps: Vec::new(),
        }
    }
}

pub(crate) fn mem(align: u64, volatile: bool) -> MemOpts {
    let mut m = MemOpts::aligned(align);
    m.volatile = volatile;
    m
}

impl Cg<'_> {
    // ---- function definitions ------------------------------------------

    /// clang `GenerateCode` for the function definition `d`.
    pub fn emit_fn_def(&mut self, d: DeclId) -> R<()> {
        let f = self.get_fn(d)?;
        let f = self.fn_for_definition(d, f)?;
        self.gen_code(d, f, None)?;
        self.add_ctor_dtor_entries(d, f);
        self.emit_thunks(d, None)
    }

    /// `llvm.trap` (`noreturn nounwind`) followed by `unreachable`.
    pub(crate) fn emit_trap_unreachable(&mut self) {
        self.emit_trap_unreachable_ex(true);
    }

    /// `llvm.trap` then `unreachable`; `site_attrs`: the call is marked
    /// `noreturn nounwind` (clang's `EmitTrapCall` alone marks nothing,
    /// as at the fall-off end of a value-returning function).
    pub(crate) fn emit_trap_unreachable_ex(&mut self, site_attrs: bool) {
        let trap = kiln_llvm::intrinsics::trap(&mut self.m);
        let fa = if site_attrs {
            self.m.attr_set(&[Attr::Flag(AttrKind::NoReturn), Attr::Flag(AttrKind::NoUnwind)])
        } else {
            self.m.attr_set(&[])
        };
        let fn_ty = self.m.func(trap).ty();
        let opts =
            CallOpts { attrs: AttrList { func: fa, ..AttrList::default() }, ..CallOpts::default() };
        let c = self.f.fir.push(
            Op::Call {
                fn_ty,
                callee: V::C(Value::func(trap)),
                args: Vec::new(),
                opts: Box::new(opts),
            },
            LTy::VOID,
        );
        self.f.calls.push((Fir::last_inst(c), Some(trap), true));
        self.f.fir.push(Op::Unreachable, LTy::VOID);
        self.f.fir.set_cur(None);
    }

    /// clang `GenerateCode` for the definition `d` into `f` (`structor`:
    /// the constructor/destructor variant being emitted).
    pub fn gen_code(&mut self, d: DeclId, f: FuncId, structor: Option<mangle::Structor>) -> R<()> {
        self.gen_code_or_thunk(d, f, structor, None)
    }

    /// `gen_code`, or with `thunk` clang `generateThunk` (`StartThunk` +
    /// `EmitCallAndReturnForThunk` + `FinishThunk`) for the method
    /// definition `d` into the thunk function `f`.
    pub(crate) fn gen_code_or_thunk(
        &mut self,
        d: DeclId,
        f: FuncId,
        structor: Option<mangle::Structor>,
        thunk: Option<kiln_ast::class::Thunk>,
    ) -> R<()> {
        let ast = self.ast;
        let first = ast.first(d);
        let sig = self.structor_sig(first, structor)?;
        let fd = *ast.decls.function(d).expect("function");
        // A thunk needs no body of its method: a thunk of a virtual
        // function defined elsewhere is emitted with a vtable above -O0
        // (clang `maybeEmitThunk` with `ForVTable`).
        let body = fd.body;
        if body.is_none() && thunk.is_none() {
            return Ok(());
        }
        let is_main = mangle::decl_name(ast, first) == "main"
            && ast.decls.get(first).ctx().is_some_and(|c| self.is_tu_level(c));

        let params: Vec<LTy> = {
            let func = self.m.func(f);
            func.param_types().to_vec()
        };
        self.f = Fx::empty();
        self.f.fir = Fir::new(params);
        self.start_fp();
        self.f.decl = Some(d);
        self.f.func = Some(f);
        self.f.is_main = is_main;
        self.f.thunk = thunk.is_some();
        self.f.san = self.fn_sanitizers(if thunk.is_some() {
            crate::class_b::StartDecl::None
        } else {
            crate::class_b::StartDecl::Fn(d)
        });
        self.f.naked = thunk.is_none() && self.has_fn_attr(d, kiln_ast::attr::AttrKind::NAKED);
        self.f.flatten = thunk.is_none() && self.has_fn_attr(d, kiln_ast::attr::AttrKind::FLATTEN);
        self.f.sig = Some(sig.clone());
        // clang `StartFunction`: `min_vector_width(N)` raises the start.
        let min = ast.redecls_of(d).find_map(|r| {
            ast.lists.iter::<kiln_ast::AttrId>(ast.attrs.of_decl(r)).find_map(|a| {
                let n = ast.attrs.get(a);
                (n.kind == kiln_ast::attr::AttrKind::MIN_VECTOR_WIDTH).then_some(n.a)
            })
        });
        self.f.vector_width = self.def_vector_width(&sig.info).max(u64::from(min.unwrap_or(0)));
        self.f.structor = structor;
        self.f.this_class = sig.this;
        self.set_def_linkage(f, d);
        if thunk.is_some() && self.m.func(f).comdat.is_some() {
            // clang `setThunkLinkage`: a thunk has its own comdat, not
            // its structor's `D5` one.
            let name = self.m.func(f).name().to_owned();
            let c = self.m.comdat(&name, kiln_llvm::ComdatKind::Any);
            self.m.func_mut(f).comdat = Some(c);
        }
        if thunk.is_some() && body.is_none() {
            // clang `setThunkLinkage` with `ForVTable`; a declaration for
            // the linker keeps only an explicit visibility.
            let vis = self.decl_visibility(d, false);
            let func = self.m.func_mut(f);
            func.visibility = vis;
            func.linkage = kiln_llvm::Linkage::AvailableExternally;
            func.comdat = None;
            func.dso_local = false;
        }
        let kind = ast.decls.get(d).kind();
        if self.is_x86()
            && matches!(
                kind,
                DeclKind::CXXMethod
                    | DeclKind::CXXConstructor
                    | DeclKind::CXXDestructor
                    | DeclKind::CXXConversion
            )
        {
            self.m.func_mut(f).align = Some(2);
        }
        // clang `SetLLVMFunctionAttributesForDefinition`: an `aligned(N)`
        // / `alignas` function is `align N` (`Decl::getMaxAlignment`).
        if thunk.is_none() {
            let bits = kiln_abi::layout::decl_max_align(ast, d, self.target);
            if bits >= 8 {
                let func = self.m.func_mut(f);
                func.align = Some(func.align.unwrap_or(1).max(bits / 8));
            }
        }
        if thunk.is_none() {
            self.note_used_attrs(d, Value::func(f));
        }
        if structor.is_some() || thunk.is_some() {
            self.m.func_mut(f).unnamed_addr = kiln_llvm::UnnamedAddr::Global;
        }

        // StartFunction.
        self.di_fn_start(d, f, thunk.is_some(), body);
        let ret_block = self.f.fir.create_block();
        self.f.ret_block = Some(ret_block);
        self.f.ret_dest = Some(JumpDest { blk: ret_block, depth: Some(0), index: 1 });
        self.f.cl.next_index = 2;
        self.check_pure_scalable(&sig, Some(ast.decls.get(d).loc()))?;
        let mut arg_no = 0u32;
        match &sig.info.ret.kind {
            ArgKind::Ignore => {
                self.f.ret_kind = RetKind::Void;
                // clang `CreateIRTemp(RetTy, "retval")` for an ignored
                // (empty-class) return; FinishFunction erases it unused.
                if !ast.types.is_void(sig.ret) {
                    let vt = self.lty(sig.ret);
                    let (_, align) = self.size_align(sig.ret);
                    let slot = self.f.fir.alloca(vt, align);
                    self.f.ret_slot = Some(Addr { v: slot, ty: vt, align });
                }
            }
            ArgKind::Indirect { byval, align } => {
                self.f.ret_kind = RetKind::Sret;
                let ty = self.mty(sig.ret);
                self.f.ret_slot = Some(Addr { v: V::Arg(0), ty, align: *align });
                arg_no = 1;
                if !*byval {
                    let p = self.f.fir.alloca(LTy::PTR, 8);
                    self.store(V::Arg(0), p, 8, false);
                    if let Some(x) = self.f.fir.dbg.as_deref_mut() {
                        x.ret_ptr = Some(p);
                    }
                }
            }
            ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. } => {
                let vt = self.lty(sig.ret);
                let (_, align) = self.size_align(sig.ret);
                let slot = self.f.fir.alloca(vt, align);
                self.f.ret_slot = Some(Addr { v: slot, ty: vt, align });
                let ct = self.ir(coerce);
                if ct == vt {
                    self.f.ret_kind = RetKind::Scalar;
                } else {
                    self.f.ret_kind = RetKind::Coerced;
                    self.f.ret_coerce = Some(coerce.clone());
                }
            }
        }
        if is_main && let Some(slot) = self.f.ret_slot {
            let z = self.m.const_int(slot.ty, 0);
            self.store(V::C(z), slot.v, slot.align, false);
        }
        // EmitStartEHSpec: a non-throwing function is a terminate scope
        // (thunks are emitted without a declaration and get none).
        let eh_spec = sig.nothrow && self.opts.exceptions && thunk.is_none();
        if eh_spec {
            self.push_terminate();
        }
        self.f.cl.prologue = self.f.cl.depth();
        // EmitFunctionProlog: every argument first, then EmitParmDecl for
        // each parameter in order (the implicit object parameter first).
        let this_arg = sig.this.map(|_| {
            let v = V::Arg(arg_no);
            arg_no += 1;
            v
        });
        let vtt_arg = sig.vtt.then(|| {
            let v = V::Arg(arg_no);
            arg_no += 1;
            v
        });
        // clang `EmitFunctionProlog`: naked functions have no prologue
        // (sema rejects parameter and `this` references in their bodies).
        let param_decls: Vec<DeclId> =
            if self.f.naked || self.inheriting_drops_params(first, structor) {
                Vec::new()
            } else {
                ast.lists.iter(fd.params).collect()
            };
        // WP-803: a `pass_object_size` parameter's hidden `size_t`
        // argument follows it (clang `GenerateCode` adds an implicit
        // parameter after each).
        let object_sizes =
            if param_decls.is_empty() { Vec::new() } else { ast.pass_object_size_params(d) };
        let size_q = QualType::unqual(ast.types.target().size_t);
        let mut vals = Vec::with_capacity(param_decls.len());
        let mut hidden = Vec::with_capacity(object_sizes.len());
        let mut si = 0;
        for (i, &p) in param_decls.iter().enumerate() {
            let pq = ast.decls.get(p).ty().expect("param type");
            let info = sig.info.args[si].clone();
            si += 1;
            let loc = Some(ast.decls.get(p).loc());
            vals.push(self.prolog_arg(&info.kind, pq, &mut arg_no, loc)?);
            if let Some(&(_, t, _)) = object_sizes.iter().find(|o| o.0 == i) {
                let info = sig.info.args[si].clone();
                si += 1;
                if let ParamVal::Direct(v) =
                    self.prolog_arg(&info.kind, size_q, &mut arg_no, loc)?
                {
                    hidden.push((p, t, v));
                }
            }
        }
        let this_class = ast.decls.get(d).ctx();
        // clang `EmitParmDecl`'s argument numbers (none in a thunk).
        let declare = thunk.is_none();
        let mut dn = 0u32;
        let this_addr = this_arg.filter(|_| !self.f.naked).map(|v| {
            let a = self.f.fir.alloca(LTy::PTR, 8);
            let st = self.store(v, a, 8, false);
            if let Some(rd) = this_class {
                self.tbaa_this_store(st, rd);
            }
            dn += 1;
            if declare {
                self.di_declare_this(a);
            }
            a
        });
        let vtt_addr = vtt_arg.filter(|_| !self.f.naked).map(|v| {
            let a = self.f.fir.alloca(LTy::PTR, 8);
            let st = self.store(v, a, 8, false);
            self.tbaa_vtt_store(st);
            dn += 1;
            if declare {
                self.di_declare_vtt(a, dn);
            }
            a
        });
        for (&p, val) in param_decls.iter().zip(vals) {
            let pq = ast.decls.get(p).ty().expect("param type");
            dn += 1;
            let (addr, slot) = match val {
                ParamVal::Indirect(a, spill) => {
                    let mut slot = VarSlot { addr: a.v, arg: Some(dn), deref: false };
                    if spill {
                        let p = self.f.fir.alloca(LTy::PTR, 8);
                        self.store(a.v, p, 8, false);
                        slot = VarSlot { addr: p, arg: Some(dn), deref: true };
                    }
                    (a, slot)
                }
                ParamVal::Direct(v) => {
                    let mt = self.mty(pq);
                    let (_, align) = self.size_align(pq);
                    let a = self.f.fir.alloca(mt, align);
                    let addr = Addr { v: a, ty: mt, align };
                    self.store_scalar(v, addr, pq);
                    (addr, VarSlot { addr: a, arg: Some(dn), deref: false })
                }
            };
            if declare {
                self.di_declare_var(p, slot);
            }
            self.f.locals.insert(p, addr);
            if let Some(&(_, t, v)) = hidden.iter().find(|h| h.0 == p) {
                let mt = self.mty(size_q);
                let (_, align) = self.size_align(size_q);
                let a = self.f.fir.alloca(mt, align);
                let addr = Addr { v: a, ty: mt, align };
                self.store_scalar(v, addr, size_q);
                dn += 1;
                if declare {
                    self.di_declare_size(a, dn);
                }
                self.f.object_sizes.insert(p, (t, addr));
            }
            // clang `EmitParmDecl`: a parameter destroyed in the callee
            // (`isParamDestroyedInCallee`, not in a thunk) gets its
            // destructor cleanup here, run at the end of the function.
            if thunk.is_none()
                && kiln_abi::call::is_destroyed_in_callee(ast, pq)
                && let Some(rd) = self.destructed_class(pq)
            {
                let c = Cleanup::Dtor { addr: addr.v, record: rd, base: false };
                self.push_cleanup_kind(c, true, true);
                let depth = self.f.cl.depth();
                self.f.param_cleanups.push((addr.v, depth));
            }
        }
        // EmitInstanceFunctionProlog: `this` is loaded once.
        if let Some(a) = this_addr {
            self.f.this = Some(self.load(LTy::PTR, a, 8, false));
            if let Some(l) = ast.call_op_lambda(d) {
                let closure = self.this_value();
                self.lambda_prolog(l, closure)?;
            }
            if thunk.is_none() {
                let this = self.this_value();
                self.ub_this_check(d, structor, this)?;
            }
        } else if let Some(l) = ast.call_op_lambda(d)
            && let Some(&obj) = param_decls.first()
            && self.is_explicit_object_method(d)
        {
            self.f.closure_obj = Some(obj);
            self.lambda_prolog(l, self.f.locals[&obj].v)?;
        }
        if let Some(a) = vtt_addr {
            self.f.vtt = Some(self.load(LTy::PTR, a, 8, false));
        }
        let start = body.map_or(ast.decls.get(d).loc(), |b| self.di_stmt_begin(b));
        self.di_prologue_end(start, thunk.is_some());

        // GenerateCode: `Bypasses.Init` over the body when lifetime
        // markers are on (a coroutine redoes it over its own body).
        if let Some(body) = body
            && self.lifetime_markers_on()
            && thunk.is_none()
        {
            self.f.bypasses = Some(Box::new(crate::lifetime::detect_bypasses(ast, body)));
        }
        // The preconditions ([dcl.contract.func], spec/contracts.md §6.3):
        // after the parameters, before the body and a constructor's
        // mem-initializers; not in a thunk, a lambda's static invoker or
        // a destructor variant that delegates to the one that checks.
        let delegating_dtor = kind == DeclKind::CXXDestructor
            && (structor == Some(mangle::Structor::Deleting)
                || structor == Some(mangle::Structor::Complete)
                    && self.structor_has_vbases(d)
                    && body.is_some_and(|b| {
                        ast.stmts.get(b).kind() != kiln_ast::stmt::StmtKind::CXXTryStmt
                    }));
        let contracts_here = body.is_some()
            && thunk.is_none()
            && !delegating_dtor
            && !matches!(
                ast.closure_thunk(d),
                Some(kiln_ast::lambda::ClosureThunk::Invoker { .. })
            );
        if contracts_here {
            self.emit_preconditions(d)?;
        }
        match (kind, body) {
            _ if let Some(th) = thunk => {
                self.f.mustprogress = false;
                self.emit_thunk_body(d, structor, th, &param_decls)?;
            }
            _ if let Some(kiln_ast::lambda::ClosureThunk::Invoker { call_op }) =
                ast.closure_thunk(d) =>
            {
                self.emit_lambda_invoke_body(call_op, &param_decls)?;
            }
            (_, None) => {}
            (DeclKind::CXXConstructor, Some(body)) => self.emit_ctor_body(d, body)?,
            (DeclKind::CXXDestructor, _) if structor == Some(mangle::Structor::Deleting) => {
                self.emit_deleting_dtor_body(d)?;
            }
            (DeclKind::CXXDestructor, Some(body))
                if structor == Some(mangle::Structor::Complete)
                    && self.structor_has_vbases(d)
                    && self.ast.stmts.get(body).kind() != kiln_ast::stmt::StmtKind::CXXTryStmt =>
            {
                self.emit_complete_dtor_body()?;
            }
            (DeclKind::CXXDestructor, Some(body)) => self.emit_dtor_body(d, body)?,
            (_, Some(body)) => self.emit_body(body)?,
        }

        // GenerateCode: falling off the end of a value-returning function
        // is `llvm.trap` + `unreachable` at -O0 and `unreachable` alone
        // above it.
        if !is_main
            && thunk.is_none()
            && ast.closure_thunk(d).is_none()
            && self.f.ret_kind != RetKind::Void
            && self.f.fir.cur().is_some()
        {
            if self.ub_on(kiln_ast::sanitize::SanitizerSet::RETURN) {
                // WP-675: the `return` check at the function's name.
                self.ub_missing_return(Some(ast.decls.get(d).loc()))?;
                self.f.fir.push(Op::Unreachable, LTy::VOID);
                self.f.fir.set_cur(None);
            } else if self.opts.optimize() {
                self.f.fir.push(Op::Unreachable, LTy::VOID);
                self.f.fir.set_cur(None);
            } else {
                self.emit_trap_unreachable_ex(false);
            }
        }
        // FinishFunction.
        let end = self.di_body_end(body);
        self.di_finish_function(end);
        let prologue = self.f.cl.prologue;
        self.pop_cleanup_blocks(prologue);
        self.emit_return_block();
        if contracts_here {
            self.emit_postconditions(d)?;
        }
        self.emit_epilog();
        if eh_spec {
            self.pop_terminate();
        }
        self.finish_indirect_goto()?;
        self.finish_eh();
        self.emit_unreachable_if_used();
        // FinishFunction: a coroutine's cleanup destination slot becomes
        // SSA values and phis (LLVM `PromoteMemToReg`, spec/codegen.md §7p).
        if self.f.coro.is_some()
            && let Some(slot) = self.f.cl.dest_slot.take()
        {
            self.f.fir.promote_alloca(&mut self.m, slot);
        }
        if let Some(e) = self.f.error.take() {
            return Err(e);
        }
        // FinishFunction: an unreachable return block is deleted, and with
        // it a return-value slot nothing else uses.
        if let Some(rb) = self.f.ret_block
            && self.f.fir.is_placed(rb)
            && self.f.fir.block_uses(rb) == 0
        {
            self.f.fir.erase_block(rb);
            self.f.ret_block = None;
        }
        // ... and so is a return-value alloca nothing uses (an ignored
        // empty-class return).
        if let Some(slot) = self.f.ret_slot
            && let V::I(i) = slot.v
            && matches!(self.f.fir.op(i), Op::Alloca { .. })
            && self.f.fir.uses(slot.v) == 0
        {
            self.f.fir.erase(i);
        }
        self.di_fn_finish();

        // Attributes.
        let mut extra = Vec::new();
        let const_pure = crate::builtins::const_pure_attrs(self.ast, d, &sig, &mut extra);
        self.coro_fn_attrs(d, &mut extra);
        if sig.no_csr {
            extra.push(Attr::Str("no_caller_saved_registers".into(), "".into()));
        }
        // A thunk is `nounwind` only by its method's type or attributes
        // (clang `ConstructAttributeList(IsThunk)`): `generateThunk` never
        // runs `TryMarkNoThrow` over its body.
        let nothrow_def = sig.nothrow
            || const_pure
            || self.ast.has_nothrow_attr(d)
            || !self.opts.exceptions
            || (thunk.is_none() && self.no_throw_calls());
        let noreturn = self.ast.is_noreturn_fn(d);
        let inline = if thunk.is_some() && !self.opts.optimize() {
            crate::module::DefInline::OptNone
        } else {
            // clang `maybeEmitThunk` gives a thunk its method's definition
            // attributes (`optnone`, `inlinehint`, ...) above -O0.
            let inline = self.def_inline_attrs(d);
            self.def_opt_attrs(d, inline, &mut extra);
            inline
        };
        let fa = self.def_fn_attrs(crate::module::DefFnAttrs {
            decl: Some(d),
            is_main,
            nounwind: nothrow_def,
            mustprogress: self.f.mustprogress,
            vector_width: self.f.vector_width,
            noreturn,
            inline,
            extra,
            start: if thunk.is_some() {
                crate::class_b::StartDecl::None
            } else {
                crate::class_b::StartDecl::Fn(d)
            },
        });
        // clang `EmitFunctionProlog` adds `noalias` to the `sret` argument,
        // and returns before that in a naked function (no `ReturnValue`
        // slot either way: the `sret` pointer is the return address).
        // A function an alias declared first by name keeps the alias
        // declaration's parameter and return attributes (clang sets them
        // once, in `GetOrCreateLLVMFunction`; spec/codegen.md "Aliases and ifuncs").
        let alias_sig = match self.alias.declared_for.get(&f) {
            Some(&a) if thunk.is_none() && structor.is_none() => {
                self.decl_sig(a).ok().filter(|s| s.fn_ty == sig.fn_ty)
            }
            _ => None,
        };
        let attr_sig = alias_sig.as_ref().unwrap_or(&sig);
        let (mut ret, mut params) = self.sig_attrs(attr_sig, attr_sig.params.len(), !self.f.naked);
        if thunk.is_some() {
            (ret, params) = self.thunk_attrs(&sig, ret, params);
        }
        let (fa, ret) = self.alloc_fn_attrs(d, fa, ret);
        let (fa, ret) = self.class_b_fn_sets(f, d, attr_sig, fa, ret, &mut params);
        if thunk.is_none() {
            self.class_b_prolog_params(d, attr_sig, &mut params);
        }
        {
            let func = self.m.func_mut(f);
            func.attrs.func = fa;
            func.attrs.ret = ret;
            func.attrs.params = params;
        }
        self.nounwind.insert(f, nothrow_def);
        if thunk.is_none()
            && structor.is_none()
            && self.is_variadic_virtual(d)
            && let Some(body) = self.f.fir.clone_body()
        {
            self.vararg_bodies.insert(f, (body, self.f.uses_ph));
        }
        if self.f.uses_ph {
            let fir = std::mem::replace(&mut self.f.fir, Fir::new(Vec::new()));
            self.pending.push((f, d, fir));
            return Ok(());
        }
        if let Err(e) = self.f.fir.lower(&mut self.m, f, &self.phs) {
            return err(Some(ast.decls.get(d).loc()), format!("IR lowering failed: {e:?}"));
        }
        Ok(())
    }

    /// Declares the complete-object structors still unresolved (used but
    /// never defined in the translation unit), then lowers the bodies
    /// that use placeholders.
    pub(crate) fn resolve_placeholders(&mut self) -> R<()> {
        let mut keys: Vec<((DeclId, mangle::Structor), u32)> =
            self.ph_of.iter().map(|(&k, &v)| (k, v)).collect();
        keys.sort_by_key(|&(_, v)| v);
        for ((first, v), p) in keys {
            if self.phs[p as usize].is_none() {
                let (val, _) = self.declare_structor(first, v)?;
                self.phs[p as usize] = Some(val);
            }
        }
        for (f, d, fir) in std::mem::take(&mut self.pending) {
            if let Err(e) = fir.lower(&mut self.m, f, &self.phs) {
                return err(
                    Some(self.ast.decls.get(d).loc()),
                    format!("IR lowering failed: {e:?}"),
                );
            }
        }
        Ok(())
    }

    pub(crate) fn is_tu_level(&self, mut c: DeclId) -> bool {
        loop {
            let n = self.ast.decls.get(c);
            match n.kind() {
                DeclKind::TranslationUnit => return true,
                DeclKind::LinkageSpec => c = n.ctx().expect("ctx"),
                _ => return false,
            }
        }
    }

    /// clang `TryMarkNoThrow`: no call or invoke in the body may throw,
    /// and nothing resumes unwinding.
    pub(crate) fn no_throw_calls(&self) -> bool {
        if self.f.cl.eh_resume_used {
            return false;
        }
        self.f.calls.iter().all(|&(i, callee, site)| {
            self.f.fir.inst_block(i).is_none()
                || site
                || callee.is_some_and(|c| self.nounwind.get(&c).copied().unwrap_or(false))
        })
    }

    /// The `"min-legal-vector-width"` a definition starts from (clang
    /// `StartFunction`): the widest vector-typed IR parameter or return
    /// type. Unlike a call's width, a vector nested in a struct return
    /// does not count; a flattened struct argument counts per element.
    fn def_vector_width(&mut self, info: &kiln_abi::call::CallInfo) -> u64 {
        fn vec_bits(cg: &mut Cg<'_>, t: &IrType) -> u64 {
            kiln_abi::call::vector_bits(&mut cg.tys, t)
        }
        let mut w = 0;
        if let ArgKind::Direct { coerce, .. } = &info.ret.kind {
            w = w.max(vec_bits(self, coerce));
        }
        for a in &info.args {
            if let ArgKind::Direct { coerce, .. } = &a.kind {
                match self.flat_fields(&a.kind) {
                    Some((m, _)) => {
                        for e in &m {
                            w = w.max(vec_bits(self, e));
                        }
                    }
                    None => w = w.max(vec_bits(self, coerce)),
                }
            }
        }
        w
    }

    /// Whether `q` is evaluated as an aggregate (`hasAggregateEvaluationKind`).
    pub fn is_aggregate(&self, q: QualType) -> bool {
        let t = &self.ast.types;
        t.as_record(q).is_some() || t.is_array(q)
    }

    /// clang `EmitReturnBlock`.
    pub(crate) fn emit_return_block(&mut self) {
        let rb = self.f.ret_block.expect("return block");
        if let Some(cur) = self.f.fir.cur() {
            if self.f.fir.block_empty(cur) || self.f.fir.block_uses(rb) == 0 {
                self.f.fir.replace_block_uses(rb, cur);
                self.f.ret_block = None;
            } else {
                self.emit_block(rb, false);
            }
            return;
        }
        if self.f.fir.block_uses(rb) == 1 {
            let preds = self.f.fir.predecessors(rb);
            if let Some(&p) = preds.first()
                && let Some(t) = self.f.fir.terminator(p)
                && matches!(self.f.fir.op(t), Op::Br { .. })
            {
                // The `ret` takes the branch's location (`ApplyDebugLocation
                // AL(*this, Loc)` in `FinishFunction`).
                if let Some(l) = self.f.fir.inst_dbg(t) {
                    self.f.fir.set_dbg_loc(Some(l));
                }
                self.f.fir.erase(t);
                self.f.fir.set_cur(Some(p));
                self.f.ret_block = None;
                return;
            }
        }
        self.emit_block(rb, false);
    }

    /// clang `EmitFunctionEpilog` (a naked function has no epilogue: the
    /// return block ends in `unreachable`).
    pub(crate) fn emit_epilog(&mut self) {
        if self.f.naked {
            self.f.fir.push(Op::Unreachable, LTy::VOID);
            return;
        }
        let mut ret_loc = None;
        let rv = match self.f.ret_kind {
            RetKind::Void => None,
            RetKind::Sret => {
                // clang `EmitFunctionEpilog`, `TEK_Scalar` returned
                // indirectly: reload the value and store it back.
                let q = self.f.sig.as_ref().map(|g| g.ret);
                // `TEK_Complex` (a Win64 `_Complex double`) likewise
                // reloads both parts and stores them back.
                if let (Some(q), Some(slot)) = (q, self.f.ret_slot)
                    && self.f.fir.cur().is_some()
                {
                    if matches!(self.ast.types.canonical_view(q), TypeView::Complex(_)) {
                        let (re, im) = self.cx_load(slot, q, (false, false));
                        if let (Some(re), Some(im)) = (re, im) {
                            self.cx_store((re, im), slot, q);
                        }
                    } else if !self.is_aggregate(q) && !self.is_atomic_aggregate(q) {
                        let v = self.load_scalar(slot, q);
                        self.store_scalar(v, slot, q);
                    }
                }
                None
            }
            RetKind::Scalar => {
                let slot = self.f.ret_slot.expect("retval");
                match self.dominating_store(slot) {
                    Some(st) => {
                        let Op::Store { val, .. } = *self.f.fir.op(st) else { unreachable!() };
                        if self.f.fir.dbg.as_deref().is_some_and(|x| x.ret_dbg) {
                            ret_loc = self.f.fir.inst_dbg(st);
                        }
                        self.f.fir.erase(st);
                        if self.f.fir.uses(slot.v) == 0 {
                            self.f.fir.erase(Fir::last_inst(slot.v));
                        }
                        Some(val)
                    }
                    None => Some(self.load(slot.ty, slot.v, slot.align, false)),
                }
            }
            RetKind::Coerced => {
                let slot = self.f.ret_slot.expect("retval");
                let ct = self.f.ret_coerce.clone().expect("coerce type");
                let kind = self.f.sig.as_ref().map(|g| g.info.ret.kind.clone());
                let slot = match kind {
                    Some(k) => self.at_abi_offset(slot, &k),
                    None => slot,
                };
                Some(self.coerced_load(slot, &ct))
            }
        };
        let ret = self.f.fir.push(Op::Ret { v: rv }, LTy::VOID);
        if ret_loc.is_some() {
            self.f.fir.set_inst_dbg(Fir::last_inst(ret), ret_loc);
        }
    }

    /// clang `findDominatingStoreToReturnValue`.
    fn dominating_store(&self, slot: Addr) -> Option<u32> {
        let fir = &self.f.fir;
        let cur = fir.cur()?;
        let valid = |i: u32| match fir.op(i) {
            Op::Store { ptr, val, .. } if *ptr == slot.v && fir.ty(&self.m, *val) == slot.ty => {
                Some(i)
            }
            _ => None,
        };
        let users = fir.users(slot.v);
        if users.len() != 1 {
            let &last = fir.block_insts(cur).last()?;
            return valid(last);
        }
        let st = valid(users[0])?;
        let sb = fir.inst_block(st)?;
        let mut ip = cur;
        let mut seen = Vec::new();
        while ip != sb {
            if seen.contains(&ip) {
                return None;
            }
            seen.push(ip);
            ip = fir.single_predecessor(ip)?;
        }
        Some(st)
    }

    // ---- blocks --------------------------------------------------------

    /// clang `EmitBranch`.
    pub fn emit_branch(&mut self, target: Blk) {
        if let Some(cur) = self.f.fir.cur()
            && !self.f.fir.is_terminated(cur)
        {
            self.br(target);
        }
        self.f.fir.set_cur(None);
    }

    /// An unconditional branch at the insertion point (loop metadata
    /// attached as clang's `LoopInfoStack::InsertHelper`).
    pub fn br(&mut self, target: Blk) -> u32 {
        let v = self.f.fir.push(Op::Br { dest: target }, LTy::VOID);
        let i = Fir::last_inst(v);
        self.loop_md(i, &[target]);
        i
    }

    /// A conditional branch (constant conditions are kept, as `IRBuilder`
    /// does).
    pub fn cond_br(&mut self, c: V, t: Blk, f: Blk) -> u32 {
        let v = self.f.fir.push(Op::CondBr { c, t, f }, LTy::VOID);
        let i = Fir::last_inst(v);
        self.loop_md(i, &[t, f]);
        i
    }

    fn loop_md(&mut self, i: u32, succ: &[Blk]) {
        if let Some(l) = self.f.loops.last()
            && let Some(md) = l.md
            && succ.contains(&l.header)
        {
            let k = self.m.md_kind("llvm.loop");
            self.f.fir.add_md(i, k, md);
        }
    }

    /// clang `EmitBlock`.
    pub fn emit_block(&mut self, b: Blk, finished: bool) {
        let cur = self.f.fir.cur();
        self.emit_branch(b);
        if finished && self.f.fir.block_uses(b) == 0 {
            return;
        }
        match cur.filter(|&c| self.f.fir.is_placed(c)) {
            Some(c) => self.f.fir.place_after(c, b),
            None => self.f.fir.place_at_end(b),
        }
        self.f.fir.set_cur(Some(b));
    }

    /// clang `EnsureInsertPoint`.
    pub fn ensure_insert_point(&mut self) {
        if self.f.fir.cur().is_none() {
            let b = self.f.fir.create_block();
            self.emit_block(b, false);
        }
    }

    /// clang `SimplifyForwardingBlocks`.
    pub fn simplify_forwarding(&mut self, b: Blk) {
        // clang bails out while any cleanup (a destructor, a lifetime end,
        // a terminate scope) is on the EH stack.
        if !self.f.cl.stack.is_empty() {
            return;
        }
        let insts = self.f.fir.block_insts(b);
        if insts.len() != 1 {
            return;
        }
        let i = insts[0];
        let Op::Br { dest } = *self.f.fir.op(i) else { return };
        if dest == b {
            return;
        }
        self.f.fir.erase(i);
        self.f.fir.replace_block_uses(b, dest);
        self.f.fir.erase_block(b);
    }

    // ---- memory --------------------------------------------------------

    /// A local temporary (`CreateMemTemp`).
    pub fn temp(&mut self, q: QualType) -> Addr {
        let ty = self.mty(q);
        let (_, align) = self.size_align(q);
        let v = self.f.fir.alloca(ty, align);
        Addr { v, ty, align }
    }

    pub fn load(&mut self, ty: LTy, ptr: V, align: u64, volatile: bool) -> V {
        self.f.fir.push(Op::Load { ty, ptr, mem: mem(align, volatile) }, ty)
    }

    pub fn store(&mut self, val: V, ptr: V, align: u64, volatile: bool) -> V {
        self.f.fir.push(Op::Store { val, ptr, mem: mem(align, volatile) }, LTy::VOID)
    }

    /// clang `EmitLoadOfScalar` (bools are truncated to `i1`).
    pub fn load_scalar(&mut self, a: Addr, q: QualType) -> V {
        self.load_scalar_tb(a, q, Tb::Type)
    }

    /// [`Self::load_scalar`] with the access path `tb`: the load carries
    /// its TBAA tag, and a `bool` load at `-O1`+ `!range !{i8 0, i8 2}` and
    /// `!noundef` (clang `getRangeForLoadFromType`). A three-element or
    /// `bool` vector is loaded untagged, as clang does.
    pub fn load_scalar_tb(&mut self, a: Addr, q: QualType, tb: Tb) -> V {
        let vol = self.ast.types.effective_quals(q).is_volatile();
        if let Some(v) = self.vec_load(a, q, vol) {
            return v;
        }
        let p = self.scalar_tls_addr(a.v);
        let ty = self.scalar_access_ty(a.ty, q);
        let v = self.load(ty, p, a.align, vol);
        self.tbaa_mark(v, q, tb);
        if self.is_bool_value(q) && self.opts.optimize() && a.ty == LTy::I8 {
            self.bool_range(v);
        }
        self.emit_from_memory(v, q)
    }

    /// clang `EmitFromMemory`: a `bool` is truncated to `i1`, a
    /// `_BitInt(N)` from its slot to `iN`.
    pub fn emit_from_memory(&mut self, v: V, q: QualType) -> V {
        if self.is_bool_value(q) {
            return self.cast(CastOp::Trunc, v, LTy::I1);
        }
        match self.bit_int_value_ty(q) {
            Some(t) if self.f.fir.ty(&self.m, v) != t => self.cast(CastOp::Trunc, v, t),
            _ => v,
        }
    }

    /// clang `EmitToMemory`: a `bool` is zero-extended to its slot `mty`,
    /// a `_BitInt(N)` sign- or zero-extended.
    pub fn emit_to_memory(&mut self, v: V, q: QualType, mty: LTy) -> V {
        let vt = self.f.fir.ty(&self.m, v);
        if vt == mty {
            return v;
        }
        if self.is_bool_value(q) && vt == LTy::I1 {
            return self.cast(CastOp::ZExt, v, mty);
        }
        if self.bit_int_value_ty(q) == Some(vt) {
            let op = if self.is_signed(q) { CastOp::SExt } else { CastOp::ZExt };
            return self.cast(op, v, mty);
        }
        v
    }

    /// The type a scalar of type `q` in an object of IR type `ty` is
    /// loaded or stored as: a `_BitInt` whose in-memory type is a byte
    /// array is accessed as the integer of its size (clang
    /// `convertTypeForLoadStore`).
    fn scalar_access_ty(&mut self, ty: LTy, q: QualType) -> LTy {
        if !matches!(self.ast.types.canonical_view(q), TypeView::BitInt { .. }) || ty != self.mty(q)
        {
            return ty;
        }
        self.lsty(q)
    }

    /// `iN` when `q` (or its `_Atomic` value type) is `_BitInt(N)` stored
    /// wider than `N` bits.
    fn bit_int_value_ty(&mut self, q: QualType) -> Option<LTy> {
        let q = match self.ast.types.canonical_view(q) {
            TypeView::Atomic(v) => v,
            _ => q,
        };
        let TypeView::BitInt { .. } = self.ast.types.canonical_view(q) else { return None };
        Some(self.lty(q))
    }

    /// `!range !{i8 0, i8 2}` and `!noundef !{}` on the `bool` load `v`.
    fn bool_range(&mut self, v: V) {
        let lo = self.m.const_int(LTy::I8, 0);
        let hi = self.m.const_int(LTy::I8, 2);
        let (Ok(lo), Ok(hi)) = (self.m.md_const(lo), self.m.md_const(hi)) else {
            unreachable!("integer metadata")
        };
        let range = self.m.md_tuple(&[Some(lo), Some(hi)]);
        let empty = self.m.md_tuple(&[]);
        let k = self.m.md_kind("range");
        let i = Fir::last_inst(v);
        self.f.fir.add_md(i, k, range);
        let k = self.m.md_kind("noundef");
        self.f.fir.add_md(i, k, empty);
    }

    /// clang `EmitStoreOfScalar` (bools are zero-extended to memory).
    pub fn store_scalar(&mut self, v: V, a: Addr, q: QualType) {
        self.store_scalar_tb(v, a, q, Tb::Type);
    }

    /// [`Self::store_scalar`] tagged with the access path `tb`; returns
    /// the store.
    pub fn store_scalar_tb(&mut self, v: V, a: Addr, q: QualType, tb: Tb) -> V {
        let vol = self.ast.types.effective_quals(q).is_volatile();
        if let Some(st) = self.vec_store(v, a, q, vol) {
            self.tbaa_mark(st, q, tb);
            return st;
        }
        let ty = self.scalar_access_ty(a.ty, q);
        let v = self.emit_to_memory(v, q, ty);
        let p = self.scalar_tls_addr(a.v);
        let st = self.store(v, p, a.align, vol);
        self.tbaa_mark(st, q, tb);
        st
    }

    /// `memcpy(dst, src, n)`.
    pub fn memcpy(&mut self, dst: Addr, src: V, src_align: u64, n: u64, volatile: bool) -> V {
        let f = kiln_llvm::intrinsics::memcpy(&mut self.m, LTy::I64);
        let fn_ty = self.m.func(f).ty();
        let n = self.m.const_int(LTy::I64, i128::from(n));
        let vb = self.m.const_bool(volatile);
        let pa = self.m.attr_set(&[Attr::Align(dst.align)]);
        let pb = self.m.attr_set(&[Attr::Align(src_align)]);
        let e = self.m.attr_set(&[]);
        let opts = CallOpts {
            attrs: AttrList { params: vec![pa, pb, e, e], ..AttrList::default() },
            ..CallOpts::default()
        };
        let c = self.f.fir.push(
            Op::Call {
                fn_ty,
                callee: V::C(Value::func(f)),
                args: vec![dst.v, src, V::C(n), V::C(vb)],
                opts: Box::new(opts),
            },
            LTy::VOID,
        );
        self.f.calls.push((Fir::last_inst(c), Some(f), true));
        c
    }

    /// `memcpy(dst, src, n)` with an instruction source.
    pub fn memcpy_v(&mut self, dst: Addr, src: V, src_align: u64, n: u64) {
        let f = kiln_llvm::intrinsics::memcpy(&mut self.m, LTy::I64);
        let fn_ty = self.m.func(f).ty();
        let nv = self.m.const_int(LTy::I64, i128::from(n));
        let vb = self.m.const_bool(false);
        let pa = self.m.attr_set(&[Attr::Align(dst.align)]);
        let pb = self.m.attr_set(&[Attr::Align(src_align)]);
        let e = self.m.attr_set(&[]);
        let opts = CallOpts {
            attrs: AttrList { params: vec![pa, pb, e, e], ..AttrList::default() },
            ..CallOpts::default()
        };
        let c = self.f.fir.push(
            Op::Call {
                fn_ty,
                callee: V::C(Value::func(f)),
                args: vec![dst.v, src, V::C(nv), V::C(vb)],
                opts: Box::new(opts),
            },
            LTy::VOID,
        );
        self.f.calls.push((Fir::last_inst(c), Some(f), true));
    }

    /// `memset(dst, byte, n)`.
    pub fn memset(&mut self, dst: Addr, byte: u8, n: u64, volatile: bool) {
        let nv = self.m.const_int(LTy::I64, i128::from(n));
        self.memset_v(dst, byte, V::C(nv), volatile);
    }

    /// `memset(dst, byte, n)` with a computed length.
    pub fn memset_v(&mut self, dst: Addr, byte: u8, n: V, volatile: bool) {
        let f = kiln_llvm::intrinsics::memset(&mut self.m, LTy::I64);
        let fn_ty = self.m.func(f).ty();
        let bv = self.m.const_int(LTy::I8, i128::from(byte));
        let vb = self.m.const_bool(volatile);
        let pa = self.m.attr_set(&[Attr::Align(dst.align)]);
        let e = self.m.attr_set(&[]);
        let opts = CallOpts {
            attrs: AttrList { params: vec![pa, e, e, e], ..AttrList::default() },
            ..CallOpts::default()
        };
        let c = self.f.fir.push(
            Op::Call {
                fn_ty,
                callee: V::C(Value::func(f)),
                args: vec![dst.v, V::C(bv), n, V::C(vb)],
                opts: Box::new(opts),
            },
            LTy::VOID,
        );
        self.f.calls.push((Fir::last_inst(c), Some(f), true));
    }

    /// One argument of clang `EmitFunctionProlog`: the parameter's value
    /// (scalars) or address (aggregates and coerced scalars' memory).
    fn prolog_arg(
        &mut self,
        kind: &ArgKind,
        pq: QualType,
        arg_no: &mut u32,
        loc: Option<Loc>,
    ) -> R<ParamVal> {
        let mt = self.mty(pq);
        // A `_Complex` parameter lives in memory like an aggregate (clang
        // `EmitFunctionProlog`: not of scalar evaluation kind).
        let agg = self.is_aggregate(pq)
            || self.is_atomic_aggregate(pq)
            || self.ast.types.as_complex(pq).is_some();
        match kind {
            ArgKind::Ignore => {
                if agg {
                    let (_, align) = self.size_align(pq);
                    let a = self.f.fir.alloca(mt, align);
                    Ok(ParamVal::Indirect(Addr { v: a, ty: mt, align }, false))
                } else {
                    err(loc, "an ignored scalar parameter is not supported yet")
                }
            }
            ArgKind::Indirect { byval, align } => {
                let addr = Addr { v: V::Arg(*arg_no), ty: mt, align: *align };
                *arg_no += 1;
                if agg {
                    Ok(ParamVal::Indirect(addr, !*byval))
                } else {
                    Ok(ParamVal::Direct(self.load_scalar(addr, pq)))
                }
            }
            ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. } => {
                let offset = if let ArgKind::Direct { offset, .. } = kind { *offset } else { 0 };
                let ct = self.ir(coerce);
                let is_struct = matches!(coerce, IrType::Struct(..));
                if !agg && !is_struct && ct == self.lty(pq) && offset == 0 {
                    let v = V::Arg(*arg_no);
                    *arg_no += 1;
                    return Ok(ParamVal::Direct(v));
                }
                // clang `EmitFunctionProlog`: a fixed-length SVE vector
                // arrives in its scalable register type (WP-810).
                if !agg && matches!(coerce, IrType::ScalableVector(..)) {
                    let lt = self.lty(pq);
                    if let Some(v) = self.sve_to_fixed(V::Arg(*arg_no), lt) {
                        *arg_no += 1;
                        return Ok(ParamVal::Direct(v));
                    }
                }
                let align = self.global_align(pq);
                let a = self.f.fir.alloca(mt, align);
                let alloca = Addr { v: a, ty: mt, align };
                if offset != 0 {
                    // x86-64 `{ [[no_unique_address]] E pad[8]; long b; }`:
                    // the register holds the bytes past the empty prefix.
                    let (size, _) = self.size_align(pq);
                    let at = self.at_abi_offset(alloca, kind);
                    self.coerced_store_sized(V::Arg(*arg_no), coerce, at, size - offset);
                    *arg_no += 1;
                    return Ok(if agg {
                        ParamVal::Indirect(alloca, false)
                    } else {
                        ParamVal::Direct(self.load_scalar(alloca, pq))
                    });
                }
                let flat = self.flat_fields(kind);
                match flat {
                    // clang `EmitFunctionProlog`: flattened stores only
                    // for a struct of more than one element; one element
                    // is a coerced store of that element.
                    Some((fs, packed)) if fs.len() > 1 => {
                        let src_size = self.alloc_size(ct);
                        let dst_size = self.alloc_size(mt);
                        let into = if src_size <= dst_size {
                            alloca.v
                        } else {
                            self.f.fir.alloca(ct, alloca.align)
                        };
                        let offs = self.tys.struct_offsets(&fs, packed);
                        for (j, off) in offs.iter().enumerate() {
                            let p = self.struct_gep(ct, into, j as u32);
                            self.store(V::Arg(*arg_no), p, min_align(alloca.align, *off), false);
                            *arg_no += 1;
                        }
                        if src_size > dst_size {
                            self.memcpy_v(alloca, into, alloca.align, dst_size);
                        }
                    }
                    Some((fs, _)) if fs.len() == 1 => {
                        let (size, _) = self.size_align(pq);
                        self.coerced_store_sized(V::Arg(*arg_no), &fs[0], alloca, size);
                        *arg_no += 1;
                    }
                    Some(_) => {}
                    None => {
                        let (size, _) = self.size_align(pq);
                        self.coerced_store_sized(V::Arg(*arg_no), coerce, alloca, size);
                        *arg_no += 1;
                    }
                }
                if agg {
                    Ok(ParamVal::Indirect(alloca, false))
                } else {
                    Ok(ParamVal::Direct(self.load_scalar(alloca, pq)))
                }
            }
        }
    }

    /// The IR arguments for one call argument (clang `EmitCall`'s
    /// per-argument lowering), appended to `out`.
    pub(crate) fn call_arg(
        &mut self,
        kind: &ArgKind,
        q: QualType,
        arg: CallArg,
        out: &mut Vec<V>,
        loc: Option<Loc>,
    ) -> R<()> {
        match kind {
            ArgKind::Ignore => Ok(()),
            ArgKind::Indirect { byval, align } => {
                match arg {
                    CallArg::Agg(a) => {
                        if *byval && a.align < *align {
                            let got = self.f.fir.enforce_alloca_align(a.v, *align);
                            if got.is_none_or(|g| g < *align) {
                                return err(
                                    loc,
                                    "copying an under-aligned byval argument is not supported yet",
                                );
                            }
                        }
                        out.push(a.v);
                    }
                    CallArg::Scalar(v) => {
                        let mt = self.mty(q);
                        let t = self.f.fir.alloca(mt, *align);
                        self.store_scalar(v, Addr { v: t, ty: mt, align: *align }, q);
                        out.push(t);
                    }
                    CallArg::Complex(re, im) => {
                        let mt = self.mty(q);
                        let t = Addr { v: self.f.fir.alloca(mt, *align), ty: mt, align: *align };
                        self.cx_store((re, im), t, q);
                        out.push(t.v);
                    }
                }
                Ok(())
            }
            ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. } => {
                let offset = if let ArgKind::Direct { offset, .. } = kind { *offset } else { 0 };
                let ct = self.ir(coerce);
                let is_struct = matches!(coerce, IrType::Struct(..));
                let src = match arg {
                    CallArg::Scalar(v) => {
                        if !is_struct && ct == self.lty(q) && offset == 0 {
                            out.push(v);
                            return Ok(());
                        }
                        let t = self.temp(q);
                        self.store_scalar(v, t, q);
                        t
                    }
                    CallArg::Agg(a) => a,
                    CallArg::Complex(re, im) => {
                        let t = self.temp(q);
                        self.cx_store((re, im), t, q);
                        t
                    }
                };
                if offset != 0 {
                    let at = self.at_abi_offset(src, kind);
                    let v = self.coerced_load(at, coerce);
                    out.push(v);
                    return Ok(());
                }
                if let Some((fs, packed)) = self.flat_fields(kind) {
                    let src_size = self.alloc_size(src.ty);
                    let dst_size = self.alloc_size(ct);
                    let from = if src_size < dst_size {
                        let t = self.f.fir.alloca(ct, src.align);
                        self.memcpy_v(
                            Addr { v: t, ty: ct, align: src.align },
                            src.v,
                            src.align,
                            src_size,
                        );
                        t
                    } else {
                        src.v
                    };
                    let offs = self.tys.struct_offsets(&fs, packed);
                    for (j, f) in fs.iter().enumerate() {
                        let ft = self.ir(f);
                        let p = self.struct_gep(ct, from, j as u32);
                        let v = self.load(ft, p, min_align(src.align, offs[j]), false);
                        out.push(v);
                    }
                } else {
                    let v = self.coerced_load(src, coerce);
                    out.push(v);
                }
                Ok(())
            }
        }
    }

    /// clang `emitAddressAtOffset`: where a `Direct` value at a nonzero
    /// ABI offset starts in its memory `a` (x86-64 skips an eightbyte
    /// that holds only empty members), typed as the coerce type; `a`
    /// itself otherwise.
    pub(crate) fn at_abi_offset(&mut self, a: Addr, kind: &ArgKind) -> Addr {
        let ArgKind::Direct { coerce, offset, .. } = kind else { return a };
        if *offset == 0 {
            return a;
        }
        let k = self.i64c(i128::from(*offset));
        let v = self.gep(LTy::I8, a.v, vec![k], true);
        Addr { v, ty: self.ir(coerce), align: min_align(a.align, *offset) }
    }

    /// WP-810: the scalable type a fixed-length SVE value `fixed` is
    /// moved through to or from the scalable vector type `sc` (clang
    /// `CreateCoercedLoad`, `EmitFunctionProlog`, `CK_BitCast`): `sc`
    /// itself for equal element types, `<vscale x N/8 x i8>` for an `i8`
    /// predicate image of `<vscale x N x i1>`; `None` when the two do not
    /// pair up that way.
    pub(crate) fn sve_pair(&mut self, fixed: LTy, sc: LTy) -> Option<LTy> {
        let (fe, _, fs) = self.m.types.vector_info(fixed)?;
        let (se, sn, ss) = self.m.types.vector_info(sc)?;
        if fs || !ss {
            return None;
        }
        if se == fe {
            return Some(sc);
        }
        (se == LTy::I1 && sn % 8 == 0 && fe == LTy::I8)
            .then(|| self.m.types.scalable_vector(LTy::I8, sn / 8))
    }

    /// WP-810: the fixed-length vector `fixed` held at the start of
    /// scalable `v` (clang `cast.fixed`: a predicate is first bitcast to
    /// its `i8` image), if [`Cg::sve_pair`] pairs them.
    pub(crate) fn sve_to_fixed(&mut self, v: V, fixed: LTy) -> Option<V> {
        let sc = self.f.fir.ty(&self.m, v);
        let via = self.sve_pair(fixed, sc)?;
        let v = if via == sc { v } else { self.cast(CastOp::BitCast, v, via) };
        let f = kiln_llvm::intrinsics::vector_extract(&mut self.m, fixed, via);
        let z = self.i64c(0);
        Some(self.intrinsic_call(f, vec![v, z]))
    }

    /// WP-810: the scalable `sc` holding fixed-length `v` at its start
    /// (clang `cast.scalable`: an insert into `undef`, `poison` from clang
    /// 22, bitcast to a predicate's `i1` type), if [`Cg::sve_pair`] pairs
    /// them.
    pub(crate) fn sve_to_scalable(&mut self, v: V, sc: LTy) -> Option<V> {
        let fixed = self.f.fir.ty(&self.m, v);
        let via = self.sve_pair(fixed, sc)?;
        let f = kiln_llvm::intrinsics::vector_insert(&mut self.m, via, fixed);
        let u = V::C(if self.opts.clang_compat >= 22 {
            self.m.const_poison(via)
        } else {
            self.m.const_undef(via)
        });
        let z = self.i64c(0);
        let r = self.intrinsic_call(f, vec![u, v, z]);
        Some(if via == sc { r } else { self.cast(CastOp::BitCast, r, sc) })
    }

    /// clang `CreateCoercedLoad` from `src` to the ABI type `ct`.
    pub fn coerced_load(&mut self, src: Addr, ct: &IrType) -> V {
        let dt = self.ir(ct);
        if src.ty == dt {
            return self.load(dt, src.v, src.align, false);
        }
        if matches!(ct, IrType::ScalableVector(..)) && self.sve_pair(src.ty, dt).is_some() {
            let v = self.load(src.ty, src.v, src.align, false);
            if let Some(r) = self.sve_to_scalable(v, dt) {
                return r;
            }
        }
        let (dsize, _) = self.tys.size_align(ct);
        let src = self.enter_struct_for_coerce(src, dsize);
        let is_ip = |m: &kiln_llvm::Module, t: LTy| m.types.is_int(t) || m.types.is_ptr(t);
        if is_ip(&self.m, dt) && is_ip(&self.m, src.ty) {
            let v = self.load(src.ty, src.v, src.align, false);
            return self.coerce_int_ptr(v, src.ty, dt);
        }
        let ssize = self.alloc_size(src.ty);
        if ssize >= dsize {
            return self.load(dt, src.v, src.align, false);
        }
        let (_, dalign) = self.tys.size_align(ct);
        let tmp_align = dalign.max(src.align);
        let tmp = self.f.fir.alloca(dt, tmp_align);
        self.memcpy_v(Addr { v: tmp, ty: dt, align: tmp_align }, src.v, src.align, ssize);
        self.load(dt, tmp, tmp_align, false)
    }

    /// clang 19 `CreateCoercedStore(Src, Dst, DstSize)` as used by
    /// `EmitFunctionProlog`: `dst_size` is the size of the object, not of
    /// its IR type, and a wider integer is truncated to it.
    pub fn coerced_store_sized(&mut self, v: V, ct: &IrType, dst: Addr, dst_size: u64) {
        if dst_size == 0 {
            return;
        }
        let st = self.ir(ct);
        let (ssize, salign) = self.tys.size_align(ct);
        let mut dst = dst;
        if st != dst.ty {
            dst = self.enter_struct_for_coerce(dst, ssize);
        }
        // clang `CreateCoercedStore`: a scalable value is stored as is.
        if matches!(ct, IrType::ScalableVector(..)) {
            self.store(v, dst.v, dst.align, false);
            return;
        }
        if ssize <= dst_size {
            if self.m.types.is_int(st)
                && self.m.types.is_ptr(dst.ty)
                && ssize == self.alloc_size(dst.ty)
            {
                let c = self.coerce_int_ptr(v, st, dst.ty);
                self.store(c, dst.v, dst.align, false);
            } else {
                self.aggregate_store(v, ct, dst);
            }
        } else if self.m.types.is_int(st) {
            let it = self.m.types.int(u32::try_from(dst_size * 8).unwrap_or(u32::MAX));
            let c = self.coerce_int_ptr(v, st, it);
            self.store(c, dst.v, dst.align, false);
        } else {
            let a = salign.max(dst.align);
            let tmp = self.f.fir.alloca(st, a);
            self.store(v, tmp, a, false);
            self.memcpy_v(dst, tmp, a, dst_size);
        }
    }

    /// clang `EmitAggregateStore`: element-wise for first-class structs.
    pub(crate) fn aggregate_store(&mut self, v: V, ct: &IrType, dst: Addr) {
        let st = self.ir(ct);
        let Some(fields) = self.struct_fields(st) else {
            self.store(v, dst.v, dst.align, false);
            return;
        };
        let offs = self.struct_field_offsets(st);
        for (i, &ft) in fields.iter().enumerate() {
            let p = self.struct_gep(st, dst.v, i as u32);
            let e = self.f.fir.push(Op::ExtractValue { agg: v, idx: i as u32 }, ft);
            let al = min_align(dst.align, offs[i]);
            self.store(e, p, al, false);
        }
    }

    /// `StructLayout::getElementOffset` for every field of struct type `t`.
    pub fn struct_field_offsets(&self, t: LTy) -> Vec<u64> {
        let ty = &self.m.types;
        let Some(fields) = ty.struct_fields(t) else { return Vec::new() };
        let packed = ty.struct_is_packed(t);
        let mut off = 0u64;
        let mut v = Vec::with_capacity(fields.len());
        for &f in fields {
            let (s, a) = self.ir_layout(f);
            let a = if packed { 1 } else { a };
            off = off.div_ceil(a) * a;
            v.push(off);
            off += s;
        }
        v
    }

    fn coerce_int_ptr(&mut self, v: V, from: LTy, to: LTy) -> V {
        if from == to {
            return v;
        }
        let mut v = v;
        let mut from = from;
        if self.m.types.is_ptr(from) {
            if self.m.types.is_ptr(to) {
                return v;
            }
            v = self.cast(CastOp::PtrToInt, v, LTy::I64);
            from = LTy::I64;
        }
        let dest_int = if self.m.types.is_ptr(to) { LTy::I64 } else { to };
        if from != dest_int {
            let fw = self.m.types.int_width(from).unwrap_or(64);
            let tw = self.m.types.int_width(dest_int).unwrap_or(64);
            v = if fw > tw {
                self.cast(CastOp::Trunc, v, dest_int)
            } else {
                self.cast(CastOp::ZExt, v, dest_int)
            };
        }
        if self.m.types.is_ptr(to) {
            v = self.cast(CastOp::IntToPtr, v, to);
        }
        v
    }

    /// clang `EnterStructPointerForCoercedAccess`.
    pub(crate) fn enter_struct_for_coerce(&mut self, a: Addr, want: u64) -> Addr {
        let Some(fields) = self.struct_fields(a.ty) else { return a };
        if fields.is_empty() {
            return a;
        }
        let first = fields[0];
        let fs = self.store_size(first);
        if fs < want && fs < self.store_size(a.ty) {
            return a;
        }
        let p = self.struct_gep(a.ty, a.v, 0);
        self.enter_struct_for_coerce(Addr { v: p, ty: first, align: a.align }, want)
    }

    /// The field types of a struct type.
    pub fn struct_fields(&self, t: LTy) -> Option<Vec<LTy>> {
        self.m.types.struct_fields(t).map(<[LTy]>::to_vec)
    }

    /// The data-layout (size, ABI alignment) of an IR type. Both
    /// supported targets agree: pointers and `i64`/`double` are 8-aligned,
    /// `i128`, `fp128` and `x86_fp80` 16-aligned.
    pub fn ir_layout(&self, t: LTy) -> (u64, u64) {
        use kiln_llvm::TypeData as D;
        let ty = &self.m.types;
        if let Some(w) = ty.int_width(t) {
            let bytes = u64::from(w).div_ceil(8);
            let align = bytes.next_power_of_two().min(16);
            return (bytes.div_ceil(align) * align, align);
        }
        if let Some((e, n)) = ty.array_info(t) {
            let (s, a) = self.ir_layout(e);
            return (s * n, a);
        }
        if let Some(fields) = ty.struct_fields(t) {
            let packed = ty.struct_is_packed(t);
            let (mut off, mut align) = (0u64, 1u64);
            for &f in fields {
                let (s, a) = self.ir_layout(f);
                let a = if packed { 1 } else { a };
                off = off.div_ceil(a) * a + s;
                align = align.max(a);
            }
            return (off.div_ceil(align) * align, align);
        }
        match ty.data(t) {
            D::Half | D::BFloat => (2, 2),
            D::Float => (4, 4),
            D::Double | D::Ptr(_) => (8, 8),
            D::Fp128 | D::X86Fp80 => (16, 16),
            // The lanes are packed (an `<N x i1>` is `N` bits), the total
            // rounded up to a power of two.
            D::Vector { len, elem, .. } => {
                let bits = match ty.int_width(*elem) {
                    Some(w) => u64::from(w) * u64::from(*len),
                    None => self.ir_layout(*elem).0 * 8 * u64::from(*len),
                };
                let bytes = bits.div_ceil(8).next_power_of_two();
                (bytes, bytes.min(16))
            }
            _ => (0, 1),
        }
    }

    /// `DataLayout::getTypeAllocSize`.
    pub fn alloc_size(&self, t: LTy) -> u64 {
        self.ir_layout(t).0
    }

    /// `DataLayout::getTypeStoreSize`.
    pub fn store_size(&self, t: LTy) -> u64 {
        if let Some(w) = self.m.types.int_width(t) {
            return u64::from(w).div_ceil(8);
        }
        if self.m.types.float_width(t) == Some(80) {
            return 10;
        }
        self.ir_layout(t).0
    }

    /// `getelementptr inbounds [nuw] %T, ptr %p, i32 0, i32 i` (clang
    /// `CreateStructGEP`; `nuw` per [`Self::gep_nuw`]).
    pub fn struct_gep(&mut self, t: LTy, p: V, i: u32) -> V {
        let z = self.m.const_int(LTy::I32, 0);
        let iv = self.m.const_int(LTy::I32, i128::from(i));
        self.gep_nuw(t, p, vec![V::C(z), V::C(iv)], true)
    }

    /// The address of field `i` of struct `t` at `p` ([`Self::struct_gep`]);
    /// clang 20+ makes it a plain `getelementptr` when pointer overflow is
    /// defined (`emitAddrOfFieldStorage` with `PointerOverflowDefined`),
    /// clang 19 keeps it `inbounds` under `-fwrapv`.
    pub fn field_gep(&mut self, t: LTy, p: V, i: u32) -> V {
        if self.opts.wrapv_pointer && self.opts.clang_compat >= 22 {
            let z = self.m.const_int(LTy::I32, 0);
            let iv = self.m.const_int(LTy::I32, i128::from(i));
            return self.gep_flags(t, p, vec![V::C(z), V::C(iv)], GepFlags::NONE);
        }
        self.struct_gep(t, p, i)
    }

    // ---- instructions with IRBuilder constant folding --------------------

    /// The integer constant `v`, if it is one: (type, zero-extended bits, width).
    pub fn cint(&self, v: V) -> Option<(LTy, u128, u32)> {
        let V::C(c) = v else { return None };
        match self.m.const_of(c)? {
            ConstData::Int { ty, value: IntValue::Small(x) } => {
                Some((*ty, *x, self.m.types.int_width(*ty).unwrap_or(0)))
            }
            _ => None,
        }
    }

    fn cfloat(&self, v: V) -> Option<(LTy, f64)> {
        let V::C(c) = v else { return None };
        match self.m.const_of(c)? {
            ConstData::Float { ty, bits } if *ty == LTy::DOUBLE => {
                Some((*ty, f64::from_bits(*bits as u64)))
            }
            ConstData::Float { ty, bits } if *ty == LTy::FLOAT => {
                Some((*ty, f64::from(f32::from_bits(*bits as u32))))
            }
            _ => None,
        }
    }

    /// A floating constant of any format as `(type, format, encoding)`.
    fn cfloat_bits(&self, v: V) -> Option<(LTy, FloatKind, u128)> {
        let V::C(c) = v else { return None };
        match self.m.const_of(c)? {
            ConstData::Float { ty, bits } => Some((*ty, float_kind(*ty)?, *bits)),
            _ => None,
        }
    }

    /// LLVM `ConstantFolder` for the formats `cfloat` does not read
    /// (`half`, `bfloat`, `fp128`, `x86_fp80`; WP-292): `APFloat`
    /// arithmetic via the Oven's `apfloat`, NaN results included (their
    /// sign and payload rules are `APFloat`'s; WP-302).
    fn fold_wide_bin(&mut self, op: BinOp, l: V, r: V) -> Option<V> {
        let (ty, k, a) = self.cfloat_bits(l)?;
        let (_, _, b) = self.cfloat_bits(r)?;
        if matches!(k, FloatKind::Single | FloatKind::Double) {
            return None;
        }
        let op = match op {
            BinOp::FAdd => apfloat::BinOp::Add,
            BinOp::FSub => apfloat::BinOp::Sub,
            BinOp::FMul => apfloat::BinOp::Mul,
            BinOp::FDiv => apfloat::BinOp::Div,
            _ => return None,
        };
        let x = apfloat::bin(op, k, a, b)?;
        Some(V::C(self.m.const_float_bits(ty, x)))
    }

    /// The `ConstantFolder` casts involving a format `cfloat` does not
    /// read: `fpext`/`fptrunc`, `[su]itofp` to `half`/`bfloat`, and
    /// `fpto[su]i` from them.
    fn fold_wide_cast(&mut self, op: CastOp, v: V, to: LTy) -> Option<V> {
        let soft = |k: FloatKind| !matches!(k, FloatKind::Single | FloatKind::Double);
        if let (CastOp::Trunc | CastOp::ZExt | CastOp::SExt, V::C(c)) = (op, v)
            && let Some(ConstData::Int { ty, value: IntValue::Big(words) }) = self.m.const_of(c)
            && let Some(w) = self.m.types.int_width(*ty)
            && self.m.types.int_width(to).is_some()
        {
            // A constant wider than 128 bits (a `_BitInt(N)` stored in its
            // slot): sign-extend the words past bit `w` for `sext`.
            let mut words = words.to_vec();
            let neg =
                op == CastOp::SExt && (words[(w as usize - 1) / 64] >> ((w - 1) % 64)) & 1 == 1;
            if neg {
                let top = (w as usize - 1) / 64;
                let used = w % 64;
                if used != 0 {
                    words[top] |= u64::MAX << used;
                }
                let tw = self.m.types.int_width(to).unwrap_or(w) as usize;
                words.resize(tw.div_ceil(64).max(words.len()), u64::MAX);
            }
            return Some(V::C(self.m.const_int_words(to, &words)));
        }
        if let Some((_, a, w)) = self.cint(v) {
            let tk =
                float_kind(to).filter(|&k| matches!(k, FloatKind::Half | FloatKind::BFloat16))?;
            let x = match op {
                CastOp::SIToFP => apfloat::from_int(tk, sext(a, w) as u128, true),
                CastOp::UIToFP => apfloat::from_int(tk, a, false),
                _ => return None,
            };
            return Some(V::C(self.m.const_float_bits(to, x)));
        }
        let (_, fk, a) = self.cfloat_bits(v)?;
        match op {
            CastOp::FPExt | CastOp::FPTrunc => {
                let tk = float_kind(to)?;
                if !soft(fk) && !soft(tk) {
                    return None;
                }
                Some(V::C(self.m.const_float_bits(to, apfloat::cast(fk, tk, a))))
            }
            CastOp::FPToSI | CastOp::FPToUI
                if matches!(fk, FloatKind::Half | FloatKind::BFloat16) =>
            {
                let d = apfloat::cast(fk, FloatKind::Double, a);
                let d = V::C(self.m.const_float_bits(LTy::DOUBLE, d));
                Some(self.cast(op, d, to))
            }
            _ => None,
        }
    }

    fn mk_float(&mut self, ty: LTy, x: f64) -> V {
        let bits = if ty == LTy::FLOAT {
            u128::from((x as f32).to_bits())
        } else {
            u128::from(x.to_bits())
        };
        V::C(self.m.const_float_bits(ty, bits))
    }

    pub fn bin(&mut self, op: BinOp, flags: Flags, l: V, r: V) -> V {
        if let (Some((ty, a, w)), Some((_, b, _))) = (self.cint(l), self.cint(r))
            && let Some(x) = fold_int_bin(op, a, b, w)
        {
            return V::C(self.m.const_int(ty, x as i128));
        }
        if let Some(v) = self.fold_wide_bin(op, l, r) {
            return v;
        }
        if let (Some((ty, a)), Some((_, b))) = (self.cfloat(l), self.cfloat(r)) {
            let single = ty == LTy::FLOAT;
            let x = match op {
                BinOp::FAdd => Some(if single { f64::from(a as f32 + b as f32) } else { a + b }),
                BinOp::FSub => Some(if single { f64::from(a as f32 - b as f32) } else { a - b }),
                BinOp::FMul => Some(if single { f64::from(a as f32 * b as f32) } else { a * b }),
                BinOp::FDiv => Some(if single { f64::from(a as f32 / b as f32) } else { a / b }),
                _ => None,
            };
            if let Some(x) = x {
                // APFloat, not the host: a NaN operand is the (quieted)
                // result, the left one first; an invalid operation gives
                // the default positive quiet NaN.
                if x.is_nan() {
                    let nan = |c: V| -> Option<u128> {
                        let V::C(c) = c else { return None };
                        match self.m.const_of(c)? {
                            ConstData::Float { bits, .. } => Some(*bits),
                            _ => None,
                        }
                    };
                    let quiet: u128 = if single { 1 << 22 } else { 1 << 51 };
                    let bits = if a.is_nan() {
                        nan(l).map(|b| b | quiet)
                    } else if b.is_nan() {
                        nan(r).map(|b| b | quiet)
                    } else {
                        None
                    };
                    let bits = bits.unwrap_or(if single { 0x7fc0_0000 } else { 0x7ff8 << 48 });
                    return V::C(self.m.const_float_bits(ty, bits));
                }
                return self.mk_float(ty, x);
            }
        }
        let ty = self.f.fir.ty(&self.m, l);
        self.f.fir.push(Op::Bin { op, flags, l, r }, ty)
    }

    pub fn icmp(&mut self, p: IntPredicate, l: V, r: V) -> V {
        if let (Some((_, a, w)), Some((_, b, _))) = (self.cint(l), self.cint(r)) {
            let sa = sext(a, w);
            let sb = sext(b, w);
            let x = match p {
                IntPredicate::Eq => a == b,
                IntPredicate::Ne => a != b,
                IntPredicate::Ugt => a > b,
                IntPredicate::Uge => a >= b,
                IntPredicate::Ult => a < b,
                IntPredicate::Ule => a <= b,
                IntPredicate::Sgt => sa > sb,
                IntPredicate::Sge => sa >= sb,
                IntPredicate::Slt => sa < sb,
                IntPredicate::Sle => sa <= sb,
            };
            return V::C(self.m.const_bool(x));
        }
        if let Some(lanes) = self.cmp_lanes(l, r, |s, x| s.cint(x).is_some()) {
            let res = lanes.into_iter().map(|(a, b)| self.icmp(p, a, b)).collect();
            if let Some(k) = self.lanes_vector(res) {
                return k;
            }
        }
        let rt = self.cmp_ty(l);
        self.f.fir.push(Op::ICmp { p, l, r }, rt)
    }

    #[allow(clippy::float_cmp, reason = "constant folding is exact IEEE comparison")]
    pub fn fcmp(&mut self, p: FloatPredicate, l: V, r: V) -> V {
        if let (Some(a), Some(b)) = (self.cmp_f64(l), self.cmp_f64(r)) {
            let uno = a.is_nan() || b.is_nan();
            let x = match p {
                FloatPredicate::Oeq => !uno && a == b,
                FloatPredicate::One => !uno && a != b,
                FloatPredicate::Ogt => !uno && a > b,
                FloatPredicate::Oge => !uno && a >= b,
                FloatPredicate::Olt => !uno && a < b,
                FloatPredicate::Ole => !uno && a <= b,
                FloatPredicate::Une => uno || a != b,
                FloatPredicate::Ueq => uno || a == b,
                _ => return self.f.fir.push(Op::FCmp { p, l, r }, LTy::I1),
            };
            return V::C(self.m.const_bool(x));
        }
        let folds = matches!(
            p,
            FloatPredicate::Oeq
                | FloatPredicate::One
                | FloatPredicate::Ogt
                | FloatPredicate::Oge
                | FloatPredicate::Olt
                | FloatPredicate::Ole
                | FloatPredicate::Une
                | FloatPredicate::Ueq
        );
        if folds && let Some(lanes) = self.cmp_lanes(l, r, |s, x| s.cmp_f64(x).is_some()) {
            let res = lanes.into_iter().map(|(a, b)| self.fcmp(p, a, b)).collect();
            if let Some(k) = self.lanes_vector(res) {
                return k;
            }
        }
        let rt = self.cmp_ty(l);
        self.f.fir.push(Op::FCmp { p, l, r }, rt)
    }

    /// A floating constant read exactly as a `f64` for a folded compare:
    /// `float`, `double`, and `half`/`bfloat` (widened exactly).
    fn cmp_f64(&self, v: V) -> Option<f64> {
        if let Some((_, x)) = self.cfloat(v) {
            return Some(x);
        }
        let (_, k, bits) = self.cfloat_bits(v)?;
        matches!(k, FloatKind::Half | FloatKind::BFloat16)
            .then(|| f64::from_bits(apfloat::cast(k, FloatKind::Double, bits) as u64))
    }

    /// The type of a comparison of `l`: `i1`, or `<N x i1>` for vectors.
    fn cmp_ty(&mut self, l: V) -> LTy {
        let t = self.f.fir.ty(&self.m, l);
        match self.m.types.vector_info(t) {
            Some((_, n, true)) => self.m.types.scalable_vector(LTy::I1, n),
            Some((_, n, false)) => self.m.types.vector(LTy::I1, n),
            None => LTy::I1,
        }
    }

    // ---- vectors (WP-156) -------------------------------------------------

    /// `(element type, count)` of a vector type `q`.
    pub fn vec_of(&self, q: QualType) -> Option<(QualType, u32)> {
        match self.ast.types.canonical_view(q) {
            TypeView::Vector { elem, count, .. } => Some((elem, count)),
            _ => None,
        }
    }

    /// The elements of a constant vector `v`, if it is one.
    fn vec_const_elems(&mut self, v: V) -> Option<Vec<Value>> {
        let V::C(c) = v else { return None };
        let (ty, data) = match self.m.const_of(c)? {
            ConstData::Vector { elems, .. } => return Some(elems.to_vec()),
            ConstData::Zero(t) => (*t, 0),
            ConstData::Undef(t) => (*t, 1),
            ConstData::Poison(t) => (*t, 2),
            _ => return None,
        };
        let (et, n, scalable) = self.m.types.vector_info(ty)?;
        if scalable {
            return None;
        }
        let e = match data {
            0 => self.m.const_zero(et),
            1 => self.m.const_undef(et),
            _ => self.m.const_poison(et),
        };
        Some(vec![e; n as usize])
    }

    /// `extractelement` (`IRBuilder` folds constants).
    pub fn extract_elt(&mut self, vec: V, idx: V) -> V {
        let t = self.f.fir.ty(&self.m, vec);
        let et = self.m.types.vector_info(t).map_or(LTy::I8, |x| x.0);
        if let Some((_, i, _)) = self.cint(idx)
            && let Some(elems) = self.vec_const_elems(vec)
            && let Some(&e) = elems.get(i as usize)
        {
            return V::C(e);
        }
        self.f.fir.push(Op::ExtractElement { vec, idx }, et)
    }

    /// `insertelement` (`IRBuilder` folds constants).
    pub fn insert_elt(&mut self, vec: V, elt: V, idx: V) -> V {
        let t = self.f.fir.ty(&self.m, vec);
        if let (V::C(e), Some((_, i, _))) = (elt, self.cint(idx))
            && let Some(mut elems) = self.vec_const_elems(vec)
            && (i as usize) < elems.len()
        {
            elems[i as usize] = e;
            if let Ok(k) = self.m.const_vector(&elems) {
                return V::C(k);
            }
        }
        self.f.fir.push(Op::InsertElement { vec, elt, idx }, t)
    }

    /// `shufflevector a, b, mask` (b poison when `None`).
    pub fn shuffle(&mut self, a: V, b: Option<V>, mask: Vec<u32>) -> V {
        let t = self.f.fir.ty(&self.m, a);
        let et = self.m.types.vector_info(t).map_or(LTy::I8, |x| x.0);
        let b = b.unwrap_or_else(|| V::C(self.m.const_poison(t)));
        let scalable = self.m.types.vector_info(t).is_some_and(|x| x.2);
        let rt = if scalable {
            // An SVE splat's `zeroinitializer` mask (clang
            // `CreateVectorSplat`); never folded here.
            self.m.types.scalable_vector(et, mask.len() as u32)
        } else {
            self.m.types.vector(et, mask.len() as u32)
        };
        if !scalable && let Some(k) = self.fold_shuffle(a, b, &mask, t) {
            return V::C(k);
        }
        self.f.fir.push(Op::ShuffleVector { a, b, mask }, rt)
    }

    /// llvm `ConstantFoldShuffleVectorInstruction` (the `IRBuilder` folds a
    /// shuffle of two constants): a poison mask lane is `undef`, any other
    /// takes that lane of `a` then `b`; an all-poison mask is `poison`.
    fn fold_shuffle(&mut self, a: V, b: V, mask: &[u32], t: LTy) -> Option<Value> {
        let (V::C(ca), V::C(cb)) = (a, b) else { return None };
        let (et, n, _) = self.m.types.vector_info(t)?;
        if mask.is_empty() {
            return None;
        }
        if mask.iter().all(|&m| m == kiln_llvm::SHUFFLE_POISON) {
            let rt = self.m.types.vector(et, mask.len() as u32);
            return Some(self.m.const_poison(rt));
        }
        let la = self.const_lanes(ca, et, n)?;
        let lb = self.const_lanes(cb, et, n)?;
        let mut out = Vec::with_capacity(mask.len());
        for &m in mask {
            let k = if m == kiln_llvm::SHUFFLE_POISON || m >= 2 * n {
                self.m.const_undef(et)
            } else if m >= n {
                lb[(m - n) as usize]
            } else {
                la[m as usize]
            };
            out.push(k);
        }
        self.m.const_vector(&out).ok()
    }

    /// The `n` lanes of the constant vector `c` (`getAggregateElement`).
    fn const_lanes(&mut self, c: Value, et: LTy, n: u32) -> Option<Vec<Value>> {
        let one = match self.m.const_of(c)? {
            ConstData::Vector { elems, .. } => return Some(elems.to_vec()),
            ConstData::Zero(_) => self.m.const_zero(et),
            ConstData::Poison(_) => self.m.const_poison(et),
            ConstData::Undef(_) => self.m.const_undef(et),
            _ => return None,
        };
        Some(vec![one; n as usize])
    }

    /// A `bitcast` of the constant vector `c` to the integer type `to`
    /// (llvm `ConstantFoldCastInstruction`): a null vector folds to `0`;
    /// an all-ones integer vector folds to `-1` from LLVM 20 on
    /// (`FoldBitCast` checks `isAllOnesValue` first); anything else stays
    /// a `bitcast (...)` constant expression.
    fn fold_vec_to_int(&mut self, c: Value, to: LTy) -> Option<V> {
        let from = self.m.const_type(c);
        self.m.types.vector_info(from)?;
        self.m.types.int_width(to)?;
        let lanes = match self.m.const_of(c)? {
            ConstData::Zero(_) => return Some(V::C(self.m.const_int(to, 0))),
            ConstData::Vector { elems, .. } => elems.to_vec(),
            _ => return None,
        };
        if self.opts.clang_compat >= 22
            && lanes.iter().all(|&e| {
                self.cint(V::C(e)).is_some_and(|(_, x, w)| w > 0 && x == u128::MAX >> (128 - w))
            })
        {
            return Some(V::C(self.m.const_int(to, -1)));
        }
        self.m.const_cast(CastOp::BitCast, c, to).ok().map(V::C)
    }

    /// The unary `__builtin_shufflevector(v, mask)` (clang
    /// `ScalarExprEmitter::VisitShuffleVectorExpr` with two operands): the
    /// mask's lanes are masked to the next power of two of `v`'s length,
    /// then each result lane is extracted from `v` and inserted in turn.
    pub fn shuffle_by_mask(&mut self, v: V, mask: V) -> V {
        let vt = self.f.fir.ty(&self.m, v);
        let mt = self.f.fir.ty(&self.m, mask);
        let (et, n) = self.m.types.vector_info(vt).map_or((LTy::I8, 0), |x| (x.0, x.1));
        let (mi, mn) = self.m.types.vector_info(mt).map_or((LTy::I32, 0), |x| (x.0, x.1));
        // llvm `NextPowerOf2(n - 1) - 1`.
        let bits = i128::from(n.max(1).next_power_of_two()) - 1;
        let k = self.m.const_int(mi, bits);
        let splat = self.m.const_vector(&vec![k; mn as usize]).map_or(V::C(k), V::C);
        let mask = self.bin(BinOp::And, Flags::NONE, mask, splat);
        let rt = self.m.types.vector(et, mn);
        let mut out = V::C(self.m.const_poison(rt));
        for i in 0..mn {
            let ii = V::C(self.m.const_int(LTy::I64, i128::from(i)));
            let idx = self.extract_elt(mask, ii);
            let elt = self.extract_elt(v, idx);
            out = self.insert_elt(out, elt, ii);
        }
        out
    }

    /// `IRBuilder::CreateVectorSplat`: a constant splat folds.
    /// `vt` is the result vector type (fixed, or an SVE scalable one).
    pub fn splat(&mut self, v: V, vt: LTy) -> V {
        let (n, scalable) = self.m.types.vector_info(vt).map_or((1, false), |x| (x.1, x.2));
        if let V::C(c) = v {
            let k = if scalable {
                self.m.const_splat(vt, c)
            } else {
                self.m.const_vector(&vec![c; n as usize])
            };
            if let Ok(k) = k {
                return V::C(k);
            }
        }
        let p = V::C(self.m.const_poison(vt));
        let z = V::C(self.m.const_int(LTy::I64, 0));
        let ins = self.insert_elt(p, v, z);
        self.shuffle(ins, None, vec![0; n as usize])
    }

    /// clang `emitBoolVecConversion`: `<S x i1>` to `<n x i1>`.
    fn bool_vec_resize(&mut self, v: V, n: u32) -> V {
        let t = self.f.fir.ty(&self.m, v);
        let s = self.m.types.vector_info(t).map_or(n, |x| x.1);
        if s == n {
            return v;
        }
        let mask =
            (0..n).map(|i| if i < s.min(n) { i } else { kiln_llvm::SHUFFLE_POISON }).collect();
        self.shuffle(v, None, mask)
    }

    /// clang `EmitFromMemory`/`EmitToMemory` for vectors: `Some(value)`
    /// when `q` is a vector of `bool` (stored as `iP`) or of three
    /// elements (accessed as four).
    fn vec_load(&mut self, a: Addr, q: QualType, vol: bool) -> Option<V> {
        let (elem, n) = self.vec_of(q)?;
        if self.is_bool(elem) {
            let raw = self.load(a.ty, a.v, a.align, vol);
            let p = self.m.types.int_width(a.ty).unwrap_or(8);
            let pv = self.m.types.vector(LTy::I1, p);
            let v = self.cast(CastOp::BitCast, raw, pv);
            return Some(self.bool_vec_resize(v, n));
        }
        if n == 3 {
            let et = self.m.types.vector_info(a.ty)?.0;
            let v4 = self.m.types.vector(et, 4);
            let v = self.load(v4, a.v, a.align, vol);
            return Some(self.shuffle(v, None, vec![0, 1, 2]));
        }
        None
    }

    /// The store, when `q` is a vector of `bool` or of three elements.
    fn vec_store(&mut self, v: V, a: Addr, q: QualType, vol: bool) -> Option<V> {
        let (elem, n) = self.vec_of(q)?;
        if self.is_bool(elem) {
            let p = self.m.types.int_width(a.ty).unwrap_or(8);
            let w = self.bool_vec_resize(v, p);
            let raw = self.cast(CastOp::BitCast, w, a.ty);
            return Some(self.store(raw, a.v, a.align, vol));
        }
        if n == 3 {
            // clang 22 widens with lane 3 taken from an `undef` operand
            // (19: a `poison` operand and a poison mask lane).
            let w = if self.opts.clang_compat >= 22 {
                let t = self.f.fir.ty(&self.m, v);
                let u = V::C(self.m.const_undef(t));
                self.shuffle(v, Some(u), vec![0, 1, 2, 3])
            } else {
                self.shuffle(v, None, vec![0, 1, 2, kiln_llvm::SHUFFLE_POISON])
            };
            return Some(self.store(w, a.v, a.align, vol));
        }
        None
    }

    pub fn fneg(&mut self, v: V) -> V {
        if let Some((ty, k, a)) = self.cfloat_bits(v)
            && !matches!(k, FloatKind::Single | FloatKind::Double)
        {
            return V::C(self.m.const_float_bits(ty, a ^ 1 << (k.bits() - 1)));
        }
        if let Some((ty, a)) = self.cfloat(v) {
            return self.mk_float(ty, -a);
        }
        let ty = self.f.fir.ty(&self.m, v);
        self.f.fir.push(Op::FNeg { v }, ty)
    }

    pub fn cast(&mut self, op: CastOp, v: V, to: LTy) -> V {
        let from = self.f.fir.ty(&self.m, v);
        if from == to && matches!(op, CastOp::BitCast | CastOp::ZExt | CastOp::SExt | CastOp::Trunc)
        {
            return v;
        }
        if let Some(r) = self.fold_cast(op, v, to).or_else(|| self.fold_vec_cast(op, v, to)) {
            return r;
        }
        self.f.fir.push(Op::Cast { op, flags: Flags::NONE, v, to }, to)
    }

    /// The `ConstantFolder` of a scalar cast (`None`: not folded; nothing
    /// is emitted).
    #[allow(clippy::cast_precision_loss, reason = "int-to-float folds round like LLVM")]
    fn fold_cast(&mut self, op: CastOp, v: V, to: LTy) -> Option<V> {
        if let Some(r) = self.fold_wide_cast(op, v, to) {
            return Some(r);
        }
        if let Some((_, a, w)) = self.cint(v) {
            let tw = self.m.types.int_width(to);
            let r = match (op, tw) {
                (CastOp::ZExt, Some(t)) if t > 128 => {
                    Some(V::C(self.m.const_int_words(to, &[a as u64, (a >> 64) as u64])))
                }
                (CastOp::Trunc | CastOp::ZExt, Some(_)) => {
                    Some(V::C(self.m.const_int(to, a as i128)))
                }
                (CastOp::SExt, Some(_)) => Some(V::C(self.m.const_int(to, sext(a, w)))),
                // Rust's int-to-float `as` rounds to nearest even, like
                // LLVM's folder; `float` is rounded once, not via `double`.
                (CastOp::SIToFP, None) if to == LTy::DOUBLE => {
                    Some(self.mk_float(to, sext(a, w) as f64))
                }
                (CastOp::UIToFP, None) if to == LTy::DOUBLE => Some(self.mk_float(to, a as f64)),
                (CastOp::SIToFP, None) if to == LTy::FLOAT => {
                    Some(self.mk_float(to, f64::from(sext(a, w) as f32)))
                }
                (CastOp::UIToFP, None) if to == LTy::FLOAT => {
                    Some(self.mk_float(to, f64::from(a as f32)))
                }
                (CastOp::SIToFP | CastOp::UIToFP, None)
                    if self.m.types.float_width(to) == Some(128) =>
                {
                    let neg = op == CastOp::SIToFP && sext(a, w) < 0;
                    let mag = if neg { sext(a, w).unsigned_abs() } else { a };
                    Some(V::C(self.m.const_float_bits(to, int_to_f128_bits(neg, mag))))
                }
                (CastOp::SIToFP | CastOp::UIToFP, None)
                    if self.m.types.float_width(to) == Some(80) =>
                {
                    let neg = op == CastOp::SIToFP && sext(a, w) < 0;
                    let mag = if neg { sext(a, w).unsigned_abs() } else { a };
                    // Exact (64-bit significand) up to 2^64; wider values
                    // would need rounding and are left to LLVM.
                    u64::try_from(mag)
                        .ok()
                        .map(|m| V::C(self.m.const_float_bits(to, int_to_x87_bits(neg, m))))
                }
                _ => None,
            };
            if let Some(r) = r {
                return Some(r);
            }
        }
        if let Some((fty, x)) = self.cfloat(v) {
            let r = match op {
                CastOp::FPExt | CastOp::FPTrunc if to == LTy::DOUBLE || to == LTy::FLOAT => {
                    let _ = fty;
                    Some(self.mk_float(to, x))
                }
                // `APFloat::convertToInteger` toward zero; a NaN, an
                // infinity or an out-of-range value folds to poison.
                CastOp::FPToSI | CastOp::FPToUI => self.m.types.int_width(to).map(|w| {
                    let t = x.trunc();
                    let ok = if op == CastOp::FPToSI {
                        let lim = 2f64.powi(w as i32 - 1);
                        t >= -lim && t < lim
                    } else {
                        t > -1.0 && t < 2f64.powi(w as i32)
                    };
                    if !ok {
                        V::C(self.m.const_poison(to))
                    } else if op == CastOp::FPToSI {
                        V::C(self.m.const_int(to, t as i128))
                    } else {
                        V::C(self.m.const_int(to, t as u128 as i128))
                    }
                }),
                _ => None,
            };
            if let Some(r) = r {
                return Some(r);
            }
        }
        // LLVM's ConstantFolder: `ptrtoint` of null is 0, and of an
        // `inttoptr` of a same-width integer that integer.
        if let V::C(c) = v
            && op == CastOp::PtrToInt
            && self.m.types.int_width(to).is_some()
        {
            let folded = match self.m.const_of(c) {
                Some(ConstData::Null(_)) => Some(None),
                Some(&ConstData::Cast { op: CastOp::IntToPtr, val, .. }) => {
                    (self.m.const_type(val) == to).then_some(Some(val))
                }
                _ => None,
            };
            if let Some(f) = folded {
                return Some(V::C(f.unwrap_or_else(|| self.m.const_int(to, 0))));
            }
        }
        if let V::C(c) = v
            && matches!(op, CastOp::PtrToInt | CastOp::IntToPtr)
            && let Ok(x) = self.m.const_cast(op, c, to)
        {
            return Some(V::C(x));
        }
        if let V::C(c) = v
            && op == CastOp::BitCast
            && let Some(r) = self.fold_vec_to_int(c, to)
        {
            return Some(r);
        }
        None
    }

    /// llvm `ConstantFoldCastInstruction` of a constant vector: lane by
    /// lane, so `__builtin_convertvector` of a `const` vector variable is a
    /// constant as in clang (WP-490). An `undef`/`poison` lane, or a lane
    /// the scalar folder leaves, is not folded.
    fn fold_vec_cast(&mut self, op: CastOp, v: V, to: LTy) -> Option<V> {
        if matches!(op, CastOp::BitCast | CastOp::PtrToInt | CastOp::IntToPtr) {
            return None;
        }
        let (tet, n, _) = self.m.types.vector_info(to)?;
        let lanes = self.vec_const_elems(v)?;
        if lanes.len() != n as usize {
            return None;
        }
        let mut out = Vec::with_capacity(lanes.len());
        for e in lanes {
            if self.cint(V::C(e)).is_none() && self.cfloat_bits(V::C(e)).is_none() {
                return None;
            }
            let V::C(r) = self.fold_cast(op, V::C(e), tet)? else { return None };
            out.push(r);
        }
        self.m.const_vector(&out).ok().map(V::C)
    }

    /// The lanes of the constant vectors `l` and `r`, when every lane of
    /// both satisfies `ok` (a vector compare folds lane by lane, WP-490).
    fn cmp_lanes(&mut self, l: V, r: V, ok: impl Fn(&Self, V) -> bool) -> Option<Vec<(V, V)>> {
        let a = self.vec_const_elems(l)?;
        let b = self.vec_const_elems(r)?;
        if a.len() != b.len() {
            return None;
        }
        let lanes: Vec<(V, V)> = a.into_iter().zip(b).map(|(x, y)| (V::C(x), V::C(y))).collect();
        lanes.iter().all(|&(x, y)| ok(self, x) && ok(self, y)).then_some(lanes)
    }

    /// A vector of folded lane results (`ConstantVector::get`).
    fn lanes_vector(&mut self, lanes: Vec<V>) -> Option<V> {
        let mut out = Vec::with_capacity(lanes.len());
        for v in lanes {
            let V::C(c) = v else { return None };
            out.push(c);
        }
        self.m.const_vector(&out).ok().map(V::C)
    }

    /// Integer cast by signedness (`CreateIntCast`).
    pub fn int_cast(&mut self, v: V, to: LTy, signed: bool) -> V {
        let from = self.f.fir.ty(&self.m, v);
        // Element-wise between integer vectors of one length (a vector
        // shift's splatted `int` count, `v2u64 << int`; WP-811).
        let elem = |t: LTy| self.m.types.vector_info(t).map_or(t, |(e, _, _)| e);
        let fw = self.m.types.int_width(elem(from)).unwrap_or(0);
        let tw = self.m.types.int_width(elem(to)).unwrap_or(0);
        if fw == tw {
            v
        } else if fw > tw {
            self.cast(CastOp::Trunc, v, to)
        } else if signed {
            self.cast(CastOp::SExt, v, to)
        } else {
            self.cast(CastOp::ZExt, v, to)
        }
    }

    pub fn gep(&mut self, src: LTy, base: V, idx: Vec<V>, inbounds: bool) -> V {
        let flags = if inbounds { GepFlags::INBOUNDS } else { GepFlags::NONE };
        self.gep_flags(src, base, idx, flags)
    }

    /// An `inbounds` GEP that is also `nuw` when `nuw` holds and the
    /// compat level is 22 or later: clang 20+ marks a struct field address
    /// (`CreateStructGEP`) and an index of unsigned type that is added
    /// (`EmitCheckedInBoundsGEP` with `!SignedIndices && !IsSubtraction`)
    /// `inbounds nuw`; clang 19 wrote `inbounds` alone (spec/codegen.md §3,
    /// "GEP flags"). LLVM 19 parses `nuw` on GEPs.
    pub fn gep_nuw(&mut self, src: LTy, base: V, idx: Vec<V>, nuw: bool) -> V {
        let mut flags = GepFlags::INBOUNDS;
        if nuw && self.opts.clang_compat >= 22 {
            flags = flags.with(GepFlags::NUW);
        }
        self.gep_flags(src, base, idx, flags)
    }

    /// [`Self::gep_nuw`] for pointer arithmetic and subscripts (clang
    /// `EmitCheckedInBoundsGEP`): a plain `getelementptr` when pointer
    /// overflow is defined ([`crate::CodegenOptions::wrapv_pointer`]).
    pub fn gep_arith(&mut self, src: LTy, base: V, idx: Vec<V>, nuw: bool) -> V {
        if self.opts.wrapv_pointer {
            return self.gep_flags(src, base, idx, GepFlags::NONE);
        }
        self.gep_nuw(src, base, idx, nuw)
    }

    /// `nsw` for arithmetic in a signed type (`signed`) unless signed
    /// overflow is defined ([`crate::CodegenOptions::wrapv`]; clang
    /// `isSignedOverflowDefined`).
    pub fn nsw_if(&self, signed: bool) -> Flags {
        if signed && !self.opts.wrapv { Flags::NSW } else { Flags::NONE }
    }

    /// The `nsw` of a variable length array index scaling, which clang
    /// drops when pointer overflow is defined
    /// ([`crate::CodegenOptions::wrapv_pointer`]).
    pub fn vla_nsw(&self) -> Flags {
        if self.opts.wrapv_pointer { Flags::NONE } else { Flags::NSW }
    }

    fn gep_flags(&mut self, src: LTy, base: V, idx: Vec<V>, flags: GepFlags) -> V {
        if let V::C(b) = base
            && idx.iter().all(|i| matches!(i, V::C(_)))
        {
            if idx.iter().all(|&i| self.cint(i).is_some_and(|x| x.1 == 0)) {
                return base;
            }
            let iv: Vec<Value> =
                idx.iter().map(|i| if let V::C(c) = i { *c } else { unreachable!() }).collect();
            if let Ok(c) = self.m.const_gep(src, b, &iv, flags, None) {
                return V::C(c);
            }
        }
        self.f.fir.push(Op::Gep { src, base, idx, flags }, LTy::PTR)
    }

    pub fn select(&mut self, c: V, t: V, f: V) -> V {
        if let Some((_, x, _)) = self.cint(c) {
            return if x != 0 { t } else { f };
        }
        let ty = self.f.fir.ty(&self.m, t);
        self.f.fir.push(Op::Select { c, t, f }, ty)
    }
}

/// Folds an integer binary operation of width `w` (`None`: not folded).
fn fold_int_bin(op: BinOp, a: u128, b: u128, w: u32) -> Option<u128> {
    let mask = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
    let sa = sext(a, w);
    let sb = sext(b, w);
    let r = match op {
        BinOp::Add => a.wrapping_add(b),
        BinOp::Sub => a.wrapping_sub(b),
        BinOp::Mul => a.wrapping_mul(b),
        BinOp::And => a & b,
        BinOp::Or => a | b,
        BinOp::Xor => a ^ b,
        BinOp::UDiv if b != 0 => a / b,
        BinOp::URem if b != 0 => a % b,
        BinOp::SDiv if sb != 0 && !(sb == -1 && sa == sext(1u128 << (w - 1), w)) => {
            (sa / sb) as u128
        }
        BinOp::SRem if sb != 0 && !(sb == -1 && sa == sext(1u128 << (w - 1), w)) => {
            (sa % sb) as u128
        }
        BinOp::Shl if b < u128::from(w) => a << b,
        BinOp::LShr if b < u128::from(w) => a >> b,
        BinOp::AShr if b < u128::from(w) => (sa >> b) as u128,
        _ => return None,
    };
    Some(r & mask)
}

/// The `x86_fp80` bits (sign and biased exponent in bits 64..80, explicit
/// integer bit at 63) of the exactly representable integer `mag`.
fn int_to_x87_bits(neg: bool, mag: u64) -> u128 {
    if mag == 0 {
        return 0;
    }
    let n = mag.ilog2();
    let exp = 16383 + u128::from(n);
    let sign = if neg { 1u128 << 79 } else { 0 };
    sign | (exp << 64) | u128::from(mag << (63 - n))
}

/// The IEEE binary128 encoding of `±mag` (round to nearest, ties to even).
fn int_to_f128_bits(neg: bool, mag: u128) -> u128 {
    if mag == 0 {
        return 0;
    }
    let sign = u128::from(neg) << 127;
    let e = mag.ilog2();
    let frac_mask = (1u128 << 112) - 1;
    if e <= 112 {
        let m = mag << (112 - e);
        return sign | (u128::from(e + 16383) << 112) | (m & frac_mask);
    }
    let shift = e - 112;
    let mut m = mag >> shift;
    let rem = mag & ((1u128 << shift) - 1);
    let half = 1u128 << (shift - 1);
    let mut e = e;
    if rem > half || (rem == half && m & 1 == 1) {
        m += 1;
        if m >> 113 != 0 {
            m >>= 1;
            e += 1;
        }
    }
    sign | (u128::from(e + 16383) << 112) | (m & frac_mask)
}

/// Sign-extends the low `w` bits of `a`.
pub(crate) fn sext(a: u128, w: u32) -> i128 {
    if w == 0 || w >= 128 {
        return a as i128;
    }
    let s = 128 - w;
    ((a << s) as i128) >> s
}

/// The alignment of `align`-aligned memory at byte offset `off`.
pub(crate) fn min_align(align: u64, off: u64) -> u64 {
    if off == 0 { align } else { align.min(1u64 << off.trailing_zeros()) }
}

/// The format of the LLVM floating type `ty`.
fn float_kind(ty: LTy) -> Option<FloatKind> {
    Some(match ty {
        LTy::HALF => FloatKind::Half,
        LTy::BFLOAT => FloatKind::BFloat16,
        LTy::FLOAT => FloatKind::Single,
        LTy::DOUBLE => FloatKind::Double,
        LTy::FP128 => FloatKind::Quad,
        LTy::X86_FP80 => FloatKind::X87Extended,
        _ => return None,
    })
}
