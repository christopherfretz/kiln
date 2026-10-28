//! LLVM types, interned in a [`TypeTable`] (`spec/llvm.md` §2).
//!
//! Every structural type (integers, pointers, vectors, arrays, literal
//! structs, function types) is interned, so two equal types always share one
//! [`TypeId`] and type equality is id equality. Identified (named) structs
//! are nominal: each [`TypeTable::named_struct`] call creates a new type.

use std::collections::HashMap;
use std::fmt;

use crate::error::{IrError, IrErrorKind};
use crate::print::write_llvm_name;

/// A type in a [`TypeTable`]. Equal structural types have equal ids.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct TypeId(pub(crate) u32);

impl TypeId {
    /// `void`.
    pub const VOID: TypeId = TypeId(0);
    /// `label`.
    pub const LABEL: TypeId = TypeId(1);
    /// `metadata`.
    pub const METADATA: TypeId = TypeId(2);
    /// `token`.
    pub const TOKEN: TypeId = TypeId(3);
    /// `half` (IEEE binary16).
    pub const HALF: TypeId = TypeId(4);
    /// `bfloat`.
    pub const BFLOAT: TypeId = TypeId(5);
    /// `float` (IEEE binary32).
    pub const FLOAT: TypeId = TypeId(6);
    /// `double` (IEEE binary64).
    pub const DOUBLE: TypeId = TypeId(7);
    /// `fp128` (IEEE binary128).
    pub const FP128: TypeId = TypeId(8);
    /// `x86_fp80`.
    pub const X86_FP80: TypeId = TypeId(9);
    /// `ptr` in address space 0.
    pub const PTR: TypeId = TypeId(10);
    /// `i1`.
    pub const I1: TypeId = TypeId(11);
    /// `i8`.
    pub const I8: TypeId = TypeId(12);
    /// `i16`.
    pub const I16: TypeId = TypeId(13);
    /// `i32`.
    pub const I32: TypeId = TypeId(14);
    /// `i64`.
    pub const I64: TypeId = TypeId(15);
    /// `i128`.
    pub const I128: TypeId = TypeId(16);

    /// Dense index of this type in its table.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// The largest integer width LLVM accepts (`IntegerType::MAX_INT_BITS`).
pub const MAX_INT_BITS: u32 = 1 << 23;

/// Structure of a type. Obtained from [`TypeTable::data`].
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum TypeData {
    /// `void`.
    Void,
    /// `label`.
    Label,
    /// `metadata`.
    Metadata,
    /// `token`.
    Token,
    /// `half`.
    Half,
    /// `bfloat`.
    BFloat,
    /// `float`.
    Float,
    /// `double`.
    Double,
    /// `fp128`.
    Fp128,
    /// `x86_fp80`.
    X86Fp80,
    /// `iN`, `1 <= N <= MAX_INT_BITS`.
    Int(u32),
    /// Opaque pointer in an address space.
    Ptr(u32),
    /// `<N x T>` or `<vscale x N x T>`.
    Vector {
        /// Element count (minimum count when scalable).
        len: u32,
        /// Element type: integer, floating-point or pointer.
        elem: TypeId,
        /// `vscale x`.
        scalable: bool,
    },
    /// `[N x T]`.
    Array {
        /// Element count.
        len: u64,
        /// Element type.
        elem: TypeId,
    },
    /// A literal (structurally uniqued) struct `{ ... }` or `<{ ... }>`.
    Struct {
        /// Field types.
        fields: Box<[TypeId]>,
        /// `<{ }>`.
        packed: bool,
    },
    /// An identified struct; the index into the table's named-struct list.
    Named(u32),
    /// `R (P...)` / `R (P..., ...)`.
    Function {
        /// Return type.
        ret: TypeId,
        /// Parameter types.
        params: Box<[TypeId]>,
        /// Trailing `...`.
        varargs: bool,
    },
}

/// An identified struct: name plus optional body (opaque until set).
#[derive(Clone, Debug)]
struct NamedStruct {
    /// Empty for an unnamed identified struct (printed `%N`).
    name: Box<str>,
    /// Number among the unnamed identified structs.
    slot: u32,
    body: Option<(Box<[TypeId]>, bool)>,
}

/// Interning table of types. Owned by a [`crate::Module`].
#[derive(Debug)]
pub struct TypeTable {
    data: Vec<TypeData>,
    map: HashMap<TypeData, TypeId>,
    named: Vec<NamedStruct>,
    named_ids: Vec<TypeId>,
    by_name: HashMap<Box<str>, TypeId>,
    unique_counter: u32,
    unnamed_count: u32,
}

impl Default for TypeTable {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeTable {
    /// A table holding the predefined types (`TypeId::VOID` ...
    /// `TypeId::I128`) at their fixed ids.
    pub fn new() -> Self {
        let mut t = TypeTable {
            data: Vec::new(),
            map: HashMap::new(),
            named: Vec::new(),
            named_ids: Vec::new(),
            by_name: HashMap::new(),
            unique_counter: 0,
            unnamed_count: 0,
        };
        let pre = [
            TypeData::Void,
            TypeData::Label,
            TypeData::Metadata,
            TypeData::Token,
            TypeData::Half,
            TypeData::BFloat,
            TypeData::Float,
            TypeData::Double,
            TypeData::Fp128,
            TypeData::X86Fp80,
            TypeData::Ptr(0),
            TypeData::Int(1),
            TypeData::Int(8),
            TypeData::Int(16),
            TypeData::Int(32),
            TypeData::Int(64),
            TypeData::Int(128),
        ];
        for d in pre {
            t.intern(d);
        }
        debug_assert_eq!(t.data[TypeId::I128.index()], TypeData::Int(128));
        t
    }

