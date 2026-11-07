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
    // | element step << 10 (ptr_add_aux)
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
    // [dst, p, class, sub]: the address of subobject `sub` (an index into
    // the class's subobject graph, in or below a virtual base) of the
    // `class` object at `p`, through the most derived object
    // ([class.mi]/4, P3533R2); aux 1 = a null pointer stays null
    VBase = 4,
    // [dst, p, type]; aux = access kind
    Load = 3,
    // [p, src, type]; aux = access kind
    Store = 3,
    InitStore = 3,
    // [dst, p, type, bit | width << 8]; aux = access kind
    LoadBits = 4,
    // [p, src, type, bit | width << 8]; aux = 1 for an initialization
    StoreBits = 4,
    // [dst, p, idx, vector type]: lane `idx` (unsigned 64-bit) of the
    // `bool` vector at `p` (one bit per lane, `ty.rs` `BoolVec`); aux =
    // access kind
    LoadLane = 4,
    // [p, src, idx, vector type]; aux = 1 for an initialization
    StoreLane = 4,
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
    // aux = 2: `__kiln_plainly_constant_evaluated()` (the run's plain
    // bit); aux = 4: saves the plain bit to [dst] and clears it; aux = 8:
    // restores it from [dst] (around a constexpr local's initializer);
    // aux = 16: the context alone, never diagnosed (`if consteval`).
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
    // [dst, mp, this, adj]: the function of a pointer to member function;
    // a virtual one is dispatched on the object at register `this` (the
    // member's class subobject, `MpAddr`'s result), which is adjusted in
    // place to the final overrider's subobject; register `adj`
    // (`u32::MAX`: none) receives the covariant return adjustment as
    // `VCall`'s does (0 for a non-virtual function)
    MpFn = 4,
    // [dst, a, b, c, t1, t2]: a library builtin evaluated natively
    // (spec/oven.md "Builtins"); aux = kind (`bi`) | sub << 8, `a`..`c`
    // registers, `t1`/`t2` types (`bi::DYN`: the designated object's).
    Builtin = 6,
    // [p, class]: the object at `p` of `class` (under construction or
    // destruction) has dynamic type `class` from now on ([class.cdtor]/4):
    // writes the dynamic-type word of each of its polymorphic subobjects
    // (spec/oven.md "Dynamic types", WP-077); aux 1 = it is a base class
    // subobject (a base-object constructor or destructor): the slots in
    // its virtual bases get relative words (P3533R2)
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
    // rolled back (a resource limit still fails the evaluation; with aux
    // bit 2, `__builtin_constant_p`'s, a lax fold outside a constant
    // context fails instead of 0, WP-403); execution resumes at `end`.
    Speculate = 3,
    // [dr, di, a, b, c, d]: GNU `_Complex` arithmetic, `dr + i di =
    // (a + ib) op (c + id)` (`crate::complex`, spec/oven.md "Complex
    // values"); aux = op (`complex::cx`) | float kind << 2, or for an
    // integer element 1 << 6 | width << 7 | signed << 15
    Cx = 6,
    // [d, a, b, type, x]: wide `_BitInt` (`TyKind::WideInt`) arithmetic on
    // memory-resident values (`crate::bigint`, spec/oven.md "`_BitInt`");
    // aux = sub-op (`wide`) | `wide::INIT` | `wide::RMW` | access kind << 8.
    // `type` is the wide type of the operation (of the operands for
    // `CMP`/`TO`/`TO_BOOL`); `d`, `a`, `b` hold addresses of wide values
    // except where `wide` says otherwise.
    Wide = 5,
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
    // within its lifetime; aux 8 = a base subobject the constructor
    // does not construct: its `copy_size` bytes begin outside their
    // lifetime, in any storage (WP-390).
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
    // [dst, src, trap]: a cast a constant expression may not perform
    // ([expr.const]/5.15; clang `CCEDiag`): outside a fold
    // (`RunState::fold`) it fails as `Trap` `trap`; a fold continues
    // (not memoized) with aux `cce::CHECK`: nothing else; `cce::PTR`:
    // `dst` = `src` pointing at nothing (`mem::NOWHERE`; null stays
    // null); `cce::INT_TO_PTR`: `dst` = the integer `src` as a pointer;
    // `cce::PTR_TO_INT`: `dst` = the integer of such a pointer (any other
    // pointer fails).
    Cce = 3,
    // [dst, expr]: the address a GNU `&&label` (`AddrLabelExpr` `expr`)
    // evaluates to (`Oven::label_addr`; spec/oven.md "Label addresses")
    LabelAddr = 2,
    // [end]: the start of a contract assertion's check (P2900,
    // [basic.contract.eval]; spec/contracts.md §6.4): a run deciding
    // constant-initializability ([expr.const.init]/4.1, every assertion
    // ignored: `RunState::contracts_off`) jumps to `end`.
    ContractSkip = 1,
    // [trap]: a contract violation ([basic.contract.eval]/7), the
    // predicate false (aux = 1: under observe; 2: the predicate was not
    // a constant, 7.3). `trap` is the assertion's note.
    ContractFail = 1,
    // [value, end, trap]: an observed predicate: the region after this
    // instruction, which ends in a `Ret` of register `value`, runs as a
    // transactional trial (spec/oven.md "Trial evaluation"); a false
    // value, or a failure (rolled back: not a constant, 7.3), is a
    // violation as `ContractFail`'s under observe; execution resumes at
    // `end`.
    ContractTrial = 3,
    // [saved]: clang's `FoldConstant` of a `__builtin_constant_p`
    // conditional (`Compiler::bcp_fold_enter`): aux = 1 saves
    // `RunState::fold` in `saved` and enters a fold; aux = 0 restores it.
    Fold = 1,
}

