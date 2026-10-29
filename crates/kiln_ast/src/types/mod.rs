//! The type system (spec/types.md): hash-consed 8-byte type nodes with a
//! canonical link, template names, nested-name-specifiers, template
//! argument lists, expression profiles, layout hooks and the printer.
//!
//! Module map:
//! - `kinds`: qualifiers, flags, kinds, builtins and small enums;
//! - `func`: function prototypes and exception specifications;
//! - `node`, `intern`: storage and hash-consing;
//! - `context`: [`TypeContext`] storage and queries; `canon`: the
//!   constructors and canonical-type rules;
//! - `names`, `nns`, `args`, `profile`: the side tables of §7.2 and §8;
//! - `layout`: [`TargetTypes`], [`RecordLayout`], [`LayoutCache`];
//! - `print`: the clang-compatible [`TypePrinter`](print_type).

#![allow(clippy::many_single_char_names, reason = "payload slots a/b/c, ids t/q/d/n")]

mod args;
mod canon;
mod context;
mod func;
mod intern;
mod kinds;
mod layout;
mod names;
mod nns;
mod node;
mod print;
mod profile;

pub use args::{ArgKind, ArgView, TemplateArg};
pub use context::{
    ExprOperand, FIRST_USER_TYPE, TypeContext, TypeOfOperand, TypeOptions, TypeView,
};
pub use func::{ExcSpec, FuncInfo, FuncProto, NoexceptValue, TypeList};
pub use kinds::{
    AutoKeyword, BuiltinType, CallConv, Quals, RefQual, TagKeyword, TypeAttrKind, TypeFlags,
    TypeKind, UnaryTransformKind, VectorKind,
};
pub use layout::{FloatFormat, LayoutCache, RecordLayout, TargetTypes};
pub use names::{DeclSet, TemplateName};
pub use nns::{Nns, NnsKind};
pub use node::{ConstArray, TypeNode};
pub use print::{
    PrintOptions, RawNames, TypeNames, aka, dump_strings, print_type, write_arg_list, write_nns,
    write_type,
};
