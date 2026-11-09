//! The KBC interpreter: one `match` per instruction over a flat register
//! file (`Oven::regs`, 128-bit slots, frame windows) and the abstract
//! memory. Faults carry the pc; notes are rendered (`notes`) only when
//! the evaluation is not silent, while the call stack is still intact.

use std::cmp::Ordering;

use kiln_ast::consts::FloatKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_source::Loc;

use crate::arith::{self, FloatErr, IntErr, canon};
use crate::compile::{FState, GState, float_of};
use crate::complex;
use crate::failure::{EvalFailure, FailKind};
use crate::kbc::{Op, ak, cce, cmp, new_aux, ptr_add_aux};
use crate::mem::{Alloc, DynKind, MemFault, NOWHERE, Origin, Ptr, Sp, TrialMode, flags};
use crate::memo::{MAX_ARGS, MemoKey, MemoVal};
use crate::ty::{Hop, OTy, TyKind, Walk, alt_members};
use crate::{Oven, SemaBridge};

mod bitcast;
mod builtins;
pub(crate) mod exc;
mod life;
mod meta;
#[cfg(feature = "op-stats")]
mod opstats;
pub(crate) mod umemo;
mod virt;
mod wide;

pub(crate) use builtins::BiFault;
pub(crate) use virt::Virt;

/// `Frame::mark` of a frame whose memory is not released on return.
pub(crate) const BOTTOM: u64 = u64::MAX;
/// `Frame::call_pc` of a bottom frame.
pub(crate) const NO_CALL: u32 = u32::MAX;

/// The counters of the running top-level evaluation.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Counters {
    pub steps: u64,
    pub bytes: u64,
    pub peak: u64,
    pub calls: u64,
    /// Peak address space (`peak_arena`).
    pub arena: u64,
    /// Heap allocations (`allocs`).
    pub allocs: u64,
    /// Reflection bytes made (`refl_bytes`; [`crate::mem::Memory::refl`]).
    pub refl: u64,
}

/// A memoizable call in progress: what it will be charged.
#[derive(Clone, Copy, Debug)]
struct Pending {
    key: MemoKey,
    steps0: u64,
    calls0: u64,
    bytes0: u64,
    /// Live bytes at entry.
    live0: u64,
    /// Address space at entry.
    arena0: u64,
    peak_saved: u64,
    arena_saved: u64,
    maxd_saved: u32,
    /// The call's own depth (active calls including it).
    d0: u32,
    /// `RunState::impure` at entry.
    impure0: u64,
    /// `Memory::heap_ops` at entry.
    heap0: u64,
}

/// The state of the running evaluation.
#[derive(Debug, Default)]
pub(crate) struct RunState {
    /// No notes are rendered.
    pub silent: bool,
    pub counters: Counters,
    /// `stack.len()` at the start of the innermost `exec`.
    pub entry: usize,
    /// Deepest active-call count since the innermost pending memo call.
    pub maxd: u32,
    pending: Vec<Pending>,
    /// The variable being initialized (its frame object freezes as the
    /// variable itself).
    pub object: Option<DeclId>,
    /// Out-of-range unscoped enumeration casts are reported
    /// ([`crate::EvalContext::enum_range_check`]).
    pub enum_check: bool,
    /// Effects a cached result would skip, executed so far: out-of-range
    /// enumeration casts (clang reports the conversion on every evaluation
    /// that checks it) and nested evaluations (R-OVEN-1: their records and
    /// diagnostics). A call that executed one is not memoized.
    pub impure: u64,
    /// Nested global initializations in progress.
    pub nesting: u32,
    /// The parameter object whose destructor an `EndParam` entered: the
    /// instruction executes again on its return and ends the object's
    /// lifetime without looking for its registration (0: none).
    pub end_param: u64,
    /// The evaluation is manifestly constant-evaluated ([expr.const]/20):
    /// `__builtin_is_constant_evaluated()` yields true. A trial fold
    /// ([`crate::Oven::try_fold`]) yields false.
    pub const_context: bool,
    /// Depth charged to the enclosing evaluation contexts (R-OVEN-1): a
    /// nested evaluation's calls count from the depth of the call that
    /// started it, plus one for the context itself.
    pub depth_base: u32,
    /// A lax fold ([`crate::Oven::fold_rvalue`]): `const` variables that
    /// are not usable in constant expressions are read; results are not
    /// memoized.
    pub lax: bool,
    /// A fold (clang `EM_ConstantFold`: [`crate::Oven::try_fold`],
    /// [`crate::Oven::fold_rvalue`], `__builtin_constant_p`): a cast a
    /// constant expression may not perform ([expr.const]/5.15) does not
    /// fail it; its pointer result points at nothing (`mem::NOWHERE`).
    pub fold: bool,
    /// A silent constant initialization that still reports
    /// `-Wconstant-evaluated` ([`crate::Oven::warn_constant_evaluated`]):
    /// clang's `evaluateValue` of every variable with static storage or
    /// usable in constant expressions. Cleared in nested evaluations, a
    /// global's initialization and `__builtin_constant_p`.
    pub cev_warn: bool,
    /// The evaluation is plainly constant-evaluated
    /// ([`crate::EvalContext::plain`]): set for the outermost or nested
    /// evaluation that asked for it, false in every other (nested ones
    /// included); read through [`crate::Oven::plain`].
    pub plain: bool,
    /// The evaluation checks for undefined behaviour
    /// ([`crate::Oven::fold_for_overflow`]): a signed `+`, `-`, `*` or
    /// negation that overflows yields its wrapped value and is recorded
    /// in `overflows`. Cleared in nested evaluations, a global's
    /// initialization and `__builtin_constant_p`.
    pub ub_check: bool,
    /// The overflows recorded under `ub_check`: the instruction's pc and
    /// the wrapped value, sign-extended.
    pub overflows: Vec<(u32, i128)>,
    /// The evaluation is a potential-constant-expression check
    /// ([`crate::Oven::evaluate_call`], clang's
    /// `CheckingPotentialConstantExpression`): `-Wconstant-evaluated` is
    /// not reported, nested evaluations included. The number is how many
    /// calls may be active: a further call of a function that can be
    /// constant-evaluated fails without a note (clang `CheckCallLimit`
    /// performs no call but the one checked; WP-873).
    pub potential: Option<u32>,
    /// Every contract assertion is ignored ([expr.const.init]/4.1: the
    /// run deciding whether an initialization is constant-initializable;
    /// [`crate::Oven::check_contract_init`]). A skipped assertion makes
    /// the calls that reach it impure (not memoized).
    pub contracts_off: bool,
}

impl RunState {
    /// Heap bytes held (see `Oven::footprint`).
    pub(crate) fn footprint(&self) -> usize {
        self.pending.capacity() * size_of::<Pending>()
            + self.overflows.capacity() * size_of::<(u32, i128)>()
    }
}

/// An activation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub func: u32,
    pub ret_pc: u32,
    pub fb: u32,
    pub mark: u64,
    pub call_pc: u32,
    pub memo: bool,
    /// Unique per activation (see `Alloc::frame`).
    pub serial: u32,
}

// Every call pushes a `Frame`: growing it past 32 bytes cost ~5% of the
// wall clock of call-heavy evaluations (bench/oven/churn-tree.cpp).
const _: () = assert!(size_of::<Frame>() <= 32);

/// A registered destruction (`Op::PushCleanup`): the destructor `func`
/// of the object at `this`, registered by the instruction at `pc` of the
/// frame at stack depth `depth` (whose `Ret` runs it if no `ScopeExit`
/// did). Kept here rather than as a mark in `Frame`, which every call
/// pushes: `exec` drops the registrations a failed evaluation left, and
/// a `Ret` drains all of its depth, so the depth names one activation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cleanup {
    pub this: u64,
    /// The destroyed object's type.
    pub ty: OTy,
    pub func: u32,
    /// The registering instruction, | [`UNWIND_ONLY`].
    pub pc: u32,
    pub depth: u32,
}

/// `Cleanup::pc` flag: a destruction run only by unwinding, dropped by
/// any other exit (the subobjects of an object under construction,
/// [except.ctor]/3-4; `Op::PushCleanup` aux 4).
pub(crate) const UNWIND_ONLY: u32 = 1 << 31;

/// Why execution stopped (rendered into notes by `notes`).
#[derive(Clone, Debug)]
pub(crate) enum Fault {
    Trap(u32),
    Int {
        err: IntErr,
        op: Op,
        a: u128,
        b: u128,
        aux: u32,
    },
    Float(FloatErr),
    /// An `Op::Wide` integer error on a `width`-bit `_BitInt`: the
    /// decimal text clang's note prints (the exact result, or the shift
    /// count; empty for a division by zero).
    Wide {
        err: IntErr,
        text: Box<str>,
        width: u32,
    },
    /// Float-to-integer conversion out of range.
    FloatRange {
        kind: FloatKind,
        bits: u128,
    },
    Mem {
        f: MemFault,
        ak: u32,
        ty: OTy,
        p: u64,
    },
    NullArith,
    ArrayIndex {
        idx: i128,
        n: u64,
        array: bool,
    },
    PtrSub,
    /// Subtraction of two pointers into one object (or two nulls) whose
    /// element type has size zero ([expr.add]/5, clang
    /// `note_constexpr_pointer_subtraction_zero_size`, WP-948).
    ZeroSizeSub {
        elem: OTy,
    },
    /// An equality comparison involving a pointer to virtual member
    /// function `member` ([expr.eq]/4.3, clang
    /// `note_constexpr_mem_pointer_weak_comparison`, WP-948).
    MpVirtCmp {
        member: DeclId,
    },
    /// Subtraction of pointers into different complete objects (or a
    /// null one): clang 19's generic note, clang 22's
    /// `note_constexpr_pointer_arith_unspecified`.
    PtrArith {
        p: u64,
        q: u64,
        pointee: OTy,
    },
    /// clang's generic `Error(E)`: "subexpression not valid".
    Invalid,
    /// An unscoped enumeration without a fixed underlying type given a
    /// value outside its range (clang 22; [expr.static.cast]/9): the
    /// value as printed, and the range.
    EnumRange {
        v: Box<str>,
        min: i128,
        max: i128,
    },
    /// A failure clang reports with no note and no call stack, so the
    /// error lands at the declaration (clang 22's `__builtin_abs` of the
    /// most negative value).
    Quiet,
    PtrCmp {
        p: u64,
        q: u64,
        pointee: OTy,
    },
    PtrPastCmp {
        p: u64,
        pointee: OTy,
    },
    LiteralCmp,
    /// `==`/`!=` of pointers to different objects, `p` the address of a
    /// weak declaration ([`kiln_ast::Ast::is_weak_decl`]), which may be
    /// null or equal to another object's at run time (clang
    /// `note_constexpr_pointer_weak_comparison`).
    WeakCmp {
        p: u64,
        pointee: OTy,
    },
    /// `==`/`!=` of pointers to different objects one of which is of
    /// unknown identity ([`Origin::Unknown`], P2280).
    PtrCmpUnknown {
        p: u64,
        q: u64,
        pointee: OTy,
    },
    NullField,
    /// `*p` of a null pointer, at clang 22 (`Op::Field` aux 2).
    NullDeref,
    /// A base-class step of a member pointer access through a null
    /// object pointer.
    NullBase,
    PastField,
    /// A base-to-derived conversion of a pointer to (or glvalue of) a
    /// base subobject that is not part of an object of the derived type
    /// ([expr.static.cast]/2, /11): the type of the designated object.
    BadDowncast {
        dynamic: OTy,
        to: OTy,
    },
    Union {
        ak: u32,
        field: DeclId,
        active: Option<DeclId>,
    },
    /// A trivial copy of the class object `src` of type `whole` whose
    /// subobject (member `field`, or of type `ty`) is not initialized
    /// (clang's `CheckFullyInitialized` in `handleTrivialCopy`).
    Uninit {
        field: Option<DeclId>,
        ty: OTy,
        src: u64,
        whole: OTy,
    },
    /// A read of a `mutable` member of an object whose lifetime began
    /// outside the evaluation ([expr.const]/5.9).
    Mutable {
        ak: u32,
        field: DeclId,
    },
    /// An access of a volatile object ([expr.const]/10.8): a subobject of
    /// volatile member `member`, else the object of the allocation of
    /// `p` (`Oven::volatile_path`).
    Volatile {
        ak: u32,
        member: Option<DeclId>,
        p: u64,
    },
    /// [expr.static.cast]/14: the `void *` `p` designates an object of
    /// type `from`, not similar to `to`.
    VoidCast {
        from: OTy,
        to: OTy,
        p: u64,
    },
    NullCallee,
    Callee(u32),
    Steps(Loc),
    Depth,
    Memory,
    /// An array `new` whose bound fails a check of [expr.new]/8 (`v` the
    /// evaluated bound, `k` the explicitly initialized elements).
    NewBound {
        err: BoundErr,
        v: u128,
        signed: bool,
        k: u64,
    },
    /// `::operator new` in `std::allocator<T>::allocate` of a byte count
    /// that is not a multiple of `sizeof(T)` (clang's
    /// `note_constexpr_operator_new_bad_size`).
    NewBadSize {
        bytes: u128,
        elem: OTy,
    },
    /// A placement new of `to` into storage whose object there is a `from`
    /// (clang's `note_constexpr_placement_new_wrong_type`).
    PlacementType {
        from: OTy,
        to: OTy,
    },
    /// A library builtin's own failure.
    Bi(BiFault),
    /// A virtual call whose final overrider is pure ([class.abstract]/6).
    PureVirtual(DeclId),
    /// `delete` of a base class subobject whose class has no virtual
    /// destructor ([expr.delete]/3).
    DeleteNonVirtual {
        stat: OTy,
        dynamic: OTy,
    },
    /// A failed reference `dynamic_cast` (clang's
    /// `note_constexpr_dynamic_cast_to_reference_failed`, `kind` its
    /// select index).
    DynCast {
        kind: u8,
        stat: OTy,
        dynamic: OTy,
        target: OTy,
    },
    /// A polymorphic operation (access kind `ak`) on an object whose
    /// dynamic type is unknown (not usable in constant expressions).
    UnknownDynamic {
        ak: u32,
        p: u64,
        ty: OTy,
    },
    /// A polymorphic operation (access kind `ak`) on a subobject whose
    /// construction has not started or whose destruction has finished:
    /// its dynamic-type word is uninitialized ([class.cdtor]/1, CWG1517).
    NoDynamic(u32),
    /// The implicit destruction (scope exit, `delete`) of the `ty` object
    /// at `p`, whose lifetime an explicit destruction already ended
    /// ([basic.life]/9; `vm/life.rs`).
    DestroyEnded {
        p: u64,
        ty: OTy,
    },
    /// An exception no handler of the evaluation catches (P3068): the
    /// exception object (of type `ty`) and its `what()` string when its
    /// class derives from `std::exception`. Rendered at its throw.
    Uncaught {
        obj: u64,
        ty: OTy,
        what: Option<Box<str>>,
    },
    /// An exception leaving a non-throwing function ([except.spec]/5).
    NoexceptExit,
    /// `throw;` with no handled exception ([expr.throw]/4).
    RethrowNone,
    /// An exception leaving a destructor run by unwinding
    /// ([except.terminate]/1.4); `prev` the active exception's throw.
    ThrowWhileActive {
        prev: u32,
    },
}

/// Why an array `new` bound is erroneous ([expr.new]/8).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BoundErr {
    Negative,
    /// The allocation's size needs more address bits than the target has,
    /// or the count exceeds 2^32 - 1 (clang's `CheckArraySize`).
    TooLarge,
    /// The count exceeds the step limit (clang's guard against evaluating
    /// a huge array element by element).
    Limit,
    /// Fewer elements than explicit initializers.
    TooSmall,
}

impl Fault {
    pub fn kind(&self) -> FailKind {
        match self {
            Fault::Steps(_) => FailKind::StepLimit,
            Fault::Depth => FailKind::DepthLimit,
            Fault::Memory | Fault::Mem { f: MemFault::Memory(_), .. } => FailKind::MemoryLimit,
            _ => FailKind::NotConstant,
        }
    }
}

type Exit = Result<(), (Fault, usize)>;

/// The value length of a scalar of `kind` in memory.
#[inline]
pub(crate) fn value_len(size: u64, kind: TyKind) -> u64 {
    if kind == TyKind::Float(FloatKind::X87Extended) { 10 } else { size }
}

#[inline]
fn compare(c: u32, o: Ordering) -> bool {
    match c {
        cmp::EQ => o == Ordering::Equal,
        cmp::NE => o != Ordering::Equal,
        cmp::LT => o == Ordering::Less,
        cmp::LE => o != Ordering::Greater,
        cmp::GT => o == Ordering::Greater,
        _ => o != Ordering::Less,
    }
}

impl Oven {
    /// clang's `warn_is_constant_evaluated_always_true_constexpr`: the
    /// builtin called directly by the top-level expression, or through
    /// `std::is_constant_evaluated` (`aux` = 1) called directly by it,
    /// in an evaluation that is always constant.
    fn always_true_diag(&self, b: &mut dyn SemaBridge, pc: usize, aux: u32) {
        if self.run.potential.is_some() {
            return;
        }
        let depth = self.stack.len() - self.run.entry;
        let (name, at) = match (depth, aux) {
            (1, _) => ("__builtin_is_constant_evaluated", pc),
            (2, 1) => match self.stack.last() {
                Some(f) if f.call_pc != NO_CALL => {
                    ("std::is_constant_evaluated", f.call_pc as usize)
                }
                _ => return,
            },
            _ => return,
        };
        let loc = self.site(at).loc;
        let d = kiln_diag::Diag::new(
            kiln_diag::DiagId::oven_constant_evaluated_always_true,
            kiln_source::Span::empty(loc),
        )
        .arg(name);
        b.diag(d);
    }

    /// Runs function `func` with its frame at register `fb` until it
    /// returns (the result is in `regs[fb]`).
    /// Reports an out-of-range unscoped enumeration value (clang's
    /// `warn_constexpr_unscoped_enum_out_of_range`).
    fn enum_range_diag(&self, b: &mut dyn SemaBridge, pc: usize, v: &str, min: i128, max: i128) {
        let site = self.site(pc);
        let ast = b.ast();
        let t = site.e.map_or_else(String::new, |e| {
            let q = ast.exprs.get(e).ty();
            Self::type_name(ast, q)
        });
        let d = kiln_diag::Diag::new(
            kiln_diag::DiagId::oven_enum_constexpr_conversion,
            kiln_source::Span::empty(site.loc),
        )
        .arg(v)
        .arg(min.to_string())
        .arg(max.to_string())
        .arg(t)
        // clang reports it on every evaluation that checks it.
        .allow_duplicate();
        b.diag(d);
    }

    pub(crate) fn exec(
        &mut self,
        b: &mut dyn SemaBridge,
        func: u32,
        fb: u32,
    ) -> Result<(), EvalFailure> {
        let entry = self.stack.len();
        let saved_entry = self.run.entry;
        self.run.entry = entry;
        let pend = self.run.pending.len();
        let clean = self.cleanups.len();
        let excs = self.exc.lens();
        let exc_base = std::mem::replace(&mut self.exc.base, excs);
        let entry_pc = self.funcs[func as usize].entry as usize;
        let r = self.run_loop(b, func, entry_pc, fb);
        self.cleanups.truncate(clean);
        self.exc.truncate(excs);
        self.exc.base = exc_base;
        let out = match r {
            Ok(()) => Ok(()),
            Err((fault, pc)) => {
                let kind = fault.kind();
                let f = if self.run.silent {
                    EvalFailure::silent(kind)
                } else {
                    EvalFailure { kind, notes: self.render(&*b, fault, pc) }
                };
                while self.run.pending.len() > pend {
                    if let Some(p) = self.run.pending.pop() {
                        self.mem.peak = self.mem.peak.max(p.peak_saved);
                        self.mem.arena_peak = self.mem.arena_peak.max(p.arena_saved);
                        self.run.maxd = self.run.maxd.max(p.maxd_saved);
                    }
                }
                Err(f)
            }
        };
        self.stack.truncate(entry);
        self.mem.cur_frame = self.stack.last().map_or(0, |f| f.serial);
        self.run.entry = saved_entry;
        out
    }

    fn grow_regs(&mut self, need: usize) {
        if self.regs.len() < need {
            self.regs.resize(need.next_power_of_two(), 0);
        }
    }

    /// clang 22: the dynamic type of a base subobject of a variable whose
    /// value is unknown (not usable in constant expressions) is the
    /// variable's declared class type, a complete object (P2280R4;
    /// spec/oven.md "Polymorphism").
    fn declared_dynamic_type(&self, raw: u64, cls: OTy) -> Option<QualType> {
        if self.clang_compat < 22 {
            return None;
        }
        let p = Ptr::decode(raw);
        let a = self.mem.alloc_any(p)?;
        (matches!(a.origin, Origin::External(_))
            && self.types.has_base_subobject(a.ty, u64::from(p.off), cls))
        .then(|| self.types.info(a.ty).q)
    }

    /// The live or dead (same generation) allocation `p` designates.
    pub(crate) fn alloc_any(&self, p: Ptr) -> Option<&Alloc> {
        self.mem.alloc_any(p)
    }

    /// The `Builtin` instruction at `pc` in the frame at `fb`, out of line
    /// so that the dispatch loop's code is unaffected by it.
    #[inline(never)]
    fn exec_builtin(
        &mut self,
        b: &mut dyn SemaBridge,
        pc: usize,
        fb: usize,
        aux: u32,
    ) -> Result<u128, Fault> {
        let arg = |i: usize| self.code[pc + 1 + i] as usize;
        let reg = |i: usize| self.regs[fb + arg(i)];
        let (t1, t2) = (arg(4) as u32, arg(5) as u32);
        if aux & 0xff == crate::kbc::bi::META {
            // Operands `[dst, base, n, _, t1, t2]` (`base`/`n` raw).
            let (base, n) = (arg(1), arg(2));
            return self.exec_meta(b, pc, fb, aux >> 8, (base, n), (t1, t2));
        }
        if aux & 0xff == crate::kbc::bi::META_EXC {
            return self.exec_meta_exc(b, pc, fb);
        }
        let (a, bb, c) = (reg(1), reg(2), reg(3));
        if aux & 0xff == crate::kbc::bi::THAW_PTR {
            let cur = self.stack[self.stack.len() - 1].func;
            let top = fb + self.funcs[cur as usize].nregs as usize + 1;
            let p = match <kiln_ast::ConstValueId as kiln_ast::RawId>::from_raw(t1) {
                Some(cv) => self.thaw_ptr(b, cv, OTy(t2), top as u32),
                None => 0,
            };
            Ok(p)
        } else if aux & 0xff == crate::kbc::bi::ASSUME_ALIGNED {
            self.bi_assume_aligned(b, aux >> 8 != 0, a as u64, bb, c).map(|()| a)
        } else if aux & 0xff == crate::kbc::bi::PTR_ALIGN {
            self.bi_ptr_align(b, aux >> 8, a as u64, bb, OTy(t1))
        } else if aux & 0xff == crate::kbc::bi::MSG_CHARS {
            self.bi_msg_chars(a, bb as u64, OTy(t1)).map(|()| a)
        } else if aux & 0xff == crate::kbc::bi::UNCAUGHT {
            Ok(self.exc.uncaught() as u128)
        } else if aux & 0xff == crate::kbc::bi::LIFETIME {
            let r = self.builtin(b.ast(), aux, a, bb, c, t1, t2)?;
            Ok(if r != 0 && self.not_yet_begun(pc, a as u64, t1) { 0 } else { r })
        } else {
            self.builtin(b.ast(), aux, a, bb, c, t1, t2)
        }
    }

