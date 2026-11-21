//! Instructions (`spec/llvm.md` §4).
//!
//! An instruction is a 24-byte [`Inst`]: a compact [`InstKind`] plus its
//! result type. Variable-length operand lists (GEP indices, phi incoming
//! pairs, switch cases, call arguments, aggregate indices) live in per
//! function pools and are referenced by [`OpRange`].

use crate::types::TypeId;
use crate::value::{BlockId, Value};

/// Binary operators (integer and floating-point).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[allow(missing_docs)]
pub enum BinOp {
    Add,
    FAdd,
    Sub,
    FSub,
    Mul,
    FMul,
    UDiv,
    SDiv,
    FDiv,
    URem,
    SRem,
    FRem,
    Shl,
    LShr,
    AShr,
    And,
    Or,
    Xor,
}

impl BinOp {
    /// Mnemonic.
    pub fn name(self) -> &'static str {
        match self {
            BinOp::Add => "add",
            BinOp::FAdd => "fadd",
            BinOp::Sub => "sub",
            BinOp::FSub => "fsub",
            BinOp::Mul => "mul",
            BinOp::FMul => "fmul",
            BinOp::UDiv => "udiv",
            BinOp::SDiv => "sdiv",
            BinOp::FDiv => "fdiv",
            BinOp::URem => "urem",
            BinOp::SRem => "srem",
            BinOp::FRem => "frem",
            BinOp::Shl => "shl",
            BinOp::LShr => "lshr",
            BinOp::AShr => "ashr",
            BinOp::And => "and",
            BinOp::Or => "or",
            BinOp::Xor => "xor",
        }
    }

    /// True for the floating-point operators.
    pub fn is_float(self) -> bool {
        matches!(self, BinOp::FAdd | BinOp::FSub | BinOp::FMul | BinOp::FDiv | BinOp::FRem)
    }

    /// The flags this operator accepts.
    pub fn allowed_flags(self) -> Flags {
        match self {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Shl => Flags(Flags::NUW.0 | Flags::NSW.0),
            BinOp::UDiv | BinOp::SDiv | BinOp::LShr | BinOp::AShr => Flags::EXACT,
            BinOp::Or => Flags::DISJOINT,
            _ => Flags::NONE,
        }
    }
}

/// Integer comparison predicates.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[allow(missing_docs)]
pub enum IntPredicate {
    Eq,
    Ne,
    Ugt,
    Uge,
    Ult,
    Ule,
    Sgt,
    Sge,
    Slt,
    Sle,
}

impl IntPredicate {
    /// Mnemonic.
    pub fn name(self) -> &'static str {
        match self {
            IntPredicate::Eq => "eq",
            IntPredicate::Ne => "ne",
            IntPredicate::Ugt => "ugt",
            IntPredicate::Uge => "uge",
            IntPredicate::Ult => "ult",
            IntPredicate::Ule => "ule",
            IntPredicate::Sgt => "sgt",
            IntPredicate::Sge => "sge",
            IntPredicate::Slt => "slt",
            IntPredicate::Sle => "sle",
        }
    }
}

/// Floating-point comparison predicates.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[allow(missing_docs)]
pub enum FloatPredicate {
    False,
    Oeq,
    Ogt,
    Oge,
    Olt,
    Ole,
    One,
    Ord,
    Uno,
    Ueq,
    Ugt,
    Uge,
    Ult,
    Ule,
    Une,
    True,
}

impl FloatPredicate {
    /// Mnemonic.
    pub fn name(self) -> &'static str {
        match self {
            FloatPredicate::False => "false",
            FloatPredicate::Oeq => "oeq",
            FloatPredicate::Ogt => "ogt",
            FloatPredicate::Oge => "oge",
            FloatPredicate::Olt => "olt",
            FloatPredicate::Ole => "ole",
            FloatPredicate::One => "one",
            FloatPredicate::Ord => "ord",
            FloatPredicate::Uno => "uno",
            FloatPredicate::Ueq => "ueq",
            FloatPredicate::Ugt => "ugt",
            FloatPredicate::Uge => "uge",
            FloatPredicate::Ult => "ult",
            FloatPredicate::Ule => "ule",
            FloatPredicate::Une => "une",
            FloatPredicate::True => "true",
        }
    }
}

