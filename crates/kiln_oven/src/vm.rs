//! The KBC interpreter: one `match` per instruction over a flat register
//! file (`Oven::regs`, 128-bit slots, frame windows) and the abstract
//! memory. Faults carry the pc; notes are rendered (`notes`) only when
//! the evaluation is not silent, while the call stack is still intact.

use std::cmp::Ordering;

use kiln_ast::consts::FloatKind;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_source::Loc;

use crate::arith::{self, FloatErr, IntErr, canon};
use crate::compile::{FState, GState, float_of};
use crate::failure::{EvalFailure, FailKind};
use crate::kbc::{Op, ak, cmp, new_aux};
use crate::mem::{Alloc, MemFault, Origin, Ptr, Sp, flags};
use crate::memo::{MAX_ARGS, MemoKey, MemoVal};
use crate::ty::{OTy, TyKind};
use crate::{Oven, SemaBridge};

mod bitcast;
mod builtins;
mod virt;

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
    /// The evaluation is manifestly constant-evaluated ([expr.const]/20):
    /// `__builtin_is_constant_evaluated()` yields true. A trial fold
    /// ([`crate::Oven::try_fold`]) yields false.
    pub const_context: bool,
    /// Depth charged to the enclosing evaluation contexts (R-OVEN-1): a
    /// nested evaluation's calls count from the depth of the call that
    /// started it, plus one for the context itself.
    pub depth_base: u32,
}

impl RunState {
    /// Heap bytes held (see `Oven::footprint`).
    pub(crate) fn footprint(&self) -> usize {
        self.pending.capacity() * size_of::<Pending>()
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
    pub func: u32,
    pub pc: u32,
    pub depth: u32,
}

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
    /// clang's generic `Error(E)`: "subexpression not valid".
    Invalid,
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
    NullField,
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
    VoidCast {
        from: OTy,
        to: OTy,
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
        let r = self.run_loop(b, func, fb);
        self.cleanups.truncate(clean);
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
        let (a, bb, c) = (reg(1), reg(2), reg(3));
        let (t1, t2) = (arg(4) as u32, arg(5) as u32);
        if aux & 0xff == crate::kbc::bi::THAW_PTR {
            let cur = self.stack[self.stack.len() - 1].func;
            let top = fb + self.funcs[cur as usize].nregs as usize + 1;
            let p = match <kiln_ast::ConstValueId as kiln_ast::RawId>::from_raw(t1) {
                Some(cv) => self.thaw_ptr(b, cv, OTy(t2), top as u32),
                None => 0,
            };
            Ok(u128::from(p))
        } else {
            self.builtin(b.ast(), aux, a, bb, c, t1, t2)
        }
    }