/// The sub-operations of `Op::Wide`. Binary ones are `*d = *a op *b`
/// (shifts: `b` a register holding the count, `x` = its width | signed
/// << 8); unary ones `*d = op *a`.
pub(crate) mod wide {
    pub const MOV: u32 = 0;
    pub const ADD: u32 = 1;
    pub const SUB: u32 = 2;
    pub const MUL: u32 = 3;
    pub const DIV: u32 = 4;
    pub const REM: u32 = 5;
    pub const SHL: u32 = 6;
    pub const SHR: u32 = 7;
    pub const AND: u32 = 8;
    pub const OR: u32 = 9;
    pub const XOR: u32 = 10;
    pub const NEG: u32 = 11;
    pub const NOT: u32 = 12;
    pub const INC: u32 = 13;
    pub const DEC: u32 = 14;
    /// Register `d` = the comparison `x` (`cmp`) of `*a` and `*b`.
    pub const CMP: u32 = 15;
    /// `*d` = register `a`, an integer of width `x & 0xFF`, signed `x >> 8 & 1`.
    pub const FROM: u32 = 16;
    /// Register `d` = `*a` converted to the integer of width `x & 0xFF`,
    /// signed `x >> 8 & 1` (with `x >> 9 & 1`: saturating, a shift count).
    pub const TO: u32 = 17;
    /// Register `d` = `*a != 0`.
    pub const TO_BOOL: u32 = 18;
    /// `*d` = `*a` of wide type `x` converted to `type`.
    pub const CAST: u32 = 19;
    /// Register `d` = `*a` converted to the float of kind `x`
    /// (`compile::float_code`), round to nearest even.
    pub const TO_FLOAT: u32 = 20;
    /// `*d` = register `a`, a float of kind `x`, truncated toward zero
    /// ([conv.fpint]/1: out of range is undefined).
    pub const FROM_FLOAT: u32 = 21;
    /// `*d` = the bit-field of declared type `type` at register `a`, bit
    /// `x & 7` of that byte, width `x >> 3` (sign- or zero-extended).
    pub const LOAD_BITS: u32 = 22;
    /// The bit-field of declared type `type` at register `d` (bit
    /// `x & 7`, width `x >> 3`) = `*a`, truncated to the width.
    pub const STORE_BITS: u32 = 23;
    /// The result object begins its lifetime (an initialization).
    pub const INIT: u32 = 1 << 6;
    /// `*a` is the destination read for a read-modify-write (with the
    /// access kind in aux).
    pub const RMW: u32 = 1 << 7;
}