    fn intern(&mut self, d: TypeData) -> TypeId {
        if let Some(&id) = self.map.get(&d) {
            return id;
        }
        let id = TypeId(self.data.len() as u32);
        self.data.push(d.clone());
        self.map.insert(d, id);
        id
    }

    pub(crate) fn lookup(&self, d: &TypeData) -> Option<TypeId> {
        self.map.get(d).copied()
    }

    /// Number of types in the table.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Never true: the predefined types are always present.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// The structure of `ty`.
    pub fn data(&self, ty: TypeId) -> &TypeData {
        &self.data[ty.index()]
    }

    /// `iN`. Panics (debug) unless `1 <= bits <= MAX_INT_BITS`.
    pub fn int(&mut self, bits: u32) -> TypeId {
        debug_assert!((1..=MAX_INT_BITS).contains(&bits), "invalid integer width {bits}");
        self.intern(TypeData::Int(bits))
    }

    /// `ptr` / `ptr addrspace(N)`.
    pub fn ptr(&mut self, addrspace: u32) -> TypeId {
        self.intern(TypeData::Ptr(addrspace))
    }

    /// `<len x elem>`.
    pub fn vector(&mut self, elem: TypeId, len: u32) -> TypeId {
        debug_assert!(self.is_valid_vector_elem(elem), "invalid vector element type");
        self.intern(TypeData::Vector { len, elem, scalable: false })
    }

    /// `<vscale x len x elem>`.
    pub fn scalable_vector(&mut self, elem: TypeId, len: u32) -> TypeId {
        debug_assert!(self.is_valid_vector_elem(elem), "invalid vector element type");
        self.intern(TypeData::Vector { len, elem, scalable: true })
    }

    /// `[len x elem]`.
    pub fn array(&mut self, elem: TypeId, len: u64) -> TypeId {
        debug_assert!(self.is_valid_aggregate_elem(elem), "invalid array element type");
        self.intern(TypeData::Array { len, elem })
    }

    /// A literal struct `{ fields }` (or `<{ fields }>` when `packed`).
    pub fn struct_literal(&mut self, fields: &[TypeId], packed: bool) -> TypeId {
        debug_assert!(fields.iter().all(|&f| self.is_valid_aggregate_elem(f)));
        self.intern(TypeData::Struct { fields: fields.into(), packed })
    }