/// Conversion operators.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[allow(missing_docs)]
pub enum CastOp {
    Trunc,
    ZExt,
    SExt,
    FPTrunc,
    FPExt,
    FPToUI,
    FPToSI,
    UIToFP,
    SIToFP,
    PtrToInt,
    IntToPtr,
    BitCast,
    AddrSpaceCast,
}

impl CastOp {
    /// Mnemonic.
    pub fn name(self) -> &'static str {
        match self {
            CastOp::Trunc => "trunc",
            CastOp::ZExt => "zext",
            CastOp::SExt => "sext",
            CastOp::FPTrunc => "fptrunc",
            CastOp::FPExt => "fpext",
            CastOp::FPToUI => "fptoui",
            CastOp::FPToSI => "fptosi",
            CastOp::UIToFP => "uitofp",
            CastOp::SIToFP => "sitofp",
            CastOp::PtrToInt => "ptrtoint",
            CastOp::IntToPtr => "inttoptr",
            CastOp::BitCast => "bitcast",
            CastOp::AddrSpaceCast => "addrspacecast",
        }
    }

    /// The flags this cast accepts (`nuw`/`nsw` on trunc, `nneg` on
    /// zext/uitofp).
    pub fn allowed_flags(self) -> Flags {
        match self {
            CastOp::Trunc => Flags(Flags::NUW.0 | Flags::NSW.0),
            CastOp::ZExt | CastOp::UIToFP => Flags::NNEG,
            _ => Flags::NONE,
        }
    }
}

/// Poison-generating flags on arithmetic and casts.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Flags(pub u8);

impl Flags {
    /// No flags.
    pub const NONE: Flags = Flags(0);
    /// `nuw`.
    pub const NUW: Flags = Flags(1);
    /// `nsw`.
    pub const NSW: Flags = Flags(2);
    /// `exact`.
    pub const EXACT: Flags = Flags(4);
    /// `disjoint` (on `or`).
    pub const DISJOINT: Flags = Flags(8);
    /// `nneg` (on `zext`, `uitofp`).
    pub const NNEG: Flags = Flags(16);

    /// Union.
    #[must_use]
    pub fn with(self, o: Flags) -> Flags {
        Flags(self.0 | o.0)
    }
    /// True when every bit of `o` is set.
    pub fn contains(self, o: Flags) -> bool {
        self.0 & o.0 == o.0
    }
}

/// Fast-math flags.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct FastMath(pub u8);

impl FastMath {
    /// None.
    pub const NONE: FastMath = FastMath(0);
    /// `reassoc`.
    pub const REASSOC: FastMath = FastMath(1);
    /// `nnan`.
    pub const NNAN: FastMath = FastMath(2);
    /// `ninf`.
    pub const NINF: FastMath = FastMath(4);
    /// `nsz`.
    pub const NSZ: FastMath = FastMath(8);
    /// `arcp`.
    pub const ARCP: FastMath = FastMath(16);
    /// `contract`.
    pub const CONTRACT: FastMath = FastMath(32);
    /// `afn`.
    pub const AFN: FastMath = FastMath(64);
    /// All of the above (printed `fast`).
    pub const FAST: FastMath = FastMath(127);

    /// Union.
    #[must_use]
    pub fn with(self, o: FastMath) -> FastMath {
        FastMath(self.0 | o.0)
    }
}

/// `getelementptr` flags.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct GepFlags(pub u8);

impl GepFlags {
    /// None.
    pub const NONE: GepFlags = GepFlags(0);
    /// `inbounds` (implies `nusw`).
    pub const INBOUNDS: GepFlags = GepFlags(1);
    /// `nusw`.
    pub const NUSW: GepFlags = GepFlags(2);
    /// `nuw`.
    pub const NUW: GepFlags = GepFlags(4);

    /// Union.
    #[must_use]
    pub fn with(self, o: GepFlags) -> GepFlags {
        GepFlags(self.0 | o.0)
    }
}

