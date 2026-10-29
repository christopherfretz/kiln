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
            /// Operand words of each opcode, indexed by its discriminant.
            pub const OPERANDS: &'static [u8] = &[$($n,)*];

            /// The opcode with discriminant `b`. A `match` on the
            /// discriminants rather than a table lookup: LLVM folds it to
            /// a range check, so the VM's dispatch jump table is indexed by
            /// the byte itself, with no table base held in a register in
            /// the hot loop (that register cost ~3% of the wall clock of
            /// every evaluation once the op set outgrew it, WP-076).
            #[inline]
            pub fn from_u8(b: u8) -> Op {
                #[allow(non_upper_case_globals)]
                mod disc {
                    $(pub const $name: u8 = super::Op::$name as u8;)*
                }
                match b {
                    $(disc::$name => Op::$name,)*
                    _ => panic!("invalid KBC opcode {b}"),
                }
            }
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
    // [dst, p, byte offset, derived type]: the derived-class object whose
    // base subobject is at `p` (`offset` bytes into it); aux 1 = a null
    // pointer stays null
    Downcast = 4,
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
    // [dst]: `__builtin_is_constant_evaluated()` ([expr.const]/20): 1 in
    // a manifestly constant-evaluated evaluation, 0 in a trial fold; aux =
    // 1 when the enclosing function is `std::is_constant_evaluated`
    // (the `-Wconstant-evaluated` warning names that call instead).
    IsConstEval = 1,
    // [src]; aux = 1 if the function returns a scalar
    Ret = 1,
    // [dst, global]
    GlobalAddr = 2,
    // [dst, p, target pointee type]
    VoidCast = 3,
    // [dst, type, count, origin, explicit inits]: a transient allocation
    // (spec/oven.md "Transient allocation"), uninitialized unless aux has
    // `new_aux::ZERO`. With `new_aux::ARRAY`, `type` is the element type,
    // `count` the bound register (its signedness in aux) and
    // `explicit inits` the number of initializers (the bound of the
    // initializer's array type): the bound is checked and the allocation
    // is an array of `count` elements; `dst` points to its first.
    New = 5,
    // [p, static pointee type]; aux 1 = `delete[]`; aux 2 = the checks
    // only (made before the destructors run)
    Delete = 2,
    // [p, dtor func, type]: registers the destruction of the object at
    // `p` (an array: of each element, in order) by the enclosing scope
    // (spec/oven.md "Destruction"): the innermost `ScopeExit` or `Ret`
    // whose mark is below it calls the destructor, last registered first.
    // aux = 1: `p` is the first element of a heap array of `type`
    // (`delete[]`), its count is the allocation's. A null `p` registers
    // nothing.
    PushCleanup = 3,
    // [p]: the complete object at `p` is const from now on (a const
    // object is modifiable during its construction, [class.ctor.general]/5).
    MakeConst = 1,
    // [mp reg, class decl]: clang's `MemberPtr::castToDerived` (aux 0)
    // or `castToBase` (aux 1) of the member pointer in the register, in
    // place ([conv.mem]/2, [expr.static.cast]/12)
    MpCast = 2,
    // [dst, object p, mp]: `obj.*mp` ([expr.mptr.oper]): the member's
    // address (a member function: the adjusted object)
    MpAddr = 3,
    // [member p, mp]: the access (aux: its kind) of the member `MpAddr`
    // found, when it is a union's: the member must be active
    // ([class.union]/2; an access through `.*` never activates one,
    // [class.union]/6)
    MpActive = 2,
    // [dst, mp, trap]: the function of a pointer to member function
    // (`trap`: the note for a virtual one, dispatch lands with WP-077)
    MpFn = 3,
}

/// The aux of `New`.
pub(crate) mod new_aux {
    pub const ARRAY: u32 = 1;
    /// A failed bound check yields a null pointer (`new (std::nothrow)`).
    pub const NOTHROW: u32 = 2;
    /// Zero-initialize the whole allocation.
    pub const ZERO: u32 = 4;
    /// The bound register holds a signed value (canonical, so a negative
    /// bound is diagnosed as such).
    pub const SIGNED: u32 = 8;
}

impl Op {
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