    /// `Op::Speculate`: runs the region of `func` at `pc0` (ending in a
    /// `Ret`) in the frame at `fb` as a speculative trial (spec/oven.md
    /// "Trial evaluation") and reports whether it completed (and, for a
    /// pointer, whether register `val`'s value is null or the start of a
    /// string literal). Modifying an object that existed before the trial
    /// (other than a heap object) or allocating fails it. A failure is 0
    /// and its effects on the stack, the cleanups and the memory are
    /// rolled back; a resource limit is rolled back and fails the
    /// evaluation.
    #[inline(never)]
    fn speculate(
        &mut self,
        b: &mut dyn SemaBridge,
        func: u32,
        pc0: usize,
        fb: usize,
        val: usize,
        ptr: bool,
    ) -> Result<bool, Fault> {
        let Some(v) = self.trial(b, func, pc0, fb, val, TrialMode::Speculative)? else {
            return Ok(false);
        };
        Ok(!ptr || v == 0 || {
            // clang `EvaluateBuiltinConstantPForLValue`: a null base
            // (an integer cast to a pointer), or a string literal at
            // offset 0.
            let p = Ptr::decode(v as u64);
            if Ptr::is_nowhere(v as u64) {
                p.slot == 0 || (p.off == 0 && self.is_literal_slot(p.slot))
            } else {
                p.off == 0 && self.is_literal(v as u64)
            }
        })
    }

    /// Runs the region of `func` at `pc0` (ending in a `Ret`) in the
    /// frame at `fb` as a silent trial of `mode` (spec/oven.md "Trial
    /// evaluation"): register `val`'s value if it completes, else `None`
    /// with its effects on the stack, the cleanups and the memory rolled
    /// back; a resource limit is rolled back and fails the evaluation. A
    /// speculative trial is a fold (`__builtin_constant_p`'s).
    fn trial(
        &mut self,
        b: &mut dyn SemaBridge,
        func: u32,
        pc0: usize,
        fb: usize,
        val: usize,
        mode: TrialMode,
    ) -> Result<Option<u128>, Fault> {
        let entry = self.stack.len();
        let saved_entry = self.run.entry;
        let pend = self.run.pending.len();
        let clean = self.cleanups.len();
        let excs = self.exc.lens();
        let mark = self.mem.mark();
        // The region's `Ret` writes the frame's result slot.
        let slot = self.regs[fb];
        let (silent, fold, cev_warn) = (self.run.silent, self.run.fold, self.run.cev_warn);
        let ub_check = std::mem::replace(&mut self.run.ub_check, false);
        let lax = (self.run.lax, self.mem.lax);
        self.run.entry = entry;
        self.run.silent = true;
        if mode == TrialMode::Speculative {
            // clang `EvaluateBuiltinConstantP` folds (`FoldConstant`,
            // `EM_ConstantFold`): a read of a `const` variable not usable
            // in constant expressions is only noted, so it is lax too
            // (`__builtin_constant_p(d)` of a `const double d = 2.5;` is
            // true).
            self.run.fold = true;
            (self.run.lax, self.mem.lax) = (true, true);
        }
        self.run.cev_warn = false;
        self.mem.checkpoint(mode);
        let r = self.run_loop(b, func, pc0, fb as u32);
        if r.is_ok() {
            self.mem.commit();
        } else {
            self.mem.rollback(&self.types);
        }
        self.run.silent = silent;
        self.run.fold = fold;
        (self.run.lax, self.mem.lax) = lax;
        self.run.cev_warn = cev_warn;
        self.run.ub_check = ub_check;
        self.run.entry = saved_entry;
        let v = self.regs[fb + val];
        self.regs[fb] = slot;
        let out = match r {
            Ok(()) => Ok(Some(v)),
            Err((f, _)) if f.kind() != FailKind::NotConstant => Err(f),
            Err(_) => {
                while self.run.pending.len() > pend {
                    if let Some(p) = self.run.pending.pop() {
                        self.mem.peak = self.mem.peak.max(p.peak_saved);
                        self.mem.arena_peak = self.mem.arena_peak.max(p.arena_saved);
                        self.run.maxd = self.run.maxd.max(p.maxd_saved);
                    }
                }
                Ok(None)
            }
        };
        self.cleanups.truncate(clean);
        self.exc.truncate(excs);
        self.stack.truncate(entry);
        self.mem.release(mark, &self.types);
        self.mem.cur_frame = self.stack.last().map_or(0, |f| f.serial);
        out
    }

    /// A contract violation ([basic.contract.eval]/7-8; spec/contracts.md
    /// §6.4) at the assertion whose note is trap `trap` (`not_constant`:
    /// its predicate is not a core constant expression, 7.3). Under a
    /// terminating semantic it fails the evaluation (diagnosed when the
    /// evaluation is: the trap's note, the call stack). Under observe, a
    /// manifestly constant-evaluated evaluation continues, warning when
    /// it is not silent; any other (a fold such as `__builtin_constant_p`'s
    /// operand included) is not a constant.
    #[inline(never)]
    fn contract_violation(
        &mut self,
        b: &mut dyn SemaBridge,
        pc: usize,
        trap: u32,
        observe: bool,
        not_constant: bool,
    ) -> Result<(), Fault> {
        self.contract_violated = true;
        if !observe || !self.run.const_context || self.run.fold {
            return Err(Fault::Trap(trap));
        }
        // A cached result would skip the diagnostic.
        self.run.impure += 1;
        if !self.run.silent {
            let mut notes = self.render(b, Fault::Trap(trap), pc).into_iter();
            let loc = notes.next().map_or(self.traps[trap as usize].loc, |n| n.loc);
            let id = if not_constant {
                kiln_diag::DiagId::oven_contract_condition_not_constant_observe
            } else {
                kiln_diag::DiagId::oven_contract_predicate_false_observe
            };
            // One assertion violated from different calls warns once per
            // call: the notes differ.
            let mut d = kiln_diag::Diag::new(id, kiln_source::Span::empty(loc)).allow_duplicate();
            for n in notes {
                d = d.note(n.msg, kiln_source::Span::empty(n.loc));
            }
            b.diag(d);
        }
        Ok(())
    }

    fn run_loop(&mut self, b: &mut dyn SemaBridge, func: u32, pc0: usize, fb0: u32) -> Exit {
        let mut pc = pc0;
        let mut fb = fb0 as usize;
        self.grow_regs(fb + self.funcs[func as usize].nregs as usize + 1);
        let serial = self.mem.next_frame();
        self.stack.push(Frame {
            func,
            ret_pc: 0,
            fb: fb0,
            mark: BOTTOM,
            call_pc: NO_CALL,
            memo: false,
            serial,
        });
        loop {
            // One checked read of the instruction's words; operands index
            // the local window with constant indices (spec/oven.md
            // "Dispatch").
            let mut tail = [0; crate::kbc::WINDOW];
            let win = crate::kbc::window_ref(&self.code, pc, &mut tail);
            let w = win[0];
            let op = Op::from_u8(w as u8);
            let aux = w >> 8;
            #[cfg(feature = "op-stats")]
            opstats::count(op);
            macro_rules! arg {
                ($i:expr) => {
                    win[1 + $i] as usize
                };
            }
            macro_rules! reg {
                ($i:expr) => {
                    self.regs[fb + arg!($i)]
                };
            }
            // The frame's register window as a local slice, for arms that
            // touch only registers: its base and length stay in machine
            // registers across the arm's stores, where `self.regs[..]`
            // reloads both after every store (spec/oven.md "Dispatch").
            macro_rules! frame {
                () => {
                    &mut self.regs[fb..]
                };
            }
            macro_rules! fail {
                ($f:expr) => {
                    return Err(($f, pc))
                };
            }
            // The `Step` whose words are `w` (op word) and `idx` (its
            // operand), at `at`.
            macro_rules! step {
                ($w:expr, $idx:expr, $at:expr) => {
                    let n = u64::from($w >> 8);
                    let before = self.run.counters.steps;
                    if before + n > self.budget.steps {
                        let i = $idx as usize + (self.budget.steps - before) as usize;
                        return Err((Fault::Steps(self.step_locs[i]), $at));
                    }
                    self.run.counters.steps = before + n;
                };
            }
            // Fused pairs: an arm whose window also holds the next
            // instruction executes that one too when it is the usual
            // successor, skipping its dispatch. The encoding is unchanged
            // (spec/oven.md "Dispatch"): the second instruction is still
            // there, still a jump target, and reports its own pc.
            macro_rules! is_next {
                ($at:expr, $op:ident) => {
                    win[$at] as u8 == Op::$op as u8
                };
            }
            match op {
                Op::Nop => pc += 1,
                Op::Halt => return Ok(()),
                Op::Step => {
                    step!(w, arg!(0), pc);
                    // In-place fusion (spec/oven.md "Dispatch"): a Step
                    // usually opens a block whose first instruction is a
                    // `Mov` or a `Const`; run it from the same window.
                    if is_next!(2, Mov) {
                        #[cfg(feature = "op-stats")]
                        opstats::count(Op::Mov);
                        let r = frame!();
                        r[win[3] as usize] = r[win[4] as usize];
                        pc += 5;
                    } else if is_next!(2, Const) {
                        #[cfg(feature = "op-stats")]
                        opstats::count(Op::Const);
                        let v = u64::from(win[4]) | u64::from(win[5]) << 32;
                        frame!()[win[3] as usize] = if win[2] >> 8 & 1 != 0 {
                            v as i64 as i128 as u128
                        } else {
                            u128::from(v)
                        };
                        pc += 6;
                    } else {
                        pc += 2;
                    }
                }
                Op::Const => {
                    let v = u64::from(win[2]) | u64::from(win[3]) << 32;
                    reg!(0) = if aux & 1 != 0 { v as i64 as i128 as u128 } else { u128::from(v) };
                    pc += 4;
                }
                Op::Mov => {
                    let r = frame!();
                    r[arg!(0)] = r[arg!(1)];
                    if is_next!(3, Mov) {
                        #[cfg(feature = "op-stats")]
                        opstats::count(Op::Mov);
                        r[win[4] as usize] = r[win[5] as usize];
                        pc += 6;
                    } else {
                        pc += 3;
                    }
                }
                Op::Jmp => pc = arg!(0),
                Op::JmpIf => {
                    pc = if reg!(0) != 0 { arg!(1) } else { pc + 3 };
                }
                Op::JmpIfNot => {
                    if reg!(0) == 0 {
                        pc = arg!(1);
                    } else if is_next!(3, Step) {
                        #[cfg(feature = "op-stats")]
                        opstats::count(Op::Step);
                        step!(win[3], win[4], pc + 3);
                        pc += 5;
                    } else {
                        pc += 3;
                    }
                }
                Op::IAdd
                | Op::ISub
                | Op::IMul
                | Op::IDiv
                | Op::IRem
                | Op::IShl
                | Op::IShr
                | Op::IAnd
                | Op::IOr
                | Op::IXor => {
                    let r = frame!();
                    let (a, bb) = (r[arg!(1)], r[arg!(2)]);
                    let d = fb + arg!(0);
                    let width = aux & 0xFF;
                    let signed = aux >> 8 & 1 != 0;
                    match arith::int_bin(
                        op,
                        width,
                        signed,
                        a,
                        bb,
                        aux >> 9 & 0xFF,
                        aux >> 17 & 1 != 0,
                    ) {
                        Ok(v) => r[arg!(0)] = v,
                        Err(err) => match self.wrap_overflow(pc, err, op, aux, a, bb) {
                            Some(v) => self.regs[d] = v,
                            None => fail!(Fault::Int { err, op, a, b: bb, aux }),
                        },
                    }
                    pc += 4;
                }
                Op::INot => {
                    reg!(0) = canon(!reg!(1), aux & 0xFF, aux >> 8 & 1 != 0);
                    pc += 3;
                }
                Op::LNot => {
                    reg!(0) = u128::from(reg!(1) == 0);
                    pc += 3;
                }
                Op::ICmp => {
                    let r = frame!();
                    let (a, bb) = (r[arg!(1)], r[arg!(2)]);
                    let o =
                        if aux >> 8 & 1 != 0 { (a as i128).cmp(&(bb as i128)) } else { a.cmp(&bb) };
                    r[arg!(0)] = u128::from(compare(aux >> 9, o));
                    if is_next!(4, JmpIfNot) || is_next!(4, JmpIf) {
                        #[cfg(feature = "op-stats")]
                        opstats::count(Op::from_u8(win[4] as u8));
                        let c = r[win[5] as usize] != 0;
                        pc = if c == is_next!(4, JmpIf) { win[6] as usize } else { pc + 7 };
                    } else {
                        pc += 4;
                    }
                }
                Op::ICast => {
                    let r = frame!();
                    r[arg!(0)] = canon(r[arg!(1)], aux >> 9 & 0xFF, aux >> 17 & 1 != 0);
                    pc += 3;
                }
                Op::ToBool => {
                    reg!(0) = u128::from(reg!(1) != 0);
                    pc += 3;
                }
                Op::PtrToBool => {
                    // The address of a weak declaration may be null at run
                    // time: no value, and no note (clang
                    // `EvalPointerValueAsBool`).
                    if self.is_weak_ptr(b, reg!(1) as u64) {
                        fail!(Fault::Quiet)
                    }
                    reg!(0) = u128::from(reg!(1) != 0);
                    pc += 3;
                }
                Op::PtrAdd => {
                    let raw = reg!(1) as u64;
                    let v = reg!(2);
                    let elem = OTy(arg!(3) as u32);
                    let p = match self.ptr_add_step(raw, v, aux, elem) {
                        Some(p) => p,
                        None => match self.ptr_add_member(raw, v, aux, elem) {
                            Some(p) => p,
                            None => match self.ptr_add_op(raw, v, aux, elem) {
                                Ok(p) => p,
                                Err(f) => fail!(f),
                            },
                        },
                    };
                    reg!(0) = u128::from(p);
                    pc += 5;
                }
                Op::PtrSub => {
                    let (p, q, elem) = (reg!(1) as u64, reg!(2) as u64, OTy(arg!(3) as u32));
                    if aux != 0 && Ptr::alloc_key(p) == Ptr::alloc_key(q) {
                        fail!(Fault::ZeroSizeSub { elem });
                    }
                    match self.ptr_sub(p, q, elem) {
                        Ok(d) => reg!(0) = d as u128,
                        Err(f) => fail!(f),
                    }
                    pc += 5;
                }
                Op::PtrCmp => {
                    let (p, q) = (reg!(1) as u64, reg!(2) as u64);
                    let weak = (self.is_weak_ptr(b, p), self.is_weak_ptr(b, q));
                    match self.ptr_cmp(p, q, OTy(arg!(3) as u32), aux, weak) {
                        Ok(r) => reg!(0) = u128::from(r),
                        Err(f) => fail!(f),
                    }
                    pc += 5;
                }
                Op::Field => {
                    let raw = reg!(1) as u64;
                    if raw == 0 {
                        // aux 1 (`.` on `*null`): stay null, so the access
                        // reports the dereferenced null pointer; aux 2 (the
                        // `*` itself, at clang 22) fails here.
                        match aux {
                            0 => fail!(Fault::NullField),
                            2 => fail!(Fault::NullDeref),
                            _ => reg!(0) = 0,
                        }
                    } else if aux == 2 {
                        // A dereference check: the pointer itself.
                        reg!(0) = u128::from(raw);
                    } else {
                        let p = Ptr::decode(raw);
                        if p.past {
                            fail!(Fault::PastField);
                        }
                        reg!(0) = u128::from(Ptr::with_off(
                            raw,
                            u64::from(p.off) + arg!(2) as u64,
                            false,
                        ));
                    }
                    pc += 4;
                }
                Op::Load => {
                    let ty = OTy(arg!(2) as u32);
                    let raw = reg!(1) as u64;
                    match self.load(raw, ty, aux) {
                        Ok(v) => reg!(0) = v,
                        Err(f) => fail!(f),
                    }
                    pc += 4;
                }
                Op::Store | Op::InitStore => {
                    let ty = OTy(arg!(2) as u32);
                    let raw = reg!(0) as u64;
                    let v = reg!(1);
                    let (k, init) =
                        if op == Op::InitStore { (ak::INIT, true) } else { (aux, false) };
                    if let Err(f) = self.store(raw, ty, v, k, init) {
                        fail!(f);
                    }
                    pc += 4;
                }
                Op::Copy => {
                    let ty = OTy(arg!(2) as u32);
                    let full = (aux & 2 != 0).then(|| b.ast());
                    if let Err(f) =
                        self.copy_obj(reg!(0) as u64, reg!(1) as u64, ty, aux & 1 != 0, full)
                    {
                        fail!(f);
                    }
                    pc += 4;
                }
                Op::Alloc => {
                    let ty = OTy(arg!(1) as u32);
                    let info = self.types.info(ty);
                    let fl = aux as u8 | if info.has_union { flags::HAS_UNION } else { 0 };
                    let origin = self.origins[arg!(2)];
                    match self.mem.alloc(ty, info.size, fl, origin) {
                        Ok(p) => reg!(0) = u128::from(p),
                        Err(_) => fail!(Fault::Memory),
                    }
                    pc += 4;
                }
                Op::EndLife => {
                    if let Err(f) = self.end_life(reg!(0) as u64, OTy(arg!(1) as u32), aux) {
                        fail!(f);
                    }
                    pc += 3;
                }
                Op::Builtin => {
                    let d = fb + arg!(0);
                    match self.exec_builtin(b, pc, fb, aux) {
                        Ok(v) => self.regs[d] = v,
                        Err(f) => fail!(f),
                    }
                    pc += 7;
                }
                Op::ScopeEnter => {
                    reg!(0) = u128::from(self.mem.mark()) | (self.cleanups.len() as u128) << 64;
                    pc += 2;
                }
                Op::ScopeExit => {
                    let m = reg!(0);
                    if self.cleanups.len() > (m >> 64) as usize {
                        // Destroy the innermost registered object, then
                        // execute this `ScopeExit` again.
                        match self.run_cleanup(b, pc, pc, fb, false) {
                            Ok(Some((npc, nfb))) => {
                                pc = npc;
                                fb = nfb;
                            }
                            Ok(None) => {}
                            Err((f, at)) => return Err((f, at)),
                        }
                        continue;
                    }
                    if self.mem.life_above(m as u64)
                        && let Err(f) = self.scope_life(b.ast(), m as u64, life::Objs::All)
                    {
                        fail!(f);
                    }
                    self.mem.release(m as u64, &self.types);
                    pc += 2;
                }
                Op::VCall => {
                    let base = arg!(1);
                    let slot = fb + base + aux as usize;
                    let m = DeclId::new(win[1]);
                    let cls = OTy(win[3]);
                    let adj = arg!(3);
                    let callee = match self.vcall(b, m, cls, self.regs[slot] as u64) {
                        Ok((f, this, cov)) => {
                            self.regs[slot] = u128::from(this);
                            if adj as u32 != u32::MAX {
                                self.regs[fb + adj] = cov;
                            } else if cov != 0 {
                                fail!(Fault::Invalid);
                            }
                            f
                        }
                        Err(f) => fail!(f),
                    };
                    match self.enter(b, callee, pc, pc + 5, fb, base) {
                        Ok(Some((npc, nfb))) => {
                            pc = npc;
                            fb = nfb;
                        }
                        Ok(None) => pc += 5,
                        Err(f) => fail!(f),
                    }
                }
                Op::Call | Op::CallInd => {
                    let callee = if op == Op::Call {
                        arg!(0) as u32
                    } else {
                        let v = reg!(0) as u64;
                        let p = Ptr::decode(v);
                        if v == 0 || !p.is_fn() {
                            fail!(Fault::NullCallee);
                        }
                        // A function pointer a fold cast (`Op::Cce`): clang
                        // calls none whose type differs from the callee's.
                        if Ptr::is_nowhere(v) && !self.cast_callee_matches(b, pc, p.off) {
                            fail!(Fault::Invalid);
                        }
                        p.off
                    };
                    match self.enter(b, callee, pc, pc + 3, fb, arg!(1)) {
                        Ok(Some((npc, nfb))) => {
                            pc = npc;
                            fb = nfb;
                        }
                        Ok(None) => pc += 3,
                        Err(f) => fail!(f),
                    }
                }
                Op::Ret => {
                    if self.cleanups.last().is_some_and(|c| c.depth as usize == self.stack.len()) {
                        // The frame's objects are destroyed after the
                        // result is initialized ([stmt.return]/3).
                        match self.run_cleanup(b, pc, pc, fb, false) {
                            Ok(Some((npc, nfb))) => {
                                pc = npc;
                                fb = nfb;
                            }
                            Ok(None) => {}
                            Err((f, at)) => return Err((f, at)),
                        }
                        continue;
                    }
                    if let Some(fr) = self.stack.last()
                        && fr.mark != BOTTOM
                        && self.mem.life_above(fr.mark)
                        && let Err(f) = self.scope_life(b.ast(), fr.mark, life::Objs::Locals)
                    {
                        fail!(f);
                    }
                    let v = reg!(0);
                    self.regs[fb] = v;
                    let Some(fr) = self.stack.pop() else { return Ok(()) };
                    if self.hooks.calls {
                        self.hook_call_ret(b);
                    }
                    if fr.mark != BOTTOM {
                        // The caller destroys the parameters ([expr.call]/6).
                        if self.mem.life_above(fr.mark)
                            && let Err(f) = self.scope_life(b.ast(), fr.mark, life::Objs::Params)
                        {
                            return Err((f, fr.ret_pc as usize));
                        }
                        self.mem.release(fr.mark, &self.types);
                    }
                    self.mem.cur_frame = self.stack.last().map_or(0, |f| f.serial);
                    if fr.memo {
                        self.finish_memo(v);
                    }
                    if self.stack.len() <= self.run.entry {
                        return Ok(());
                    }
                    pc = fr.ret_pc as usize;
                    fb = self.stack[self.stack.len() - 1].fb as usize;
                }
                Op::GlobalAddr => {
                    let (d, g) = (fb + arg!(0), arg!(1));
                    let p = match self.globals[g].state {
                        GState::Resolved(p) => p,
                        GState::Unresolved => {
                            let cur = self.stack[self.stack.len() - 1].func;
                            let top = fb + self.funcs[cur as usize].nregs as usize + 1;
                            self.resolve_global(b, g as u32, top as u32)
                        }
                    };
                    self.regs[d] = u128::from(p);
                    pc += 3;
                }
                // Listed rather than `_`, so the jump table covers every
                // opcode and needs no second range check.
                Op::Trap
                | Op::Const2
                | Op::INeg
                | Op::EnumCheck
                | Op::FAdd
                | Op::FSub
                | Op::FMul
                | Op::FDiv
                | Op::FNeg
                | Op::FCmp
                | Op::FCast
                | Op::IToF
                | Op::FToI
                | Op::FToBool
                | Op::Downcast
                | Op::VBase
                | Op::LoadBits
                | Op::StoreBits
                | Op::LoadLane
                | Op::StoreLane
                | Op::ZeroInit
                | Op::New
                | Op::Delete
                | Op::Placement
                | Op::PushCleanup
                | Op::MakeConst
                | Op::SetActive
                | Op::ActiveCheck
                | Op::MpCast
                | Op::MpAddr
                | Op::MpActive
                | Op::MpFn
                | Op::MpCmp
                | Op::RefChk
                | Op::EndParam
                | Op::SetVptr
                | Op::CovAdj
                | Op::DynCast
                | Op::TypeId
                | Op::IsConstEval
                | Op::Wide
                | Op::Cx
                | Op::Speculate
                | Op::LabelAddr
                | Op::VoidCast
                | Op::Cce
                | Op::TryEnter
                | Op::Throw
                | Op::Unwind
                | Op::Catch
                | Op::CatchBegin
                | Op::ContractSkip
                | Op::ContractFail
                | Op::ContractTrial
                | Op::Fold => {
                    (pc, fb) = self.run_cold(b, op, pc, fb)?;
                }
            }
        }
    }