/// Atomic memory ordering.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub enum Ordering {
    /// Not atomic.
    #[default]
    NotAtomic,
    /// `unordered`.
    Unordered,
    /// `monotonic`.
    Monotonic,
    /// `acquire`.
    Acquire,
    /// `release`.
    Release,
    /// `acq_rel`.
    AcqRel,
    /// `seq_cst`.
    SeqCst,
}

impl Ordering {
    /// Mnemonic.
    pub fn name(self) -> &'static str {
        match self {
            Ordering::NotAtomic => "",
            Ordering::Unordered => "unordered",
            Ordering::Monotonic => "monotonic",
            Ordering::Acquire => "acquire",
            Ordering::Release => "release",
            Ordering::AcqRel => "acq_rel",
            Ordering::SeqCst => "seq_cst",
        }
    }
}

/// A synchronization scope: system (default), `singlethread`, or a named
/// scope registered with [`crate::Module::sync_scope`].
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct SyncScope(pub(crate) u8);

impl SyncScope {
    /// The default (cross-thread) scope; not printed.
    pub const SYSTEM: SyncScope = SyncScope(0);
    /// `syncscope("singlethread")`.
    pub const SINGLE_THREAD: SyncScope = SyncScope(1);

    /// Dense index into the module's scope table (0 and 1 are the two
    /// fixed scopes; [`crate::Module::sync_scope_name`]).
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// `atomicrmw` operations.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[allow(missing_docs)]
pub enum RmwOp {
    Xchg,
    Add,
    Sub,
    And,
    Nand,
    Or,
    Xor,
    Max,
    Min,
    UMax,
    UMin,
    FAdd,
    FSub,
    FMax,
    FMin,
    UIncWrap,
    UDecWrap,
}

impl RmwOp {
    /// Mnemonic.
    pub fn name(self) -> &'static str {
        match self {
            RmwOp::Xchg => "xchg",
            RmwOp::Add => "add",
            RmwOp::Sub => "sub",
            RmwOp::And => "and",
            RmwOp::Nand => "nand",
            RmwOp::Or => "or",
            RmwOp::Xor => "xor",
            RmwOp::Max => "max",
            RmwOp::Min => "min",
            RmwOp::UMax => "umax",
            RmwOp::UMin => "umin",
            RmwOp::FAdd => "fadd",
            RmwOp::FSub => "fsub",
            RmwOp::FMax => "fmax",
            RmwOp::FMin => "fmin",
            RmwOp::UIncWrap => "uinc_wrap",
            RmwOp::UDecWrap => "udec_wrap",
        }
    }

    /// True for the floating-point operations.
    pub fn is_float(self) -> bool {
        matches!(self, RmwOp::FAdd | RmwOp::FSub | RmwOp::FMax | RmwOp::FMin)
    }
}

/// Call-site tail marker.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum TailKind {
    /// No marker.
    #[default]
    None,
    /// `tail`.
    Tail,
    /// `musttail`.
    MustTail,
    /// `notail`.
    NoTail,
}

/// A calling convention (LLVM's numeric `CallingConv::ID`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct CallConv(pub u32);

