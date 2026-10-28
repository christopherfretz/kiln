//! Metadata (`spec/llvm.md` §2.6).
//!
//! Strings, value wrappers and non-`distinct` nodes are uniqued; `distinct`
//! nodes are always new. Tuples (`!{...}`) and specialized nodes
//! (`!DILocation(line: 1, ...)`, a generic name-plus-fields carrier that is
//! enough to express every `DI*` node) receive `!N` numbers when printed;
//! strings and values print inline, as do `DIExpression` and `DIArgList`
//! nodes, as in LLVM.

use crate::types::TypeId;
use crate::value::{MetadataId, Value};

/// A field value of a specialized node.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum MdField {
    /// Signed integer.
    Int(i64),
    /// Unsigned integer.
    UInt(u64),
    /// `true` / `false`.
    Bool(bool),
    /// Quoted, escaped string.
    Str(Box<str>),
    /// Reference to another node, or `null`.
    Node(Option<MetadataId>),
    /// Printed verbatim (`DW_TAG_member`, `DIFlagPrototyped |
    /// DIFlagPublic`, `DW_OP_deref`).
    Raw(Box<str>),
}

/// A metadata node.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum MdNode {
    /// `!"..."` (arbitrary bytes).
    String(Box<[u8]>),
    /// A value as metadata: `i32 1`, `ptr @g`, or a function-local value
    /// (only as a call argument). The type is the value's type.
    Value(TypeId, Value),
    /// `!{...}`; `None` operands print as `null`.
    Tuple {
        /// `distinct`.
        distinct: bool,
        /// Operands.
        ops: Box<[Option<MetadataId>]>,
    },
    /// `!Name(key: value, ...)`; an empty key prints the value positionally
    /// (`!DIExpression(DW_OP_deref)`).
    Specialized {
        /// `distinct`.
        distinct: bool,
        /// Node class name without `!` (e.g. `DILocation`).
        name: Box<str>,
        /// Fields in print order.
        fields: Box<[(Box<str>, MdField)]>,
    },
}

impl MdNode {
    /// True if the node gets a `!N` slot (tuples and specialized nodes
    /// other than `DIExpression`/`DIArgList`).
    pub(crate) fn is_numbered(&self) -> bool {
        match self {
            MdNode::Tuple { .. } => true,
            MdNode::Specialized { name, .. } => !is_inline_specialized(name),
            _ => false,
        }
    }

    pub(crate) fn is_distinct(&self) -> bool {
        matches!(
            self,
            MdNode::Tuple { distinct: true, .. } | MdNode::Specialized { distinct: true, .. }
        )
    }
}

pub(crate) fn is_inline_specialized(name: &str) -> bool {
    name == "DIExpression" || name == "DIArgList"
}
