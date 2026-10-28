//! KBC, the Oven's bytecode: a `Vec<u32>` stream of fixed-size
//! instructions. Word 0 of an instruction is `opcode | aux << 8` (`aux` is
//! a 24-bit immediate: widths, signedness, comparison kind, access kind);
//! the operand words follow. Every opcode has a fixed operand count, so
//! decoding is one table lookup and a `match`.
//!
//! Registers are 128-bit slots in a frame window; every scalar fits one
//! (integers canonical: sign- or zero-extended from their width; floats
//! as their IEEE bit pattern; pointers as an encoded `mem::Ptr`).

macro_rules! ops {
    ($($name:ident = $n:expr,)*) => {
        /// A KBC opcode.
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        #[repr(u8)]
        pub(crate) enum Op { $($name,)* }

        impl Op {
            /// Every opcode, indexed by its discriminant.
            pub const ALL: &'static [Op] = &[$(Op::$name,)*];
            /// Operand words of each opcode, indexed by its discriminant.
            pub const OPERANDS: &'static [u8] = &[$($n,)*];
        }
    };
}

ops! {
    Nop = 0,
    // [first index into the step-location pool]; aux = number of steps.
    Step = 1,
    Halt = 0,
    // [trap index]
    Trap = 1,
    // [dst, lo, hi]; aux = 1 to sign-extend the 64-bit value
    Const = 3,
    // [dst, w0, w1, w2, w3]: a 128-bit value, low word first
    Const2 = 5,
    // [dst, src]
    Mov = 2,
    // [target]
    Jmp = 1,
    // [cond, target]
    JmpIf = 2,
    JmpIfNot = 2,
    // Integer binary ops [dst, a, b]; aux = width | signed << 8; shifts
    // add the count's width << 9 and signedness << 17.
    IAdd = 3,
    ISub = 3,
    IMul = 3,
    IDiv = 3,
    IRem = 3,
    IShl = 3,
    IShr = 3,
    IAnd = 3,
    IOr = 3,
    IXor = 3,
    // [dst, a]; aux = width | signed << 8
    INeg = 2,
    INot = 2,
    LNot = 2,
    // [dst, a, b]; aux = width | signed << 8 | cmp << 9
    ICmp = 3,
    // [dst, a]; aux = from width | from signed << 8 | to width << 9 | to signed << 17
    ICast = 2,
    // [dst, a]; aux = width
    ToBool = 2,
    // [v, min lo, min hi, max lo, max hi]: [expr.static.cast]/10, the
    // value of a conversion to an enumeration without a fixed underlying
    // type lies in its range ([dcl.enum]/8); bounds are 64-bit, sign-extended;
    // `v` is the operand (before the conversion), aux = its width | signed << 8
    EnumCheck = 5,
    // Float binary ops [dst, a, b]; aux = kind
    FAdd = 3,
    FSub = 3,
    FMul = 3,
    FDiv = 3,
    // [dst, a]; aux = kind
    FNeg = 2,
    // [dst, a, b]; aux = kind | cmp << 4
    FCmp = 3,
    // [dst, a]; aux = from | to << 4
    FCast = 2,
    // [dst, a]; aux = int width | signed << 8 | kind << 9
    IToF = 2,
    // [dst, a]; aux = kind | width << 4 | signed << 12
    FToI = 2,
    // [dst, a]; aux = kind
    FToBool = 2,
    // [dst, p, idx, elem type]; aux = idx width | idx signed << 8 | negate << 9
    PtrAdd = 4,
    // [dst, p, q, elem type]
    PtrSub = 4,
    // [dst, p, q, pointee type]; aux = cmp
    PtrCmp = 4,
    // [dst, p]
    PtrToBool = 2,
    // [dst, p, byte offset]
    Field = 3,
    // [dst, p, type]; aux = access kind
    Load = 3,
    // [p, src, type]; aux = access kind
    Store = 3,
    InitStore = 3,
    // [dst, p, type, bit | width << 8]; aux = access kind
    LoadBits = 4,
    // [p, src, type, bit | width << 8]; aux = 1 for an initialization
    StoreBits = 4,
    // [dst p, src p, type]; aux = 1 for an initialization
    Copy = 3,
    // [p, type]: zero-initialization ([dcl.init.general]/6)
    ZeroInit = 2,
    // [dst, type, origin]; aux = allocation flags
    Alloc = 3,
    // [mark reg]
    ScopeEnter = 1,
    ScopeExit = 1,
    // [union p, field index, union type]
    SetActive = 3,
    // [union p, field index, union type]; aux = access kind
    ActiveCheck = 3,
    // [func, arg base]
    Call = 2,
    // [fn ptr reg, arg base]
    CallInd = 2,
    // [src]; aux = 1 if the function returns a scalar
    Ret = 1,
    // [dst, global]
    GlobalAddr = 2,
    // [dst, p, target pointee type]
    VoidCast = 3,
}

impl Op {
    pub fn from_u8(b: u8) -> Op {
        Op::ALL[b as usize]
    }

    pub fn operands(self) -> usize {
        usize::from(Op::OPERANDS[self as usize])
    }
}

/// Comparison kinds (the `cmp` field of `ICmp`/`FCmp`/`PtrCmp`).
pub(crate) mod cmp {
    pub const EQ: u32 = 0;
    pub const NE: u32 = 1;
    pub const LT: u32 = 2;
    pub const LE: u32 = 3;
    pub const GT: u32 = 4;
    pub const GE: u32 = 5;
}

/// Access kinds (clang's `AccessKinds`, for the wording of notes).
pub(crate) mod ak {
    pub const READ: u32 = 0;
    pub const ASSIGN: u32 = 1;
    pub const INCREMENT: u32 = 2;
    pub const DECREMENT: u32 = 3;
    pub const INIT: u32 = 4;
    /// Compound assignment: a lifetime fault reads "assignment to", an
    /// uninitialized operand "read of".
    pub const COMPOUND: u32 = 5;

    pub fn verb(k: u32) -> &'static str {
        match k {
            ASSIGN | COMPOUND => "assignment to",
            INCREMENT => "increment of",
            DECREMENT => "decrement of",
            INIT => "construction of",
            _ => "read of",
        }
    }
}

/// Packs the aux of an integer op.
pub(crate) fn int_aux(width: u8, signed: bool) -> u32 {
    u32::from(width) | u32::from(signed) << 8
}

/// Disassembles `code` (debugging aid).
pub(crate) fn disassemble(code: &[u32], from: usize, to: usize) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let mut pc = from;
    while pc < to.min(code.len()) {
        let w = code[pc];
        let op = Op::from_u8(w as u8);
        let n = op.operands();
        let _ = write!(s, "{pc:5}: {op:?} aux={}", w >> 8);
        for i in 0..n {
            let _ = write!(s, " {}", code[pc + 1 + i]);
        }
        s.push('\n');
        pc += 1 + n;
    }
    s
}
