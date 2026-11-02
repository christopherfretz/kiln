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
    // [dst p, src p, type]; aux = 1 for an initialization, | 2 to require
    // a fully initialized source, | 4 for a trivial copy/move constructor
    // or assignment call run inline (its failure notes the call's frame)
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
    // [dst, p, target pointee type]; aux = `VOID_TAG`: [dst, p, source
    // pointee type], a `T *` to `void *` conversion that tags `p`
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
    // only (made before the destructors run); aux 4 = through a virtual
    // destructor; aux 8 = `std::allocator<T>::deallocate` (`::operator
    // delete` in it), `p` a pointer to the first `T` of the storage
    Delete = 2,
    // [p, dtor func, type]: registers the destruction of the object at
    // `p` (an array: of each element, in order) by the enclosing scope
    // (spec/oven.md "Destruction"): the innermost `ScopeExit` or `Ret`
    // whose mark is below it calls the destructor, last registered first.
    // aux = 1: `p` is the first element of a heap array of `type`
    // (`delete[]`), its count is the allocation's; aux 2 = a subobject
    // destroyed by a destructor (notes); aux 4 = run only by unwinding
    // (a subobject of an object under construction, [except.ctor]/3-4),
    // dropped by any other exit. A null `p` registers nothing.
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
    // [dst, mp, this]: the function of a pointer to member function; a
    // virtual one is dispatched on the object at register `this` (the
    // member's class subobject, `MpAddr`'s result), which is adjusted in
    // place to the final overrider's subobject
    MpFn = 3,
    // [dst, a, b, c, t1, t2]: a library builtin evaluated natively
    // (spec/oven.md "Builtins"); aux = kind (`bi`) | sub << 8, `a`..`c`
    // registers, `t1`/`t2` types (`bi::DYN`: the designated object's).
    Builtin = 6,
    // [p, class]: the object at `p` of `class` (under construction or
    // destruction) has dynamic type `class` from now on ([class.cdtor]/4):
    // writes the dynamic-type word of each of its polymorphic subobjects
    // (spec/oven.md "Dynamic types", WP-077)
    SetVptr = 2,
    // [method, arg base, class, adj]: a virtual call of `method` (first
    // declaration, a member of `class`) on the object argument of the
    // window at `arg base` (after the result object when aux = 1): the
    // final overrider for the object's dynamic type, `this` adjusted to
    // its subobject ([class.virtual]/2); register `adj` (`u32::MAX`:
    // none) receives the covariant return adjustment ([class.virtual]/8)
    VCall = 4,
    // [p, adj]: a covariant return: `p` (unless null) moved by the byte
    // offset in register `adj`, to the called function's return class
    CovAdj = 2,
    // [dst, p, static class, target class (`u32::MAX`: `void`)]:
    // `dynamic_cast` ([expr.dynamic.cast]/6-9); aux = 1: a reference cast
    // (`p` a glvalue's address; failure is diagnosed, not null)
    DynCast = 4,
    // [dst, type | p, type_info type, class]: the address of the
    // `std::type_info` object of `type` (a canonical `QualType`), or with
    // aux = 1 of the dynamic type of the polymorphic glvalue of `class`
    // at `p` ([expr.typeid]/3-4)
    TypeId = 4,
    // [dst, value, end]: `__builtin_constant_p` (clang
    // `EvaluateBuiltinConstantP`): the region after this instruction,
    // which ends in a `Ret` of register `value`, runs as a speculative
    // trial (spec/oven.md "Trial evaluation") in this frame's registers;
    // `dst` is 1 if it completes (aux = 1: and the value, a pointer, is
    // null or the start of a string literal), else 0 and its effects are
    // rolled back (a resource limit still fails the evaluation); execution
    // resumes at `end`.
    Speculate = 3,
    // [dr, di, a, b, c, d]: GNU `_Complex` arithmetic, `dr + i di =
    // (a + ib) op (c + id)` (`crate::complex`, spec/oven.md "Complex
    // values"); aux = op (`complex::cx`) | float kind << 2, or for an
    // integer element 1 << 6 | width << 7 | signed << 15
    Cx = 6,
    // [p, type, stat, n, k]: a placement new of `type` (aux
    // `new_aux::ARRAY`: of `type[n]`, register `n` checked as an array
    // new's bound against `k` explicit initializers; `new_aux::ZERO`:
    // zero-filled) at `p` (P2747, [expr.const]/5.18, `std::construct_at`):
    // checks the storage (null, deleted, bounds, const, the designated
    // object of static type `stat` — `bi::DYN` when unknown — must be at
    // least as large an array of a similar element type) and begins the
    // object's lifetime, its bytes uninitialized for the initialization
    // that follows (spec/oven.md "Transient allocation").
    Placement = 5,
    // [p, type]: an explicit destructor call (or pseudo-destructor call)
    // of the `type` object at `p`; aux 1 = check that its lifetime has
    // not ended, aux 2 = end it (`std::allocator` storage only); aux 4 =
    // a member call on it instead: `std::allocator` storage must be
    // within its lifetime.
    EndLife = 2,
    // [handler]: enters a try-block (P3068, [except.handle]): registers a
    // handler marker in the cleanup stack (spec/oven.md "Exceptions");
    // an exception thrown before the marker is popped resumes at
    // `handler`.
    TryEnter = 1,
    // [p, type, dtor]: throws the `type` exception object at `p` (a heap
    // object; `dtor` its destructor's function, `NO_DTOR` for none);
    // aux 1 = rethrow the handled exception (`throw;`, operands unused);
    // aux 2 = resume unwinding the in-flight exception (no handler of a
    // try-block matched).
    Throw = 3,
    // Continues unwinding the in-flight exception (the return point of a
    // destructor run by unwinding; one per Oven).
    Unwind = 0,
    // [dst, type, next]: a handler of a try-block: matches the in-flight
    // exception against handler type `type` ([except.handle]/3; aux 0 =
    // `...`, 1 = an object or reference handler: same type or an
    // unambiguous public base class, `dst` = the (sub)object's address;
    // 2 = a pointer or pointer-to-member handler by value: the pointer
    // conversions, `dst` = the converted value), else jumps to `next`.
    Catch = 3,
    // The matched handler becomes active: the in-flight exception is the
    // handled one until the handler's scope exits.
    CatchBegin = 0,
}