    /// A function type.
    pub fn function(&mut self, ret: TypeId, params: &[TypeId], varargs: bool) -> TypeId {
        self.intern(TypeData::Function { ret, params: params.into(), varargs })
    }

    /// Create a new identified struct, opaque until [`TypeTable::set_body`].
    ///
    /// The name is made unique as LLVM does (`name.N` with a table-wide
    /// counter). An empty name creates an unnamed identified struct, printed
    /// `%N`.
    pub fn named_struct(&mut self, name: &str) -> TypeId {
        let name: Box<str> = if name.is_empty() || !self.by_name.contains_key(name) {
            name.into()
        } else {
            loop {
                self.unique_counter += 1;
                let cand = format!("{name}.{}", self.unique_counter);
                if !self.by_name.contains_key(cand.as_str()) {
                    break cand.into_boxed_str();
                }
            }
        };
        let idx = self.named.len() as u32;
        let id = TypeId(self.data.len() as u32);
        self.data.push(TypeData::Named(idx));
        let slot = if name.is_empty() {
            self.unnamed_count += 1;
            self.unnamed_count - 1
        } else {
            self.by_name.insert(name.clone(), id);
            0
        };
        self.named.push(NamedStruct { name, slot, body: None });
        self.named_ids.push(id);
        id
    }

    /// Set the body of an identified struct. Errors if `ty` is not an
    /// identified struct or already has a body.
    pub fn set_body(&mut self, ty: TypeId, fields: &[TypeId], packed: bool) -> Result<(), IrError> {
        let TypeData::Named(i) = self.data[ty.index()] else {
            return Err(IrError::new(
                IrErrorKind::InvalidType,
                "set_body on a non-identified type",
            ));
        };
        if let Some(f) = fields.iter().find(|&&f| !self.is_valid_aggregate_elem(f)) {
            return Err(IrError::new(
                IrErrorKind::InvalidType,
                format!("invalid struct field type {}", self.display(*f)),
            ));
        }
        let n = &mut self.named[i as usize];
        if n.body.is_some() {
            return Err(IrError::new(IrErrorKind::InvalidType, "struct body already set"));
        }
        n.body = Some((fields.into(), packed));
        Ok(())
    }

    /// Look up an identified struct by its (uniqued) name.
    pub fn get_named(&self, name: &str) -> Option<TypeId> {
        self.by_name.get(name).copied()
    }

    /// The name of an identified struct (empty if unnamed).
    pub fn struct_name(&self, ty: TypeId) -> Option<&str> {
        match self.data[ty.index()] {
            TypeData::Named(i) => Some(&self.named[i as usize].name),
            _ => None,
        }
    }

    /// Identified structs in creation order.
    pub fn named_structs(&self) -> &[TypeId] {
        &self.named_ids
    }

    /// True for an identified struct without a body.
    pub fn is_opaque_struct(&self, ty: TypeId) -> bool {
        matches!(self.data[ty.index()], TypeData::Named(i) if self.named[i as usize].body.is_none())
    }

    /// Fields of a (literal or identified) struct; `None` for opaque or
    /// non-struct types.
    pub fn struct_fields(&self, ty: TypeId) -> Option<&[TypeId]> {
        match &self.data[ty.index()] {
            TypeData::Struct { fields, .. } => Some(fields),
            TypeData::Named(i) => self.named[*i as usize].body.as_ref().map(|b| &*b.0),
            _ => None,
        }
    }

    /// Packedness of a struct with a body.
    pub fn struct_is_packed(&self, ty: TypeId) -> bool {
        match &self.data[ty.index()] {
            TypeData::Struct { packed, .. } => *packed,
            TypeData::Named(i) => self.named[*i as usize].body.as_ref().is_some_and(|b| b.1),
            _ => false,
        }
    }