/// The conversions of `Op::Cce`.
pub(crate) mod cce {
    pub const CHECK: u32 = 0;
    pub const PTR: u32 = 1;
    pub const INT_TO_PTR: u32 = 2;
    pub const PTR_TO_INT: u32 = 3;
}

/// `Op::Throw` operand: the exception object has no destructor to run.
pub(crate) const NO_DTOR: u32 = u32::MAX;

/// `VoidCast` aux: the conversion of a non-null `T *` to `void *`, which
/// records `T` (+ 1) in bits 64..96 of the pointer register, the
/// designator a placement new of the `void *` and a cast from it read
/// (spec/oven.md "Construction"). Pointer operations read the low 64
/// bits only; a store to memory keeps the tag in the granule's pointer
/// mark (`Memory::set_void_tag`) and a load restores it.
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
    /// sub = float kind | `FABS`..`FMINIMUM_NUM` << 4 (clang 19's
    /// `FloatExprEvaluator::VisitCallExpr`).
    pub const FP_OP: u32 = 11;
    /// `__atomic_always_lock_free`/`__atomic_is_lock_free`/
    /// `__c11_atomic_is_lock_free` of size `a`: `t1` the object's
    /// alignment (0: unknown); sub = the target's inline width in bytes
    /// | any object qualifies << 8 | `always` (an unknown answer is 0,
    /// not a failure) << 9 | `b` holds the object pointer's integer
    /// value << 10.
    pub const LOCK_FREE: u32 = 12;

    /// A reflection intrinsic (spec/reflection.md §3): sub =
    /// `MetaOp::code`; `a` is the first of the consecutive registers
    /// holding the call's arguments and `b` their count (both raw, not
    /// register reads; at most `MAX_META_ARGS`); `t1` the element type of
    /// the last pointer argument, `t2` the result type.
    pub const META: u32 = 30;
    /// The arguments of the `std::meta::exception` constructor a failing
    /// intrinsic throws with (spec/reflection.md §8.2): the message
    /// string view built in the object at `a`, the source location in the
    /// object at `b` (register reads), `^^fn` as `dst`; `c` is raw, the
    /// call's `MetaSite` index + 1.
    pub const META_EXC: u32 = 31;
    /// `__builtin_is_string_literal(a)` (WP-484; GCC 16): whether the
    /// pointer `a` points into a string literal object.
    pub const STRING_LIT: u32 = 33;
    /// The carry builtins (WP-557; clang `IntExprEvaluator`
    /// `BI__builtin_addcb`..): on `w`-bit unsigned canonical `a`, `b` and
    /// the carry-in `c`, sub = `w` | subtract << 8 | [`CARRY_VALUE`].
    /// With `CARRY_VALUE` the result `a + b + c` (`a - b - c`) modulo
    /// 2^`w`; without, the carry-out: 1 when either of the two wrapping
    /// steps overflowed.
    pub const CARRY: u32 = 34;
    /// sub flag of `CARRY`: produce the wrapped result.
    pub const CARRY_VALUE: u32 = 1 << 9;
    /// sub flag of `CARRY`: the carry-in is 1 when nonzero (x86
    /// `addcarryx` / `subborrow`, WP-630), not taken modulo 2^`w`.
    pub const CARRY_BOOL_IN: u32 = 1 << 10;
    /// `__builtin_start_lifetime(a)` (WP-773; P3726R2 [obj.lifetime]):
    /// begins the lifetime of the object of static type `t1` at `a`
    /// without initializing it (`Oven::start_lifetime`); the result is 0.
    pub const START_LIFETIME: u32 = 35;

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
    /// `INT_OP`: `__builtin_elementwise_add_sat` of canonical signed
    /// `a`, `b` (WP-432; clamped to the `w`-bit range).
    pub const SADD_SAT: u32 = 7;
    /// `INT_OP`: `__builtin_elementwise_add_sat`, unsigned.
    pub const UADD_SAT: u32 = 8;
    /// `INT_OP`: `__builtin_elementwise_sub_sat`, signed.
    pub const SSUB_SAT: u32 = 9;
    /// `INT_OP`: `__builtin_elementwise_sub_sat`, unsigned.
    pub const USUB_SAT: u32 = 10;
    // WP-630: the x86 BMI / TBM builtins.
    /// `INT_OP`: `bextr` of `a` by the control `b` (start in bits 0-7,
    /// length in bits 8-15).
    pub const BEXTR: u32 = 11;
    /// `INT_OP`: `bzhi`: `a` with the bits from index `b & 0xff` up
    /// cleared.
    pub const BZHI: u32 = 12;
    /// `INT_OP`: `pdep`: the low bits of `a` deposited at the set bits of
    /// the mask `b`.
    pub const PDEP: u32 = 13;
    /// `INT_OP`: `pext`: the bits of `a` at the set bits of the mask `b`,
    /// packed low.
    pub const PEXT: u32 = 14;
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
    /// `__builtin_is_within_lifetime(a)` ([meta.const.eval]/4, WP-418;
    /// clang `EvaluateBuiltinIsWithinLifetime`): a null, function or
    /// one-past-the-end `a` fails (`BiFault::Lifetime`); a pointer to no
    /// live object fails as a read of it; else whether the object of
    /// static type `t1` (or [`DYN`]) at `a` is within its lifetime: `b`
    /// (the argument's static union designators, `UNION_LIVE`) and every
    /// union on its path from the complete object has it in its active
    /// member.
    pub const LIFETIME: u32 = 25;
    /// `b != 0` and member `sub` of the union of type `t1` at `a` is
    /// (or may be) its active member: one union member access of an
    /// `is_within_lifetime` argument's designator (WP-418).
    pub const UNION_LIVE: u32 = 27;
    /// WP-416: `__builtin_align_up` / `align_down` / `is_aligned` of the
    /// integer `a` (canonical in width | signed << 8 of sub) to the
    /// alignment `b` (canonical in `t2`'s width | signed << 8); sub's
    /// operation << 9 is 0 up, 1 down, 2 `is_aligned` (clang
    /// `getBuiltinAlignArguments`: a negative or non-power-of-two
    /// alignment, or one above 2^(width-1), is a fault).
    pub const ALIGN: u32 = 26;
    /// WP-432: `__builtin_align_up` / `align_down` / `is_aligned` (sub
    /// 0 / 1 / 2) of the pointer `a` (to elements of type `t1`) to the
    /// checked alignment `b` (clang `PointerExprEvaluator` and
    /// `IntExprEvaluator`): the known alignment of `a` is its complete
    /// object's (a variable's declared alignment, else its type's) at
    /// `a`'s byte offset. `is_aligned` is 1 when that is at least `b`, 0
    /// when the object's is but the offset's is not; `align_up` / `down`
    /// is `a` when its alignment is at least `b`, else `a` with its offset
    /// rounded when the object's is; otherwise a fault
    /// (`BiFault::AlignRun`). Executed with the bridge.
    pub const PTR_ALIGN: u32 = 28;
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
    /// WP-403: `fmax` / `fmin` as LLVM 22's `maxnum` / `minnum` (clang
    /// 22): a signaling NaN operand (`a` first) yields itself quieted,
    /// otherwise as [`FMAX`] / [`FMIN`].
    pub const FMAX_22: u32 = 5;
    pub const FMIN_22: u32 = 6;
    /// WP-403: `fmaximum_num` / `fminimum_num` (`maximumnum` /
    /// `minimumnum`): a NaN `a` yields `b` (quieted if a NaN), a NaN `b`
    /// yields `a`; `-0 < +0`.
    pub const FMAXIMUM_NUM: u32 = 7;
    pub const FMINIMUM_NUM: u32 = 8;

    // WP-365: P0533 constexpr <cmath>.
    /// A `<cmath>` operation on `a` (`b`, `c`) in the float kind of sub's
    /// low 4 bits: sub = float kind | `CEIL`..`SQRT` << 4 | [`SECOND`]
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
    /// `sqrt(a)`, correctly rounded (WP-393).
    pub const SQRT: u32 = 21;
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