/// `Op::Throw` operand: the exception object has no destructor to run.
pub(crate) const NO_DTOR: u32 = u32::MAX;

/// `VoidCast` aux: the conversion of a non-null `T *` to `void *`, which
/// records `T` (+ 1) in bits 64..96 of the pointer register, the
/// designator a placement new of the `void *` and a cast from it read
/// (spec/oven.md "Construction"). Pointer operations read the low 64
/// bits only; a store to memory keeps the tag in the granule's pointer
/// mark and `Memory::vtags` (`Memory::set_void_tag`), and a load
/// restores it.
pub(crate) const VOID_TAG: u32 = 1;

/// The kinds of `Builtin` and their operands.
pub(crate) mod bi {
    /// `t1`/`t2`: no static type; the type of the designated object.
    pub const DYN: u32 = u32::MAX;
    /// `clz`/`clzg` of `a`; sub = width | has fallback (`b`) << 8.
    pub const CLZ: u32 = 0;
    /// `ctz`/`ctzg`, as `CLZ`.
    pub const CTZ: u32 = 1;
    /// `popcount`/`popcountg` of `a`; sub = width.
    pub const POPCOUNT: u32 = 2;
    /// `bswap` of `a`; sub = width.
    pub const BSWAP: u32 = 3;
    /// Floating-point classification of `a`; sub = float kind | class << 4.
    pub const FP_CLASS: u32 = 4;
    /// `strlen`/`wcslen` of `a`; `t1` the character type.
    pub const STRLEN: u32 = 5;
    /// `memcmp`/`wmemcmp` of `a` and `b` over `c` elements; `t1`/`t2`
    /// the element types; sub = `WIDE` | `STR` | `UNBOUNDED`
    /// (`strcmp`/`strncmp`/`wcscmp`/`wcsncmp`).
    pub const MEMCMP: u32 = 6;
    /// `memchr`/`char_memchr`/`wmemchr` of `b` in `c` elements at `a`;
    /// `t1` the element type; sub = `WIDE` | `STR` | `UNBOUNDED`
    /// (`strchr`/`wcschr`; `strchr` gives null for a value its element
    /// type cannot represent).
    pub const MEMCHR: u32 = 7;
    /// `memcpy`/`memmove` of `c` bytes from `b` to `a`; `t1`/`t2` the
    /// destination/source element types; sub = `MOVE`.
    pub const MEMCPY: u32 = 8;
    /// `__builtin_bit_cast` of the object at `a` (type `t1`) to `t2`:
    /// into the object at `b` when sub = `TO_MEM`, else as `dst`.
    pub const BIT_CAST: u32 = 9;
    /// The pointer stored as constant `t1` (a raw `ConstValueId`) of
    /// pointer type `t2`, thawed: the value of a source-location builtin
    /// (spec/oven.md, Builtins).
    pub const THAW_PTR: u32 = 10;
    /// A sign-bit or comparison operation on the floats `a` and `b`;
    /// sub = float kind | `FABS`..`ISINF_SIGN` << 4 (clang 19's
    /// `FloatExprEvaluator::VisitCallExpr`).
    pub const FP_OP: u32 = 11;
    /// `__atomic_always_lock_free`/`__atomic_is_lock_free`/
    /// `__c11_atomic_is_lock_free` of size `a`: `t1` the object's
    /// alignment (0: unknown); sub = the target's inline width in bytes
    /// | any object qualifies << 8 | `always` (an unknown answer is 0,
    /// not a failure) << 9.
    pub const LOCK_FREE: u32 = 12;