    /// Bit width of an integer type.
    pub fn int_width(&self, ty: TypeId) -> Option<u32> {
        match self.data[ty.index()] {
            TypeData::Int(n) => Some(n),
            _ => None,
        }
    }

    /// `iN`.
    pub fn is_int(&self, ty: TypeId) -> bool {
        matches!(self.data[ty.index()], TypeData::Int(_))
    }

    /// Any floating-point type.
    pub fn is_float(&self, ty: TypeId) -> bool {
        matches!(
            self.data[ty.index()],
            TypeData::Half
                | TypeData::BFloat
                | TypeData::Float
                | TypeData::Double
                | TypeData::Fp128
                | TypeData::X86Fp80
        )
    }

    /// Bit width of a floating-point type.
    pub fn float_width(&self, ty: TypeId) -> Option<u32> {
        Some(match self.data[ty.index()] {
            TypeData::Half | TypeData::BFloat => 16,
            TypeData::Float => 32,
            TypeData::Double => 64,
            TypeData::X86Fp80 => 80,
            TypeData::Fp128 => 128,
            _ => return None,
        })
    }

    /// `ptr` in any address space.
    pub fn is_ptr(&self, ty: TypeId) -> bool {
        matches!(self.data[ty.index()], TypeData::Ptr(_))
    }

    /// Address space of a pointer type.
    pub fn ptr_addrspace(&self, ty: TypeId) -> Option<u32> {
        match self.data[ty.index()] {
            TypeData::Ptr(a) => Some(a),
            _ => None,
        }
    }

    /// Vector element type and fixed length.
    pub fn vector_info(&self, ty: TypeId) -> Option<(TypeId, u32, bool)> {
        match self.data[ty.index()] {
            TypeData::Vector { len, elem, scalable } => Some((elem, len, scalable)),
            _ => None,
        }
    }

    /// Array element type and length.
    pub fn array_info(&self, ty: TypeId) -> Option<(TypeId, u64)> {
        match self.data[ty.index()] {
            TypeData::Array { len, elem } => Some((elem, len)),
            _ => None,
        }
    }

    /// The scalar type of `ty`: the element type of a vector, else `ty`.
    pub fn scalar(&self, ty: TypeId) -> TypeId {
        match self.data[ty.index()] {
            TypeData::Vector { elem, .. } => elem,
            _ => ty,
        }
    }

    /// Integer or vector of integers.
    pub fn is_int_or_int_vector(&self, ty: TypeId) -> bool {
        self.is_int(self.scalar(ty))
    }

    /// Floating-point or vector of floating-point.
    pub fn is_float_or_float_vector(&self, ty: TypeId) -> bool {
        self.is_float(self.scalar(ty))
    }

    /// Pointer or vector of pointers.
    pub fn is_ptr_or_ptr_vector(&self, ty: TypeId) -> bool {
        self.is_ptr(self.scalar(ty))
    }

    /// Function type parts: return type, parameters, varargs.
    pub fn function_info(&self, ty: TypeId) -> Option<(TypeId, &[TypeId], bool)> {
        match &self.data[ty.index()] {
            TypeData::Function { ret, params, varargs } => Some((*ret, params, *varargs)),
            _ => None,
        }
    }

    /// True for a function type.
    pub fn is_function(&self, ty: TypeId) -> bool {
        matches!(self.data[ty.index()], TypeData::Function { .. })
    }

    /// Struct, array (aggregates usable with extractvalue/insertvalue).
    pub fn is_aggregate(&self, ty: TypeId) -> bool {
        matches!(
            self.data[ty.index()],
            TypeData::Struct { .. } | TypeData::Named(_) | TypeData::Array { .. }
        )
    }

    /// A first-class type: may be the type of an SSA value.
    pub fn is_first_class(&self, ty: TypeId) -> bool {
        !matches!(self.data[ty.index()], TypeData::Void | TypeData::Function { .. })
    }

    fn is_valid_vector_elem(&self, ty: TypeId) -> bool {
        self.is_int(ty) || self.is_float(ty) || self.is_ptr(ty)
    }

