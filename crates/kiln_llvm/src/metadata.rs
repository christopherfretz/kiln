//! Metadata (`spec/llvm.md` §2.6).
//!
//! Strings, value wrappers and non-`distinct` nodes are uniqued; `distinct`
//! nodes are always new. Tuples (`!{...}`) and specialized nodes
//! (`!DILocation(line: 1, ...)`, a generic name-plus-fields carrier that is
//! enough to express every `DI*` node) receive `!N` numbers when printed;
//! strings and values print inline, as do `DIExpression` and `DIArgList`
//! nodes, as in LLVM.

use std::borrow::Cow;
use std::hash::{Hash, Hasher};

use kiln_source::fx::FxHasher;

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
    /// DIFlagPublic`, `DW_OP_deref`): usually a literal, so borrowed.
    Raw(Cow<'static, str>),
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
        name: &'static str,
        /// Fields in print order; the keys are literals.
        fields: Box<[(&'static str, MdField)]>,
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

/// The uniquing hash of a node: consistent with `Eq`, and cheaper than
/// the derived `Hash` (a specialized node's name and keys are literals
/// that are hashed by length only; its values carry the information).
pub(crate) fn md_hash(n: &MdNode) -> u64 {
    let mut h = FxHasher::default();
    match n {
        MdNode::String(s) => {
            h.write_u8(0);
            h.write(s);
        }
        MdNode::Value(t, v) => {
            h.write_u8(1);
            t.hash(&mut h);
            v.hash(&mut h);
        }
        MdNode::Tuple { distinct, ops } => {
            h.write_u8(2 + u8::from(*distinct));
            h.write_usize(ops.len());
            for o in ops {
                h.write_u32(o.map_or(u32::MAX, |m| m.0));
            }
        }
        MdNode::Specialized { distinct, name, fields } => {
            h.write_u8(4 + u8::from(*distinct));
            h.write_usize(name.len());
            h.write_usize(fields.len());
            for (k, f) in fields {
                h.write_usize(k.len());
                match f {
                    MdField::Int(x) => h.write_i64(*x),
                    MdField::UInt(x) => h.write_u64(*x),
                    MdField::Bool(b) => h.write_u8(u8::from(*b)),
                    MdField::Str(s) => h.write(s.as_bytes()),
                    MdField::Node(o) => h.write_u32(o.map_or(u32::MAX, |m| m.0)),
                    MdField::Raw(r) => h.write(r.as_bytes()),
                }
            }
        }
    }
    h.finish()
}

pub(crate) fn is_inline_specialized(name: &str) -> bool {
    name == "DIExpression" || name == "DIArgList"
}