/// Words the VM fetches per instruction: the op word and up to
/// `WINDOW - 1` operands, in one checked slice read (spec/oven.md
/// "Dispatch"). Every instruction fits one window.
pub(crate) const WINDOW: usize = 8;

const _: () = {
    let mut i = 0;
    while i < Op::OPERANDS.len() {
        assert!((Op::OPERANDS[i] as usize) < WINDOW, "an instruction outgrew the fetch window");
        i += 1;
    }
};

/// Ends a unit of code with `WINDOW - 1` `Nop` words, so the fetch window
/// of its last instruction lies inside the code; `padded` records where
/// the padding ends. The words are never executed (control never falls
/// off the end of a unit).
pub(crate) fn pad(code: &mut Vec<u32>, padded: &mut usize) {
    code.extend_from_slice(&[Op::Nop as u32; WINDOW - 1]);
    *padded = code.len();
}

/// Before a unit is appended: drops the previous unit's padding when it
/// is still the end of the code, so the new unit starts where the
/// padding did and only the last unit carries any. The previous unit's
/// last windows then read the new unit's first words, which no
/// instruction interprets (they lie past its operands, and control never
/// falls off the end of a unit into them). Until the new unit is padded,
/// a window at the previous unit's end takes [`tail_window`].
pub(crate) fn unpad(code: &mut Vec<u32>, padded: &mut usize) {
    if *padded != 0 && code.len() == *padded {
        code.truncate(*padded - (WINDOW - 1));
    }
    *padded = 0;
}

