//! Values: 4-byte tagged handles (`spec/llvm.md` §3).

/// Declares a `u32` id newtype.
macro_rules! id_type {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
        pub struct $name(pub(crate) u32);
        impl $name {
            /// Dense index into the owning arena.
            pub fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

id_type!(
    /// An instruction in a function (function-local).
    InstId
);
id_type!(
    /// A basic block in a function (function-local).
    BlockId
);
id_type!(
    /// An interned constant in a module.
    ConstId
);
id_type!(
    /// A function in a module.
    FuncId
);
id_type!(
    /// A global variable in a module.
    GlobalId
);
id_type!(
    /// A global alias in a module.
    AliasId
);
id_type!(
    /// An inline asm blob in a module.
    AsmId
);
id_type!(
    /// A metadata node in a module.
    MetadataId
);
id_type!(
    /// A comdat in a module.
    ComdatId
);
id_type!(
    /// An interned attribute set in a module (0 is the empty set).
    AttrSetId
);
id_type!(
    /// An interned metadata kind name (`dbg`, `tbaa`, ...) in a module.
    MdKindId
);

impl AttrSetId {
    /// The empty attribute set.
    pub const EMPTY: AttrSetId = AttrSetId(0);
}

const TAG_SHIFT: u32 = 28;
const INDEX_MASK: u32 = (1 << TAG_SHIFT) - 1;

const TAG_INST: u32 = 0;
const TAG_ARG: u32 = 1;
const TAG_CONST: u32 = 2;
const TAG_FUNC: u32 = 3;
const TAG_GLOBAL: u32 = 4;
const TAG_ALIAS: u32 = 5;
const TAG_ASM: u32 = 6;
const TAG_META: u32 = 7;
const TAG_BLOCK: u32 = 8;
const TAG_NONE: u32 = 15;

/// An SSA value or operand: an instruction result, a function argument, a
/// constant, a global value (function, variable, alias), inline asm (only as
/// a callee), metadata (only as an intrinsic argument) or, inside operand
/// pools, a basic block.
///
/// 4 bytes: a 4-bit tag and a 28-bit index. Instruction, argument and block
/// values are only meaningful inside the function that created them.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Value(u32);

/// The decoded form of a [`Value`].
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum ValueRef {
    /// Result of an instruction.
    Inst(InstId),
    /// Function argument by position.
    Arg(u32),
    /// Interned constant.
    Const(ConstId),
    /// Function (a `ptr` constant).
    Func(FuncId),
    /// Global variable (a `ptr` constant).
    Global(GlobalId),
    /// Alias (a `ptr` constant).
    Alias(AliasId),
    /// Inline asm (callee only).
    Asm(AsmId),
    /// Metadata wrapped as a value (type `metadata`).
    Metadata(MetadataId),
    /// A basic block (operand pools only).
    Block(BlockId),
}

impl Value {
    /// Absent operand sentinel (never a real value).
    pub(crate) const NONE: Value = Value(TAG_NONE << TAG_SHIFT);

    const fn pack(tag: u32, idx: u32) -> Value {
        debug_assert!(idx <= INDEX_MASK, "arena index exceeds 2^28");
        Value((tag << TAG_SHIFT) | idx)
    }

    /// Instruction result.
    pub fn inst(i: InstId) -> Value {
        Value::pack(TAG_INST, i.0)
    }
    /// Function argument `n`.
    pub fn arg(n: u32) -> Value {
        Value::pack(TAG_ARG, n)
    }
    /// Constant.
    pub fn constant(c: ConstId) -> Value {
        Value::pack(TAG_CONST, c.0)
    }
    /// Function as a value.
    pub fn func(f: FuncId) -> Value {
        Value::pack(TAG_FUNC, f.0)
    }
    /// Global variable as a value.
    pub fn global(g: GlobalId) -> Value {
        Value::pack(TAG_GLOBAL, g.0)
    }
    /// Alias as a value.
    pub fn alias(a: AliasId) -> Value {
        Value::pack(TAG_ALIAS, a.0)
    }
    /// Inline asm as a callee.
    pub fn asm(a: AsmId) -> Value {
        Value::pack(TAG_ASM, a.0)
    }
    /// Metadata as a value (intrinsic argument of type `metadata`).
    pub fn metadata(m: MetadataId) -> Value {
        Value::pack(TAG_META, m.0)
    }
    pub(crate) fn block(b: BlockId) -> Value {
        Value::pack(TAG_BLOCK, b.0)
    }

    pub(crate) fn is_none(self) -> bool {
        self == Value::NONE
    }

    pub(crate) fn opt(self) -> Option<Value> {
        (!self.is_none()).then_some(self)
    }

    /// Decode.
    pub fn get(self) -> ValueRef {
        let idx = self.0 & INDEX_MASK;
        match self.0 >> TAG_SHIFT {
            TAG_INST => ValueRef::Inst(InstId(idx)),
            TAG_ARG => ValueRef::Arg(idx),
            TAG_CONST => ValueRef::Const(ConstId(idx)),
            TAG_FUNC => ValueRef::Func(FuncId(idx)),
            TAG_GLOBAL => ValueRef::Global(GlobalId(idx)),
            TAG_ALIAS => ValueRef::Alias(AliasId(idx)),
            TAG_ASM => ValueRef::Asm(AsmId(idx)),
            TAG_META => ValueRef::Metadata(MetadataId(idx)),
            TAG_BLOCK => ValueRef::Block(BlockId(idx)),
            _ => unreachable!("decoding Value::NONE"),
        }
    }

    /// True for function-local values (instruction results, arguments,
    /// blocks).
    pub fn is_local(self) -> bool {
        matches!(self.0 >> TAG_SHIFT, TAG_INST | TAG_ARG | TAG_BLOCK)
    }

    /// True for values usable as constant operands: constants and global
    /// values.
    pub fn is_constant(self) -> bool {
        matches!(self.0 >> TAG_SHIFT, TAG_CONST | TAG_FUNC | TAG_GLOBAL | TAG_ALIAS)
    }

    /// The constant id, if this is an interned constant.
    pub fn as_const(self) -> Option<ConstId> {
        match self.get_opt()? {
            ValueRef::Const(c) => Some(c),
            _ => None,
        }
    }

    /// The instruction id, if this is an instruction result.
    pub fn as_inst(self) -> Option<InstId> {
        match self.get_opt()? {
            ValueRef::Inst(i) => Some(i),
            _ => None,
        }
    }

    pub(crate) fn as_block(self) -> BlockId {
        match self.get() {
            ValueRef::Block(b) => b,
            _ => unreachable!("operand pool slot is not a block"),
        }
    }

    fn get_opt(self) -> Option<ValueRef> {
        (!self.is_none()).then(|| self.get())
    }
}