impl CallConv {
    /// `ccc` (default; not printed).
    pub const C: CallConv = CallConv(0);
    /// `fastcc`.
    pub const FAST: CallConv = CallConv(8);
    /// `coldcc`.
    pub const COLD: CallConv = CallConv(9);
    /// `preserve_mostcc`.
    pub const PRESERVE_MOST: CallConv = CallConv(14);
    /// `preserve_allcc`.
    pub const PRESERVE_ALL: CallConv = CallConv(15);
    /// `swiftcc`.
    pub const SWIFT: CallConv = CallConv(16);
    /// `cxx_fast_tlscc`.
    pub const CXX_FAST_TLS: CallConv = CallConv(17);
    /// `tailcc`.
    pub const TAIL: CallConv = CallConv(18);
    /// `swifttailcc`.
    pub const SWIFT_TAIL: CallConv = CallConv(20);
    /// `preserve_nonecc`.
    pub const PRESERVE_NONE: CallConv = CallConv(21);
    /// `x86_stdcallcc`.
    pub const X86_STDCALL: CallConv = CallConv(64);
    /// `x86_fastcallcc`.
    pub const X86_FASTCALL: CallConv = CallConv(65);
    /// `x86_thiscallcc`.
    pub const X86_THISCALL: CallConv = CallConv(70);
    /// `intel_ocl_bicc`.
    pub const INTEL_OCL_BI: CallConv = CallConv(77);
    /// `x86_64_sysvcc`.
    pub const X86_64_SYSV: CallConv = CallConv(78);
    /// `win64cc`.
    pub const WIN64: CallConv = CallConv(79);
    /// `x86_vectorcallcc`.
    pub const X86_VECTORCALL: CallConv = CallConv(80);
    /// `x86_intrcc` (an x86 `interrupt` handler).
    pub const X86_INTR: CallConv = CallConv(83);
    /// `x86_regcallcc`.
    pub const X86_REGCALL: CallConv = CallConv(92);
    /// `aarch64_vector_pcs`.
    pub const AARCH64_VECTOR_PCS: CallConv = CallConv(97);
    /// `aarch64_sve_vector_pcs`.
    pub const AARCH64_SVE_VECTOR_PCS: CallConv = CallConv(98);
    /// `aarch64_sme_preservemost_from_x0` (an SME ABI support routine
    /// that preserves `x0`-`x13` and the vector state).
    pub const AARCH64_SME_PRESERVEMOST_FROM_X0: CallConv = CallConv(102);
    /// `aarch64_sme_preservemost_from_x2` (`__arm_sme_state`).
    pub const AARCH64_SME_PRESERVEMOST_FROM_X2: CallConv = CallConv(103);
    /// `aarch64_sme_preservemost_from_x1`.
    pub const AARCH64_SME_PRESERVEMOST_FROM_X1: CallConv = CallConv(111);

    /// The textual keyword: `None` for `ccc` (not printed), the empty
    /// string for a number without a keyword (printed `cc N`).
    pub fn keyword(self) -> Option<&'static str> {
        Some(match self.0 {
            0 => return None,
            8 => "fastcc",
            9 => "coldcc",
            10 => "ghccc",
            13 => "anyregcc",
            14 => "preserve_mostcc",
            15 => "preserve_allcc",
            16 => "swiftcc",
            17 => "cxx_fast_tlscc",
            18 => "tailcc",
            19 => "cfguard_checkcc",
            20 => "swifttailcc",
            21 => "preserve_nonecc",
            64 => "x86_stdcallcc",
            65 => "x86_fastcallcc",
            66 => "arm_apcscc",
            67 => "arm_aapcscc",
            68 => "arm_aapcs_vfpcc",
            70 => "x86_thiscallcc",
            77 => "intel_ocl_bicc",
            78 => "x86_64_sysvcc",
            79 => "win64cc",
            80 => "x86_vectorcallcc",
            83 => "x86_intrcc",
            92 => "x86_regcallcc",
            97 => "aarch64_vector_pcs",
            98 => "aarch64_sve_vector_pcs",
            102 => "aarch64_sme_preservemost_from_x0",
            103 => "aarch64_sme_preservemost_from_x2",
            111 => "aarch64_sme_preservemost_from_x1",
            _ => "",
        })
    }
}

/// Alignment stored as `log2(bytes) + 1` (0 = none).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Align(pub(crate) u8);

impl Align {
    /// No alignment specified.
    pub const NONE: Align = Align(0);

    /// From a byte count: 0 means none; otherwise must be a power of two
    /// no larger than 2^32.
    pub fn from_bytes(bytes: u64) -> Option<Align> {
        if bytes == 0 {
            return Some(Align::NONE);
        }
        if !bytes.is_power_of_two() || bytes > (1 << 32) {
            return None;
        }
        Some(Align(bytes.trailing_zeros() as u8 + 1))
    }

    /// The byte count, if any.
    pub fn bytes(self) -> Option<u64> {
        (self.0 != 0).then(|| 1u64 << (self.0 - 1))
    }
}

/// A range in a function's operand pool ([`crate::Function::ops`]) or index
/// pool ([`crate::Function::idx`]); which pool is fixed by the
/// [`InstKind`] field that holds it.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct OpRange {
    pub(crate) start: u32,
    pub(crate) len: u32,
}

