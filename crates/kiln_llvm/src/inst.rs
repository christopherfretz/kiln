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
    /// `x86_stdcallcc`.
    pub const X86_STDCALL: CallConv = CallConv(64);
    /// `x86_fastcallcc`.
    pub const X86_FASTCALL: CallConv = CallConv(65);
    /// `x86_thiscallcc`.
    pub const X86_THISCALL: CallConv = CallConv(70);
    /// `x86_64_sysvcc`.
    pub const X86_64_SYSV: CallConv = CallConv(78);
    /// `win64cc`.
    pub const WIN64: CallConv = CallConv(79);
    /// `x86_vectorcallcc`.
    pub const X86_VECTORCALL: CallConv = CallConv(80);
    /// `aarch64_vector_pcs`.
    pub const AARCH64_VECTOR_PCS: CallConv = CallConv(97);

    /// The textual keyword (`None` for ccc; `cc N` is used for unknown
    /// numbers).
    pub(crate) fn keyword(self) -> Option<&'static str> {
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
            64 => "x86_stdcallcc",
            65 => "x86_fastcallcc",
            66 => "arm_apcscc",
            67 => "arm_aapcscc",
            68 => "arm_aapcs_vfpcc",
            70 => "x86_thiscallcc",
            78 => "x86_64_sysvcc",
            79 => "win64cc",
            80 => "x86_vectorcallcc",
            92 => "x86_regcallcc",
            97 => "aarch64_vector_pcs",
            98 => "aarch64_sve_vector_pcs",
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

/// A range in a function operand pool.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct OpRange {
    pub(crate) start: u32,
    pub(crate) len: u32,
}

impl OpRange {
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

/// Compact instruction payload.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum InstKind {
    Binary {
        op: BinOp,
        flags: Flags,
        fmf: FastMath,
        lhs: Value,
        rhs: Value,
    },
    FNeg {
        fmf: FastMath,
        val: Value,
    },
    ICmp {
        pred: IntPredicate,
        lhs: Value,
        rhs: Value,
    },
    FCmp {
        pred: FloatPredicate,
        fmf: FastMath,
        lhs: Value,
        rhs: Value,
    },
    Cast {
        op: CastOp,
        flags: Flags,
        val: Value,
    },
    Alloca {
        allocated: TypeId,
        count: Value,
        align: Align,
        inalloca: bool,
    },
    Load {
        ptr: Value,
        align: Align,
        volatile: bool,
        ordering: Ordering,
        scope: SyncScope,
    },
    Store {
        val: Value,
        ptr: Value,
        align: Align,
        volatile: bool,
        ordering: Ordering,
        scope: SyncScope,
    },
    Fence {
        ordering: Ordering,
        scope: SyncScope,
    },
    CmpXchg {
        ptr: Value,
        cmp: Value,
        new: Value,
        align: Align,
        success: Ordering,
        failure: Ordering,
        weak: bool,
        volatile: bool,
        scope: SyncScope,
    },
    AtomicRmw {
        op: RmwOp,
        ptr: Value,
        val: Value,
        align: Align,
        ordering: Ordering,
        volatile: bool,
        scope: SyncScope,
    },
    /// `ops[0]` is the base pointer.
    Gep {
        source: TypeId,
        flags: GepFlags,
        ops: OpRange,
    },
    /// `idx` ranges into the index pool.
    ExtractValue {
        agg: Value,
        idx: OpRange,
    },
    InsertValue {
        agg: Value,
        val: Value,
        idx: OpRange,
    },
    ExtractElement {
        vec: Value,
        idx: Value,
    },
    InsertElement {
        vec: Value,
        elt: Value,
        idx: Value,
    },
    /// `mask` ranges into the index pool; `u32::MAX` is a poison element.
    ShuffleVector {
        a: Value,
        b: Value,
        mask: OpRange,
    },
    /// Pairs `(value, block)` in the operand pool.
    Phi {
        fmf: FastMath,
        incoming: OpRange,
    },
    Select {
        fmf: FastMath,
        cond: Value,
        t: Value,
        f: Value,
    },
    Call(u32),
    Invoke(u32),
    LandingPad(u32),
    Resume {
        val: Value,
    },
    /// `handlers` are blocks; `unwind` is `Value::NONE` for "to caller".
    CatchSwitch {
        parent: Value,
        handlers: OpRange,
        unwind: Value,
    },
    CatchPad {
        parent: Value,
        args: OpRange,
    },
    /// `parent` is `Value::NONE` for `within none`.
    CleanupPad {
        parent: Value,
        args: OpRange,
    },
    CatchRet {
        pad: Value,
        dest: BlockId,
    },
    CleanupRet {
        pad: Value,
        unwind: Value,
    },
    Br {
        dest: BlockId,
    },
    CondBr {
        cond: Value,
        t: BlockId,
        f: BlockId,
    },
    /// Pairs `(constant, block)` in the operand pool.
    Switch {
        cond: Value,
        default: BlockId,
        cases: OpRange,
    },
    IndirectBr {
        addr: Value,
        dests: OpRange,
    },
    Ret {
        val: Value,
    },
    Unreachable,
    Freeze {
        val: Value,
    },
    VaArg {
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

/// Side data of `call`/`invoke`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct CallData {
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
    pub(crate) bundles: Vec<OperandBundle>,
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

/// Side data of `landingpad`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct LandingPadData {
    pub(crate) cleanup: bool,
    pub(crate) clauses: Vec<Clause>,
}
