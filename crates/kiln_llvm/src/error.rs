//! Errors reported by the builder and the light verifier (`spec/llvm.md` §4.2).

use std::fmt;

/// What kind of invariant an [`IrError`] reports.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum IrErrorKind {
    /// Operand types disagree with each other or with the instruction.
    TypeMismatch,
    /// A type is not valid in this position.
    InvalidType,
    /// An operand is not valid in this position (e.g. a local value in a
    /// constant, a non-constant where a constant is required).
    InvalidOperand,
    /// A flag is not valid on this instruction (e.g. `nsw` on `udiv`).
    InvalidFlags,
    /// An atomic ordering is not valid for this instruction.
    InvalidOrdering,
    /// An aggregate or vector index is out of range.
    BadIndex,
    /// A block does not end in a terminator, or is empty.
    MissingTerminator,
    /// A terminator appears before the end of its block, or an instruction
    /// was appended after one.
    TerminatorNotLast,
    /// `phi` nodes not at the start of a block.
    PhiNotFirst,
    /// `phi` incoming list does not match the block's predecessors.
    PhiArity,
    /// A block was created but never placed in the function layout, or is
    /// placed twice.
    DetachedBlock,
    /// A call's arguments do not match the callee's function type.
    BadCall,
    /// A named metadata operand or attachment is not a node.
    BadMetadata,
    /// A function definition or declaration is malformed.
    BadFunction,
    /// A global variable or alias is malformed.
    BadGlobal,
}

/// An IR construction or verification error. Never a panic.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IrError {
    /// Category.
    pub kind: IrErrorKind,
    /// Human-readable description.
    pub message: String,
}

impl IrError {
    pub(crate) fn new(kind: IrErrorKind, message: impl Into<String>) -> Self {
        IrError { kind, message: message.into() }
    }
}

impl fmt::Display for IrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for IrError {}

/// Shorthand result type.
pub type IrResult<T> = Result<T, IrError>;