impl OpRange {
    /// Number of entries.
    pub fn len(self) -> usize {
        self.len as usize
    }

    /// True when the range is empty.
    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    pub(crate) fn range(self) -> std::ops::Range<usize> {
        self.start as usize..(self.start + self.len) as usize
    }
}

/// Instruction opcodes, for inspection.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[allow(missing_docs)]
pub enum Opcode {
    Binary(BinOp),
    FNeg,
    ICmp,
    FCmp,
    Cast(CastOp),
    Alloca,
    Load,
    Store,
    Fence,
    CmpXchg,
    AtomicRmw,
    GetElementPtr,
    ExtractValue,
    InsertValue,
    ExtractElement,
    InsertElement,
    ShuffleVector,
    Phi,
    Select,
    Call,
    Invoke,
    CallBr,
    LandingPad,
    Resume,
    CatchSwitch,
    CatchPad,
    CleanupPad,
    CatchRet,
    CleanupRet,
    Br,
    CondBr,
    Switch,
    IndirectBr,
    Ret,
    Unreachable,
    Freeze,
    VaArg,
}

impl Opcode {
    /// True for block terminators.
    pub fn is_terminator(self) -> bool {
        matches!(
            self,
            Opcode::Invoke
                | Opcode::CallBr
                | Opcode::Resume
                | Opcode::CatchSwitch
                | Opcode::CatchRet
                | Opcode::CleanupRet
                | Opcode::Br
                | Opcode::CondBr
                | Opcode::Switch
                | Opcode::IndirectBr
                | Opcode::Ret
                | Opcode::Unreachable
        )
    }
}