    /// The opcodes `run_loop` does not dispatch itself, the rarely executed
    /// ones, out of line so the hot loop stays small (WP-648): the
    /// instruction at `pc` in the frame at `fb`.
    /// Returns the next `(pc, fb)`.
    #[inline(never)]
    fn run_cold(
        &mut self,
        b: &mut dyn SemaBridge,
        op: Op,
        mut pc: usize,
        mut fb: usize,
    ) -> Result<(usize, usize), (Fault, usize)> {
        // Fetched again rather than passed: a window whose address is
        // taken lives on `run_loop`'s stack, and the store and the reload
        // of every instruction's words through it cost more wall time
        // than they save (spec/oven.md "Dispatch").
        let win = crate::kbc::window(&self.code, pc);
        let aux = win[0] >> 8;
        macro_rules! arg {
            ($i:expr) => {
                win[1 + $i] as usize
            };
        }
        macro_rules! reg {
            ($i:expr) => {
                self.regs[fb + arg!($i)]
            };
        }
        macro_rules! fail {
            ($f:expr) => {
                return Err(($f, pc))
            };
        }
        match op {
            Op::Trap => fail!(Fault::Trap(arg!(0) as u32)),
            Op::Const2 => {
                let mut v = 0u128;
                for i in (0..4).rev() {
                    v = v << 32 | u128::from(win[2 + i]);
                }
                reg!(0) = v;
                pc += 6;
            }
            Op::INeg => {
                let a = reg!(1);
                let width = aux & 0xFF;
                let signed = aux >> 8 & 1 != 0;
                match arith::int_bin(Op::ISub, width, signed, 0, a, 0, false) {
                    Ok(v) => reg!(0) = v,
                    Err(err) => match self.wrap_overflow(pc, err, Op::ISub, aux, 0, a) {
                        Some(v) => reg!(0) = v,
                        None => fail!(Fault::Int { err, op, a, b: 0, aux }),
                    },
                }
                pc += 3;
            }
            Op::EnumCheck => {
                let w = |i: usize| u64::from(win[i]);
                let min = i128::from((w(2) | w(3) << 32).cast_signed());
                let max = i128::from((w(4) | w(5) << 32).cast_signed());
                // clang 19's `HandleCast`: the operand, sign-extended
                // for an enumeration with negative values, else
                // zero-extended, and printed in its own signedness.
                let (fw, fs) = (aux & 0xFF, aux >> 8 & 1 != 0);
                let raw = reg!(0);
                let oob = if min < 0 {
                    let v = canon(raw, fw, true).cast_signed();
                    v < min || v > max
                } else {
                    canon(raw, fw, false) > max.cast_unsigned()
                };
                // clang 19 accepts the value and reports it as an error
                // (`-Wenum-constexpr-conversion`) only where a constant
                // expression proper is required (spec/oven.md
                // "Integration"); elsewhere the cast is not checked.
                // clang 22 (`note_constexpr_unscoped_enum_out_of_range`):
                // the value makes any evaluation non-constant (a speculative
                // one goes on, as clang's `CCEDiag` when folding).
                if oob && self.clang_compat >= 22 && !self.run.silent {
                    let v = if fs { raw.cast_signed().to_string() } else { raw.to_string() };
                    fail!(Fault::EnumRange { v: v.into(), min, max });
                }
                if oob {
                    self.run.impure += 1;
                    if self.run.enum_check && !self.run.silent {
                        let v = if fs { raw.cast_signed().to_string() } else { raw.to_string() };
                        self.enum_range_diag(b, pc, &v, min, max);
                    }
                }
                pc += 6;
            }
            Op::FAdd | Op::FSub | Op::FMul | Op::FDiv => {
                let kind = float_of(aux);
                match arith::float_bin(op, kind, reg!(1), reg!(2)) {
                    Ok(v) => reg!(0) = v,
                    Err(FloatErr::NaN | FloatErr::DivZero) if self.fold_float_ub => {
                        let Some(v) = arith::float_bin_raw(op, kind, reg!(1), reg!(2)) else {
                            fail!(Fault::Float(FloatErr::Unsupported))
                        };
                        reg!(0) = v;
                        self.run.impure += 1;
                    }
                    Err(e) => fail!(Fault::Float(e)),
                }
                pc += 4;
            }
            Op::FNeg => {
                reg!(0) = arith::float_neg(float_of(aux), reg!(1));
                pc += 3;
            }
            Op::FCmp => {
                let c = aux >> 4;
                let r = match arith::float_cmp(float_of(aux), reg!(1), reg!(2)) {
                    Some(o) => compare(c, o),
                    None => c == cmp::NE,
                };
                reg!(0) = u128::from(r);
                pc += 4;
            }
            Op::FCast => {
                reg!(0) = arith::float_cast(float_of(aux), float_of(aux >> 4), reg!(1));
                pc += 3;
            }
            Op::IToF => {
                reg!(0) = arith::int_to_float(float_of(aux >> 9), reg!(1), aux >> 8 & 1 != 0);
                pc += 3;
            }
            Op::FToI => {
                let kind = float_of(aux);
                let a = reg!(1);
                match arith::float_to_int(kind, a, aux >> 4 & 0xFF, aux >> 12 & 1 != 0) {
                    Ok(v) => reg!(0) = v,
                    Err(FloatErr::Range) => fail!(Fault::FloatRange { kind, bits: a }),
                    Err(e) => fail!(Fault::Float(e)),
                }
                pc += 3;
            }
            Op::FToBool => {
                reg!(0) = u128::from(arith::float_truth(float_of(aux), reg!(1)));
                pc += 3;
            }
            Op::Downcast => {
                let raw = reg!(1) as u64;
                let to = OTy(arg!(3) as u32);
                if raw == 0 {
                    if aux == 0 {
                        fail!(Fault::NullField);
                    }
                    reg!(0) = 0;
                } else {
                    match self.downcast(raw, arg!(2) as u64, to) {
                        Ok(q) => reg!(0) = u128::from(q),
                        Err(f) => fail!(f),
                    }
                }
                pc += 5;
            }
            Op::VBase => {
                let raw = reg!(1) as u64;
                if raw == 0 {
                    if aux == 0 {
                        fail!(Fault::NullField);
                    }
                    reg!(0) = 0;
                } else {
                    let (cls, k) = (OTy(arg!(2) as u32), arg!(3) as u16);
                    match self.sub_addr(b, raw, cls, k) {
                        Some(q) => reg!(0) = u128::from(q),
                        None => fail!(Fault::Invalid),
                    }
                }
                pc += 5;
            }
            Op::LoadBits => {
                let ty = OTy(arg!(2) as u32);
                let raw = reg!(1) as u64;
                let bw = arg!(3) as u32;
                match self.load_bits(raw, ty, bw & 0xFF, bw >> 8, aux) {
                    Ok(v) => reg!(0) = v,
                    Err(f) => fail!(f),
                }
                pc += 5;
            }
            Op::StoreBits => {
                let ty = OTy(arg!(2) as u32);
                let raw = reg!(0) as u64;
                let v = reg!(1);
                let bw = arg!(3) as u32;
                if let Err(f) = self.store_bits(raw, ty, v, bw & 0xFF, bw >> 8, aux & 1 != 0) {
                    fail!(f);
                }
                pc += 5;
            }
            Op::LoadLane => {
                let vty = OTy(arg!(3) as u32);
                let r = self
                    .lane_at(reg!(1) as u64, reg!(2) as u64, vty, aux)
                    .and_then(|(q, bit, elem)| self.load_bits(q, elem, bit, 1, aux));
                match r {
                    Ok(v) => reg!(0) = v,
                    Err(f) => fail!(f),
                }
                pc += 5;
            }
            Op::StoreLane => {
                let vty = OTy(arg!(3) as u32);
                let v = reg!(1);
                let init = aux & 1 != 0;
                let k = if init { ak::INIT } else { ak::ASSIGN };
                let r = self
                    .lane_at(reg!(0) as u64, reg!(2) as u64, vty, k)
                    .and_then(|(q, bit, elem)| self.store_bits(q, elem, v, bit, 1, init));
                if let Err(f) = r {
                    fail!(f);
                }
                pc += 5;
            }
            Op::ZeroInit => {
                let ty = OTy(arg!(1) as u32);
                let raw = reg!(0) as u64;
                // An empty class writes nothing: its byte may be
                // another subobject's (libc++'s `__compressed_pair`
                // value-initializes its empty deleter base over the
                // pointer).
                let size = if self.types.holds_no_data(ty) { 0 } else { self.types.size(ty) };
                match self.mem.check(raw, size, true).map(|(st, abs, _)| (st, abs)) {
                    Ok((st, abs)) => {
                        if let Err(f) = self.mem.trial_write(raw, size) {
                            fail!(Fault::Mem { f, ak: ak::INIT, ty, p: raw });
                        }
                        if size != 0 {
                            self.mem.zero(st, abs, size);
                        }
                        if self.types.info(ty).has_union {
                            self.zero_unions(raw, u64::from(Ptr::decode(raw).off), ty);
                        }
                    }
                    Err(f) => fail!(Fault::Mem { f, ak: ak::INIT, ty, p: raw }),
                }
                pc += 3;
            }
            Op::New => {
                let elem = OTy(arg!(1) as u32);
                let mut e = match self.origins[arg!(3)] {
                    Origin::Heap(e, _) => e,
                    _ => ExprId::new(0),
                };
                if aux & new_aux::STD != 0 && self.clang_compat >= 22 {
                    e = self.std_alloc_site(b, e);
                }
                let origin = Origin::Heap(e, self.mem.heap_allocs as u32);
                let bound = if aux & new_aux::ARRAY != 0 {
                    let mut v = reg!(2);
                    if aux & new_aux::STD != 0 {
                        // A byte count ([allocator.members]/5).
                        let es = u128::from(self.types.size(elem).max(1));
                        if !v.is_multiple_of(es) {
                            fail!(Fault::NewBadSize { bytes: v, elem });
                        }
                        v /= es;
                    }
                    match self.new_bound(v, aux, elem, arg!(4) as u64) {
                        Ok(n) => Some(n),
                        Err(_) if aux & new_aux::NOTHROW != 0 => None,
                        Err(f) => fail!(f),
                    }
                } else {
                    Some(u64::MAX)
                };
                let raw = match bound {
                    None => 0,
                    Some(n) => match self.new_alloc(elem, n, aux, origin) {
                        Ok(p) => p,
                        Err(f) => fail!(f),
                    },
                };
                reg!(0) = u128::from(raw);
                pc += 6;
            }
            Op::Delete => {
                let mut raw = reg!(0) as u64;
                let mut pointee = OTy(arg!(1) as u32);
                let kind = match aux & 9 {
                    0 => DynKind::New,
                    1 => DynKind::ArrayNew,
                    _ => DynKind::Std,
                };
                if kind == DynKind::New && raw != 0 {
                    match self.delete_base(raw, pointee, aux & 4 != 0) {
                        Ok(Some((r, t))) => (raw, pointee) = (r, t),
                        Ok(None) => {}
                        Err(f) => fail!(f),
                    }
                }
                if kind != DynKind::Std
                    && let Err(f) = self.delete_life(raw)
                {
                    fail!(f);
                }
                // aux 2: the checks only (before the destructors run).
                let r = if aux & 2 != 0 {
                    self.mem.heap_check(raw, kind, Some(pointee), &self.types).map(drop)
                } else {
                    self.mem.heap_free(raw, kind, Some(pointee), &self.types)
                };
                if let Err(f) = r {
                    fail!(Fault::Mem { f, ak: ak::READ, ty: pointee, p: raw });
                }
                pc += 3;
            }
            Op::Placement => {
                let elem = OTy(arg!(1) as u32);
                let ty = if aux & new_aux::ARRAY != 0 {
                    match self.new_bound(reg!(3), aux, elem, arg!(4) as u64) {
                        Ok(n) => self.types.array_of(elem, n),
                        Err(f) => fail!(f),
                    }
                } else {
                    elem
                };
                // A `void *` argument: the subobject type its
                // conversion from `T *` recorded (`kbc::VOID_TAG`).
                let stat = match arg!(2) as u32 {
                    crate::kbc::bi::DYN => (reg!(0) >> 64) as u32,
                    s => s.wrapping_add(1),
                };
                let stat = stat.checked_sub(1).map(OTy);
                if aux & new_aux::BOUND == 0 {
                    if let Err(f) = self.placement(reg!(0) as u64, ty, stat, aux) {
                        fail!(f);
                    }
                    reg!(0) = u128::from(reg!(0) as u64);
                }
                pc += 6;
            }
            Op::PushCleanup => {
                let raw = reg!(0) as u64;
                if let Err(f) = self.push_cleanup(raw, arg!(1) as u32, OTy(arg!(2) as u32), aux, pc)
                {
                    fail!(f);
                }
                pc += 4;
            }
            Op::MakeConst => {
                let p = Ptr::decode(reg!(0) as u64);
                if reg!(0) != 0
                    && let Ok(a) = self.mem.alloc_of(p)
                {
                    let fl = a.flags | flags::CONST;
                    self.mem.set_flags(p, fl);
                }
                pc += 2;
            }
            Op::SetActive => {
                let raw = reg!(0) as u64;
                self.revive_union(raw, OTy(arg!(2) as u32));
                if let Err(f) =
                    self.set_active(raw, arg!(1) as u32, OTy(arg!(2) as u32), aux & 1 != 0)
                {
                    fail!(f);
                }
                pc += 4;
            }
            Op::ActiveCheck => {
                let raw = reg!(0) as u64;
                if let Err(f) = self.active_check(raw, arg!(1) as u32, OTy(arg!(2) as u32), aux) {
                    fail!(f);
                }
                pc += 4;
            }
            Op::MpCast => {
                let v = reg!(0);
                let class = DeclId::new(arg!(1) as u32);
                match self.mp_cast(b, v, class, aux == 1) {
                    Some(r) => reg!(0) = r,
                    None => fail!(Fault::Invalid),
                }
                pc += 3;
            }
            Op::MpAddr => {
                let (raw, v) = (reg!(1) as u64, reg!(2));
                match self.mp_addr(raw, v, aux == 1) {
                    Ok(q) => reg!(0) = u128::from(q),
                    Err(f) => fail!(f),
                }
                pc += 4;
            }
            Op::MpActive => {
                if let Some(e) = self.mp.get(reg!(1))
                    && let Some(cls) = e.union
                    && let Some((_, _, idx)) = e.field
                {
                    // A union's members are all at its offset 0.
                    if let Err(f) = self.active_check(reg!(0) as u64, idx, cls, aux) {
                        fail!(f);
                    }
                }
                pc += 3;
            }
            Op::MpCmp => {
                // [expr.eq]/4.1-2: a comparison with the null member
                // pointer is specified.
                let both = reg!(0) != 0 && reg!(1) != 0;
                for v in [reg!(0), reg!(1)].into_iter().filter(|_| both) {
                    if let Some(e) = self.mp.get(v)
                        && e.virt
                    {
                        fail!(Fault::MpVirtCmp { member: e.member });
                    }
                }
                pc += 3;
            }
            Op::RefChk => {
                let (raw, ty) = (reg!(0) as u64, OTy(arg!(1) as u32));
                let p = Ptr::decode(raw);
                let past = raw != 0
                    && !p.is_fn()
                    && self.types.size(ty) != 0
                    && self
                        .mem
                        .alloc_of(p)
                        .is_ok_and(|a| p.past || u64::from(p.off) >= u64::from(a.size));
                if past {
                    fail!(Fault::Mem { f: MemFault::PastEnd, ak: ak::READ, ty, p: raw });
                }
                pc += 3;
            }
            Op::EndParam => {
                let raw = reg!(0) as u64;
                if aux & 1 != 0 && self.run.end_param != raw {
                    let depth = self.stack.len() as u32;
                    let n = self.cleanups.len();
                    // The registration is near the top: below it only the
                    // temporaries of the arguments after it.
                    let at = self
                        .cleanups
                        .iter()
                        .rev()
                        .take_while(|c| c.depth == depth)
                        .position(|c| c.this == raw);
                    if let Some(i) = at {
                        let c = self.cleanups.remove(n - 1 - i);
                        self.cleanups.push(c);
                        self.run.end_param = raw;
                        match self.run_cleanup(b, pc, pc, fb, false) {
                            Ok(Some(x)) => return Ok(x),
                            Ok(None) => {}
                            Err(e) => return Err(e),
                        }
                    }
                }
                self.run.end_param = 0;
                if let Err(f) = self.end_param_life(raw, aux & 1 != 0) {
                    fail!(f);
                }
                pc += 2;
            }
            Op::MpFn => match self.mp.get(reg!(1)) {
                Some(e) if e.virt => {
                    let (m, cls) = (e.member, e.cls);
                    let adj = arg!(3);
                    match self.vcall(b, m, cls, reg!(2) as u64) {
                        Ok((f, this, cov)) => {
                            reg!(0) = u128::from(crate::mem::fn_ptr(f));
                            reg!(2) = u128::from(this);
                            // A covariant return is adjusted as by a
                            // direct call ([class.virtual]/8).
                            if adj as u32 != u32::MAX {
                                self.regs[fb + adj] = cov;
                            } else if cov != 0 {
                                fail!(Fault::Invalid);
                            }
                        }
                        Err(f) => fail!(f),
                    }
                    pc += 5;
                }
                Some(e) => {
                    reg!(0) = u128::from(crate::mem::fn_ptr(e.fn_index));
                    let adj = arg!(3);
                    if adj as u32 != u32::MAX {
                        self.regs[fb + adj] = 0;
                    }
                    pc += 5;
                }
                None => fail!(Fault::Invalid),
            },
            Op::SetVptr => {
                let cls = OTy(arg!(1) as u32);
                if let Err(f) = self.set_vptrs(b, reg!(0) as u64, cls, aux & 1 != 0) {
                    fail!(f);
                }
                pc += 3;
            }
            Op::CovAdj => {
                let p = reg!(0) as u64;
                if p != 0 {
                    match virt::cov_adj(reg!(1)) {
                        virt::CovAdj::Off(o) => {
                            let off = i64::from(Ptr::decode(p).off) + o;
                            reg!(0) = u128::from(Ptr::with_off(p, off as u64, false));
                        }
                        virt::CovAdj::VBase(cls, k) => match self.sub_addr(b, p, cls, k) {
                            Some(q) => reg!(0) = u128::from(q),
                            None => fail!(Fault::Invalid),
                        },
                    }
                }
                pc += 3;
            }
            Op::DynCast => {
                let raw = reg!(1) as u64;
                let stat = OTy(arg!(2) as u32);
                let t = arg!(3) as u32;
                let target = (t != u32::MAX).then_some(OTy(t));
                match self.dyn_cast(b, raw, stat, target, aux & 1 != 0) {
                    Ok(r) => reg!(0) = u128::from(r),
                    Err(f) => fail!(f),
                }
                pc += 5;
            }
            Op::TypeId => {
                let ti = OTy(arg!(2) as u32);
                let r = if aux & 1 == 0 {
                    let Some(q) = QualType::from_raw(arg!(1) as u32) else { fail!(Fault::Invalid) };
                    self.type_info(q, ti)
                } else {
                    let (raw, cls) = (reg!(1) as u64, OTy(arg!(3) as u32));
                    match self.type_info_dyn(b.ast(), raw, cls, ti) {
                        Ok(r) => r,
                        Err(Fault::UnknownDynamic { .. })
                            if let Some(q) = self.declared_dynamic_type(raw, cls) =>
                        {
                            self.type_info(q, ti)
                        }
                        Err(f) => fail!(f),
                    }
                };
                reg!(0) = u128::from(r);
                pc += 5;
            }
            Op::IsConstEval => {
                if aux & 2 != 0 {
                    // `__kiln_plainly_constant_evaluated()` (WP-473).
                    reg!(0) = u128::from(self.plain());
                } else if aux & 4 != 0 {
                    // A constexpr local's initializer: its own
                    // evaluation, never plainly (spec/consteval-blocks.md §3).
                    reg!(0) = u128::from(std::mem::take(&mut self.run.plain));
                } else if aux & 8 != 0 {
                    self.run.plain = reg!(0) != 0;
                } else if aux & 16 != 0 {
                    // `if consteval` ([stmt.if]/4).
                    reg!(0) = u128::from(self.run.const_context);
                } else {
                    let cc = self.run.const_context;
                    reg!(0) = u128::from(cc);
                    if cc && (!self.run.silent || self.run.cev_warn) {
                        self.always_true_diag(b, pc, aux);
                    }
                }
                pc += 2;
            }
            Op::Wide => {
                let regs = [reg!(0), reg!(1), reg!(2)];
                match self.exec_wide(regs, OTy(arg!(3) as u32), arg!(4) as u32, aux) {
                    Ok(Some(v)) => reg!(0) = v,
                    Ok(None) => {}
                    Err(f) => fail!(f),
                }
                pc += 6;
            }
            Op::Cx => {
                let v = [reg!(2), reg!(3), reg!(4), reg!(5)];
                let sub = aux & 3;
                let r = if aux >> 6 & 1 == 0 {
                    Some(complex::float_op(sub, float_of(aux >> 2 & 0xF), v))
                } else {
                    complex::int_op(sub, aux >> 7 & 0xFF, aux >> 15 & 1 != 0, v)
                };
                let Some([x, y]) = r else {
                    let err = IntErr::DivZero;
                    fail!(Fault::Int { err, op: Op::IDiv, a: v[0], b: 0, aux: 0 });
                };
                reg!(0) = x;
                reg!(1) = y;
                pc += 7;
            }
            Op::Speculate => {
                let func = self.stack[self.stack.len() - 1].func;
                match self.speculate(b, func, pc + 4, fb, arg!(1), aux & 1 != 0) {
                    // clang `EvaluateBuiltinConstantP` outside a
                    // constant context: a lax fold (`EvaluateAsInt`)
                    // of a non-constant argument fails rather than
                    // folding to 0.
                    Ok(false) if aux & 2 != 0 && self.run.lax && !self.run.const_context => {
                        fail!(Fault::Invalid)
                    }
                    Ok(c) => reg!(0) = u128::from(c),
                    Err(f) => fail!(f),
                }
                pc = arg!(2);
            }
            Op::Fold => {
                if aux == 1 {
                    reg!(0) = u128::from(self.run.fold);
                    self.run.fold = true;
                } else {
                    self.run.fold = reg!(0) != 0;
                }
                pc += 2;
            }
            Op::ContractSkip => {
                if self.run.contracts_off {
                    self.run.impure += 1;
                    pc = arg!(0);
                } else {
                    pc += 2;
                }
            }
            Op::ContractFail => {
                if let Err(f) =
                    self.contract_violation(b, pc, arg!(0) as u32, aux & 1 != 0, aux & 2 != 0)
                {
                    fail!(f);
                }
                pc += 2;
            }
            Op::ContractTrial => {
                let func = self.stack[self.stack.len() - 1].func;
                let r = match self.trial(b, func, pc + 4, fb, arg!(0), TrialMode::Transactional) {
                    Ok(Some(v)) if v != 0 => Ok(()),
                    Ok(v) => self.contract_violation(b, pc, arg!(2) as u32, true, v.is_none()),
                    Err(f) => Err(f),
                };
                if let Err(f) = r {
                    fail!(f);
                }
                pc = arg!(1);
            }
            Op::LabelAddr => {
                let Some(e) = ExprId::try_new(arg!(1) as u32) else { fail!(Fault::Invalid) };
                reg!(0) = u128::from(self.label_addr(b, e));
                pc += 3;
            }
            Op::VoidCast if aux == crate::kbc::VOID_TAG => {
                let raw = reg!(1) as u64;
                let tag = if raw == 0 { 0 } else { u128::from(arg!(2) as u32) + 1 };
                reg!(0) = u128::from(raw) | tag << 64;
                pc += 4;
            }
            Op::VoidCast => {
                let (raw, tag) = (reg!(1) as u64, (reg!(1) >> 64) as u32);
                match self.void_cast(raw, tag, OTy(arg!(2) as u32)) {
                    Ok(()) => reg!(0) = u128::from(raw),
                    Err(f) => fail!(f),
                }
                pc += 4;
            }
            Op::Cce => {
                if !self.run.fold {
                    fail!(Fault::Trap(arg!(2) as u32));
                }
                match self.cce(aux, reg!(1)) {
                    Some(v) => {
                        if aux != cce::CHECK {
                            reg!(0) = v;
                        }
                    }
                    None => fail!(Fault::Trap(arg!(2) as u32)),
                }
                pc += 4;
            }
            Op::TryEnter => {
                let depth = self.stack.len() as u32;
                let (this, at) = (arg!(0) as u64, pc as u32);
                let c = Cleanup { this, ty: OTy(0), func: exc::TRY_MARK, pc: at, depth };
                self.cleanups.push(c);
                pc += 2;
            }
            Op::Throw | Op::Unwind => (pc, fb) = self.throw_op(b, op, pc, fb, aux)?,
            Op::Catch => match self.catch_match(b, aux, OTy(arg!(1) as u32)) {
                Ok(Some(v)) => {
                    reg!(0) = v;
                    pc += 4;
                }
                Ok(None) => pc = arg!(2),
                Err(f) => fail!(f),
            },
            Op::CatchBegin => {
                if let Err(f) = self.catch_begin(pc) {
                    fail!(f);
                }
                pc += 1;
            }
            _ => unreachable!("{op:?} is dispatched in run_loop"),
        }
        Ok((pc, fb))
    }