    /// A reflection intrinsic (WP-267 spike): sub = `MetaOp::code`; the
    /// operands are the call's arguments (`a`, `b`, `c`); `t1` the
    /// character type of a name argument, `t2` the result's pointer type
    /// (`name_of`).
    pub const META: u32 = 30;

    /// sub of `MEMCMP`/`MEMCHR`: the `w` form (elements compared as
    /// `wchar_t`, no narrow-character restriction).
    pub const WIDE: u32 = 1;
    /// sub of `MEMCMP`/`MEMCHR`: the `str`/`wcs` form: stops at a null
    /// element and takes any element type (clang `StopAtNull`, not
    /// `IsRawByte`).
    pub const STR: u32 = 2;
    /// sub of `MEMCMP`/`MEMCHR`: no element count (`c` is ignored).
    pub const UNBOUNDED: u32 = 4;
    /// sub of `MEMCPY`: `memmove` (overlap allowed).
    pub const MOVE: u32 = 1;
    /// sub of `BIT_CAST`: the result initializes the object at `b`.
    pub const TO_MEM: u32 = 1;
    /// sub of `BIT_CAST`: only read the source object (its faults come
    /// before the compiler's static ones).
    pub const READ_ONLY: u32 = 2;

    // WP-150: the GCC integer builtins and `wmemcpy`/`wmemmove`.
    /// An integer operation on `a` (and the amount `b`): sub = width |
    /// `FFS`..`ROTR` << 8 (clang 19 `IntExprEvaluator::VisitBuiltinCallExpr`).
    pub const INT_OP: u32 = 20;
    /// `INT_OP`: `__builtin_ffs*`.
    pub const FFS: u32 = 0;
    /// `INT_OP`: `__builtin_parity*`.
    pub const PARITY: u32 = 1;
    /// `INT_OP`: `__builtin_clrsb*`.
    pub const CLRSB: u32 = 2;
    /// `INT_OP`: `__builtin_bitreverse*`.
    pub const BITREVERSE: u32 = 3;
    /// `INT_OP`: `__builtin_rotateleft*`.
    pub const ROTL: u32 = 4;
    /// `INT_OP`: `__builtin_rotateright*`.
    pub const ROTR: u32 = 5;
    /// `INT_OP`: `__builtin_abs`/`labs`/`llabs` of the canonical signed
    /// `a` (WP-365); the most negative value is undefined (a fault).
    pub const IABS: u32 = 6;
    // WP-270: checked arithmetic.
    /// `__builtin_{add,sub,mul}_overflow` on canonical `a`, `b`: sub = op
    /// (0 add, 1 sub, 2 mul) | `a` signed << 2 | `b` signed << 3 | result
    /// signed << 4 | [`VALUE`] | result width << 6. Without `VALUE` the
    /// value is the overflow flag (the infinitely precise result does not
    /// fit the result type); with it, the wrapped result, canonical in the
    /// result type.
    pub const OVERFLOW: u32 = 21;
    /// sub flag of `OVERFLOW`: produce the wrapped result.
    pub const VALUE: u32 = 32;
    // WP-282.
    /// `__builtin_object_size` / `__builtin_dynamic_object_size` of the
    /// pointer `a` with type `b` (0-3) and `t1` the statically designated
    /// type (or [`DYN`]): the bytes from `a` to the end of its complete
    /// object (types 0/2) or of the closest subobject of type `t1` (types
    /// 1/3); 0 past the end (clang `tryEvaluateBuiltinObjectSize`). With
    /// [`SIDE`], the operand has side effects and `a` was evaluated as a
    /// speculative trial whose success is register `c`: on failure, the
    /// default -1 (types 0/1) or 0 (types 2/3).
    pub const OBJECT_SIZE: u32 = 22;
    /// sub flag of `OBJECT_SIZE`: the operand has side effects.
    pub const SIDE: u32 = 1;
    /// `__builtin_assume_aligned(a, b[, c])` (WP-292; clang
    /// `PointerExprEvaluator`): the pointer `a`, after checking that its
    /// complete object is aligned to `b` bytes and that its byte offset
    /// in it, minus the offset `c`, is a multiple of `b`. Executed with
    /// the bridge (the declared alignment of a variable).
    pub const ASSUME_ALIGNED: u32 = 23;
    /// The characters of a `static_assert` message (P2741,
    /// `Oven::evaluate_chars`): the `a` elements of character type `t1`
    /// that `b` points to, read into the Oven's message buffer.
    pub const MSG_CHARS: u32 = 24;
    /// `std::uncaught_exceptions()` / `__builtin_uncaught_exceptions()`
    /// ([except.uncaught]/1, P3068): the number of exceptions thrown and
    /// not yet caught in this evaluation (a nested evaluation counts its
    /// own, spec/oven.md "Exceptions").
    pub const UNCAUGHT: u32 = 29;
    /// sub of `MEMCPY`: `wmemcpy`/`wmemmove`: `c` counts elements, not
    /// bytes.
    pub const WCHAR: u32 = 2;