/// An instruction's payload (part of the read view, `spec/llvm.md` "Read
/// view"; [`Inst::kind`]).
///
/// The enum is exhaustive on purpose: a reader that matches it fails to
/// build when a kind is added, instead of skipping it. Operands are
/// [`Value`]s; [`OpRange`]s are ranges of the function's operand pool
/// ([`crate::Function::ops`]) or index pool ([`crate::Function::idx`]);
/// `Call`, `Invoke`, `CallBr` and `LandingPad` carry the index of their
/// side data ([`crate::Function::call_data`],
/// [`crate::Function::landing_pad`]). An absent optional operand is a value
/// for which [`Value::is_none`] holds. Operands are stored as built: a use
/// of an erased declaration reads through [`crate::Module::resolve`].
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum InstKind {
    /// A binary operator (`add`, `fmul`, ...).
    Binary {
        /// Operator.
        op: BinOp,
        /// `nuw`/`nsw`/`exact`/`disjoint` (only those the operator allows).
        flags: Flags,
        /// Fast-math flags (floating-point operators).
        fmf: FastMath,
        /// Left operand.
        lhs: Value,
        /// Right operand.
        rhs: Value,
    },
    /// `fneg`.
    FNeg {
        /// Fast-math flags.
        fmf: FastMath,
        /// Operand.
        val: Value,
    },
    /// `icmp`.
    ICmp {
        /// Predicate.
        pred: IntPredicate,
        /// Left operand.
        lhs: Value,
        /// Right operand.
        rhs: Value,
    },
    /// `fcmp`.
    FCmp {
        /// Predicate.
        pred: FloatPredicate,
        /// Fast-math flags.
        fmf: FastMath,
        /// Left operand.
        lhs: Value,
        /// Right operand.
        rhs: Value,
    },
    /// A conversion; the destination type is the instruction's type.
    Cast {
        /// Conversion.
        op: CastOp,
        /// `nuw`/`nsw` (`trunc`), `nneg` (`zext`, `uitofp`).
        flags: Flags,
        /// Fast-math flags (`fpext`/`fptrunc`, LLVM 20+).
        fmf: FastMath,
        /// Operand.
        val: Value,
    },
    /// `alloca`.
    Alloca {
        /// The allocated type.
        allocated: TypeId,
        /// The element count; `is_none` when absent (one element).
        count: Value,
        /// Alignment.
        align: Align,
        /// `inalloca`.
        inalloca: bool,
        /// `swifterror`.
        swifterror: bool,
    },
    /// `load`; the loaded type is the instruction's type.
    Load {
        /// Address.
        ptr: Value,
        /// Alignment.
        align: Align,
        /// `volatile`.
        volatile: bool,
        /// Atomic ordering ([`Ordering::NotAtomic`] for a plain load).
        ordering: Ordering,
        /// Synchronization scope (atomic loads).
        scope: SyncScope,
    },
    /// `store`.
    Store {
        /// Stored value.
        val: Value,
        /// Address.
        ptr: Value,
        /// Alignment.
        align: Align,
        /// `volatile`.
        volatile: bool,
        /// Atomic ordering ([`Ordering::NotAtomic`] for a plain store).
        ordering: Ordering,
        /// Synchronization scope (atomic stores).
        scope: SyncScope,
    },
    /// `fence`.
    Fence {
        /// Ordering.
        ordering: Ordering,
        /// Synchronization scope.
        scope: SyncScope,
    },
    /// `cmpxchg`.
    CmpXchg {
        /// Address.
        ptr: Value,
        /// Expected value.
        cmp: Value,
        /// New value.
        new: Value,
        /// Alignment.
        align: Align,
        /// Success ordering.
        success: Ordering,
        /// Failure ordering.
        failure: Ordering,
        /// `weak`.
        weak: bool,
        /// `volatile`.
        volatile: bool,
        /// Synchronization scope.
        scope: SyncScope,
    },
    /// `atomicrmw`.
    AtomicRmw {
        /// Operation.
        op: RmwOp,
        /// Address.
        ptr: Value,
        /// Operand.
        val: Value,
        /// Alignment.
        align: Align,
        /// Ordering.
        ordering: Ordering,
        /// `volatile`.
        volatile: bool,
        /// Synchronization scope.
        scope: SyncScope,
    },
    /// `getelementptr`.
    Gep {
        /// Source element type.
        source: TypeId,
        /// `inbounds`/`nusw`/`nuw`.
        flags: GepFlags,
        /// Operand pool: the base pointer, then the indices.
        ops: OpRange,
    },
    /// `extractvalue`.
    ExtractValue {
        /// Aggregate.
        agg: Value,
        /// Index pool: the indices.
        idx: OpRange,
    },
    /// `insertvalue`.
    InsertValue {
        /// Aggregate.
        agg: Value,
        /// Inserted value.
        val: Value,
        /// Index pool: the indices.
        idx: OpRange,
    },
    /// `extractelement`.
    ExtractElement {
        /// Vector.
        vec: Value,
        /// Lane index.
        idx: Value,
    },
    /// `insertelement`.
    InsertElement {
        /// Vector.
        vec: Value,
        /// Inserted element.
        elt: Value,
        /// Lane index.
        idx: Value,
    },
    /// `shufflevector`.
    ShuffleVector {
        /// First vector.
        a: Value,
        /// Second vector.
        b: Value,
        /// Index pool: the mask; [`crate::SHUFFLE_POISON`] (`u32::MAX`) is a
        /// poison lane.
        mask: OpRange,
    },
    /// `phi`.
    Phi {
        /// Fast-math flags.
        fmf: FastMath,
        /// Operand pool: pairs `(value, block)`, the block a
        /// [`crate::ValueRef::Block`].
        incoming: OpRange,
    },
    /// `select`.
    Select {
        /// Fast-math flags.
        fmf: FastMath,
        /// Condition.
        cond: Value,
        /// Value if true.
        t: Value,
        /// Value if false.
        f: Value,
    },
    /// `call`: the index of its [`CallData`].
    Call(u32),
    /// `invoke`: the index of its [`CallData`] (with `normal` and `unwind`).
    Invoke(u32),
    /// `callbr` (`asm goto`): the index of its [`CallData`], whose `normal`
    /// is the fallthrough and `indirect` the label destinations.
    CallBr(u32),
    /// `landingpad`: the index of its [`LandingPadData`].
    LandingPad(u32),
    /// `resume`.
    Resume {
        /// The exception value.
        val: Value,
    },
    /// `catchswitch`.
    CatchSwitch {
        /// Parent pad; `is_none` for `within none`.
        parent: Value,
        /// Operand pool: the handler blocks.
        handlers: OpRange,
        /// Unwind block; `is_none` for "to caller".
        unwind: Value,
    },
    /// `catchpad`.
    CatchPad {
        /// The `catchswitch`.
        parent: Value,
        /// Operand pool: the arguments.
        args: OpRange,
    },
    /// `cleanuppad`.
    CleanupPad {
        /// Parent pad; `is_none` for `within none`.
        parent: Value,
        /// Operand pool: the arguments.
        args: OpRange,
    },
    /// `catchret`.
    CatchRet {
        /// The `catchpad`.
        pad: Value,
        /// Destination.
        dest: BlockId,
    },
    /// `cleanupret`.
    CleanupRet {
        /// The `cleanuppad`.
        pad: Value,
        /// Unwind block; `is_none` for "to caller".
        unwind: Value,
    },
    /// Unconditional `br`.
    Br {
        /// Destination.
        dest: BlockId,
    },
    /// Conditional `br`.
    CondBr {
        /// Condition.
        cond: Value,
        /// Destination if true.
        t: BlockId,
        /// Destination if false.
        f: BlockId,
    },
    /// `switch`.
    Switch {
        /// Condition.
        cond: Value,
        /// Default destination.
        default: BlockId,
        /// Operand pool: pairs `(constant, block)`.
        cases: OpRange,
    },
    /// `indirectbr`.
    IndirectBr {
        /// Address.
        addr: Value,
        /// Operand pool: the possible destinations (blocks).
        dests: OpRange,
    },
    /// `ret`.
    Ret {
        /// Returned value; `is_none` for `ret void`.
        val: Value,
    },
    /// `unreachable`.
    Unreachable,
    /// `freeze`.
    Freeze {
        /// Operand.
        val: Value,
    },
    /// `va_arg`; the result type is the instruction's type.
    VaArg {
        /// The `va_list` pointer.
        list: Value,
    },
}