    /// Pops the innermost registered destruction and enters its
    /// destructor from the instruction at `pc` (which executes again on
    /// return, or `ret` when `unwinding`); the frame of the running
    /// function is at `fb`. An [`UNWIND_ONLY`] registration is dropped
    /// unless `unwinding`.
    #[inline(never)]
    fn run_cleanup(
        &mut self,
        b: &mut dyn SemaBridge,
        pc: usize,
        ret: usize,
        fb: usize,
        unwinding: bool,
    ) -> Result<Option<(usize, usize)>, (Fault, usize)> {
        let Some(mut c) = self.cleanups.pop() else { return Ok(None) };
        if c.pc & UNWIND_ONLY != 0 {
            if !unwinding {
                return Ok(None);
            }
            c.pc &= !UNWIND_ONLY;
        }
        if c.func == exc::LIFE_CHECK {
            // A trivially destructible base: only its lifetime is checked
            // (not while unwinding, which destroys no constructed object's
            // base twice).
            if !unwinding {
                self.cleanup_life(&c, fb).map_err(|f| (f, self.cleanup_fault_pc(&c, pc)))?;
            }
            return Ok(None);
        }
        if c.func >= exc::SENTINEL {
            // A try-block's marker or an exception's end (`vm/exc.rs`).
            match self.exc_cleanup(c).map_err(|f| (f, pc))? {
                Some(d) => c = d,
                None => return Ok(None),
            }
        } else {
            self.cleanup_life(&c, fb).map_err(|f| (f, self.cleanup_fault_pc(&c, pc)))?;
        }
        // A const object is modifiable during its destruction
        // ([class.dtor]/5).
        let p = Ptr::decode(c.this);
        if c.this != 0
            && let Ok(a) = self.mem.alloc_of(p)
        {
            let fl = a.flags & !flags::CONST;
            self.mem.set_flags(p, fl);
        }
        let cur = self.stack.last().map_or(0, |f| f.func);
        let base = self.funcs[cur as usize].nregs as usize;
        self.grow_regs(fb + base + 2);
        self.regs[fb + base] = u128::from(c.this);
        self.enter(b, c.func, c.pc as usize, ret, fb, base).map_err(|f| (f, c.pc as usize))
    }

    /// `Op::PushCleanup`: registers the destruction of the object (or
    /// each element of the array) of type `ty` at `raw`.
    #[inline(never)]
    fn push_cleanup(
        &mut self,
        raw: u64,
        func: u32,
        ty: OTy,
        aux: u32,
        pc: usize,
    ) -> Result<(), Fault> {
        // A trivially destructible base can be outside its lifetime only
        // if a constructor left it unconstructed (`Memory::set_unconstructed`).
        if raw == 0 || (func == exc::LIFE_CHECK && self.mem.unconstructed.is_empty()) {
            return Ok(());
        }
        let (elem, n) = if aux & 1 != 0 {
            let p = Ptr::decode(raw);
            let n = match self.mem.alloc_of(p) {
                Ok(a) => u64::from(a.size) / self.types.size(ty).max(1),
                Err(f) => return Err(Fault::Mem { f, ak: ak::READ, ty, p: raw }),
            };
            (ty, n)
        } else {
            let (mut t, mut n) = (ty, 1u64);
            while let TyKind::Array { elem, n: k } = self.types.kind(t) {
                t = elem;
                n *= k;
            }
            (t, n)
        };
        let es = self.types.size(elem);
        let off = u64::from(Ptr::decode(raw).off);
        let depth = self.stack.len() as u32;
        let pc = pc as u32 | if aux & 4 != 0 { UNWIND_ONLY } else { 0 };
        self.cleanups.reserve(n as usize);
        for i in 0..n {
            let this = if i == 0 { raw } else { Ptr::with_off(raw, off + i * es, false) };
            self.cleanups.push(Cleanup { this, ty: elem, func, pc, depth });
        }
        Ok(())
    }

    /// Enters function `callee` (called by the instruction at `pc`,
    /// returning to `ret_pc`, arguments at `fb + base`): the new pc and
    /// frame base, or `None` if a memoized result was used.
    // Always inlined: `run_cleanup` is a second caller, and an
    // outlined `enter` costs ~6% of the instructions of a call-heavy
    // evaluation (bench/oven/churn-tree.cpp).
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn enter(
        &mut self,
        b: &mut dyn SemaBridge,
        callee: u32,
        pc: usize,
        ret_pc: usize,
        fb: usize,
        base: usize,
    ) -> Result<Option<(usize, usize)>, Fault> {
        if matches!(self.funcs[callee as usize].state, FState::Stub | FState::Undefined) {
            self.hook_compile(b, callee);
        }
        let f = &self.funcs[callee as usize];
        if f.state != FState::Ready {
            return Err(Fault::Callee(callee));
        }
        let active = self.active_calls();
        if self.run.potential.is_some_and(|n| active >= n) {
            return Err(Fault::Quiet);
        }
        if active >= self.budget.depth {
            return Err(Fault::Depth);
        }
        let nb = fb + base;
        let memo = f.memo && self.memo.enabled();
        let (entry, nregs) = (f.entry as usize, f.nregs as usize);
        if memo {
            // A call's result can depend on the evaluation context
            // (`__builtin_is_constant_evaluated` in a callee): the context
            // is part of the key, with the plain bit (consteval blocks).
            let func =
                callee | u32::from(self.run.const_context) << 31 | u32::from(self.plain()) << 30;
            let mut key = MemoKey { func, args: [0; MAX_ARGS] };
            for (i, p) in f.params.iter().enumerate() {
                key.args[i] = self.regs[nb + p.reg as usize];
            }
            let c = self.run.counters;
            if let Some(v) = self.memo.get(&key) {
                let (live, arena) = (self.mem.live_bytes(), self.mem.arena());
                if c.steps + v.steps <= self.budget.steps
                    && active + v.depth <= self.budget.depth
                    && arena + v.arena <= self.mem.limit
                {
                    self.run.counters.steps += v.steps;
                    self.run.counters.calls += v.calls;
                    self.mem.allocated += v.bytes;
                    self.mem.peak = self.mem.peak.max(live + v.peak);
                    self.mem.arena_peak = self.mem.arena_peak.max(arena + v.arena);
                    self.run.maxd = self.run.maxd.max(active + v.depth);
                    self.regs[nb] = v.result;
                    return Ok(None);
                }
            }
            let (live, arena) = (self.mem.live_bytes(), self.mem.arena());
            self.run.pending.push(Pending {
                key,
                steps0: c.steps,
                calls0: c.calls,
                bytes0: self.mem.allocated,
                live0: live,
                arena0: arena,
                peak_saved: self.mem.peak,
                arena_saved: self.mem.arena_peak,
                maxd_saved: self.run.maxd,
                d0: active + 1,
                impure0: self.run.impure,
                heap0: self.mem.heap_ops,
            });
            self.mem.peak = live;
            self.mem.arena_peak = arena;
            self.run.maxd = active + 1;
        }
        self.run.counters.calls += 1;
        self.run.maxd = self.run.maxd.max(active + 1);
        self.grow_regs(nb + nregs + 1);
        let mark = self.mem.mark();
        let serial = self.mem.next_frame();
        self.stack.push(Frame {
            func: callee,
            ret_pc: ret_pc as u32,
            fb: nb as u32,
            mark,
            call_pc: pc as u32,
            memo,
            serial,
        });
        if self.hooks.calls {
            self.hook_call_enter(b, callee);
        }
        Ok(Some((entry, nb)))
    }

    /// Under `RunState::ub_check`, the wrapped result of a signed `+`,
    /// `-` or `*` (`op`; a negation is `0 - a`) that overflowed (`err`),
    /// recorded; `None` otherwise (the evaluation fails).
    #[cold]
    #[inline(never)]
    fn wrap_overflow(
        &mut self,
        pc: usize,
        err: IntErr,
        op: Op,
        aux: u32,
        a: u128,
        b: u128,
    ) -> Option<u128> {
        let width = aux & 0xFF;
        if !self.run.ub_check
            || err != IntErr::Overflow
            || aux >> 8 & 1 == 0
            || !matches!(op, Op::IAdd | Op::ISub | Op::IMul)
        {
            return None;
        }
        let r = match op {
            Op::IAdd => a.wrapping_add(b),
            Op::ISub => a.wrapping_sub(b),
            _ => a.wrapping_mul(b),
        };
        let v = canon(r, width, true);
        self.run.overflows.push((pc as u32, v as i128));
        Some(v)
    }

    fn finish_memo(&mut self, result: u128) {
        let Some(p) = self.run.pending.pop() else { return };
        let c = self.run.counters;
        let val = MemoVal {
            result,
            steps: c.steps - p.steps0,
            calls: c.calls - p.calls0,
            bytes: self.mem.allocated - p.bytes0,
            peak: self.mem.peak.saturating_sub(p.live0),
            arena: self.mem.arena_peak.saturating_sub(p.arena0),
            depth: self.run.maxd.saturating_sub(p.d0) + 1,
        };
        // A call that allocated or freed heap memory is not pure (a cached
        // result would skip the allocation's accounting, or return a
        // pointer to an object a later delete frees).
        // A lax fold's result may depend on a read a strict evaluation
        // rejects.
        if self.run.impure == p.impure0 && self.mem.heap_ops == p.heap0 && !self.run.lax {
            self.memo.insert(p.key, val);
        }
        self.mem.peak = self.mem.peak.max(p.peak_saved);
        self.mem.arena_peak = self.mem.arena_peak.max(p.arena_saved);
        self.run.maxd = self.run.maxd.max(p.maxd_saved);
    }

    /// Active calls of the running evaluation, counting those of the
    /// evaluation contexts it is nested in (`RunState::depth_base`).
    pub(crate) fn active_calls(&self) -> u32 {
        let own = self.stack.len().saturating_sub(self.run.entry + 1) as u32;
        own + self.run.depth_base
    }

    // ---- transient allocation ---------------------------------------------

    /// The element count of an array `new` with bound register value `v`,
    /// checked in clang's order ([expr.new]/8): negative, too large for the
    /// address space, beyond the step limit, smaller than the explicit
    /// initializers.
    fn new_bound(&self, v: u128, aux: u32, elem: OTy, k: u64) -> Result<u64, Fault> {
        let signed = aux & new_aux::SIGNED != 0;
        let bad = |err| Fault::NewBound { err, v, signed, k };
        if signed && (v as i128) < 0 {
            return Err(bad(BoundErr::Negative));
        }
        // clang's `ConstantArrayType::getMaxSizeBits`: 61 for a 64-bit
        // `size_t`.
        let es = u128::from(self.types.size(elem));
        if v > u128::from(u32::MAX) || 128 - (v * es).leading_zeros() > 61 {
            return Err(bad(BoundErr::TooLarge));
        }
        if v > u128::from(self.budget.steps) {
            return Err(bad(BoundErr::Limit));
        }
        if v < u128::from(k) {
            return Err(bad(BoundErr::TooSmall));
        }
        Ok(v as u64)
    }

    /// A heap allocation of `elem` (`n == u64::MAX`) or `elem[n]`,
    /// zero-filled with [`new_aux::ZERO`].
    pub(crate) fn new_alloc(
        &mut self,
        elem: OTy,
        n: u64,
        aux: u32,
        origin: Origin,
    ) -> Result<u64, Fault> {
        let (ty, fl) = if n == u64::MAX {
            (elem, 0)
        } else if aux & new_aux::STD != 0 {
            (self.types.array_of(elem, n), flags::STD_ALLOC)
        } else {
            (self.types.array_of(elem, n), flags::ARRAY_NEW)
        };
        let info = *self.types.info(ty);
        let fl = fl
            | if info.has_union { flags::HAS_UNION } else { 0 }
            | if info.has_volatile || aux & new_aux::VOLATILE != 0 { flags::VOLATILE } else { 0 };
        let raw = self.mem.heap_alloc(ty, info.size, fl, origin).map_err(|f| match f {
            MemFault::Speculative => Fault::Mem { f, ak: ak::INIT, ty, p: 0 },
            _ => Fault::Memory,
        })?;
        if aux & new_aux::ZERO != 0 && info.size > 0 {
            let (st, abs, _) = self.mem.check(raw, info.size, true).map_err(|f| Fault::Mem {
                f,
                ak: ak::INIT,
                ty,
                p: raw,
            })?;
            self.mem.zero(st, abs, info.size);
            if info.has_union {
                self.zero_unions(raw, 0, ty);
            }
        }
        Ok(raw)
    }

    // ---- pointers ---------------------------------------------------------