    fn is_valid_aggregate_elem(&self, ty: TypeId) -> bool {
        !matches!(
            self.data[ty.index()],
            TypeData::Void
                | TypeData::Label
                | TypeData::Metadata
                | TypeData::Function { .. }
                | TypeData::Token
        )
    }

    /// True when `ty` has a size: everything except void, label, metadata,
    /// token, functions, opaque structs and aggregates containing them.
    pub fn is_sized(&self, ty: TypeId) -> bool {
        match &self.data[ty.index()] {
            TypeData::Void
            | TypeData::Label
            | TypeData::Metadata
            | TypeData::Token
            | TypeData::Function { .. } => false,
            TypeData::Array { elem, .. } | TypeData::Vector { elem, .. } => self.is_sized(*elem),
            TypeData::Struct { fields, .. } => fields.iter().all(|&f| self.is_sized(f)),
            TypeData::Named(i) => match &self.named[*i as usize].body {
                Some((fields, _)) => fields.iter().all(|&f| self.is_sized(f)),
                None => false,
            },
            _ => true,
        }
    }

    /// The type at aggregate index `idx` of `ty` (struct field or array
    /// element), as used by `extractvalue`/`insertvalue`.
    pub fn aggregate_elem(&self, ty: TypeId, idx: u32) -> Option<TypeId> {
        match &self.data[ty.index()] {
            TypeData::Array { len, elem } => (u64::from(idx) < *len).then_some(*elem),
            _ => self.struct_fields(ty).and_then(|f| f.get(idx as usize).copied()),
        }
    }

    /// Follow `extractvalue` indices.
    pub fn indexed_type(&self, mut ty: TypeId, idx: &[u32]) -> Option<TypeId> {
        for &i in idx {
            ty = self.aggregate_elem(ty, i)?;
        }
        Some(ty)
    }

    /// Display adapter printing a type in LLVM syntax.
    pub fn display(&self, ty: TypeId) -> TypeDisplay<'_> {
        TypeDisplay { table: self, ty }
    }