/// An instruction: payload plus result type (`void` for instructions
/// without a result). At most 24 bytes.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Inst {
    pub(crate) kind: InstKind,
    pub(crate) ty: TypeId,
}

impl Inst {
    /// The payload: opcode, flags and operands.
    pub fn kind(&self) -> &InstKind {
        &self.kind
    }

    /// Result type (`void` if none).
    pub fn ty(&self) -> TypeId {
        self.ty
    }

    /// Opcode.
    pub fn opcode(&self) -> Opcode {
        match self.kind {
            InstKind::Binary { op, .. } => Opcode::Binary(op),
            InstKind::FNeg { .. } => Opcode::FNeg,
            InstKind::ICmp { .. } => Opcode::ICmp,
            InstKind::FCmp { .. } => Opcode::FCmp,
            InstKind::Cast { op, .. } => Opcode::Cast(op),
            InstKind::Alloca { .. } => Opcode::Alloca,
            InstKind::Load { .. } => Opcode::Load,
            InstKind::Store { .. } => Opcode::Store,
            InstKind::Fence { .. } => Opcode::Fence,
            InstKind::CmpXchg { .. } => Opcode::CmpXchg,
            InstKind::AtomicRmw { .. } => Opcode::AtomicRmw,
            InstKind::Gep { .. } => Opcode::GetElementPtr,
            InstKind::ExtractValue { .. } => Opcode::ExtractValue,
            InstKind::InsertValue { .. } => Opcode::InsertValue,
            InstKind::ExtractElement { .. } => Opcode::ExtractElement,
            InstKind::InsertElement { .. } => Opcode::InsertElement,
            InstKind::ShuffleVector { .. } => Opcode::ShuffleVector,
            InstKind::Phi { .. } => Opcode::Phi,
            InstKind::Select { .. } => Opcode::Select,
            InstKind::Call(_) => Opcode::Call,
            InstKind::Invoke(_) => Opcode::Invoke,
            InstKind::CallBr(_) => Opcode::CallBr,
            InstKind::LandingPad(_) => Opcode::LandingPad,
            InstKind::Resume { .. } => Opcode::Resume,
            InstKind::CatchSwitch { .. } => Opcode::CatchSwitch,
            InstKind::CatchPad { .. } => Opcode::CatchPad,
            InstKind::CleanupPad { .. } => Opcode::CleanupPad,
            InstKind::CatchRet { .. } => Opcode::CatchRet,
            InstKind::CleanupRet { .. } => Opcode::CleanupRet,
            InstKind::Br { .. } => Opcode::Br,
            InstKind::CondBr { .. } => Opcode::CondBr,
            InstKind::Switch { .. } => Opcode::Switch,
            InstKind::IndirectBr { .. } => Opcode::IndirectBr,
            InstKind::Ret { .. } => Opcode::Ret,
            InstKind::Unreachable => Opcode::Unreachable,
            InstKind::Freeze { .. } => Opcode::Freeze,
            InstKind::VaArg { .. } => Opcode::VaArg,
        }
    }