    fn run_loop(&mut self, b: &mut dyn SemaBridge, func: u32, fb0: u32) -> Exit {
        let mut pc = self.funcs[func as usize].entry as usize;
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
            let w = self.code[pc];
            let op = Op::from_u8(w as u8);
            let aux = w >> 8;
            macro_rules! arg {
                ($i:expr) => {
                    self.code[pc + 1 + $i] as usize
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
                Op::Nop => pc += 1,
                Op::Halt => return Ok(()),
                Op::Step => {
                    let n = u64::from(aux);
                    let before = self.run.counters.steps;
                    if before + n > self.budget.steps {
                        let i = arg!(0) + (self.budget.steps - before) as usize;
                        fail!(Fault::Steps(self.step_locs[i]));
                    }
                    self.run.counters.steps = before + n;
                    pc += 2;
                }
                Op::Trap => fail!(Fault::Trap(arg!(0) as u32)),
                Op::Const => {
                    let v = u64::from(self.code[pc + 2]) | u64::from(self.code[pc + 3]) << 32;
                    reg!(0) = if aux & 1 != 0 { v as i64 as i128 as u128 } else { u128::from(v) };
                    pc += 4;
                }
                Op::Const2 => {
                    let mut v = 0u128;
                    for i in (0..4).rev() {
                        v = v << 32 | u128::from(self.code[pc + 2 + i]);
                    }
                    reg!(0) = v;
                    pc += 6;
                }
                Op::Mov => {
                    reg!(0) = reg!(1);
                    pc += 3;
                }
                Op::Jmp => pc = arg!(0),
                Op::JmpIf => {
                    pc = if reg!(0) != 0 { arg!(1) } else { pc + 3 };
                }
                Op::JmpIfNot => {
                    pc = if reg!(0) == 0 { arg!(1) } else { pc + 3 };
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
                    let (a, bb) = (reg!(1), reg!(2));
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
                        Ok(v) => reg!(0) = v,
                        Err(err) => fail!(Fault::Int { err, op, a, b: bb, aux }),
                    }
                    pc += 4;
                }
                Op::INeg => {
                    let a = reg!(1);
                    let width = aux & 0xFF;
                    let signed = aux >> 8 & 1 != 0;
                    match arith::int_bin(Op::ISub, width, signed, 0, a, 0, false) {
                        Ok(v) => reg!(0) = v,
                        Err(err) => fail!(Fault::Int { err, op, a, b: 0, aux }),
                    }
                    pc += 3;
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
                    let (a, bb) = (reg!(1), reg!(2));
                    let o =
                        if aux >> 8 & 1 != 0 { (a as i128).cmp(&(bb as i128)) } else { a.cmp(&bb) };
                    reg!(0) = u128::from(compare(aux >> 9, o));
                    pc += 4;
                }
                Op::ICast => {
                    reg!(0) = canon(reg!(1), aux >> 9 & 0xFF, aux >> 17 & 1 != 0);
                    pc += 3;
                }
                Op::EnumCheck => {
                    let w = |i: usize| u64::from(self.code[pc + i]);
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
                    if oob {
                        self.run.impure += 1;
                        if self.run.enum_check && !self.run.silent {
                            let v =
                                if fs { raw.cast_signed().to_string() } else { raw.to_string() };
                            self.enum_range_diag(b, pc, &v, min, max);
                        }
                    }
                    pc += 6;
                }
                Op::ToBool | Op::PtrToBool => {
                    reg!(0) = u128::from(reg!(1) != 0);
                    pc += 3;
                }
                Op::FAdd | Op::FSub | Op::FMul | Op::FDiv => {
                    match arith::float_bin(op, float_of(aux), reg!(1), reg!(2)) {
                        Ok(v) => reg!(0) = v,
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
                Op::PtrAdd => {
                    let raw = reg!(1) as u64;
                    let v = reg!(2);
                    let mut i = if aux >> 8 & 1 != 0 || (aux & 0xFF) < 128 {
                        v as i128
                    } else {
                        i128::try_from(v).unwrap_or(i128::MAX)
                    };
                    if aux >> 9 & 1 != 0 {
                        i = i.saturating_neg();
                    }
                    match self.ptr_add(raw, i, OTy(arg!(3) as u32)) {
                        Ok(p) => reg!(0) = u128::from(p),
                        Err(f) => fail!(f),
                    }
                    pc += 5;
                }
                Op::PtrSub => {
                    match self.ptr_sub(reg!(1) as u64, reg!(2) as u64, OTy(arg!(3) as u32)) {
                        Ok(d) => reg!(0) = d as u128,
                        Err(f) => fail!(f),
                    }
                    pc += 5;
                }
                Op::PtrCmp => {
                    match self.ptr_cmp(reg!(1) as u64, reg!(2) as u64, OTy(arg!(3) as u32), aux) {
                        Ok(r) => reg!(0) = u128::from(r),
                        Err(f) => fail!(f),
                    }
                    pc += 5;
                }
                Op::Field => {
                    let raw = reg!(1) as u64;
                    if raw == 0 {
                        // aux 1 (`.` on `*null`): stay null, so the access
                        // reports the dereferenced null pointer.
                        if aux == 0 {
                            fail!(Fault::NullField);
                        }
                        reg!(0) = 0;
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
                Op::Copy => {
                    let ty = OTy(arg!(2) as u32);
                    if let Err(f) = self.copy_obj(reg!(0) as u64, reg!(1) as u64, ty, aux & 1 != 0)
                    {
                        fail!(f);
                    }
                    pc += 4;
                }
                Op::ZeroInit => {
                    let ty = OTy(arg!(1) as u32);
                    let raw = reg!(0) as u64;
                    let size = self.types.size(ty);
                    match self.mem.check(raw, size, true) {
                        Ok((st, abs, _)) => {
                            self.mem.zero(st, abs, size);
                            if self.types.info(ty).has_union {
                                self.zero_unions(raw, u64::from(Ptr::decode(raw).off), ty);
                            }
                        }
                        Err(f) => fail!(Fault::Mem { f, ak: ak::INIT, ty, p: raw }),
                    }
                    pc += 3;
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
                Op::New => {
                    let elem = OTy(arg!(1) as u32);
                    let e = match self.origins[arg!(3)] {
                        Origin::Heap(e, _) => e,
                        _ => ExprId::new(0),
                    };
                    let origin = Origin::Heap(e, self.mem.heap_allocs as u32);
                    let bound = if aux & new_aux::ARRAY != 0 {
                        match self.new_bound(reg!(2), aux, elem, arg!(4) as u64) {
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
                    if aux & 1 == 0 && raw != 0 {
                        match self.delete_base(raw, pointee, aux & 4 != 0) {
                            Ok(Some((r, t))) => (raw, pointee) = (r, t),
                            Ok(None) => {}
                            Err(f) => fail!(f),
                        }
                    }
                    // aux 2: the checks only (before the destructors run).
                    let r = if aux & 2 != 0 {
                        self.mem.heap_check(raw, aux & 1 != 0, Some(pointee), &self.types).map(drop)
                    } else {
                        self.mem.heap_free(raw, aux & 1 != 0, Some(pointee), &self.types)
                    };
                    if let Err(f) = r {
                        fail!(Fault::Mem { f, ak: ak::READ, ty: pointee, p: raw });
                    }
                    pc += 3;
                }
                Op::Builtin => {
                    match self.exec_builtin(b, pc, fb, aux) {
                        Ok(v) => reg!(0) = v,
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
                        match self.run_cleanup(b, pc, fb) {
                            Ok(Some((npc, nfb))) => {
                                pc = npc;
                                fb = nfb;
                            }
                            Ok(None) => {}
                            Err((f, at)) => return Err((f, at)),
                        }
                        continue;
                    }
                    self.mem.release(m as u64, &self.types);
                    pc += 2;
                }
                Op::PushCleanup => {
                    let raw = reg!(0) as u64;
                    if let Err(f) =
                        self.push_cleanup(raw, arg!(1) as u32, OTy(arg!(2) as u32), aux, pc)
                    {
                        fail!(f);
                    }
                    pc += 4;
                }
                Op::MakeConst => {
                    let p = Ptr::decode(reg!(0) as u64);
                    if reg!(0) != 0 && self.mem.alloc_of(p).is_ok() {
                        self.mem.alloc_of_mut(p).flags |= flags::CONST;
                    }
                    pc += 2;
                }
                Op::SetActive => {
                    let raw = reg!(0) as u64;
                    if let Err(f) =
                        self.set_active(raw, arg!(1) as u32, OTy(arg!(2) as u32), aux & 1 != 0)
                    {
                        fail!(f);
                    }
                    pc += 4;
                }
                Op::ActiveCheck => {
                    let raw = reg!(0) as u64;
                    if let Err(f) = self.active_check(raw, arg!(1) as u32, OTy(arg!(2) as u32), aux)
                    {
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
                    match self.mp_addr(raw, v) {
                        Ok(q) => reg!(0) = u128::from(q),
                        Err(f) => fail!(f),
                    }
                    pc += 4;
                }
                Op::MpActive => {
                    if let Some(e) = self.mp.get(reg!(1))
                        && e.union
                        && let Some((_, _, idx)) = e.field
                    {
                        // A union's members are all at its offset 0.
                        let cls = e.cls;
                        if let Err(f) = self.active_check(reg!(0) as u64, idx, cls, aux) {
                            fail!(f);
                        }
                    }
                    pc += 3;
                }
                Op::MpFn => match self.mp.get(reg!(1)) {
                    Some(e) if e.virt => {
                        let (m, cls) = (e.member, e.cls);
                        match self.vcall(b, m, cls, reg!(2) as u64) {
                            Ok((f, this)) => {
                                reg!(0) = u128::from(crate::mem::fn_ptr(f));
                                reg!(2) = u128::from(this);
                            }
                            Err(f) => fail!(f),
                        }
                        pc += 4;
                    }
                    Some(e) => {
                        reg!(0) = u128::from(crate::mem::fn_ptr(e.fn_index));
                        pc += 4;
                    }
                    None => fail!(Fault::Invalid),
                },
                Op::SetVptr => {
                    if let Err(f) = self.set_vptrs(b, reg!(0) as u64, OTy(arg!(1) as u32)) {
                        fail!(f);
                    }
                    pc += 3;
                }
                Op::VCall => {
                    let base = arg!(1);
                    let slot = fb + base + aux as usize;
                    let m = DeclId::new(self.code[pc + 1]);
                    let cls = OTy(self.code[pc + 3]);
                    let callee = match self.vcall(b, m, cls, self.regs[slot] as u64) {
                        Ok((f, this)) => {
                            self.regs[slot] = u128::from(this);
                            f
                        }
                        Err(f) => fail!(f),
                    };
                    match self.enter(b, callee, pc, pc + 4, fb, base) {
                        Ok(Some((npc, nfb))) => {
                            pc = npc;
                            fb = nfb;
                        }
                        Ok(None) => pc += 4,
                        Err(f) => fail!(f),
                    }
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
                        let Some(q) = QualType::from_raw(arg!(1) as u32) else {
                            fail!(Fault::Invalid)
                        };
                        self.type_info(q, ti)
                    } else {
                        match self.type_info_dyn(reg!(1) as u64, OTy(arg!(3) as u32), ti) {
                            Ok(r) => r,
                            Err(f) => fail!(f),
                        }
                    };
                    reg!(0) = u128::from(r);
                    pc += 5;
                }
                Op::IsConstEval => {
                    let cc = self.run.const_context;
                    reg!(0) = u128::from(cc);
                    if cc && !self.run.silent {
                        self.always_true_diag(b, pc, aux);
                    }
                    pc += 2;
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
                        match self.run_cleanup(b, pc, fb) {
                            Ok(Some((npc, nfb))) => {
                                pc = npc;
                                fb = nfb;
                            }
                            Ok(None) => {}
                            Err((f, at)) => return Err((f, at)),
                        }
                        continue;
                    }
                    let v = reg!(0);
                    self.regs[fb] = v;
                    let Some(fr) = self.stack.pop() else { return Ok(()) };
                    if fr.mark != BOTTOM {
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
                    let g = arg!(1);
                    let p = match self.globals[g].state {
                        GState::Resolved(p) => p,
                        GState::Unresolved => {
                            let cur = self.stack[self.stack.len() - 1].func;
                            let top = fb + self.funcs[cur as usize].nregs as usize + 1;
                            self.resolve_global(b, g as u32, top as u32)
                        }
                    };
                    reg!(0) = u128::from(p);
                    pc += 3;
                }
                Op::VoidCast => {
                    let raw = reg!(1) as u64;
                    match self.void_cast(raw, OTy(arg!(2) as u32)) {
                        Ok(()) => reg!(0) = u128::from(raw),
                        Err(f) => fail!(f),
                    }
                    pc += 4;
                }
            }
        }
    }

    /// Pops the innermost registered destruction and enters its
    /// destructor from the instruction at `pc` (which executes again on
    /// return); the frame of the running function is at `fb`.
    #[inline(never)]
    fn run_cleanup(
        &mut self,
        b: &mut dyn SemaBridge,
        pc: usize,
        fb: usize,
    ) -> Result<Option<(usize, usize)>, (Fault, usize)> {
        let Some(c) = self.cleanups.pop() else { return Ok(None) };
        // A const object is modifiable during its destruction
        // ([class.dtor]/5).
        let p = Ptr::decode(c.this);
        if c.this != 0 && self.mem.alloc_of(p).is_ok() {
            self.mem.alloc_of_mut(p).flags &= !flags::CONST;
        }
        let cur = self.stack.last().map_or(0, |f| f.func);
        let base = self.funcs[cur as usize].nregs as usize;
        self.grow_regs(fb + base + 2);
        self.regs[fb + base] = u128::from(c.this);
        self.enter(b, c.func, c.pc as usize, pc, fb, base).map_err(|f| (f, c.pc as usize))
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
        if raw == 0 {
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
        self.cleanups.reserve(n as usize);
        for i in 0..n {
            let this = if i == 0 { raw } else { Ptr::with_off(raw, off + i * es, false) };
            self.cleanups.push(Cleanup { this, func, pc: pc as u32, depth });
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
            self.compile_func(b, callee);
        }
        let f = &self.funcs[callee as usize];
        if f.state != FState::Ready {
            return Err(Fault::Callee(callee));
        }
        let active = self.active_calls();
        if active >= self.budget.depth {
            return Err(Fault::Depth);
        }
        let nb = fb + base;
        let memo = f.memo && self.memo.enabled();
        let (entry, nregs) = (f.entry as usize, f.nregs as usize);
        if memo {
            // A call's result can depend on the evaluation context
            // (`__builtin_is_constant_evaluated` in a callee): the context
            // is part of the key.
            let func = callee | u32::from(self.run.const_context) << 31;
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
        Ok(Some((entry, nb)))
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
        if self.run.impure == p.impure0 && self.mem.heap_ops == p.heap0 {
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
    fn new_alloc(&mut self, elem: OTy, n: u64, aux: u32, origin: Origin) -> Result<u64, Fault> {
        let (ty, fl) = if n == u64::MAX {
            (elem, 0)
        } else {
            (self.types.array_of(elem, n), flags::ARRAY_NEW)
        };
        let info = *self.types.info(ty);
        let fl = fl | if info.has_union { flags::HAS_UNION } else { 0 };
        let raw = self.mem.heap_alloc(ty, info.size, fl, origin).map_err(|_| Fault::Memory)?;
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
            && e == elem
        {
            return Ok((0, n, off / es, true));
        }
        match self.types.extent(a.ty, off, p.past, elem) {
            Some((start, n, arr)) => Ok((start, n, (off - start) / es, arr)),
            None => Err(Fault::ArrayIndex { idx: 1, n: 1, array: false }),
        }
    }

    fn ptr_add(&self, raw: u64, i: i128, elem: OTy) -> Result<u64, Fault> {
        if raw == 0 {
            return if i == 0 { Ok(0) } else { Err(Fault::NullArith) };
        }
        if i == 0 {
            return Ok(raw);
        }
        let (start, n, cur, array) = self.extent_of(raw, elem)?;
        let new = i128::from(cur) + i;
        if new < 0 || new > i128::from(n) {
            return Err(Fault::ArrayIndex { idx: new, n, array });
        }
        let es = self.types.size(elem).max(1);
        let new = new as u64;
        Ok(Ptr::with_off(raw, start + new * es, new == n))
    }

    fn ptr_sub(&self, p: u64, q: u64, elem: OTy) -> Result<i64, Fault> {
        if p == 0 && q == 0 {
            return Ok(0);
        }
        if p == 0 || q == 0 || Ptr::alloc_key(p) != Ptr::alloc_key(q) {
            // Different complete objects: clang has no specific note.
            return Err(Fault::Invalid);
        }
        let (sp, _, ip, _) = self.extent_of(p, elem).map_err(|_| Fault::PtrSub)?;
        let (sq, _, iq, _) = self.extent_of(q, elem).map_err(|_| Fault::PtrSub)?;
        if sp != sq {
            return Err(Fault::PtrSub);
        }
        Ok(ip as i64 - iq as i64)
    }

    fn is_literal(&self, raw: u64) -> bool {
        let p = Ptr::decode(raw);
        self.alloc_any(p).is_some_and(|a| matches!(a.origin, Origin::Str(..)))
    }

    /// One past the end of a complete object.
    fn past_complete(&self, raw: u64) -> bool {
        let p = Ptr::decode(raw);
        self.alloc_any(p).is_some_and(|a| p.off >= a.size && a.size != 0)
    }

    fn ptr_cmp(&self, p: u64, q: u64, pointee: OTy, c: u32) -> Result<bool, Fault> {
        let eq = c == cmp::EQ || c == cmp::NE;
        let same =
            (p == 0 && q == 0) || (p != 0 && q != 0 && Ptr::alloc_key(p) == Ptr::alloc_key(q));
        if same {
            let (a, b) = (Ptr::decode(p), Ptr::decode(q));
            return Ok(compare(c, a.off.cmp(&b.off)));
        }
        if !eq {
            return Err(Fault::PtrCmp { p, q, pointee });
        }
        if p != 0 && q != 0 {
            if self.is_literal(p) || self.is_literal(q) {
                return Err(Fault::LiteralCmp);
            }
            if Ptr::decode(p).off == 0 && !Ptr::decode(p).is_fn() && self.past_complete(q) {
                return Err(Fault::PtrPastCmp { p: q, pointee });
            }
            if Ptr::decode(q).off == 0 && !Ptr::decode(q).is_fn() && self.past_complete(p) {
                return Err(Fault::PtrPastCmp { p, pointee });
            }
        }
        Ok(c == cmp::NE)
    }

    fn void_cast(&self, raw: u64, to: OTy) -> Result<(), Fault> {
        if raw == 0 {
            return Ok(());
        }
        let p = Ptr::decode(raw);
        let Some(a) = self.alloc_any(p) else { return Ok(()) };
        let off = u64::from(p.off);
        let target = if p.past { off.saturating_sub(self.types.size(to).max(1)) } else { off };
        if self.types.has_subobject(a.ty, target, to) {
            return Ok(());
        }
        let from = self.deepest_at(a.ty, off);
        Err(Fault::VoidCast { from, to })
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
                    let Some(f) = self.types.fields(t).iter().find(|f| {
                        f.width.is_none()
                            && off >= f.offset
                            && off < f.offset + self.types.size(f.ty).max(1)
                    }) else {
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

    /// Checks the active members of the unions on the path from the
    /// allocation root to the `len`-byte object of type `ty` at `raw`.
    fn union_path(&self, raw: u64, ty: OTy, k: u32) -> Result<(), Fault> {
        let p = Ptr::decode(raw);
        let Some(a) = self.alloc_any(p) else { return Ok(()) };
        let (mut t, mut base) = (a.ty, 0u64);
        let target = u64::from(p.off);
        loop {
            if t == ty && base == target {
                return Ok(());
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
                TyKind::Record { union, .. } => {
                    let rel = target - base;
                    let fields = self.types.fields(t);
                    let contains = |f: &crate::ty::FieldInfo| {
                        rel >= f.offset && rel < f.offset + self.types.size(f.ty).max(1)
                    };
                    if union {
                        let active = self.mem.unions.get(&Ptr::union_key(raw, base)).copied();
                        let reach = |f: &crate::ty::FieldInfo| {
                            contains(f)
                                && (f.width.is_some()
                                    || self.types.has_subobject(f.ty, rel - f.offset, ty))
                        };
                        if let Some(i) = active {
                            let f = fields[i as usize];
                            if reach(&f) {
                                base += f.offset;
                                t = f.ty;
                                if f.width.is_some() {
                                    return Ok(());
                                }
                                continue;
                            }
                        }
                        let Some(f) = fields
                            .iter()
                            .find(|f| reach(f))
                            .or_else(|| fields.iter().find(|f| contains(f)))
                        else {
                            return Ok(());
                        };
                        return Err(Fault::Union {
                            ak: k,
                            field: f.decl,
                            active: active.map(|i| fields[i as usize].decl),
                        });
                    }
                    let Some(f) = fields.iter().find(|f| contains(f)) else { return Ok(()) };
                    if f.width.is_some() {
                        return Ok(());
                    }
                    base += f.offset;
                    t = f.ty;
                }
                _ => return Ok(()),
            }
        }
    }

    /// The bounds check of a load; the read half of a read-modify-write
    /// (`k` other than [`ak::READ`]) is checked as the write it precedes, so
    /// modifying an object visible outside the evaluation is diagnosed
    /// before reading it ([expr.const]/5.16).
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
        if a.flags & flags::HAS_UNION != 0 {
            self.union_path(raw, ty, k)?;
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
            _ => v,
        })
    }

    fn store(&mut self, raw: u64, ty: OTy, v: u128, k: u32, init: bool) -> Result<(), Fault> {
        let info = *self.types.info(ty);
        let len = value_len(info.size, info.kind);
        let is_ptr = matches!(info.kind, TyKind::Ptr(_) | TyKind::NullPtr);
        let fault = |f| Fault::Mem { f, ak: k, ty, p: raw };
        let (st, abs, a) = self.mem.check(raw, len, true).map_err(fault)?;
        if !init && let Some(f) = write_fault(st, a, ty) {
            return Err(fault(f));
        }
        if a.flags & flags::HAS_UNION != 0 {
            self.union_path(raw, ty, k)?;
        }
        let sp = self.mem.space_mut(st);
        sp.write(abs, len, v);
        sp.set_init(abs, len, true);
        sp.set_ptr(abs, len, is_ptr);
        Ok(())
    }

    fn load_bits(&self, raw: u64, ty: OTy, bit: u32, width: u32, k: u32) -> Result<u128, Fault> {
        let n = u64::from((bit + width).div_ceil(8));
        let fault = |f| Fault::Mem { f, ak: k, ty, p: raw };
        let (st, abs, a) = self.rmw_check(raw, n, ty, k)?;
        if a.flags & flags::HAS_UNION != 0 {
            self.union_path(raw, ty, k)?;
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
        if a.flags & flags::HAS_UNION != 0 {
            self.union_path(raw, ty, k)?;
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

    fn copy_obj(&mut self, dst: u64, src: u64, ty: OTy, init: bool) -> Result<(), Fault> {
        let size = self.types.size(ty);
        let (ss, sa, _) = self.mem.check(src, size, false).map_err(|f| Fault::Mem {
            f,
            ak: ak::READ,
            ty,
            p: src,
        })?;
        let k = if init { ak::INIT } else { ak::ASSIGN };
        let (ds, da, a) =
            self.mem.check(dst, size, true).map_err(|f| Fault::Mem { f, ak: k, ty, p: dst })?;
        if !init && let Some(f) = write_fault(ds, a, ty) {
            return Err(Fault::Mem { f, ak: k, ty, p: dst });
        }
        if size == 0 {
            return Ok(());
        }
        self.mem.copy(ds, da, ss, sa, size);
        if self.types.info(ty).has_union {
            let (so, dof) = (u64::from(Ptr::decode(src).off), u64::from(Ptr::decode(dst).off));
            let (a, z) = self.types.info(ty).upos;
            for i in a..z {
                let o = self.types.union_pos[i as usize].0;
                match self.mem.unions.get(&Ptr::union_key(src, so + o)).copied() {
                    Some(v) => {
                        self.mem.unions.insert(Ptr::union_key(dst, dof + o), v);
                    }
                    None => {
                        self.mem.unions.remove(&Ptr::union_key(dst, dof + o));
                    }
                }
            }
        }
        Ok(())
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
                        let o = self.types.union_pos[i as usize].0;
                        self.mem.unions.remove(&Ptr::union_key(raw, off + o));
                    }
                    if count == 0 {
                        return;
                    }
                    self.mem.unions.insert(Ptr::union_key(raw, off), 0);
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
            || a.flags & flags::ARRAY_NEW != 0
            || !self.types.has_subobject(a.ty, off, pointee)
            || self.dynamic_type(a.ty, off) != a.ty
        {
            return Ok(None);
        }
        if !virt {
            return Err(Fault::DeleteNonVirtual { stat: pointee, dynamic: a.ty });
        }
        Ok(Some((Ptr::with_off(raw, 0, false), a.ty)))
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
                    let f = self.types.fields(t).iter().find(|f| {
                        f.width.is_none()
                            && off >= f.offset
                            && off < f.offset + self.types.size(f.ty).max(1)
                    });
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
        let (st, abs, a) = self.mem.check(raw, size, true).map_err(|f| Fault::Mem {
            f,
            ak: k,
            ty: uty,
            p: raw,
        })?;
        if !init && let Some(f) = write_fault(st, a, uty) {
            return Err(Fault::Mem { f, ak: k, ty: uty, p: raw });
        }
        let off = u64::from(Ptr::decode(raw).off);
        let key = Ptr::union_key(raw, off);
        if self.mem.unions.get(&key) == Some(&idx) {
            return Ok(());
        }
        let sp = self.mem.space_mut(st);
        sp.set_init(abs, size, false);
        if size > 0 && abs % crate::mem::GRAN == 0 && size.is_multiple_of(crate::mem::GRAN) {
            sp.set_ptr(abs, size, false);
        }
        let (a0, z) = self.types.info(uty).upos;
        for i in a0..z {
            let o = self.types.union_pos[i as usize].0;
            self.mem.unions.remove(&Ptr::union_key(raw, off + o));
        }
        self.mem.unions.insert(key, idx);
        Ok(())
    }

    /// `Op::MpAddr`: the member of the object at `raw` that member
    /// pointer `v` designates (clang's `HandleMemberPointerAccess`).
    fn mp_addr(&mut self, raw: u64, v: u128) -> Result<u64, Fault> {
        // [expr.mptr.oper]/6: a null member pointer (clang's generic
        // note at the operand).
        let Some(e) = self.mp.get(v) else { return Err(Fault::Invalid) };
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
        let active = self.mem.unions.get(&Ptr::union_key(raw, u64::from(p.off))).copied();
        if active == Some(idx) {
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