    /// The array extent a pointer to `elem` lies in: (start, n, cur index,
    /// is an array).
    // Always inlined into `ptr_add` (the `PtrAdd` op) and `ptr_sub`:
    // outlined, its `Result` returns through memory on every step.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn extent_of(&self, raw: u64, elem: OTy) -> Result<(u64, u64, u64, bool), Fault> {
        let p = Ptr::decode(raw);
        let Some(a) = self.alloc_any(p) else {
            return Err(Fault::Mem {
                f: if p.is_fn() { MemFault::FnPtr } else { MemFault::Stale },
                ak: ak::READ,
                ty: elem,
                p: raw,
            });
        };
        let es = self.types.size(elem).max(1);
        let off = u64::from(p.off);
        if let TyKind::Array { elem: e, n } = self.types.kind(a.ty)
            && self.types.alike(e, elem)
        {
            return Ok((0, n, off / es, true));
        }
        match self.types.extent(a.ty, off, p.past, elem) {
            Some((start, n, arr)) => Ok((start, n, (off - start) / es, arr)),
            None => Err(Fault::ArrayIndex { idx: 1, n: 1, array: false }),
        }
    }

    /// `p + i` for a pointer `raw` to `elem` (spec/oven.md "Pointer
    /// arithmetic"): the step within an array allocation of `elem`
    /// ([`Oven::ptr_add_array`]), else [`Oven::ptr_add_slow`].
    pub(crate) fn ptr_add(&self, raw: u64, i: i128, elem: OTy) -> Result<u64, Fault> {
        match self.ptr_add_array(raw, i, false, elem, None) {
            Some(p) => Ok(p),
            None => match self.ptr_add_front(raw, i, elem) {
                Some(p) => Ok(p),
                None => self.ptr_add_slow(raw, i, elem),
            },
        }
    }

    /// [`Oven::ptr_add_slow`]'s step when the type table's whole-array
    /// table already holds the extent `raw` lies in ([`Types::extent_hit`]:
    /// an element of an array member, such as an `inplace_vector`'s
    /// `union { T data[N]; }`) and the new index stays in `[0, n]`
    /// (WP-760). `None` for anything else, the slow path then deciding.
    #[inline]
    fn ptr_add_front(&self, raw: u64, i: i128, elem: OTy) -> Option<u64> {
        let a = self.mem.alloc_raw(raw)?;
        let p = Ptr::decode(raw);
        let off = u64::from(p.off);
        let (start, n) = self.types.extent_hit(a.ty, off, p.past, elem)?;
        let es = self.types.size(elem).max(1);
        let new = i128::from((off - start) / es).checked_add(i)?;
        if new < 0 || new > i128::from(n) {
            return None;
        }
        let new = new as u64;
        Some(Ptr::with_off(raw, start + new * es, new == n))
    }

    /// The `PtrAdd` op's inline step: `raw + v` within an array allocation
    /// of `elem` ([`Oven::ptr_add_array`]) when the compiler put the
    /// element size's shift in `aux` ([`ptr_add_aux`]: a power of two that
    /// can no longer change, and an index whose register value is the
    /// index). `None` sends the op to [`Oven::ptr_add_op`].
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn ptr_add_step(&self, raw: u64, v: u128, aux: u32, elem: OTy) -> Option<u64> {
        let field = aux >> ptr_add_aux::SHIFT & ptr_add_aux::MASK;
        if field == 0 {
            return None;
        }
        self.ptr_add_array(raw, v as i128, aux >> 9 & 1 != 0, elem, Some(field - 1))
    }

    /// The `PtrAdd` op's step within an array member the type table's
    /// whole-array table holds ([`Oven::ptr_add_front`]: an
    /// `inplace_vector`'s `&data[n]`), when its aux says the register
    /// value is the index ([`ptr_add_aux`]), off [`Oven::ptr_add_step`]
    /// (WP-760). Out of line and not cold: the hot loop keeps one call.
    /// `None` sends the op to [`Oven::ptr_add_op`].
    #[inline(never)]
    fn ptr_add_member(&self, raw: u64, v: u128, aux: u32, elem: OTy) -> Option<u64> {
        if aux >> ptr_add_aux::SHIFT & ptr_add_aux::MASK == 0 {
            return None;
        }
        let i = v as i128;
        let i = if aux >> 9 & 1 != 0 { i.checked_neg()? } else { i };
        self.ptr_add_front(raw, i, elem)
    }

    /// The `PtrAdd` op off its array step: the index `v` read as its
    /// aux says (width, signedness, negated), then [`Oven::ptr_add`].
    #[cold]
    #[inline(never)]
    fn ptr_add_op(&self, raw: u64, v: u128, aux: u32, elem: OTy) -> Result<u64, Fault> {
        let mut i = if aux >> 8 & 1 != 0 || (aux & 0xFF) < 128 {
            v as i128
        } else {
            i128::try_from(v).unwrap_or(i128::MAX)
        };
        if aux >> 9 & 1 != 0 {
            i = i.saturating_neg();
        }
        self.ptr_add(raw, i, elem)
    }

    /// [`Oven::ptr_add`] of `i`, negated when `neg`, in the common case,
    /// returning what [`Oven::ptr_add_slow`] returns for it: `i` is 0, or
    /// `raw` points into an allocation (live or not, as
    /// [`crate::mem::Memory::alloc_any`] finds it) whose type is an array
    /// of exactly `elem` and the new index stays in `[0, n]`. `None` for
    /// anything else, the slow path then deciding (the fault included).
    /// `sh` is the element size's shift when the compiler knew it
    /// ([`ptr_add_aux`]), else `None` and the size is read from the type
    /// table. The wrapped negation differs from the slow path's saturating
    /// one only for `i128::MIN`, which this step refuses either way.
    // Always inlined into the `PtrAdd` arm and `ptr_add`: a handful of
    // compares and one type-table read.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn ptr_add_array(
        &self,
        raw: u64,
        i: i128,
        neg: bool,
        elem: OTy,
        sh: Option<u32>,
    ) -> Option<u64> {
        if i == 0 {
            return Some(raw);
        }
        let a = self.mem.alloc_raw(raw)?;
        let TyKind::Array { elem: e, n } = self.types.kind(a.ty) else {
            return None;
        };
        if e != elem {
            return None;
        }
        let off = raw & 0xFFFF_FFFF;
        let es = if sh.is_some() { 0 } else { self.types.size(elem).max(1) };
        let cur = match sh {
            Some(sh) => off >> sh,
            None => off / es,
        };
        // Wrapping as the slow path's sum does: `cur` is below 2^32, so
        // a wrapped sum is negative and refused here.
        let cur = i128::from(cur);
        let new = if neg { cur.wrapping_sub(i) } else { cur.wrapping_add(i) };
        if new as u128 > u128::from(n) {
            return None;
        }
        let new = new as u64;
        let off = match sh {
            Some(sh) => new << sh,
            None => new * es,
        };
        Some(Ptr::with_off(raw, off, new == n))
    }

    /// [`Oven::ptr_add`] by the extent of the subobject `raw` lies in, the
    /// fault when the result leaves it.
    fn ptr_add_slow(&self, raw: u64, i: i128, elem: OTy) -> Result<u64, Fault> {
        if raw == 0 {
            return if i == 0 { Ok(0) } else { Err(Fault::NullArith) };
        }
        if i == 0 {
            return Ok(raw);
        }
        let (start, n, cur, array) = match self.extent_of(raw, elem) {
            Ok(x) => x,
            Err(_) if Ptr::is_nowhere(raw) => return self.nowhere_add(raw, i, elem),
            Err(f) => return Err(f),
        };
        let new = i128::from(cur).wrapping_add(i);
        if new < 0 || new > i128::from(n) {
            return Err(Fault::ArrayIndex { idx: new, n, array });
        }
        let es = self.types.size(elem).max(1);
        let new = new as u64;
        Ok(Ptr::with_off(raw, start + new * es, new == n))
    }

    pub(crate) fn ptr_sub(&self, p: u64, q: u64, elem: OTy) -> Result<i64, Fault> {
        if p == 0 && q == 0 {
            return Ok(0);
        }
        if p == 0 || q == 0 || Ptr::alloc_key(p) != Ptr::alloc_key(q) {
            if let Some(d) = self.nowhere_sub(p, q, elem) {
                return d;
            }
            // Different complete objects.
            return Err(Fault::PtrArith { p, q, pointee: elem });
        }
        let Ok((sp, _, ip, _)) = self.extent_of(p, elem) else {
            return self.nowhere_sub(p, q, elem).unwrap_or(Err(Fault::PtrSub));
        };
        let (sq, _, iq, _) = self.extent_of(q, elem).map_err(|_| Fault::PtrSub)?;
        if sp != sq {
            return Err(Fault::PtrSub);
        }
        Ok(ip as i64 - iq as i64)
    }

    /// Whether function `f`, called through a function pointer a fold
    /// cast (`Op::Cce`) by the indirect call at `pc`, has the type the
    /// call names, ignoring the exception specification (clang
    /// `EvaluateCall`'s `hasSameFunctionTypeIgnoringExceptionSpec`): a
    /// pointer cast back to its function's type calls it (spec/oven.md
    /// "Pointers that point at nothing").
    #[cold]
    fn cast_callee_matches(&self, b: &dyn SemaBridge, pc: usize, f: u32) -> bool {
        let ast = b.ast();
        let Some(e) = self.site(pc).e.filter(|&e| ast.exprs.get(e).kind() == ExprKind::CallExpr)
        else {
            return false;
        };
        let Some(d) = self.funcs.get(f as usize).and_then(|x| x.decl) else { return false };
        let callee = ast.exprs.get(ast.exprs.callee(e)).ty();
        let Some(want) = ast.types.pointee(callee) else { return false };
        let Some(have) = ast.decls.get(d).ty() else { return false };
        let t = &ast.types;
        match (t.proto(want), t.proto(have)) {
            (Some(w), Some(h)) => {
                t.same_type(w.ret(), h.ret())
                    && w.variadic() == h.variadic()
                    && t.params(w).len() == t.params(h).len()
                    && t.params(w).iter().zip(t.params(h)).all(|(&x, &y)| t.same_type(x, y))
            }
            _ => false,
        }
    }

    /// The conversion `aux` (`kbc::cce`) of register value `v` by a cast
    /// a fold continues past (`Op::Cce`; clang's invalid designator); the
    /// result is not memoized. `None` fails the fold.
    #[cold]
    fn cce(&mut self, aux: u32, v: u128) -> Option<u128> {
        self.run.impure += 1;
        let raw = v as u64;
        match aux {
            cce::PTR => Some(if raw == 0 { 0 } else { u128::from(raw | NOWHERE) }),
            cce::INT_TO_PTR => match u32::try_from(v.cast_signed()) {
                Ok(0) => Some(0),
                Ok(n) => Some(u128::from(NOWHERE | u64::from(n))),
                Err(_) => None,
            },
            cce::PTR_TO_INT => {
                let p = Ptr::decode(raw);
                (raw == 0 || (Ptr::is_nowhere(raw) && p.slot == 0 && !p.past))
                    .then_some(u128::from(p.off))
            }
            _ => Some(v),
        }
    }

    /// [`Oven::ptr_add`] of a pointer that points at nothing (clang: an
    /// invalid designator only moves the offset).
    #[cold]
    fn nowhere_add(&self, raw: u64, i: i128, elem: OTy) -> Result<u64, Fault> {
        let es = i128::from(self.types.size(elem).max(1));
        let off = i128::from(Ptr::decode(raw).off) + i * es;
        match u32::try_from(off) {
            Ok(off) => Ok(Ptr::with_off(raw, u64::from(off), false)),
            Err(_) => Err(Fault::NullArith),
        }
    }

    /// [`Oven::ptr_sub`] when `p` or `q` points at nothing: the difference
    /// of their offsets within one object (or two integers cast to
    /// pointers); `None` for two pointers that do not.
    #[cold]
    fn nowhere_sub(&self, p: u64, q: u64, elem: OTy) -> Option<Result<i64, Fault>> {
        let (sp, sq) = Ptr::nowhere_slots(p, q)?;
        if sp != sq {
            return Some(Err(Fault::PtrArith { p, q, pointee: elem }));
        }
        let es = i64::from(self.types.size(elem).max(1) as u32);
        let d = i64::from(Ptr::decode(p).off) - i64::from(Ptr::decode(q).off);
        Some(if d % es == 0 { Ok(d / es) } else { Err(Fault::PtrSub) })
    }

    /// Whether static slot `slot` holds a string literal (the object of
    /// a pointer that points at nothing).
    fn is_literal_slot(&self, slot: u32) -> bool {
        self.mem.static_in_slot(slot).is_some_and(|a| matches!(a.origin, Origin::Str(..)))
    }

    fn is_literal(&self, raw: u64) -> bool {
        let p = Ptr::decode(raw);
        self.alloc_any(p).is_some_and(|a| matches!(a.origin, Origin::Str(..)))
    }

    /// clang 19's literal bases (`IsLiteralLValue`): a string literal or
    /// a label address.
    fn is_literal_or_label(&self, raw: u64) -> bool {
        let p = Ptr::decode(raw);
        self.alloc_any(p).is_some_and(|a| matches!(a.origin, Origin::Str(..) | Origin::Label(_)))
    }

    /// clang 22 notes a `std::allocator<T>::allocate` allocation at the
    /// call of `allocate` (its member name for a member call), not at the
    /// `operator new` inside it; `e` when there is no such call.
    #[inline(never)]
    fn std_alloc_site(&self, b: &mut dyn SemaBridge, e: ExprId) -> ExprId {
        let Some(f) = self.stack.last() else { return e };
        if f.call_pc == NO_CALL {
            return e;
        }
        let Some(ce) = self.site(f.call_pc as usize).e else { return e };
        let ast = b.ast();
        if ast.exprs.get(ce).kind() == ExprKind::CXXMemberCallExpr
            && let Some(&m) = ast.exprs.children(ce, &ast.lists).first()
        {
            return m;
        }
        ce
    }

    /// The bytes of the string literal `raw` points into without its
    /// terminator, and its character width; `None` for anything else.
    fn literal_bytes(&self, raw: u64) -> Option<(&[u8], usize)> {
        let ptr = Ptr::decode(raw);
        let a = self.alloc_any(ptr)?;
        if !matches!(a.origin, Origin::Str(..)) || ptr.space() != Sp::Static {
            return None;
        }
        let width = match self.types.kind(a.ty) {
            TyKind::Array { elem, .. } => self.types.size(elem) as usize,
            _ => return None,
        };
        let (b, n) = (a.base as usize, a.size as usize);
        let bytes = self.mem.stat.bytes.get(b..b + n)?;
        Some((&bytes[..n.saturating_sub(width)], width.max(1)))
    }

    /// clang 22's `ArePotentiallyOverlappingStringLiterals`: `p` and `q`
    /// point into string literals that, merged so the two pointers
    /// coincide, agree on every overlapping byte (terminators included).
    fn literals_may_overlap(&self, p: u64, q: u64) -> bool {
        let (Some((mut l, _)), Some((mut r, _))) = (self.literal_bytes(p), self.literal_bytes(q))
        else {
            return false;
        };
        let (lo, ro) = (i64::from(Ptr::decode(p).off), i64::from(Ptr::decode(q).off));
        let off = ro - lo;
        if off < 0 {
            let k = off.unsigned_abs() as usize;
            if l.len() < k {
                return false;
            }
            l = &l[k..];
        } else {
            let k = off as usize;
            if r.len() < k {
                return false;
            }
            r = &r[k..];
        }
        let (lw, rw) =
            (self.literal_bytes(p).map_or(1, |x| x.1), self.literal_bytes(q).map_or(1, |x| x.1));
        let (longer, shorter, width) = if l.len() > r.len() { (l, r, rw) } else { (r, l, lw) };
        for nb in 0..width {
            match longer.get(shorter.len() + nb) {
                None => break,
                Some(&0) => {}
                Some(_) => return false,
            }
        }
        longer.starts_with(shorter)
    }

    /// A pointer into an object of unknown identity (P2280).
    fn is_unknown(&self, raw: u64) -> bool {
        raw != 0
            && self
                .alloc_any(Ptr::decode(raw))
                .is_some_and(|a| matches!(a.origin, Origin::Unknown(_)))
    }

    /// One past the end of a complete object.
    fn past_complete(&self, raw: u64) -> bool {
        let p = Ptr::decode(raw);
        self.alloc_any(p).is_some_and(|a| p.off >= a.size && a.size != 0)
    }

    /// Whether the raw pointer `raw` is the address of (an object in) a
    /// weak declaration: a function or a namespace-scope or static
    /// variable that is `weak` or a `weakref` (clang `IsWeakLValue`).
    fn is_weak_ptr(&self, b: &dyn SemaBridge, raw: u64) -> bool {
        if raw == 0 {
            return false;
        }
        let p = Ptr::decode(raw);
        if !p.is_static() {
            return false;
        }
        let d = if p.is_fn() {
            self.funcs.get(p.off as usize).and_then(|f| f.decl)
        } else {
            // A pointer pointing at nothing keeps its slot, not its
            // generation; static slots are never reused.
            let a = if Ptr::is_nowhere(raw) {
                self.mem.statics.get((p.slot & !crate::mem::STATIC_BIT) as usize)
            } else {
                self.alloc_any(p)
            };
            match a.map(|a| a.origin) {
                Some(Origin::Global(d) | Origin::External(d) | Origin::Unknown(d)) => Some(d),
                _ => None,
            }
        };
        d.is_some_and(|d| b.ast().is_weak_decl(d))
    }

    /// `weak`: whether `p` and `q` are addresses of weak declarations
    /// ([`Self::is_weak_ptr`]).
    fn ptr_cmp(
        &self,
        p: u64,
        q: u64,
        pointee: OTy,
        c: u32,
        weak: (bool, bool),
    ) -> Result<bool, Fault> {
        let eq = c == cmp::EQ || c == cmp::NE;
        let same =
            (p == 0 && q == 0) || (p != 0 && q != 0 && Ptr::alloc_key(p) == Ptr::alloc_key(q));
        if same {
            let (a, b) = (Ptr::decode(p), Ptr::decode(q));
            return Ok(compare(c, a.off.cmp(&b.off)));
        }
        // clang: whether weak symbols end up at one address (or null) is
        // known only at run time (also through a cast that left the
        // pointer pointing at nothing, `(void *)&f`).
        if eq && (weak.0 || weak.1) {
            return Err(Fault::WeakCmp { p: if weak.0 { p } else { q }, pointee });
        }
        if let Some((sp, sq)) = Ptr::nowhere_slots(p, q) {
            // clang compares an invalid designator's base and offset: an
            // integer cast to a pointer has no base (as null).
            let (a, b) = (Ptr::decode(p), Ptr::decode(q));
            if sp == sq {
                return Ok(compare(c, a.off.cmp(&b.off)));
            }
            if !eq || (sp == 0 && a.off != 0) || (sq == 0 && b.off != 0) {
                return Err(Fault::PtrCmp { p, q, pointee });
            }
            return Ok(c == cmp::NE);
        }
        if !eq {
            return Err(Fault::PtrCmp { p, q, pointee });
        }
        if p != 0 && q != 0 {
            if self.clang_compat >= 22 {
                // clang 22: only two string literals whose merged storage
                // could put `p` and `q` at one address compare
                // unspecified; a literal is distinct from any other object.
                if self.literals_may_overlap(p, q) {
                    return Err(Fault::LiteralCmp);
                }
            } else if self.is_literal_or_label(p) || self.is_literal_or_label(q) {
                return Err(Fault::LiteralCmp);
            }
            if Ptr::decode(p).off == 0 && !Ptr::decode(p).is_fn() && self.past_complete(q) {
                return Err(Fault::PtrPastCmp { p: q, pointee });
            }
            if Ptr::decode(q).off == 0 && !Ptr::decode(q).is_fn() && self.past_complete(p) {
                return Err(Fault::PtrPastCmp { p, pointee });
            }
        }
        // clang: an object of unknown identity may be any other object.
        if self.is_unknown(p) || self.is_unknown(q) {
            return Err(Fault::PtrCmpUnknown { p, q, pointee });
        }
        Ok(c == cmp::NE)
    }

    /// [expr.static.cast]/14 (P2738): a cast of the `void *` `raw` to
    /// `to *` designates an object whose type is similar to `to`: the
    /// type `tag` (+ 1) its conversion to `void *` recorded
    /// (`kbc::VOID_TAG`), or for an untagged one, some subobject at its
    /// address.
    fn void_cast(&self, raw: u64, tag: u32, to: OTy) -> Result<(), Fault> {
        if raw == 0 {
            return Ok(());
        }
        if let Some(from) = tag.checked_sub(1).map(OTy) {
            if self.types.similar(from, to) {
                return Ok(());
            }
            return Err(Fault::VoidCast { from, to, p: raw });
        }
        let p = Ptr::decode(raw);
        let Some(a) = self.alloc_any(p) else { return Ok(()) };
        let off = u64::from(p.off);
        let target = if p.past { off.saturating_sub(self.types.size(to).max(1)) } else { off };
        if self.types.has_subobject(a.ty, target, to) {
            return Ok(());
        }
        let from = self.deepest_at(a.ty, off);
        Err(Fault::VoidCast { from, to, p: raw })
    }

    /// The most derived (innermost) type of an object at byte `off` of
    /// an object of type `t`, for notes.
    pub(crate) fn deepest_at(&self, mut t: OTy, mut off: u64) -> OTy {
        loop {
            match self.types.kind(t) {
                TyKind::Array { elem, n } => {
                    let es = self.types.size(elem).max(1);
                    if off / es >= n {
                        return t;
                    }
                    off %= es;
                    t = elem;
                }
                TyKind::Record { .. } => {
                    let Some(f) = self.types.field_at(t, off) else {
                        return t;
                    };
                    off -= f.offset;
                    t = f.ty;
                }
                _ => return t,
            }
        }
    }

    // ---- memory -----------------------------------------------------------

    /// [expr.const]/5.9: an access `k` (a read, or a write of an object that
    /// is not `const`) of the object of type `ty` at `raw`, in an
    /// allocation marked [`flags::MUTABLE_OUTSIDE`], must not reach a
    /// `mutable` member on the path from the allocation root, nor (a
    /// whole-object read of a class, a trivial copy) contain one that it
    /// reads (clang `diagnoseMutableFields`). The slow path of a load:
    /// only allocations with the flag take it. A frame or heap allocation
    /// with the bit is one marked [`flags::VOLATILE`] instead
    /// ([`Oven::volatile_path`]).
    #[cold]
    pub(crate) fn mutable_path(&self, raw: u64, ty: OTy, k: u32) -> Result<(), Fault> {
        let p = Ptr::decode(raw);
        if p.space() != Sp::Static {
            return self.volatile_path(raw, ty, k);
        }
        let Some(a) = self.alloc_any(p) else { return Ok(()) };
        let (mut t, mut base) = (a.ty, 0u64);
        let target = u64::from(p.off);
        loop {
            if t == ty && base == target {
                return match self.mutable_read(ty) {
                    Some(field) => Err(Fault::Mutable { ak: k, field }),
                    None => Ok(()),
                };
            }
            match self.types.kind(t) {
                TyKind::Array { elem, n } => {
                    let es = self.types.size(elem).max(1);
                    let i = (target - base) / es;
                    if i >= n {
                        return Ok(());
                    }
                    base += i * es;
                    t = elem;
                }
                TyKind::Record { .. } => {
                    match self.types.hop(Walk::Mutable, t, target - base, ty) {
                        Hop::Reached => {
                            t = ty;
                            base = target;
                        }
                        Hop::Stop => return Ok(()),
                        Hop::Mutable(field) => return Err(Fault::Mutable { ak: k, field }),
                        Hop::Array { at, t: arr } => {
                            base += at;
                            t = arr;
                        }
                        Hop::Union { at, u, reach } => {
                            base += at;
                            let rel = target - base;
                            let fields = self.types.fields(u);
                            // The active member (a read of another is
                            // diagnosed by `union_path`), else the first one
                            // holding the byte.
                            let active =
                                self.mem.unions.get(&Ptr::union_key(raw, base, u)).copied().filter(
                                    |&i| self.types.reaches(Walk::Mutable, u, rel, ty, reach, i),
                                );
                            let f = match active {
                                Some(i) => fields.get(i as usize),
                                None if reach != 0 => fields.get(reach.trailing_zeros() as usize),
                                None => self.types.near(u, rel).find(|f| {
                                    self.types.union_member_reaches(Walk::Mutable, rel, ty, f)
                                }),
                            };
                            let Some(f) = f else { return Ok(()) };
                            if f.mutable {
                                return Err(Fault::Mutable { ak: k, field: f.decl });
                            }
                            if f.width.is_some() {
                                return Ok(());
                            }
                            base += f.offset;
                            t = f.ty;
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    /// [expr.const]/10.8: an access `k` of the object of type `ty` at
    /// `raw`, in a frame or heap allocation marked [`flags::VOLATILE`],
    /// must not be of a volatile object: the allocation's object if it is
    /// volatile (`Oven::volatile_decls`, `volatile_exprs`), or a subobject
    /// of a volatile member, on the path from the allocation root to it.
    /// An object under construction or destruction is not volatile
    /// ([class.ctor.general]/5, [class.dtor]/5): one a constructor or
    /// destructor on the stack has as `this`. Only the accessed object's
    /// volatility counts, as clang's `findSubobject` checks it: a
    /// whole-object copy of a class with a volatile member reads no
    /// volatile object. The walk is uncached: only allocations with the
    /// flag take it.
    #[cold]
    #[inline(never)]
    fn volatile_path(&self, raw: u64, ty: OTy, k: u32) -> Result<(), Fault> {
        let p = Ptr::decode(raw);
        let Some(a) = self.alloc_any(p) else { return Ok(()) };
        let key = Ptr::alloc_key(raw);
        // The offsets of the objects in this allocation a constructor or
        // destructor is running for.
        let structors: Vec<u32> = self
            .stack
            .iter()
            .filter_map(|fr| {
                let f = &self.funcs[fr.func as usize];
                let r = f.this_reg.filter(|_| f.structor)?;
                let this = self.regs.get((fr.fb + r) as usize).map(|&v| v as u64)?;
                (this != 0 && Ptr::alloc_key(this) == key).then(|| Ptr::decode(this).off)
            })
            .collect();
        let mut vol = match a.origin {
            Origin::Var(d) => self.volatile_decls.contains(&d),
            Origin::Temp(e) | Origin::Heap(e, _) => self.volatile_exprs.contains(&e),
            _ => false,
        };
        let mut member = None;
        let (mut t, mut base) = (a.ty, 0u64);
        let target = u64::from(p.off);
        loop {
            if vol
                && matches!(self.types.kind(t), TyKind::Record { .. })
                && u32::try_from(base).is_ok_and(|b| structors.contains(&b))
            {
                vol = false;
            }
            if t == ty && base == target {
                break;
            }
            match self.types.kind(t) {
                TyKind::Array { elem, n } => {
                    let es = self.types.size(elem).max(1);
                    let i = (target - base) / es;
                    if i >= n {
                        break;
                    }
                    base += i * es;
                    t = elem;
                }
                TyKind::Record { union, .. } => {
                    let rel = target - base;
                    let contains = |f: &&crate::ty::FieldInfo| {
                        rel >= f.offset && rel < f.offset + self.types.size(f.ty).max(1)
                    };
                    // A union's active member, else the first holding the byte.
                    let active = union
                        .then(|| self.mem.unions.get(&Ptr::union_key(raw, base, t)).copied())
                        .flatten()
                        .and_then(|i| self.types.fields(t).get(i as usize))
                        .filter(|f| contains(f));
                    let Some(f) = active.or_else(|| self.types.near(t, rel).find(|f| contains(f)))
                    else {
                        break;
                    };
                    if f.volatile {
                        vol = true;
                        member = Some(f.decl);
                    }
                    if f.width.is_some() {
                        break;
                    }
                    base += f.offset;
                    t = f.ty;
                }
                _ => break,
            }
        }
        if vol { Err(Fault::Volatile { ak: k, member, p: raw }) } else { Ok(()) }
    }

    /// The first `mutable` member a read of a whole object of type `t`
    /// reads: members before bases, recursively; a member of empty class
    /// type is not read (clang `isReadByLvalueToRvalueConversion`), any
    /// `mutable` member of a union is.
    fn mutable_read(&self, t: OTy) -> Option<DeclId> {
        let info = self.types.info(t);
        if !info.has_mutable {
            return None;
        }
        match info.kind {
            TyKind::Array { elem, n } if n > 0 => self.mutable_read(elem),
            TyKind::Record { union, .. } => {
                let fields = self.types.fields(t);
                let member = fields.iter().filter(|f| !f.base).find_map(|f| {
                    if f.mutable && (union || self.reads_storage(f.ty)) {
                        Some(f.decl)
                    } else {
                        self.mutable_read(f.ty)
                    }
                });
                member.or_else(|| {
                    fields.iter().filter(|f| f.base).find_map(|f| self.mutable_read(f.ty))
                })
            }
            _ => None,
        }
    }

    /// Whether an lvalue-to-rvalue conversion of an object of type `t`
    /// reads anything (not an empty class or an array of them).
    fn reads_storage(&self, t: OTy) -> bool {
        match self.types.kind(t) {
            TyKind::Array { elem, n } => n > 0 && self.reads_storage(elem),
            TyKind::Record { union, .. } => {
                let fields = self.types.fields(t);
                if union {
                    !fields.is_empty()
                } else {
                    fields.iter().any(|f| self.reads_storage(f.ty))
                }
            }
            _ => true,
        }
    }

    /// Checks the active members of the unions on the path from the
    /// allocation root to the `len`-byte object of type `ty` at `raw`.
    /// `Ok` answers are remembered ([`umemo`], WP-760).
    fn union_path(&self, raw: u64, ty: OTy, k: u32) -> Result<(), Fault> {
        let Some(a) = self.alloc_any(Ptr::decode(raw)) else { return Ok(()) };
        self.union_path_in(a.ty, raw, ty, k)
    }

    /// [`Oven::union_path`] in an allocation of type `aty`: nothing to
    /// check when no union overlaps the accessed object.
    #[inline]
    fn union_path_in(&self, aty: OTy, raw: u64, ty: OTy, k: u32) -> Result<(), Fault> {
        if !self.union_overlaps(aty, Ptr::decode(raw).off, ty) {
            return Ok(());
        }
        self.union_path_memo(aty, raw, ty, k)
    }

    /// [`Oven::union_path_in`] past its overlap test: the memo, else the
    /// walk.
    #[inline(never)]
    fn union_path_memo(&self, aty: OTy, raw: u64, ty: OTy, k: u32) -> Result<(), Fault> {
        if self.union_memo_hit(aty, raw, ty) {
            return Ok(());
        }
        self.union_path_miss(aty, raw, ty, k)
    }

    /// Whether the union-path memo holds the `Ok` answer of the access to
    /// the object of type `ty` at `raw` in an allocation of type `aty`.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn union_memo_hit(&self, aty: OTy, raw: u64, ty: OTy) -> bool {
        let stamp = umemo::Stamp { epoch: self.mem.union_epoch, rebuilt: self.types.rebuilt() };
        let (akey, off) = (Ptr::alloc_key(raw), Ptr::decode(raw).off);
        self.umemo.hit(umemo::Walk::Access, akey, aty, ty, off, stamp)
    }

    /// [`Oven::union_path_memo`] on a memo miss: the walk, whose `Ok`
    /// answer is remembered when it may be.
    #[inline(never)]
    fn union_path_miss(&self, aty: OTy, raw: u64, ty: OTy, k: u32) -> Result<(), Fault> {
        let stamp = umemo::Stamp { epoch: self.mem.union_epoch, rebuilt: self.types.rebuilt() };
        if let Some(cover) = self.union_walk(aty, raw, ty, k)? {
            self.umemo.put(umemo::Walk::Access, Ptr::alloc_key(raw), aty, ty, cover, stamp);
        }
        Ok(())
    }

    /// Whether a union subobject of an object of type `aty` may overlap
    /// the object of type `ty` at offset `off`: `false` when the object
    /// lies outside the bytes spanning them ([`crate::ty::TyInfo::uhull`]), so no
    /// union is on the path to it and [`Oven::union_walk`] has nothing to
    /// check; and for the object of type `aty` itself, the walk's first
    /// step.
    #[inline]
    fn union_overlaps(&self, aty: OTy, off: u32, ty: OTy) -> bool {
        let (ulo, uhi) = self.types.info(aty).uhull;
        let lo = u64::from(off);
        let hi = lo + self.types.size(ty).max(1);
        lo < u64::from(uhi) && u64::from(ulo) < hi && !(off == 0 && ty == aty)
    }

    /// [`Oven::union_path`]'s walk from `root` (the type of `raw`'s
    /// allocation): on success, the offsets the same answer covers, or
    /// `None` when the answer depends on the access kind `k` (a fault
    /// path that lets the access through) and must not be remembered.
    #[inline(never)]
    fn union_walk(
        &self,
        root: OTy,
        raw: u64,
        ty: OTy,
        k: u32,
    ) -> Result<Option<umemo::Cover>, Fault> {
        let target = u64::from(Ptr::decode(raw).off);
        let exact = Some(umemo::Cover::At(target as u32));
        let (mut t, mut base) = (root, 0u64);
        // Every union so far was resolved by its active member.
        let mut by_active = true;
        loop {
            if t == ty && base == target {
                return Ok(exact);
            }
            let info = self.types.info(t);
            if !info.has_union {
                // No union below: the rest of the walk cannot fail.
                return Ok(if by_active {
                    Some(self.union_free_cover(t, base, target, ty))
                } else {
                    exact
                });
            }
            match info.kind {
                TyKind::Array { elem, n } => {
                    let es = self.types.size(elem).max(1);
                    let i = (target - base) / es;
                    if i >= n {
                        return Ok(exact);
                    }
                    base += i * es;
                    t = elem;
                }
                // The member choices are cached per (record, offset,
                // type) (WP-644); the active members are read here.
                TyKind::Record { .. } => match self.types.hop(Walk::Union, t, target - base, ty) {
                    Hop::Reached | Hop::Stop | Hop::Mutable(_) => return Ok(exact),
                    Hop::Array { at, t: arr } => {
                        base += at;
                        t = arr;
                    }
                    Hop::Union { at, u, reach } => {
                        base += at;
                        let rel = target - base;
                        let fields = self.types.fields(u);
                        let key = Ptr::union_key(raw, base, u);
                        let active = self.mem.unions.get(&key).copied();
                        let alts = self.mem.union_alts.get(&key).copied().unwrap_or(0);
                        let alt = alt_members(alts, fields.len() as u32, active);
                        let Some(i) = active
                            .into_iter()
                            .chain(alt)
                            .find(|&i| self.types.reaches(Walk::Union, u, rel, ty, reach, i))
                        else {
                            self.union_fault(u, rel, ty, k, active)?;
                            return Ok(None);
                        };
                        by_active &= active == Some(i);
                        let f = fields[i as usize];
                        base += f.offset;
                        t = f.ty;
                        if f.width.is_some() {
                            return Ok(exact);
                        }
                    }
                },
                _ => return Ok(exact),
            }
        }
    }

    /// The offsets [`Oven::union_walk`]'s answer covers once it reaches
    /// `t`, a type without unions, at byte `base`, having resolved every
    /// union above by its active member: when `t` is an array and the
    /// accessed object of type `ty` at `target` is a subobject of its
    /// element, the same position in every element (the walk toward any of
    /// them makes the same choices: no two subobjects of one type share an
    /// address, [intro.object]/9, so a non-union class has one member
    /// toward it, and the active member holding this one holds those
    /// too); else `target` alone.
    fn union_free_cover(&self, t: OTy, base: u64, target: u64, ty: OTy) -> umemo::Cover {
        let exact = umemo::Cover::At(target as u32);
        let TyKind::Array { elem, n } = self.types.kind(t) else { return exact };
        let es = self.types.size(elem);
        let len = es.saturating_mul(n);
        if es == 0 || target < base || target - base >= len || base + len > u64::from(u32::MAX) {
            return exact;
        }
        let phase = (target - base) % es;
        if !self.types.has_subobject(elem, phase, ty) {
            return exact;
        }
        umemo::Cover::Region {
            lo: base as u32,
            len: len as u32,
            stride: es as u32,
            phase: phase as u32,
        }
    }

    /// The fault of an access `k` of the `ty` object at byte `rel` of union
    /// `u` that no (possibly) active member holds: the member that would
    /// hold it, when there is one.
    #[cold]
    fn union_fault(
        &self,
        u: OTy,
        rel: u64,
        ty: OTy,
        k: u32,
        active: Option<u32>,
    ) -> Result<(), Fault> {
        let fields = self.types.fields(u);
        let reach =
            |f: &crate::ty::FieldInfo| self.types.union_member_reaches(Walk::Union, rel, ty, f);
        let contains = |f: &crate::ty::FieldInfo| {
            rel >= f.offset && rel < f.offset + self.types.size(f.ty).max(1)
        };
        // The object of a member call can be one the layout does not
        // model (an empty base sharing the union's offset, libc++'s
        // `__compressed_pair` allocator): only a member holding it is
        // wrong.
        let fallback = || {
            (k != ak::MEMBER_CALL).then(|| self.types.near(u, rel).find(|f| contains(f))).flatten()
        };
        let Some(f) = self.types.near(u, rel).find(|f| reach(f)).or_else(fallback) else {
            return Ok(());
        };
        Err(Fault::Union { ak: k, field: f.decl, active: active.map(|i| fields[i as usize].decl) })
    }

    /// The checks of a load from an allocation with unions or with
    /// `mutable` members it may not read (outlined from the hot path; not
    /// `#[cold]`, since union-backed storage takes it on every access).
    #[inline(never)]
    fn slow_access(&self, a: &Alloc, raw: u64, ty: OTy, k: u32) -> Result<(), Fault> {
        let fl = a.flags;
        // A write is checked in an object that is not `const`: the one
        // `Oven::has_constant_destruction` destroys, whose `mutable`
        // members are outside their lifetime.
        if fl & flags::MUTABLE_OUTSIDE != 0 && (k == ak::READ || fl & flags::CONST == 0) {
            self.mutable_path(raw, ty, k)?;
        }
        // The memo probe inlined: a load of union storage hits it.
        if fl & flags::HAS_UNION != 0
            && self.union_overlaps(a.ty, Ptr::decode(raw).off, ty)
            && !self.union_memo_hit(a.ty, raw, ty)
        {
            self.union_path_miss(a.ty, raw, ty, k)?;
        }
        Ok(())
    }

    /// The bounds check of a load; the read half of a read-modify-write
    /// (`k` other than [`ak::READ`]) is checked as the write it precedes, so
    /// modifying an object visible outside the evaluation is diagnosed
    /// before reading it ([expr.const]/5.16).
    // Always inlined into `load`: outlined, its `Result` returns through
    // memory on every `Load`.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn rmw_check(&self, raw: u64, len: u64, ty: OTy, k: u32) -> Result<(Sp, u64, &Alloc), Fault> {
        let fault = |f| Fault::Mem { f, ak: k, ty, p: raw };
        let w = k != ak::READ;
        let (st, abs, a) = self.mem.check(raw, len, w).map_err(fault)?;
        if w && let Some(f) = write_fault(st, a, ty) {
            return Err(fault(f));
        }
        Ok((st, abs, a))
    }

    // Always inlined into the `Load` op: `freeze_object` is a second
    // caller, which otherwise outlines it from the hot loop.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub(crate) fn load(&self, raw: u64, ty: OTy, k: u32) -> Result<u128, Fault> {
        let info = self.types.info(ty);
        let len = value_len(info.size, info.kind);
        let is_ptr = matches!(info.kind, TyKind::Ptr(_) | TyKind::NullPtr);
        let fault = |f| Fault::Mem { f, ak: k, ty, p: raw };
        let (st, abs, a) = self.rmw_check(raw, len, ty, k)?;
        if a.flags & (flags::HAS_UNION | flags::MUTABLE_OUTSIDE) != 0 {
            self.slow_access(a, raw, ty, k)?;
        }
        let sp = self.mem.space(st);
        if !sp.is_init(abs, len) {
            return Err(fault(MemFault::Uninit));
        }
        let v = sp.read(abs, len);
        if sp.any_ptr(abs, len) != is_ptr && !(is_ptr && v == 0) {
            return Err(fault(MemFault::Incompatible));
        }
        Ok(match info.kind {
            TyKind::Int { width, signed } => canon(v, u32::from(width), signed),
            // A `void *` stored with its tag gets it back (`kbc::VOID_TAG`).
            TyKind::Ptr(_) if sp.ptr_mark(abs) >= crate::mem::PTR_TAGGED => {
                v | u128::from(self.mem.void_tag(st, abs)) << 64
            }
            _ => v,
        })
    }

    pub(crate) fn store(
        &mut self,
        raw: u64,
        ty: OTy,
        v: u128,
        k: u32,
        init: bool,
    ) -> Result<(), Fault> {
        let info = *self.types.info(ty);
        let len = value_len(info.size, info.kind);
        let is_ptr = matches!(info.kind, TyKind::Ptr(_) | TyKind::NullPtr);
        let fault = |f| Fault::Mem { f, ak: k, ty, p: raw };
        let (st, abs, a) = self.mem.check(raw, len, true).map_err(fault)?;
        if !init && let Some(f) = write_fault(st, a, ty) {
            return Err(fault(f));
        }
        let has_union = a.flags & flags::HAS_UNION != 0;
        let aty = a.ty;
        let mutable = a.flags & (flags::MUTABLE_OUTSIDE | flags::CONST) == flags::MUTABLE_OUTSIDE;
        if mutable && !init {
            self.mutable_path(raw, ty, k)?;
        }
        self.mem.trial_write(raw, len).map_err(fault)?;
        if has_union {
            self.union_path_in(aty, raw, ty, k)?;
        }
        // A pointer keeps its `void *` tag in its mark (`kbc::VOID_TAG`).
        let mark = if !is_ptr {
            0
        } else if v >> 64 == 0 {
            1
        } else {
            self.mem.set_void_tag(st, abs, (v >> 64) as u32)
        };
        let sp = self.mem.space_mut(st);
        sp.write(abs, len, v);
        sp.set_init(abs, len, true);
        sp.set_ptr_mark(abs, len, mark);
        Ok(())
    }

    fn load_bits(&self, raw: u64, ty: OTy, bit: u32, width: u32, k: u32) -> Result<u128, Fault> {
        let n = u64::from((bit + width).div_ceil(8));
        let fault = |f| Fault::Mem { f, ak: k, ty, p: raw };
        let (st, abs, a) = self.rmw_check(raw, n, ty, k)?;
        if a.flags & (flags::HAS_UNION | flags::MUTABLE_OUTSIDE) != 0 {
            self.slow_access(a, raw, ty, k)?;
        }
        let sp = self.mem.space(st);
        if !sp.is_init(abs, n) {
            return Err(fault(MemFault::Uninit));
        }
        if width == 0 {
            return Ok(0);
        }
        let v = sp.read(abs, n) >> bit;
        let mask = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
        let v = v & mask;
        Ok(match self.types.kind(ty) {
            TyKind::Int { signed, .. } => canon(v, width, signed),
            _ => v,
        })
    }

    /// The byte address, bit and lane type of lane `idx` of the `bool`
    /// vector of type `vty` at `raw` (`Op::LoadLane`, `Op::StoreLane`):
    /// lane `i` is bit `i % 8` of byte `i / 8`. A lane past the end is
    /// stepped to as an array element would be (clang 22): `n` is the
    /// one-past-the-end lane, which access `k` cannot dereference.
    fn lane_at(&self, raw: u64, idx: u64, vty: OTy, k: u32) -> Result<(u64, u32, OTy), Fault> {
        let TyKind::BoolVec { elem, n } = self.types.kind(vty) else {
            return Err(Fault::Invalid);
        };
        if idx > n {
            return Err(Fault::ArrayIndex { idx: i128::from(idx), n, array: true });
        }
        if idx == n {
            return Err(Fault::Mem { f: MemFault::PastEnd, ak: k, ty: elem, p: raw });
        }
        if raw == 0 || idx < 8 {
            return Ok((raw, (idx % 8) as u32, elem));
        }
        let off = u64::from(Ptr::decode(raw).off) + idx / 8;
        Ok((Ptr::with_off(raw, off, false), (idx % 8) as u32, elem))
    }

    fn store_bits(
        &mut self,
        raw: u64,
        ty: OTy,
        v: u128,
        bit: u32,
        width: u32,
        init: bool,
    ) -> Result<(), Fault> {
        let n = u64::from((bit + width).div_ceil(8));
        let k = if init { ak::INIT } else { ak::ASSIGN };
        let fault = |f| Fault::Mem { f, ak: k, ty, p: raw };
        let (st, abs, a) = self.mem.check(raw, n, true).map_err(fault)?;
        if !init && let Some(f) = write_fault(st, a, ty) {
            return Err(fault(f));
        }
        let has_union = a.flags & flags::HAS_UNION != 0;
        let aty = a.ty;
        let mutable = a.flags & (flags::MUTABLE_OUTSIDE | flags::CONST) == flags::MUTABLE_OUTSIDE;
        if mutable && !init {
            self.mutable_path(raw, ty, k)?;
        }
        self.mem.trial_write(raw, n).map_err(fault)?;
        if has_union {
            self.union_path_in(aty, raw, ty, k)?;
        }
        if width == 0 {
            return Ok(());
        }
        let sp = self.mem.space_mut(st);
        // Bytes not yet initialized hold zero bits.
        let mut old = 0u128;
        for i in (0..n).rev() {
            let byte = if sp.is_init(abs + i, 1) { sp.bytes[(abs + i) as usize] } else { 0 };
            old = old << 8 | u128::from(byte);
        }
        let mask = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 } << bit;
        let new = (old & !mask) | ((v << bit) & mask);
        sp.write(abs, n, new);
        sp.set_init(abs, n, true);
        sp.set_ptr(abs, n, false);
        Ok(())
    }

    /// Copies the object of type `ty` at `src` to `dst` (`init`: into
    /// storage whose object's lifetime begins). With `full` (a trivial
    /// copy constructor or assignment operator of a class, clang's
    /// `handleTrivialCopy`), the source must be fully initialized
    /// ([expr.const]/5.9 via the lvalue-to-rvalue conversion of the
    /// object); a union copies its object representation unchecked.
    pub(crate) fn copy_obj(
        &mut self,
        dst: u64,
        src: u64,
        ty: OTy,
        init: bool,
        full: Option<&kiln_ast::Ast>,
    ) -> Result<(), Fault> {
        // An empty class copies no byte (see `Op::ZeroInit`), a class
        // no tail padding.
        let empty = self.types.holds_no_data(ty);
        let size = if empty { 0 } else { self.types.copy_size(ty) };
        // Nor is its source read (clang's `isReadByLvalueToRvalueConversion`):
        // copying an `extern const` empty object is a constant.
        let mut src_at = None;
        if !empty {
            let (ss, sa, sa_alloc) = self.mem.check(src, size, false).map_err(|f| Fault::Mem {
                f,
                ak: ak::READ,
                ty,
                p: src,
            })?;
            src_at = Some((ss, sa));
            if sa_alloc.flags & flags::MUTABLE_OUTSIDE != 0 {
                self.mutable_path(src, ty, ak::READ)?;
            }
            if let Some(ast) = full
                && !matches!(self.types.kind(ty), TyKind::Record { union: true, .. })
                && let off = u64::from(Ptr::decode(src).off)
                && let Some((field, sub)) = self.first_uninit(ast, ss, sa, src, off, ty, None)
            {
                return Err(Fault::Uninit { field, ty: sub, src, whole: ty });
            }
        }
        let k = if init { ak::INIT } else { ak::ASSIGN };
        let (ds, da, a) =
            self.mem.check(dst, size, true).map_err(|f| Fault::Mem { f, ak: k, ty, p: dst })?;
        if !init && let Some(f) = write_fault(ds, a, ty) {
            return Err(Fault::Mem { f, ak: k, ty, p: dst });
        }
        if !init && a.flags & flags::VOLATILE != 0 && ds != Sp::Static {
            self.volatile_path(dst, ty, k)?;
        }
        let Some((ss, sa)) = src_at.filter(|_| size != 0) else { return Ok(()) };
        self.mem.trial_write(dst, size).map_err(|f| Fault::Mem { f, ak: k, ty, p: dst })?;
        self.mem.copy(ds, da, ss, sa, size);
        if self.types.info(ty).has_union {
            let (so, dof) = (u64::from(Ptr::decode(src).off), u64::from(Ptr::decode(dst).off));
            let (a, z) = self.types.info(ty).upos;
            for i in a..z {
                let (o, ut) = self.types.union_pos[i as usize];
                let sk = Ptr::union_key(src, so + o, ut);
                let v = self.mem.unions.get(&sk).copied();
                self.mem.union_put(Ptr::union_key(dst, dof + o, ut), v);
                if let Some(&m) = self.mem.union_alts.get(&sk) {
                    self.mem.union_alts_put(Ptr::union_key(dst, dof + o, ut), Some(m));
                }
            }
        }
        Ok(())
    }

    /// The first subobject of the object of type `t` at byte `abs` of
    /// space `sp` (byte `off` of the allocation of `raw`) that is not
    /// initialized, as `(member, type)`: clang's `CheckFullyInitialized`,
    /// walked as freezing walks a value (an unnamed or zero-width
    /// bit-field and an inactive union member are not part of the value;
    /// a base class subobject has no member name).
    #[allow(clippy::too_many_arguments)]
    fn first_uninit(
        &self,
        ast: &kiln_ast::Ast,
        sp: Sp,
        abs: u64,
        raw: u64,
        off: u64,
        t: OTy,
        field: Option<DeclId>,
    ) -> Option<(Option<DeclId>, OTy)> {
        let info = *self.types.info(t);
        let s = self.mem.space(sp);
        if s.is_init(abs, info.size) {
            return None;
        }
        match info.kind {
            TyKind::Bool | TyKind::Int { .. } | TyKind::WideInt { .. } | TyKind::Float(_) => {
                (!s.is_init(abs, value_len(info.size, info.kind))).then_some((field, t))
            }
            TyKind::Ptr(_) | TyKind::NullPtr | TyKind::MemPtr { .. } | TyKind::Reflection => {
                Some((field, t))
            }
            TyKind::BoolVec { n, .. } => (!s.is_init(abs, n.div_ceil(8))).then_some((field, t)),
            TyKind::Array { elem, n } => {
                let es = self.types.size(elem);
                (0..n).find_map(|i| {
                    self.first_uninit(ast, sp, abs + i * es, raw, off + i * es, elem, field)
                })
            }
            TyKind::Record { union, first, count, .. } => {
                let fields = if union {
                    let &i = self.mem.unions.get(&Ptr::union_key(raw, off, t))?;
                    let f = self.types.fields[(first + i) as usize];
                    if self.cxx_year >= 2026
                        && f.width.is_none()
                        && matches!(self.types.kind(f.ty), TyKind::Array { .. })
                    {
                        let (a, o) = (abs + f.offset, off + f.offset);
                        return self.first_uninit_elements(ast, sp, a, raw, o, f.ty, Some(f.decl));
                    }
                    first + i..first + i + 1
                } else {
                    first..first + count
                };
                fields.into_iter().find_map(|i| {
                    let f = self.types.fields[i as usize];
                    match f.width {
                        None => {
                            let named = (!f.base).then_some(f.decl);
                            let (a, o) = (abs + f.offset, off + f.offset);
                            self.first_uninit(ast, sp, a, raw, o, f.ty, named)
                        }
                        Some(w) if w == 0 || ast.decls.get(f.decl).name().is_empty() => None,
                        Some(w) => {
                            let n = u64::from((f.bit + w).div_ceil(8));
                            (!s.is_init(abs + f.offset, n)).then_some((Some(f.decl), f.ty))
                        }
                    }
                })
            }
            TyKind::Void | TyKind::Func | TyKind::Opaque => None,
        }
    }

    /// `Oven::first_uninit` of the active array member of a union, `t`
    /// (C++26, P3726R2): its elements outside their lifetime (holding no
    /// initialized byte) are not part of the value, as freezing skips
    /// them (`Oven::walk_elements`).
    #[allow(clippy::too_many_arguments)]
    fn first_uninit_elements(
        &self,
        ast: &kiln_ast::Ast,
        sp: Sp,
        abs: u64,
        raw: u64,
        off: u64,
        t: OTy,
        field: Option<DeclId>,
    ) -> Option<(Option<DeclId>, OTy)> {
        let TyKind::Array { elem, n } = self.types.kind(t) else {
            return self.first_uninit(ast, sp, abs, raw, off, t, field);
        };
        let es = self.types.size(elem);
        (0..n).find_map(|i| {
            if es != 0 && !self.mem.space(sp).any_init(abs + i * es, es) {
                return None;
            }
            self.first_uninit_elements(ast, sp, abs + i * es, raw, off + i * es, elem, field)
        })
    }

    /// Activates the first member of every union reached through first
    /// members of the zero-initialized object of type `t` at `off`
    /// ([dcl.init.general]/6.4).
    pub(crate) fn zero_unions(&mut self, raw: u64, off: u64, t: OTy) {
        match self.types.kind(t) {
            TyKind::Array { elem, n } => {
                if self.types.info(elem).has_union {
                    let es = self.types.size(elem);
                    for i in 0..n {
                        self.zero_unions(raw, off + i * es, elem);
                    }
                }
            }
            TyKind::Record { union, first, count, .. } => {
                if union {
                    // Clear stale entries of nested unions first.
                    let (a, z) = self.types.info(t).upos;
                    for i in a..z {
                        let (o, ut) = self.types.union_pos[i as usize];
                        self.mem.union_put(Ptr::union_key(raw, off + o, ut), None);
                    }
                    if count == 0 {
                        return;
                    }
                    self.mem.union_put(Ptr::union_key(raw, off, t), Some(0));
                    let f = self.types.fields[first as usize];
                    if self.types.info(f.ty).has_union {
                        self.zero_unions(raw, off + f.offset, f.ty);
                    }
                } else {
                    for i in first..first + count {
                        let f = self.types.fields[i as usize];
                        if f.width.is_none() && self.types.info(f.ty).has_union {
                            self.zero_unions(raw, off + f.offset, f.ty);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// `Op::Downcast`: the object of type `to` whose base subobject is
    /// `d` bytes into it, at `raw` ([expr.static.cast]/2, /11).
    fn downcast(&self, raw: u64, d: u64, to: OTy) -> Result<u64, Fault> {
        let p = Ptr::decode(raw);
        let a = self.mem.alloc_of(p).map_err(|f| Fault::Mem { f, ak: ak::READ, ty: to, p: raw })?;
        let off = u64::from(p.off);
        match off.checked_sub(d) {
            Some(o) if !p.past && self.types.has_subobject(a.ty, o, to) => {
                Ok(Ptr::with_off(raw, o, false))
            }
            _ => Err(Fault::BadDowncast { dynamic: self.dynamic_type(a.ty, off), to }),
        }
    }

    /// A single-object `delete` of `raw` (static type `pointee`): a base
    /// class subobject of a `new`ed object is deleted as the whole object
    /// (`virt`: `pointee` has a virtual destructor, [expr.delete]/3).
    fn delete_base(&self, raw: u64, pointee: OTy, virt: bool) -> Result<Option<(u64, OTy)>, Fault> {
        let p = Ptr::decode(raw);
        if p.is_fn() || !p.is_heap() || p.past {
            return Ok(None);
        }
        let Some(a) = self.mem.alloc_any(p) else { return Ok(None) };
        let off = u64::from(p.off);
        if a.ty == pointee
            || a.flags & (flags::ARRAY_NEW | flags::STD_ALLOC) != 0
            || !self.types.has_base_subobject(a.ty, off, pointee)
        {
            return Ok(None);
        }
        if !virt {
            return Err(Fault::DeleteNonVirtual { stat: pointee, dynamic: a.ty });
        }
        Ok(Some((Ptr::with_off(raw, 0, false), a.ty)))
    }

    /// `Op::Placement`: a placement new of a `ty` at `raw` (clang's
    /// `VisitCXXNewExpr` placement path: `findCompleteObject` for
    /// construction, then the type of the designated subobject).
    /// `bi::START_LIFETIME`: `__builtin_start_lifetime(p)` (P3726R2
    /// [obj.lifetime], WP-773) of the `ty` object at `raw`. It begins the
    /// object's lifetime without initializing it or any subobject: a
    /// union member becomes the active member, its bytes uninitialized
    /// (an array member's elements are then outside their lifetime,
    /// `Oven::walk_elements`), and an object of tracked storage none of
    /// whose bytes is within a lifetime gets its bytes back. An object
    /// already within its lifetime (the active member) is left as it is.
    /// Its storage is checked as a placement new's; an object in a
    /// subobject of an inactive union member is a fault (GCC's
    /// "containing object not within lifetime"): unlike placement new,
    /// no array member is activated for its element.
    pub(super) fn start_lifetime(&mut self, raw: u64, ty: OTy) -> Result<(), Fault> {
        let fault = |f| Fault::Mem { f, ak: ak::INIT, ty, p: raw };
        let size = self.types.size(ty);
        let (sp, _, a) = self.mem.check_storage(raw, size, true).map_err(fault)?;
        let a = *a;
        if let Some(f) = write_fault(sp, &a, ty) {
            return Err(fault(f));
        }
        if a.flags & flags::HAS_UNION != 0 {
            self.place_in_unions(raw, a.ty, ty, false)?;
        }
        if size != 0 && self.mem.ended(raw, size) {
            self.mem.set_lifetime(raw, size, true).map_err(fault)?;
        }
        Ok(())
    }

    /// `__builtin_clear_padding(raw)` for an object of type `ty` (WP-774;
    /// spec/oven.md "Clearing padding"): the storage must exist, be in
    /// bounds and be modifiable by this evaluation. Padding bits are not
    /// modelled (they read as indeterminate), so nothing is written.
    pub(super) fn clear_padding(&mut self, raw: u64, ty: OTy) -> Result<(), Fault> {
        let fault = |f| Fault::Mem { f, ak: ak::ASSIGN, ty, p: raw };
        let size = self.types.size(ty);
        let (sp, _, a) = self.mem.check_storage(raw, size, true).map_err(fault)?;
        if let Some(f) = write_fault(sp, a, ty) {
            return Err(fault(f));
        }
        Ok(())
    }

    fn placement(&mut self, raw: u64, ty: OTy, stat: Option<OTy>, aux: u32) -> Result<(), Fault> {
        let fault = |f| Fault::Mem { f, ak: ak::INIT, ty, p: raw };
        let (sp, _, a) = self.mem.check_storage(raw, 0, true).map_err(fault)?;
        let (sp, a) = (sp, *a);
        let off = u64::from(Ptr::decode(raw).off);
        // The designated object: of the argument's static type or, for a
        // `void *` argument, of the type its conversion from `T *`
        // recorded; failing both, the outermost object at `off`.
        let target = match stat {
            Some(s) if self.types.has_subobject(a.ty, off, s) => s,
            _ => self.outermost_at(a.ty, off).unwrap_or_else(|| self.dynamic_type(a.ty, off)),
        };
        if !self.placement_fits(target, ty) {
            return Err(Fault::PlacementType { from: target, to: ty });
        }
        if let Some(f) = write_fault(sp, &a, target) {
            return Err(Fault::Mem { f, ak: ak::INIT, ty: target, p: raw });
        }
        if self.types.size(target) < self.types.size(ty) {
            return Err(Fault::PlacementType { from: target, to: ty });
        }
        // An object holding no data (an empty class) may share its bytes
        // with another subobject (an empty base, `[[no_unique_address]]`):
        // its construction touches none of them. A complete object or an
        // array element owns its bytes: their lifetime is its own, which
        // its destruction checks (`end_life`; `std::vector<std::bitset<0>>`).
        let owns = self.element_type(a.ty, off) == Some(target);
        let size = if !owns && self.types.holds_no_data(ty) { 0 } else { self.types.size(ty) };
        if a.flags & flags::HAS_UNION != 0 {
            self.place_in_unions(raw, a.ty, ty, true)?;
        }
        self.mem.set_lifetime(raw, size, true).map_err(fault)?;
        if size == 0 && self.types.size(ty) > 0 {
            // Its lifetime begins all the same: an explicit destruction
            // (`end_life`) ended it over its whole size, and a member call
            // on the new object checks that size (`std::destroy_at` then
            // `std::construct_at` of an empty member, libc++'s
            // `__movable_box` assignment; [basic.life]/8). Only the
            // lifetime bits are set: the bytes' values stay the sharer's.
            self.mem.revive(raw, self.types.size(ty)).map_err(fault)?;
        }
        if aux & new_aux::ZERO != 0 && size > 0 {
            let (st, abs, _) = self.mem.check(raw, size, true).map_err(fault)?;
            self.mem.zero(st, abs, size);
            if self.types.info(ty).has_union {
                self.zero_unions(raw, 0, ty);
            }
        }
        Ok(())
    }

    /// clang 22's `HandlePlacementNew` rule: an object of type `s` may
    /// hold a new `t` when its array extent is at least `t`'s and their
    /// element types are similar.
    fn placement_fits(&self, s: OTy, t: OTy) -> bool {
        let extent = |t: OTy| match self.types.kind(t) {
            TyKind::Array { n, .. } => n,
            _ => 1,
        };
        let base = |mut t: OTy| {
            while let TyKind::Array { elem, .. } = self.types.kind(t) {
                t = elem;
            }
            t
        };
        extent(s) >= extent(t) && base(s) == base(t)
    }

    /// The type of the complete object or array element at offset `off`
    /// of a `root` (arrays stripped), if `off` is where one begins.
    fn element_type(&self, mut t: OTy, mut off: u64) -> Option<OTy> {
        while let TyKind::Array { elem, n } = self.types.kind(t) {
            let es = self.types.size(elem);
            if es == 0 || off / es >= n {
                break;
            }
            off %= es;
            t = elem;
        }
        (off == 0).then_some(t)
    }

    /// The outermost object at offset `off` of a `root`.
    fn outermost_at(&self, mut t: OTy, mut off: u64) -> Option<OTy> {
        while off != 0 {
            match self.types.kind(t) {
                TyKind::Array { elem, n } => {
                    let es = self.types.size(elem);
                    if es == 0 || off / es >= n {
                        return None;
                    }
                    off %= es;
                    t = elem;
                }
                TyKind::Record { .. } => match self.types.field_at(t, off) {
                    Some(f) if f.width.is_none() => {
                        off -= f.offset;
                        t = f.ty;
                    }
                    _ => return None,
                },
                _ => return None,
            }
        }
        Some(t)
    }

    /// The unions on the path to a placement new of a `ty` at `raw` (in an
    /// allocation of type `root`): clang's `findSubobject` with
    /// `AK_Construct` makes an inactive union member the active one when
    /// it is the constructed object itself, and diagnoses one holding it
    /// ("construction of subobject of member 'p' of union with active
    /// member 'a'", [class.union.general]/6, [expr.const]/5.10). Members of
    /// `ty` at the object's offset are the ones the pointer may name; when
    /// several may (two members of one type, or a member and a subobject
    /// of the active member), each of them is recorded as possibly active
    /// (`Memory::union_alts`, WP-233).
    ///
    /// `implicit` (placement new): an element of an inactive array member
    /// activates the array (below); `__builtin_start_lifetime` passes
    /// false, the array then being a containing object outside its
    /// lifetime ("construction of subobject of member").
    ///
    /// A walk that succeeds and changes nothing is remembered ([`umemo`],
    /// WP-760): while the active members stay as they are, the same
    /// construction again would do nothing.
    fn place_in_unions(
        &mut self,
        raw: u64,
        root: OTy,
        ty: OTy,
        implicit: bool,
    ) -> Result<(), Fault> {
        let p = Ptr::decode(raw);
        let akey = Ptr::alloc_key(raw);
        let stamp = umemo::Stamp { epoch: self.mem.union_epoch, rebuilt: self.types.rebuilt() };
        if self.umemo.hit(umemo::Walk::Place, akey, root, ty, p.off, stamp) {
            return Ok(());
        }
        if let Some(cover) = self.place_walk(raw, root, ty, implicit)? {
            self.umemo.put(umemo::Walk::Place, akey, root, ty, cover, stamp);
        }
        Ok(())
    }

    /// [`Oven::place_in_unions`]'s walk: on success, the offsets the same
    /// answer covers, or `None` when the walk changed an active member
    /// (or recorded possibly active ones), which is not remembered. The
    /// region is the elements of the active array member of the only
    /// union on the way, when the object is one of them: the walk toward
    /// any of them makes the same choices (no two objects of one type
    /// share an address, [intro.object]/9, so each non-union record
    /// leads toward that union; at it, the active array member holds
    /// every element).
    fn place_walk(
        &mut self,
        raw: u64,
        root: OTy,
        ty: OTy,
        implicit: bool,
    ) -> Result<Option<umemo::Cover>, Fault> {
        let target = u64::from(Ptr::decode(raw).off);
        let exact = Some(umemo::Cover::At(target as u32));
        let (mut t, mut base) = (root, 0u64);
        let (mut clean, mut unions, mut region) = (true, 0u32, None);
        loop {
            if t == ty && base == target {
                return Ok(if !clean {
                    None
                } else if unions == 1 && region.is_some() {
                    region
                } else {
                    exact
                });
            }
            match self.types.kind(t) {
                TyKind::Array { elem, n } => {
                    let es = self.types.size(elem).max(1);
                    let i = (target - base) / es;
                    if i >= n {
                        return Ok(exact.filter(|_| clean));
                    }
                    base += i * es;
                    t = elem;
                }
                TyKind::Record { .. } => {
                    let (at, u, reach) = match self.types.hop(Walk::Place, t, target - base, ty) {
                        Hop::Reached | Hop::Stop | Hop::Mutable(_) => {
                            return Ok(exact.filter(|_| clean));
                        }
                        Hop::Array { at, t: arr } => {
                            base += at;
                            t = arr;
                            continue;
                        }
                        Hop::Union { at, u, reach } => (at, u, reach),
                    };
                    base += at;
                    t = u;
                    unions += 1;
                    let rel = target - base;
                    let fields = self.types.fields(t);
                    let reach = |i: u32| self.types.reaches(Walk::Place, u, rel, ty, reach, i);
                    let key = Ptr::union_key(raw, base, t);
                    let active = self.mem.unions.get(&key).copied();
                    let alts = self.mem.union_alts.get(&key).copied().unwrap_or(0);
                    let n = fields.len().min(64) as u32;
                    // The members that are the constructed object.
                    let direct: u64 = (0..n)
                        .filter(|&i| {
                            let f = fields[i as usize];
                            f.width.is_none()
                                && f.offset == rel
                                && self.types.same_unqualified(f.ty, ty)
                        })
                        .fold(0, |m, i| m | 1 << i);
                    // C++26: the array members of whose elements the
                    // object is one (the `inplace_vector` pattern, WP-743).
                    let arrs: u64 = if self.cxx_year >= 2026 {
                        (0..n)
                            .filter(|&i| {
                                let f = fields[i as usize];
                                f.width.is_none()
                                    && rel >= f.offset
                                    && self.array_element_at(f.ty, rel - f.offset, ty)
                            })
                            .fold(0, |m, i| m | 1 << i)
                    } else {
                        0
                    };
                    // The (possibly) active member holding it.
                    let held =
                        active.into_iter().chain(alt_members(alts, n, active)).find(|&i| reach(i));
                    match held {
                        None if direct == 0 && arrs != 0 && implicit => {
                            // [class.union.general] (P3726R2): constructing
                            // an element of an inactive array member begins
                            // the array's lifetime and makes it the active
                            // member, its other elements holes (GCC 17;
                            // spec/oven.md "Unions").
                            let size = self.types.size(t);
                            let at = Ptr::with_off(raw, base, false);
                            self.mem.trial_write(at, size).map_err(|f| Fault::Mem {
                                f,
                                ak: ak::INIT,
                                ty,
                                p: raw,
                            })?;
                            let i = arrs.trailing_zeros();
                            let (fo, ft) = (fields[i as usize].offset, fields[i as usize].ty);
                            clean = false;
                            self.activate(raw, base, t, i);
                            if arrs.count_ones() > 1 {
                                self.mem.union_alts_put(key, Some(arrs));
                            }
                            base += fo;
                            t = ft;
                        }
                        None if direct == 0 => {
                            let Some(f) = self
                                .types
                                .near(t, rel)
                                .find(|f| self.types.union_member_reaches(Walk::Place, rel, ty, f))
                            else {
                                return Ok(exact.filter(|_| clean));
                            };
                            return Err(Fault::Union {
                                ak: ak::CONSTRUCT,
                                field: f.decl,
                                active: active.map(|i| fields[i as usize].decl),
                            });
                        }
                        None => {
                            let size = self.types.size(t);
                            let at = Ptr::with_off(raw, base, false);
                            self.mem.trial_write(at, size).map_err(|f| Fault::Mem {
                                f,
                                ak: ak::INIT,
                                ty,
                                p: raw,
                            })?;
                            self.activate(raw, base, t, direct.trailing_zeros());
                            if (direct | arrs).count_ones() > 1 {
                                self.mem.union_alts_put(key, Some(direct | arrs));
                            }
                            return Ok(None);
                        }
                        Some(i) if arrs & 1 << i != 0 => {
                            // An element of the active array member: the
                            // array is the member (a possibly active one
                            // becomes the active one).
                            if active != Some(i) {
                                clean = false;
                                self.mem.union_put(key, Some(i));
                            }
                            let f = fields[i as usize];
                            base += f.offset;
                            t = f.ty;
                            region = self.place_region(base, t, ty);
                        }
                        Some(i) if (direct | arrs) & !(1 << i) == 0 => {
                            let f = fields[i as usize];
                            base += f.offset;
                            t = f.ty;
                        }
                        Some(i) => {
                            self.mem.union_alts_put(key, Some(alts | direct | arrs | 1 << i));
                            return Ok(None);
                        }
                    }
                }
                _ => return Ok(exact.filter(|_| clean)),
            }
        }
    }

    /// The elements of type `ty` of the array member of type `t` at byte
    /// `base`, as a [`umemo::Cover`] ([`Oven::place_walk`]).
    fn place_region(&self, base: u64, t: OTy, ty: OTy) -> Option<umemo::Cover> {
        let (len, stride) = (self.types.size(t), self.types.size(ty));
        (stride > 0 && len.is_multiple_of(stride) && u32::try_from(base + len).is_ok()).then_some(
            umemo::Cover::Region {
                lo: base as u32,
                len: len as u32,
                stride: stride as u32,
                phase: 0,
            },
        )
    }

    /// Whether the byte at `off` of an object of type `t` begins an element
    /// of type `ty` of it: `t` is an array (of arrays) of `ty`.
    fn array_element_at(&self, mut t: OTy, mut off: u64, ty: OTy) -> bool {
        let mut array = false;
        loop {
            match self.types.kind(t) {
                TyKind::Array { elem, n } => {
                    let es = self.types.size(elem);
                    if es == 0 || off / es >= n {
                        return false;
                    }
                    off %= es;
                    (t, array) = (elem, true);
                }
                _ => return array && off == 0 && self.types.same_unqualified(t, ty),
            }
        }
    }

    /// `Op::EndLife`: an explicit destruction of the `ty` object at `raw`
    /// (aux 1: check that its lifetime has not ended; aux 2: end it).
    fn end_life(&mut self, raw: u64, ty: OTy, aux: u32) -> Result<(), Fault> {
        if aux & 8 != 0 {
            // aux 16: an empty base, recorded by identity (WP-401).
            let n = if aux & 16 != 0 { 0 } else { self.types.copy_size(ty) };
            return self.mem.set_unconstructed(raw, n, ty).map_err(|f| Fault::Mem {
                f,
                ak: ak::INIT,
                ty,
                p: raw,
            });
        }
        let size = self.types.size(ty);
        if aux & 4 != 0 {
            // A member call (clang's `checkDynamicType` with
            // `AK_MemberCall`): the lifetime of `std::allocator` storage
            // and the active members of the unions on the path to the
            // object ([class.union.general]/6, WP-224); a `this` that
            // designates no object within its lifetime: one past the end
            // ("member call on dereferenced one-past-the-end pointer"), a
            // dead or reused allocation ("member call on object outside
            // its lifetime"; [expr.const]/10.8, [basic.life]/7, WP-948).
            // Other faults (an object not usable in constant expressions,
            // say) surface in the body, at the accesses that make them.
            let (abs, a) = match self.mem.check_storage(raw, size, false) {
                Ok((_, abs, a)) => (abs, a),
                Err(f @ (MemFault::PastEnd | MemFault::Dead(_) | MemFault::Stale)) => {
                    return Err(Fault::Mem { f, ak: ak::MEMBER_CALL, ty, p: raw });
                }
                Err(_) => return Ok(()),
            };
            if a.flags & flags::STD_ALLOC != 0 && !self.mem.heap.is_live(abs, size) {
                return Err(Fault::Mem { f: MemFault::Lifetime, ak: ak::MEMBER_CALL, ty, p: raw });
            }
            if a.flags & flags::HAS_UNION != 0 {
                self.union_path_in(a.ty, raw, ty, ak::MEMBER_CALL)?;
            }
            return self.member_call_life(raw, ty);
        }
        let fault = |f| Fault::Mem { f, ak: ak::DESTROY, ty, p: raw };
        if aux & 1 != 0 {
            match self.mem.check(raw, size, true) {
                // A union whose active member's lifetime an explicit
                // destructor call ended is itself still within its
                // lifetime ([class.union.general]/6, [basic.life]/1): its
                // bytes are the member's, so the union's entry (kept until
                // the union is destroyed, below) tells it from a union
                // destroyed already (libc++'s `expected` destroys the
                // member, then the union, then constructs it anew, WP-334).
                Err(MemFault::Lifetime)
                    if self.union_outlives_member(raw, ty) || self.live_but_unions(raw, ty) => {}
                r => {
                    r.map_err(fault)?;
                }
            }
            // C++26: a union whose active member was destroyed is alive
            // with no active member (below), so destroying a member that
            // is not active is checked as an access ("destruction of
            // member 's' of union with no active member").
            if self.cxx_year >= 2026
                && let Some(a) = self.alloc_any(Ptr::decode(raw))
                && a.flags & flags::HAS_UNION != 0
            {
                self.union_path_in(a.ty, raw, ty, ak::DESTROY)?;
            }
        }
        if aux & 2 != 0 {
            if self.cxx_year >= 2026
                && let Some((uoff, ut)) = self.union_of_member(raw, ty)
            {
                // C++26 ([class.union.general], P3726R2): destroying the
                // active member of a union leaves the union within its
                // lifetime with no active member, its storage as its
                // trivial default constructor leaves it (spec/oven.md
                // "Unions").
                let at = Ptr::with_off(raw, uoff, false);
                self.mem.trial_write(at, self.types.size(ut)).map_err(fault)?;
                self.activate(raw, uoff, ut, 0);
                self.mem.union_put(Ptr::union_key(raw, uoff, ut), None);
                return Ok(());
            }
            self.mem.set_lifetime(raw, size, false).map_err(fault)?;
            // The unions of the destroyed object lose their active members
            // with their lifetimes.
            if self.types.info(ty).has_union {
                let off = u64::from(Ptr::decode(raw).off);
                let (a0, z) = self.types.info(ty).upos;
                for i in a0..z {
                    let (o, ut) = self.types.union_pos[i as usize];
                    self.mem.union_put(Ptr::union_key(raw, off + o, ut), None);
                }
            }
        }
        Ok(())
    }

    /// The union (its offset in `raw`'s allocation and type) of which the
    /// `ty` object at `raw` is the (possibly) active member, if any.
    fn union_of_member(&self, raw: u64, ty: OTy) -> Option<(u64, OTy)> {
        let p = Ptr::decode(raw);
        let a = self.alloc_any(p)?;
        if a.flags & flags::HAS_UNION == 0 {
            return None;
        }
        let (mut t, mut base) = (a.ty, 0u64);
        let target = u64::from(p.off);
        loop {
            if t == ty && base == target {
                return None;
            }
            match self.types.kind(t) {
                TyKind::Array { elem, n } => {
                    let es = self.types.size(elem).max(1);
                    let i = (target - base) / es;
                    if i >= n {
                        return None;
                    }
                    base += i * es;
                    t = elem;
                }
                TyKind::Record { .. } => match self.types.hop(Walk::Union, t, target - base, ty) {
                    Hop::Reached | Hop::Stop | Hop::Mutable(_) => return None,
                    Hop::Array { at, t: arr } => {
                        base += at;
                        t = arr;
                    }
                    Hop::Union { at, u, reach } => {
                        base += at;
                        let rel = target - base;
                        let key = Ptr::union_key(raw, base, u);
                        let fields = self.types.fields(u);
                        let n = fields.len().min(64) as u32;
                        let i = (0..n).find(|&i| {
                            self.mem.may_be_active(key, i)
                                && self.types.reaches(Walk::Union, u, rel, ty, reach, i)
                        })?;
                        let f = fields[i as usize];
                        // With a member of an array member at the same
                        // offset possibly active too (an element was
                        // constructed through a pointer naming both and no
                        // other element since, `place_in_unions`), ending
                        // the union's member ends the element as well.
                        if f.width.is_none()
                            && f.offset == rel
                            && self.types.same_unqualified(f.ty, ty)
                        {
                            return Some((base, u));
                        }
                        base += f.offset;
                        t = f.ty;
                        if f.width.is_some() {
                            return None;
                        }
                    }
                },
                _ => return None,
            }
        }
    }

    /// Whether the union of type `ty` at `raw` (whose bytes are outside
    /// their lifetime) is still within its own: it has an active member,
    /// whose lifetime alone has ended (see `end_life`).
    fn union_outlives_member(&self, raw: u64, ty: OTy) -> bool {
        matches!(self.types.kind(ty), TyKind::Record { union: true, .. })
            && self.mem.check_storage(raw, self.types.size(ty), true).is_ok()
            && {
                let off = u64::from(Ptr::decode(raw).off);
                self.mem.unions.contains_key(&Ptr::union_key(raw, off, ty))
            }
    }

    /// Whether the class object of type `ty` at `raw` (some of whose bytes
    /// are outside their lifetime) is within its lifetime but for the
    /// members of its unions: it has bytes outside its unions, and every
    /// one of them is. A union whose member's lifetime an explicit
    /// destructor call ended has no active member, which does not end the
    /// lifetime of the object enclosing it ([class.union.general]/2,
    /// [basic.life]/1): libc++ 22's list nodes destroy the value in their
    /// anonymous union, then the node itself (`__delete_node`, WP-446).
    /// A class that is all union bytes keeps the stricter check.
    fn live_but_unions(&self, raw: u64, ty: OTy) -> bool {
        let info = self.types.info(ty);
        if !info.has_union || !matches!(info.kind, TyKind::Record { union: false, .. }) {
            return false;
        }
        let mut us: Vec<(u64, u64)> =
            self.types.unions(ty).iter().map(|&(o, u)| (o, o + self.types.size(u))).collect();
        us.sort_unstable();
        let off = u64::from(Ptr::decode(raw).off);
        let live = |a: u64, b: u64| {
            self.mem.check(Ptr::with_off(raw, off + a, false), b - a, true).is_ok()
        };
        let (mut at, mut rest) = (0, 0);
        for (a, b) in us.into_iter().chain([(info.size, info.size)]) {
            if a > at {
                if !live(at, a) {
                    return false;
                }
                rest += a - at;
            }
            at = at.max(b);
        }
        rest > 0
    }

    /// The most derived object containing the byte at `off` of an object
    /// of type `t` (a complete object, member or array element of class
    /// type; base subobjects belong to their derived object).
    fn dynamic_type(&self, mut t: OTy, mut off: u64) -> OTy {
        loop {
            match self.types.kind(t) {
                TyKind::Array { elem, n } => {
                    let es = self.types.size(elem);
                    if es == 0 || off / es >= n {
                        return t;
                    }
                    off %= es;
                    t = elem;
                }
                TyKind::Record { .. } => {
                    let f = self.types.field_at(t, off);
                    match f {
                        Some(f)
                            if !f.base
                                && matches!(
                                    self.types.kind(f.ty),
                                    TyKind::Record { .. } | TyKind::Array { .. }
                                ) =>
                        {
                            off -= f.offset;
                            t = f.ty;
                        }
                        _ => return t,
                    }
                }
                _ => return t,
            }
        }
    }

    fn set_active(&mut self, raw: u64, idx: u32, uty: OTy, init: bool) -> Result<(), Fault> {
        let size = self.types.size(uty);
        let k = if init { ak::INIT } else { ak::ASSIGN };
        let (st, _, a) = self.mem.check(raw, size, true).map_err(|f| Fault::Mem {
            f,
            ak: k,
            ty: uty,
            p: raw,
        })?;
        if !init && let Some(f) = write_fault(st, a, uty) {
            return Err(Fault::Mem { f, ak: k, ty: uty, p: raw });
        }
        let off = u64::from(Ptr::decode(raw).off);
        let key = Ptr::union_key(raw, off, uty);
        if self.mem.may_be_active(key, idx) {
            // One of the members a placement new may have constructed
            // (`Memory::union_alts`) is now the active one.
            if !self.mem.union_alts.is_empty() && self.mem.union_alts.contains_key(&key) {
                self.mem.union_put(key, Some(idx));
            }
            return Ok(());
        }
        self.mem.trial_write(raw, size).map_err(|f| Fault::Mem { f, ak: k, ty: uty, p: raw })?;
        self.activate(raw, off, uty, idx);
        Ok(())
    }

    /// Makes field `idx` the active member of the union of type `uty` at
    /// byte `off` of `raw`'s allocation: the union's bytes become
    /// uninitialized and the entries of the unions nested in it are
    /// cleared ([class.union.general]/6). The caller checks the write.
    fn activate(&mut self, raw: u64, off: u64, uty: OTy, idx: u32) {
        let size = self.types.size(uty);
        let Ok((st, abs, _)) = self.mem.check_storage(Ptr::with_off(raw, off, false), size, true)
        else {
            return;
        };
        let sp = self.mem.space_mut(st);
        sp.set_init(abs, size, false);
        if size > 0 && abs % crate::mem::GRAN == 0 && size.is_multiple_of(crate::mem::GRAN) {
            sp.set_ptr(abs, size, false);
        }
        let (a0, z) = self.types.info(uty).upos;
        for i in a0..z {
            let (o, ut) = self.types.union_pos[i as usize];
            self.mem.union_put(Ptr::union_key(raw, off + o, ut), None);
        }
        self.mem.union_put(Ptr::union_key(raw, off, uty), Some(idx));
    }

    /// `Op::MpAddr`: the member of the object at `raw` that member
    /// pointer `v` designates (clang's `HandleMemberPointerAccess`).
    /// `null_field` (clang 22): a null object fails a data member access
    /// at the member pointer, "cannot access field of null pointer".
    fn mp_addr(&mut self, raw: u64, v: u128, null_field: bool) -> Result<u64, Fault> {
        // [expr.mptr.oper]/6: a null member pointer (clang's generic
        // note at the operand).
        let Some(e) = self.mp.get(v).filter(|e| !e.pending) else { return Err(Fault::Invalid) };
        let (derived, adj, cls, field) = (e.derived, e.adj, e.cls, e.field);
        let e_path_empty = e.path.is_empty();
        if raw == 0 {
            // clang steps a null object along the member pointer's base
            // path (failing on the first step) and otherwise reports it at
            // its use: the read, write or call, not the `.*`.
            return if derived {
                Err(Fault::Invalid)
            } else if !e_path_empty {
                Err(Fault::NullBase)
            } else if null_field && field.is_some() {
                Err(Fault::NullField)
            } else {
                Ok(0)
            };
        }
        let obj = if derived {
            // The object must be the base subobject of an object of the
            // member's class along the path (clang: generic note).
            let p = Ptr::decode(raw);
            let a = self.mem.alloc_of(p).map_err(|f| Fault::Mem {
                f,
                ak: ak::READ,
                ty: cls,
                p: raw,
            })?;
            match u64::from(p.off).checked_sub(adj) {
                Some(o) if !p.past && self.types.has_subobject(a.ty, o, cls) => {
                    Ptr::with_off(raw, o, false)
                }
                _ => return Err(Fault::Invalid),
            }
        } else if adj == 0 {
            raw
        } else {
            let p = Ptr::decode(raw);
            Ptr::with_off(raw, u64::from(p.off) + adj, false)
        };
        let Some((off, _, _)) = field else { return Ok(obj) };
        let p = Ptr::decode(obj);
        Ok(Ptr::with_off(obj, u64::from(p.off) + off, false))
    }

    fn active_check(&self, raw: u64, idx: u32, uty: OTy, k: u32) -> Result<(), Fault> {
        if raw == 0 {
            return Ok(());
        }
        let p = Ptr::decode(raw);
        if p.past || self.mem.alloc_of(p).is_err() {
            // The access itself reports it.
            return Ok(());
        }
        let key = Ptr::union_key(raw, u64::from(p.off), uty);
        if self.mem.may_be_active(key, idx) {
            return Ok(());
        }
        let active = self.mem.unions.get(&key).copied();
        if active.is_none()
            && matches!(self.mem.check(raw, self.types.size(uty), false), Err(MemFault::Lifetime))
        {
            // A destroyed union (`end_life` dropped its entry): the access
            // reports the lifetime fault, as for any destroyed object.
            return Ok(());
        }
        let fields = self.types.fields(uty);
        let Some(f) = fields.get(idx as usize) else { return Ok(()) };
        Err(Fault::Union {
            ak: k,
            field: f.decl,
            active: active.and_then(|i| fields.get(i as usize)).map(|f| f.decl),
        })
    }
}

/// The fault of modifying allocation `a` (in space `sp`): an
/// object whose lifetime began outside the evaluation ([expr.const]/5.16,
/// clang's `note_constexpr_modify_global`) is diagnosed before a const one
/// ([dcl.type.cv]/4).
fn write_fault(sp: Sp, a: &Alloc, ty: OTy) -> Option<MemFault> {
    if sp == Sp::Static && a.flags & flags::RESOLVING == 0 && matches!(a.origin, Origin::Global(_))
    {
        return Some(MemFault::ModifyGlobal);
    }
    if let Origin::Str(_, e) = a.origin {
        return Some(MemFault::Literal(e));
    }
    (a.flags & flags::CONST != 0).then_some(MemFault::Const(ty))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Budget;
    use crate::kbc::int_aux;
    use crate::mem::{FN_SLOT, HEAP_BIT, STATIC_BIT};
    use kiln_ast::ExprId;

    /// `ptr_add` as it was before its array step (WP-651), its sum
    /// wrapping as the release profile's did: the reference the new one
    /// must agree with.
    fn ptr_add_reference(o: &Oven, raw: u64, i: i128, elem: OTy) -> Result<u64, Fault> {
        if raw == 0 {
            return if i == 0 { Ok(0) } else { Err(Fault::NullArith) };
        }
        if i == 0 {
            return Ok(raw);
        }
        let (start, n, cur, array) = match o.extent_of(raw, elem) {
            Ok(x) => x,
            Err(_) if Ptr::is_nowhere(raw) => return o.nowhere_add(raw, i, elem),
            Err(f) => return Err(f),
        };
        let new = i128::from(cur).wrapping_add(i);
        if new < 0 || new > i128::from(n) {
            return Err(Fault::ArrayIndex { idx: new, n, array });
        }
        let es = o.types.size(elem).max(1);
        let new = new as u64;
        Ok(Ptr::with_off(raw, start + new * es, new == n))
    }

    /// The `PtrAdd` op's index as it was read before WP-651.
    fn index_reference(v: u128, aux: u32) -> i128 {
        let mut i = if aux >> 8 & 1 != 0 || (aux & 0xFF) < 128 {
            v as i128
        } else {
            i128::try_from(v).unwrap_or(i128::MAX)
        };
        if aux >> 9 & 1 != 0 {
            i = i.saturating_neg();
        }
        i
    }

    fn ident(r: &Result<u64, Fault>) -> String {
        format!("{r:?}")
    }

    /// `ptr_add`, the `PtrAdd` op's step (with and without the element
    /// size's shift in its aux) and the reference agree on pointers into
    /// arrays of elements of every size class (0, odd, powers of two and
    /// not), scalars and nested arrays, in each space, dead, stale, of a
    /// wrong generation, null, function and pointing at nothing; at every
    /// element, one past the end, unaligned and far past; with element
    /// types that match the array's, do not, or match its rows; and with
    /// indices into bounds, exactly to one past the end, beyond either
    /// end and near the `i128` limits.
    #[test]
    fn ptr_add_agrees_with_the_reference() {
        let mut o = Oven::new(Budget::default());
        let scalars: Vec<OTy> = [0, 1, 3, 4, 8, 12, 16].map(|s| o.types.test_scalar(s)).to_vec();
        let other4 = o.types.test_scalar(4);
        let mut objs: Vec<(OTy, u64)> = Vec::new();
        for &s in &scalars {
            for n in [0, 1, 2, 5, 7] {
                objs.push((o.types.array_of(s, n), n));
            }
            objs.push((s, 1));
        }
        let row = o.types.array_of(scalars[3], 3);
        objs.push((o.types.array_of(row, 4), 4));
        let d = DeclId::new(3);
        let e = ExprId::new(9);
        let mut ptrs = vec![0, Ptr::with_off(NOWHERE, 5, false), (u64::from(FN_SLOT) << 33) | 4];
        for &(t, _) in &objs {
            let size = o.types.size(t);
            ptrs.push(o.mem.alloc(t, size, 0, Origin::Var(d)).unwrap());
            ptrs.push(o.mem.heap_alloc(t, size, 0, Origin::Heap(e, 0)).unwrap());
            ptrs.push(o.mem.alloc_static(t, size, 0, Origin::Global(d)));
        }
        // A dead frame object, a wrong generation, slots no allocation has.
        let mk = o.mem.mark();
        let dead = o.mem.alloc(objs[8].0, o.types.size(objs[8].0), 0, Origin::Var(d)).unwrap();
        o.mem.release(mk, &o.types);
        ptrs.push(dead);
        ptrs.push(ptrs[5] ^ 1 << 55);
        for slot in [900, HEAP_BIT + 900, STATIC_BIT + 900] {
            ptrs.push(Ptr { slot, generation: 0, past: false, off: 0 }.encode());
        }
        let mut elems = scalars.clone();
        elems.extend([other4, row]);
        let mut seen = std::collections::BTreeSet::new();
        let (mut cases, mut oks) = (0u64, 0u64);
        for &p in &ptrs {
            let size = o.mem.alloc_any(Ptr::decode(p)).map_or(16, |a| u64::from(a.size));
            for &elem in &elems {
                let es = o.types.size(elem).max(1);
                let mut offs: Vec<u64> = (0..=8).map(|k| k * es).collect();
                offs.extend([1, es - 1, es + 1, size, size + 1, size.saturating_sub(1)]);
                offs.push(u64::from(u32::MAX));
                for off in offs {
                    for past in [false, true] {
                        let raw = if p == 0 { 0 } else { Ptr::with_off(p, off, past) };
                        let mut idx: Vec<i128> = (-9..=9).collect();
                        if !Ptr::is_nowhere(raw) {
                            // `nowhere_add` multiplies by the element
                            // size unchecked, before and after WP-651.
                            idx.extend([
                                i128::from(i64::MAX),
                                i128::from(i64::MIN),
                                i128::from(u32::MAX),
                                1 << 40,
                                -(1 << 40),
                                i128::MAX,
                                i128::MAX - 3,
                                i128::MIN,
                                i128::MIN + 3,
                            ]);
                        }
                        for i in idx {
                            let want = ptr_add_reference(&o, raw, i, elem);
                            let got = o.ptr_add(raw, i, elem);
                            assert_eq!(ident(&got), ident(&want), "{raw:#x} {i} {elem:?}");
                            let negated = ptr_add_reference(&o, raw, i.saturating_neg(), elem);
                            let mut steps = vec![None];
                            if es.is_power_of_two() {
                                steps.push(Some(es.trailing_zeros()));
                            }
                            for step in steps {
                                for (neg, want) in [(false, &want), (true, &negated)] {
                                    if let Some(q) = o.ptr_add_array(raw, i, neg, elem, step) {
                                        assert_eq!(
                                            ident(&Ok(q)),
                                            ident(want),
                                            "{raw:#x} {i} {neg} {step:?}"
                                        );
                                    }
                                }
                            }
                            seen.insert(match &want {
                                Ok(_) => "Ok".to_owned(),
                                Err(f) => {
                                    let f = format!("{f:?}");
                                    f.split([' ', '{', '(']).next().unwrap_or("").to_owned()
                                }
                            });
                            oks += u64::from(want.is_ok());
                            cases += 1;
                        }
                    }
                }
            }
        }
        // The op's index decoding: every width, signedness and negation.
        // An array of 5 eight-byte elements (shift 3), at its element 2.
        let arr = objs[4 * 6 + 3].0;
        let p = o.mem.alloc(arr, o.types.size(arr), 0, Origin::Var(d)).unwrap();
        let raw = Ptr::with_off(p, 8 * 2, false);
        for width in [8u8, 16, 32, 64, 128] {
            for signed in [false, true] {
                for neg in [0, 1u32 << 9] {
                    for v in [0u128, 1, 2, 3, 4, u128::MAX, u128::MAX - 1, 1 << 127, (1 << 127) - 1]
                    {
                        let base = int_aux(width, signed) | neg;
                        let want = ptr_add_reference(&o, raw, index_reference(v, base), scalars[4]);
                        let steps: &[u32] = if width == 128 && !signed { &[0] } else { &[0, 4] };
                        for aux in steps.iter().map(|s| base | s << ptr_add_aux::SHIFT) {
                            let got = match o.ptr_add_step(raw, v, aux, scalars[4]) {
                                Some(p) => Ok(p),
                                None => match o.ptr_add_member(raw, v, aux, scalars[4]) {
                                    Some(p) => Ok(p),
                                    None => o.ptr_add_op(raw, v, aux, scalars[4]),
                                },
                            };
                            assert_eq!(ident(&got), ident(&want), "{v} {aux:#x}");
                        }
                    }
                }
            }
        }
        assert!(cases > 100_000 && oks > 10_000, "{cases} {oks}");
        for kind in ["Ok", "ArrayIndex", "Mem", "NullArith"] {
            assert!(seen.iter().any(|k| k == kind), "{seen:?}");
        }
    }
}