    /// Classes of `FP_CLASS`.
    pub const FINITE: u32 = 0;
    pub const INF: u32 = 1;
    pub const NAN: u32 = 2;
    pub const NORMAL: u32 = 3;
    pub const SUBNORMAL: u32 = 4;
    pub const ZERO: u32 = 5;
    pub const SIGNALING: u32 = 6;
    /// `__builtin_isfpclass`: whether `a`'s class is in the LLVM
    /// `FPClassTest` mask `b` (0x1 sNaN, 0x2 qNaN, 0x4 -inf, 0x8 -normal,
    /// 0x10 -subnormal, 0x20 -zero, 0x40 +zero, 0x80 +subnormal,
    /// 0x100 +normal, 0x200 +inf).
    pub const MASK: u32 = 7;

    /// Operations of `FP_OP`: `|a|`; `a` with the sign of `b`; `fmax`
    /// and `fmin` (a NaN operand yields the other; `-0 < +0`); the
    /// `isinf_sign` of `a` (an `int`).
    pub const FABS: u32 = 0;
    pub const COPYSIGN: u32 = 1;
    pub const FMAX: u32 = 2;
    pub const FMIN: u32 = 3;
    pub const ISINF_SIGN: u32 = 4;

    // WP-365: P0533 constexpr <cmath>.
    /// A `<cmath>` operation on `a` (`b`, `c`) in the float kind of sub's
    /// low 4 bits: sub = float kind | `CEIL`..`CMP` << 4 | [`SECOND`]
    /// (`softfloat::math`; IEEE-exact, GCC's folding rules: an invalid,
    /// overflowing, inexact-underflowing or signaling-NaN case is not a
    /// constant).
    pub const FP_MATH: u32 = 13;
    /// sub flag of `FP_MATH`: the operation's second result (`frexp`'s
    /// exponent, `modf`'s integral part, `remquo`'s quotient bits).
    pub const SECOND: u32 = 1 << 9;
    /// `FP_MATH` operations: rounding to an integral value of the same
    /// type.
    pub const CEIL: u32 = 0;
    pub const FLOOR: u32 = 1;
    pub const TRUNC: u32 = 2;
    pub const ROUND: u32 = 3;
    pub const ROUNDEVEN: u32 = 4;
    /// `rint`/`nearbyint`: the default rounding, to nearest even.
    pub const RINT: u32 = 5;
    /// `lround`/`llround`, `lrint`/`llrint`: to a signed integer of `t1`
    /// bits (canonical).
    pub const LROUND: u32 = 6;
    pub const LRINT: u32 = 7;
    pub const FMOD: u32 = 8;
    pub const REMAINDER: u32 = 9;
    /// `remquo`: the remainder; with `SECOND` the quotient bits (`int`).
    pub const REMQUO: u32 = 10;
    pub const FDIM: u32 = 11;
    /// `fma(a, b, c)` with one rounding.
    pub const FMA: u32 = 12;
    /// `frexp`: the fraction; with `SECOND` the exponent (`int`).
    pub const FREXP: u32 = 13;
    /// `ldexp`/`scalbn`/`scalbln(a, b)`, `b` a canonical signed integer.
    pub const LDEXP: u32 = 14;
    /// `ilogb(a)` (`int`).
    pub const ILOGB: u32 = 15;
    pub const LOGB: u32 = 16;
    /// `modf`: the fraction; with `SECOND` the integral part.
    pub const MODF: u32 = 17;
    /// `nextafter`/`nexttoward(a, b)`, `b` of float kind `t1`.
    pub const NEXTAFTER: u32 = 18;
    /// `signbit(a)` (`int` 0 or 1; never faults).
    pub const SIGNBIT: u32 = 19;
    /// `isgreater`..`isunordered(a, b)`: `t1` = `cmp::GT`, `GE`, `LT`,
    /// `LE`, `NE` (`islessgreater`) or [`UNORDERED`]; quiet, never
    /// faults.
    pub const CMP: u32 = 20;
    /// `t1` of `FP_MATH` `CMP`: `isunordered`.
    pub const UNORDERED: u32 = 6;
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
    /// `std::allocator<T>::allocate` (`::operator new` in it): with
    /// `ARRAY`, the count register holds a byte count, a multiple of
    /// `sizeof(T)`; the elements are outside their lifetime.
    pub const STD: u32 = 16;
    /// `Op::Placement` with `ARRAY`: only check the bound (an instruction
    /// at the bound's location, so its notes point there).
    pub const BOUND: u32 = 32;
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
    /// A member function call (clang's `AK_MemberCall`).
    pub const MEMBER_CALL: u32 = 6;
    pub const DYNAMIC_CAST: u32 = 7;
    pub const TYPEID: u32 = 8;
    pub const DESTROY: u32 = 9;
    /// The construction of a subobject of an inactive union member by a
    /// placement new (clang's `AK_Construct` in
    /// `note_constexpr_access_inactive_union_member`).
    pub const CONSTRUCT: u32 = 10;

    pub fn verb(k: u32) -> &'static str {
        match k {
            ASSIGN | COMPOUND => "assignment to",
            INCREMENT => "increment of",
            DECREMENT => "decrement of",
            INIT => "construction of",
            CONSTRUCT => "construction of subobject of",
            MEMBER_CALL => "member call on",
            DYNAMIC_CAST => "dynamic_cast of",
            TYPEID => "typeid applied to",
            DESTROY => "destruction of",
            _ => "read of",
        }
    }

    /// The verb of clang's `note_constexpr_polymorphic_unknown_dynamic_type`.
    pub fn poly_verb(k: u32) -> &'static str {
        match k {
            DYNAMIC_CAST => "dynamic_cast applied to",
            TYPEID => "typeid applied to",
            DESTROY => "destruction of",
            _ => "virtual function called on",
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