    /// True for block terminators.
    pub fn is_terminator(&self) -> bool {
        self.opcode().is_terminator()
    }
}

/// Side data of `call`, `invoke` and `callbr` ([`InstKind::Call`],
/// [`crate::Function::call_data`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CallData {
    pub(crate) callee: Value,
    pub(crate) fn_ty: TypeId,
    pub(crate) args: OpRange,
    pub(crate) attrs: crate::attr::AttrList,
    pub(crate) cc: CallConv,
    pub(crate) tail: TailKind,
    pub(crate) fmf: FastMath,
    /// Invoke destinations.
    pub(crate) normal: Option<BlockId>,
    pub(crate) unwind: Option<BlockId>,
    /// `callbr` indirect destinations.
    pub(crate) indirect: Vec<BlockId>,
    pub(crate) bundles: Vec<OperandBundle>,
}

impl CallData {
    /// The callee: a function, an alias, inline asm
    /// ([`crate::ValueRef::Asm`]) or any pointer value.
    pub fn callee(&self) -> Value {
        self.callee
    }

    /// The function type of the call.
    pub fn fn_ty(&self) -> TypeId {
        self.fn_ty
    }

    /// Operand pool: the arguments.
    pub fn args(&self) -> OpRange {
        self.args
    }

    /// Call-site attributes (function, return, parameters).
    pub fn attrs(&self) -> &crate::attr::AttrList {
        &self.attrs
    }

    /// Calling convention.
    pub fn cc(&self) -> CallConv {
        self.cc
    }

    /// Tail marker (`call` only).
    pub fn tail(&self) -> TailKind {
        self.tail
    }

    /// Fast-math flags.
    pub fn fmf(&self) -> FastMath {
        self.fmf
    }

    /// `invoke`: the normal destination; `callbr`: the fallthrough.
    pub fn normal(&self) -> Option<BlockId> {
        self.normal
    }

    /// `invoke`: the unwind destination.
    pub fn unwind(&self) -> Option<BlockId> {
        self.unwind
    }

    /// `callbr`: the indirect (label) destinations.
    pub fn indirect(&self) -> &[BlockId] {
        &self.indirect
    }

    /// Operand bundles, in order.
    pub fn bundles(&self) -> &[OperandBundle] {
        &self.bundles
    }
}

/// An operand bundle `"tag"(ty val, ...)` on a call.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OperandBundle {
    /// Bundle tag (`"deopt"`, `"funclet"`, ...).
    pub tag: Box<str>,
    /// Operands.
    pub args: Vec<Value>,
}

/// A `landingpad` clause.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Clause {
    /// `catch <ty> <val>`.
    Catch(Value),
    /// `filter <array ty> <val>`.
    Filter(Value),
}

/// Side data of `landingpad` ([`InstKind::LandingPad`],
/// [`crate::Function::landing_pad`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LandingPadData {
    pub(crate) cleanup: bool,
    pub(crate) clauses: Vec<Clause>,
}

impl LandingPadData {
    /// `cleanup`.
    pub fn cleanup(&self) -> bool {
        self.cleanup
    }

    /// The clauses, in order.
    pub fn clauses(&self) -> &[Clause] {
        &self.clauses
    }
}