    /// Write `ty` in LLVM syntax.
    pub(crate) fn write<W: fmt::Write + ?Sized>(&self, w: &mut W, ty: TypeId) -> fmt::Result {
        match &self.data[ty.index()] {
            TypeData::Void => w.write_str("void"),
            TypeData::Label => w.write_str("label"),
            TypeData::Metadata => w.write_str("metadata"),
            TypeData::Token => w.write_str("token"),
            TypeData::Half => w.write_str("half"),
            TypeData::BFloat => w.write_str("bfloat"),
            TypeData::Float => w.write_str("float"),
            TypeData::Double => w.write_str("double"),
            TypeData::Fp128 => w.write_str("fp128"),
            TypeData::X86Fp80 => w.write_str("x86_fp80"),
            TypeData::Int(n) => write!(w, "i{n}"),
            TypeData::Ptr(0) => w.write_str("ptr"),
            TypeData::Ptr(a) => write!(w, "ptr addrspace({a})"),
            TypeData::Vector { len, elem, scalable } => {
                w.write_str(if *scalable { "<vscale x " } else { "<" })?;
                write!(w, "{len} x ")?;
                self.write(w, *elem)?;
                w.write_char('>')
            }
            TypeData::Array { len, elem } => {
                write!(w, "[{len} x ")?;
                self.write(w, *elem)?;
                w.write_char(']')
            }
            TypeData::Struct { fields, packed } => self.write_body(w, fields, *packed),
            TypeData::Named(i) => {
                let n = &self.named[*i as usize];
                if n.name.is_empty() {
                    write!(w, "%{}", n.slot)
                } else {
                    write_llvm_name(w, '%', &n.name)
                }
            }
            TypeData::Function { ret, params, varargs } => {
                self.write(w, *ret)?;
                w.write_str(" (")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        w.write_str(", ")?;
                    }
                    self.write(w, *p)?;
                }
                if *varargs {
                    w.write_str(if params.is_empty() { "..." } else { ", ..." })?;
                }
                w.write_char(')')
            }
        }
    }

    pub(crate) fn write_body<W: fmt::Write + ?Sized>(
        &self,
        w: &mut W,
        fields: &[TypeId],
        packed: bool,
    ) -> fmt::Result {
        if packed {
            w.write_char('<')?;
        }
        if fields.is_empty() {
            w.write_str("{}")?;
        } else {
            w.write_str("{ ")?;
            for (i, f) in fields.iter().enumerate() {
                if i > 0 {
                    w.write_str(", ")?;
                }
                self.write(w, *f)?;
            }
            w.write_str(" }")?;
        }
        if packed {
            w.write_char('>')?;
        }
        Ok(())
    }

    /// Write the definition line body of an identified struct: `type { ... }`
    /// or `type opaque`.
    pub(crate) fn write_definition<W: fmt::Write + ?Sized>(
        &self,
        w: &mut W,
        ty: TypeId,
    ) -> fmt::Result {
        let TypeData::Named(i) = self.data[ty.index()] else {
            unreachable!("write_definition on a non-identified struct")
        };
        self.write(w, ty)?;
        w.write_str(" = type ")?;
        match &self.named[i as usize].body {
            None => w.write_str("opaque"),
            Some((fields, packed)) => self.write_body(w, fields, *packed),
        }
    }

    /// Suffix for an overloaded intrinsic name (`i64`, `p0`, `f32`,
    /// `v4i32`, `a2i8`, `sl_i32i64s`, struct name, `f_...f`), as LLVM's
    /// `Intrinsic::getName` mangles it.
    pub fn intrinsic_suffix(&self, ty: TypeId) -> String {
        let mut s = String::new();
        self.mangle(&mut s, ty);
        s
    }

    fn mangle(&self, s: &mut String, ty: TypeId) {
        use fmt::Write as _;
        match &self.data[ty.index()] {
            TypeData::Void => s.push_str("isVoid"),
            TypeData::Label => s.push_str("label"),
            TypeData::Metadata => s.push_str("Metadata"),
            TypeData::Token => s.push_str("token"),
            TypeData::Half => s.push_str("f16"),
            TypeData::BFloat => s.push_str("bf16"),
            TypeData::Float => s.push_str("f32"),
            TypeData::Double => s.push_str("f64"),
            TypeData::Fp128 => s.push_str("f128"),
            TypeData::X86Fp80 => s.push_str("f80"),
            TypeData::Int(n) => {
                let _ = write!(s, "i{n}");
            }
            TypeData::Ptr(a) => {
                let _ = write!(s, "p{a}");
            }
            TypeData::Vector { len, elem, scalable } => {
                let _ = write!(s, "{}v{len}", if *scalable { "nx" } else { "" });
                self.mangle(s, *elem);
            }
            TypeData::Array { len, elem } => {
                let _ = write!(s, "a{len}");
                self.mangle(s, *elem);
            }
            TypeData::Struct { fields, .. } => {
                s.push_str("sl_");
                for f in fields {
                    self.mangle(s, *f);
                }
                s.push('s');
            }
            TypeData::Named(i) => {
                let n = &self.named[*i as usize];
                if n.name.is_empty() {
                    let _ = write!(s, "s_{}", n.slot);
                } else {
                    s.push_str("s_");
                    s.push_str(&n.name);
                }
            }
            TypeData::Function { ret, params, varargs } => {
                s.push_str("f_");
                self.mangle(s, *ret);
                for p in params {
                    self.mangle(s, *p);
                }
                if *varargs {
                    s.push_str("vararg");
                }
                s.push('f');
            }
        }
    }
}

/// [`fmt::Display`] adapter for a type (see [`TypeTable::display`]).
#[derive(Debug, Clone, Copy)]
pub struct TypeDisplay<'a> {
    table: &'a TypeTable,
    ty: TypeId,
}

impl fmt::Display for TypeDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.table.write(f, self.ty)
    }
}