/// The fetch window at `pc` when fewer than [`WINDOW`] words follow it
/// (code read before its unit is padded): the instruction's own words,
/// zero-filled after. Panics, as indexing the code does, when the
/// instruction runs past the end of the code.
#[cold]
#[inline(never)]
pub(crate) fn tail_window(code: &[u32], pc: usize) -> [u32; WINDOW] {
    let n = Op::from_u8(code[pc] as u8).operands();
    let mut w = [0; WINDOW];
    w[..=n].copy_from_slice(&code[pc..=pc + n]);
    w
}

/// The fetch window at `pc`: `code[pc..pc + WINDOW]`, or
/// [`tail_window`]'s when the code ends sooner.
#[inline]
pub(crate) fn window(code: &[u32], pc: usize) -> [u32; WINDOW] {
    let mut tail = [0; WINDOW];
    *window_ref(code, pc, &mut tail)
}

/// [`window`] borrowed: the words in place in `code` (no copy), or
/// [`tail_window`]'s written into `tail` when the code ends sooner. The
/// dispatch loop reads operands through it straight from the code, where
/// a copied window is spilled to the stack and read back (spec/oven.md
/// "Dispatch").
#[inline]
pub(crate) fn window_ref<'a>(
    code: &'a [u32],
    pc: usize,
    tail: &'a mut [u32; WINDOW],
) -> &'a [u32; WINDOW] {
    match code.get(pc..).and_then(|s| s.first_chunk::<WINDOW>()) {
        Some(w) => w,
        None => {
            *tail = tail_window(code, pc);
            tail
        }
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

/// The aux of `Op::PtrAdd` beyond the integer aux of its index
/// ([`int_aux`]) and the negate bit 9: bits 10..=15 say how the run loop
/// steps within an array allocation of the element type inline. 0: it
/// does not (the element's size is not a power of two or may still
/// change, or the index is unsigned 128-bit, whose value may exceed
/// `i128`), and the op runs `Oven::ptr_add`; otherwise the element's size
/// is `1 << (field - 1)`, known for good (the type is not an incomplete
/// class's `Opaque` slot).
pub(crate) mod ptr_add_aux {
    pub const SHIFT: u32 = 10;
    pub const MASK: u32 = 0x3F;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit of every operand count: `Const2` (5), `Builtin` (6) and a
    /// `Ret` (1) last.
    fn unit() -> Vec<u32> {
        let mut code = vec![Op::Const2 as u32, 7, 1, 2, 3, 4];
        code.extend_from_slice(&[Op::Builtin as u32 | 3 << 8, 1, 2, 3, 4, 5, 6]);
        code.extend_from_slice(&[Op::Ret as u32 | 1 << 8, 7]);
        code
    }

    fn pcs(code: &[u32], end: usize) -> Vec<usize> {
        let mut pc = 0;
        let mut out = Vec::new();
        while pc < end {
            out.push(pc);
            pc += 1 + Op::from_u8(code[pc] as u8).operands();
        }
        out
    }

    #[test]
    fn padded_unit_windows_are_in_bounds() {
        let mut code = unit();
        let end = code.len();
        let mut padded = 0;
        pad(&mut code, &mut padded);
        assert_eq!(padded, code.len());
        for pc in pcs(&code, end) {
            assert!(code.len() - pc >= WINDOW, "pc {pc}");
            let w = window(&code, pc);
            assert_eq!(w[..], code[pc..pc + WINDOW]);
        }
        // The padding disassembles as `Nop`s after the unit's code.
        assert!(code[end..].iter().all(|&w| w == Op::Nop as u32));
    }

    /// Only the last unit carries padding: the next unit starts where the
    /// previous one's padding did, and every window of both units still
    /// holds its instruction.
    #[test]
    fn next_unit_reclaims_the_padding() {
        let mut code = unit();
        let mut padded = 0;
        pad(&mut code, &mut padded);
        unpad(&mut code, &mut padded);
        let second = code.len();
        assert_eq!(second, unit().len());
        code.extend_from_slice(&unit());
        pad(&mut code, &mut padded);
        assert_eq!(code.len(), 2 * unit().len() + WINDOW - 1);
        for pc in pcs(&code, code.len() - (WINDOW - 1)) {
            let n = 1 + Op::from_u8(code[pc] as u8).operands();
            assert_eq!(window(&code, pc)[..n], code[pc..pc + n], "pc {pc}");
        }
        // Mid-unit (the code does not end in padding) `unpad` is a no-op.
        code.push(Op::Nop as u32);
        let len = code.len();
        unpad(&mut code, &mut padded);
        assert_eq!(code.len(), len);
    }

    #[test]
    fn tail_window_holds_the_instruction_zero_filled() {
        let code = unit();
        for pc in pcs(&code, code.len()) {
            let n = 1 + Op::from_u8(code[pc] as u8).operands();
            let w = window(&code, pc);
            assert_eq!(w[..n], code[pc..pc + n], "pc {pc}");
            if code.len() - pc < WINDOW {
                assert!(w[n..].iter().all(|&x| x == 0), "pc {pc}");
            }
        }
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn tail_window_of_a_truncated_instruction_panics() {
        let code = [Op::Const as u32, 1, 2];
        let _ = window(&code, 0);
    }
}
