//! Printing constant values (spec/ast.md §9.3): the `value=` fields of the
//! AST dump, diagnostics, and template arguments in type names.
//!
//! The store knows bytes, not types: an aggregate image is decoded through
//! a [`PrintEnv`], which describes each type's [`Shape`] (array element
//! stride, record member offsets, integer width/signedness/style) and names
//! declarations and types. The AST core implements it over `TypeContext`
//! and the record layouts; [`NoEnv`] is a context-free fallback (opaque
//! aggregates, `#d17`-style names).

use std::fmt::Write as _;

use super::ConstStore;
use super::float::format_float;
use super::value::{ByteImage, ConstValue, FloatKind, IntBits, PtrBase, Scalar};
use crate::ids::{ConstValueId, DeclId, QualType, ReflectionId};
use crate::strings::{StringEncoding, StringPool, push_unit};

/// Which syntax values print in.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PrintStyle {
    /// The AST dump's `value=` fields: integers in plain decimal, references
    /// like pointers (`&x`), aggregates as `{1, 2}`.
    #[default]
    Dump,
    /// Template arguments in type names, after clang's
    /// `TemplateArgument::print`: integer suffixes or casts, character
    /// literals, enumerators, references without `&`, class values as
    /// `S{1, 2}`.
    TemplateArg,
}

/// Print settings.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PrintOptions {
    /// The syntax.
    pub style: PrintStyle,
    /// Aggregates nested deeper than this print as `{...}` (the top-level
    /// aggregate is depth 0, so 0 prints every aggregate as `{...}`).
    pub max_depth: u32,
    /// Arrays and records print at most this many elements, then `...`.
    pub max_elems: u32,
}

impl PrintOptions {
    /// The default limits: depth 10, 32 elements.
    pub const DEFAULT: PrintOptions =
        PrintOptions { style: PrintStyle::Dump, max_depth: 10, max_elems: 32 };

    /// The defaults in `style`.
    pub const fn with_style(style: PrintStyle) -> Self {
        PrintOptions { style, ..Self::DEFAULT }
    }
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// How an integer prints in [`PrintStyle::TemplateArg`] (in
/// [`PrintStyle::Dump`] every integer is plain decimal).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntStyle {
    /// Decimal with a literal suffix (`""` for `int`, `"U"`, `"L"`, `"UL"`,
    /// `"LL"`, `"ULL"`).
    Plain {
        /// The suffix.
        suffix: &'static str,
    },
    /// No literal form exists: `(short)5`.
    Cast,
    /// A character type: `'a'` with the encoding's prefix.
    Char(StringEncoding),
    /// An enumeration: its enumerator ([`PrintEnv::enumerator`]) or `(E)7`.
    Enum,
}

/// A record member as the printer needs it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Member {
    /// The field name; empty for a base class subobject or an unnamed
    /// bit-field.
    pub name: String,
    /// The member's type.
    pub ty: QualType,
    /// Offset from the start of the record, in bits (a multiple of 8
    /// except for bit-fields).
    pub offset_bits: u64,
    /// Size in bytes (0 for bit-fields and empty members).
    pub size: u64,
    /// Width of a bit-field.
    pub bit_width: Option<u32>,
    /// The `FieldDecl` (`None` for bases); matches
    /// [`UnionMember::field`](super::UnionMember::field).
    pub field: Option<DeclId>,
}

/// What the printer needs to know about a type.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Shape {
    /// Unknown: an aggregate prints as `{...}`, an image scalar as `?`.
    Unknown,
    /// `bool` (one byte in images).
    Bool,
    /// An integer, character or enumeration type.
    Int {
        /// Value bits (`8 * sizeof` for ordinary integers; N for
        /// `_BitInt(N)`, whose remaining object bits are padding).
        width: u32,
        /// Signedness.
        signed: bool,
        /// Template-argument style.
        style: IntStyle,
    },
    /// A floating-point type (its significant bytes are
    /// [`FloatKind::value_bytes`]).
    Float(FloatKind),
    /// A pointer or reference.
    Pointer {
        /// The pointee type, when known (used to stop designator recovery:
        /// `&a` vs `&a[0]`).
        pointee: Option<QualType>,
        /// A reference (prints without `&` in template arguments).
        reference: bool,
    },
    /// A pointer to member.
    MemberPointer,
    /// `std::meta::info`.
    Reflection,
    /// `std::nullptr_t`.
    NullPtr,
    /// An array (also `_Complex` as `[2]` and vectors).
    Array {
        /// Element type.
        elem: QualType,
        /// Bytes between elements.
        stride: u64,
        /// Number of elements.
        len: u64,
    },
    /// A class or union, members in declaration order with bases first.
    Record {
        /// The members.
        members: Vec<Member>,
        /// A union.
        union: bool,
    },
}

/// The type and declaration knowledge printing needs; every method has a
/// context-free default.
pub trait PrintEnv {
    /// The shape of `ty`.
    fn shape(&self, ty: QualType) -> Shape {
        let _ = ty;
        Shape::Unknown
    }
    /// Appends a declaration's (qualified, as appropriate) name.
    fn decl_name(&self, decl: DeclId, out: &mut String) {
        let _ = write!(out, "#d{}", decl.index());
    }
    /// The declared type of a variable or temporary (for designators).
    fn decl_type(&self, decl: DeclId) -> Option<QualType> {
        let _ = decl;
        None
    }
    /// Appends a type's name.
    fn type_name(&self, ty: QualType, out: &mut String) {
        let _ = write!(out, "#t{}", ty.ty().index());
    }
    /// Appends the enumerator of enumeration `ty` with value `bits`
    /// (qualified, `E::X`); returns false if there is none.
    fn enumerator(&self, ty: QualType, bits: &IntBits<'_>, out: &mut String) -> bool {
        let _ = (ty, bits, out);
        false
    }
    /// Appends a reflection (`^^x`).
    fn reflection(&self, id: ReflectionId, out: &mut String) {
        let _ = write!(out, "^^#r{}", id.index());
    }
}

/// The context-free [`PrintEnv`].
#[derive(Clone, Copy, Debug, Default)]
pub struct NoEnv;

impl PrintEnv for NoEnv {}

impl IntBits<'_> {
    /// Appends the value in decimal, as signed or unsigned.
    pub fn write_decimal(&self, signed: bool, out: &mut String) {
        if self.width() <= 128 {
            if signed {
                let _ = write!(out, "{}", self.to_i128().unwrap_or(0));
            } else {
                let _ = write!(out, "{}", self.to_u128().unwrap_or(0));
            }
            return;
        }
        let mut w: Vec<u64> = self.words().to_vec();
        if signed && self.sign_bit() {
            out.push('-');
            // Two's-complement negation; bits above `width` are discarded.
            let mut carry = true;
            for x in &mut w {
                let (v, c) = (!*x).overflowing_add(u64::from(carry));
                *x = v;
                carry = c;
            }
            let rem = self.width() % 64;
            if rem != 0 {
                let last = w.len() - 1;
                w[last] &= (1u64 << rem) - 1;
            }
        }
        let mut chunks = Vec::new();
        while w.iter().any(|&x| x != 0) {
            let mut r = 0u128;
            for x in w.iter_mut().rev() {
                let cur = (r << 64) | u128::from(*x);
                *x = (cur / 1_000_000_000) as u64;
                r = cur % 1_000_000_000;
            }
            chunks.push(r as u32);
        }
        match chunks.split_last() {
            None => out.push('0'),
            Some((top, rest)) => {
                let _ = write!(out, "{top}");
                for c in rest.iter().rev() {
                    let _ = write!(out, "{c:09}");
                }
            }
        }
    }
}

struct Printer<'a> {
    store: &'a ConstStore,
    strings: &'a StringPool,
    env: &'a dyn PrintEnv,
    opts: &'a PrintOptions,
}

impl ConstStore {
    /// Appends value `id` to `out` (spec/ast.md §9.3).
    pub fn print(
        &self,
        id: ConstValueId,
        strings: &StringPool,
        env: &dyn PrintEnv,
        opts: &PrintOptions,
        out: &mut String,
    ) {
        Printer { store: self, strings, env, opts }.value(id, 0, out);
    }

    /// Value `id` as a string ([`ConstStore::print`]).
    pub fn display(
        &self,
        id: ConstValueId,
        strings: &StringPool,
        env: &dyn PrintEnv,
        opts: &PrintOptions,
    ) -> String {
        let mut s = String::new();
        self.print(id, strings, env, opts, &mut s);
        s
    }
}

/// Reads `width` bits starting at bit `start` of `bytes` (little-endian)
/// as normalized words; bits past the end read as zero.
fn read_bits(bytes: &[u8], start: u64, width: u32) -> Vec<u64> {
    let mut w = vec![0u64; IntBits::words_for(width)];
    for i in 0..u64::from(width) {
        let b = start + i;
        let set = bytes.get((b / 8) as usize).is_some_and(|&x| x >> (b % 8) & 1 != 0);
        if set {
            w[(i / 64) as usize] |= 1 << (i % 64);
        }
    }
    w
}

fn read_le(bytes: &[u8], at: u64, n: u32) -> u128 {
    (0..u64::from(n.min(16))).rev().fold(0u128, |acc, i| {
        (acc << 8) | u128::from(bytes.get((at + i) as usize).copied().unwrap_or(0))
    })
}

impl Printer<'_> {
    fn template(&self) -> bool {
        self.opts.style == PrintStyle::TemplateArg
    }

    fn value(&self, id: ConstValueId, depth: u32, out: &mut String) {
        match self.store.get(id) {
            ConstValue::Scalar(s) => self.scalar(&s, out),
            ConstValue::Aggregate(img) => self.object(&img, img.ty, 0, depth, out),
        }
    }

    fn scalar(&self, s: &Scalar<'_>, out: &mut String) {
        match *s {
            Scalar::Int { ty, signed, bits } => self.int(ty, signed, &bits, out),
            Scalar::Bool { value, .. } => out.push_str(if value { "true" } else { "false" }),
            Scalar::Float { kind, bits, .. } => format_float(kind, bits, out),
            Scalar::Null { .. } => out.push_str("nullptr"),
            Scalar::Pointer { ty, base, offset } => self.pointer(ty, base, offset, out),
            Scalar::MemberPtr { member, .. } => {
                out.push('&');
                self.env.decl_name(member, out);
            }
            Scalar::Reflection { id, .. } => self.env.reflection(id, out),
        }
    }

    fn int(&self, ty: QualType, signed: bool, bits: &IntBits<'_>, out: &mut String) {
        if !self.template() {
            bits.write_decimal(signed, out);
            return;
        }
        let style = match self.env.shape(ty) {
            Shape::Int { style, .. } => style,
            _ => IntStyle::Plain { suffix: "" },
        };
        match style {
            IntStyle::Plain { suffix } => {
                bits.write_decimal(signed, out);
                out.push_str(suffix);
            }
            IntStyle::Char(enc) => {
                out.push_str(enc.prefix());
                out.push('\'');
                let u = bits.to_u128().map_or(0, |v| v as u32);
                push_unit(u, '\'', out);
                out.push('\'');
            }
            IntStyle::Enum if self.env.enumerator(ty, bits, out) => {}
            IntStyle::Enum | IntStyle::Cast => {
                out.push('(');
                self.env.type_name(ty, out);
                out.push(')');
                bits.write_decimal(signed, out);
            }
        }
    }

    fn pointer(&self, ty: QualType, base: PtrBase, offset: u64, out: &mut String) {
        let (pointee, reference) = match self.env.shape(ty) {
            Shape::Pointer { pointee, reference } => (pointee, reference),
            _ => (None, false),
        };
        let amp = !(reference && self.template());
        match base {
            PtrBase::StringLit(lit) => {
                let unit = u64::from(self.strings.encoding(lit).unit_bytes());
                if offset == 0 {
                    self.strings.write_literal(lit, out);
                } else {
                    out.push('&');
                    self.strings.write_literal(lit, out);
                    if offset.is_multiple_of(unit) {
                        let _ = write!(out, "[{}]", offset / unit);
                    } else {
                        let _ = write!(out, "+{offset}");
                    }
                }
            }
            PtrBase::Function(f) => {
                if amp {
                    out.push('&');
                }
                self.env.decl_name(f, out);
            }
            PtrBase::TypeInfo(t) => {
                if amp {
                    out.push('&');
                }
                out.push_str("typeid(");
                self.env.type_name(t, out);
                out.push(')');
                if offset != 0 {
                    let _ = write!(out, "+{offset}");
                }
            }
            PtrBase::Decl(d) | PtrBase::Temporary(d) => {
                if amp {
                    out.push('&');
                }
                self.env.decl_name(d, out);
                self.designator(self.env.decl_type(d), offset, pointee, out);
            }
            PtrBase::Const(c) => {
                if amp {
                    out.push('&');
                }
                let _ = write!(out, "#c{}", c.index());
                self.designator(Some(self.store.ty(c)), offset, pointee, out);
            }
        }
    }

    /// Appends the designator (`[2].x`) of byte `offset` in an object of
    /// type `ty`, or `+offset` if it cannot be recovered.
    fn designator(
        &self,
        ty: Option<QualType>,
        offset: u64,
        pointee: Option<QualType>,
        out: &mut String,
    ) {
        let mut d = String::new();
        if let Some(ty) = ty
            && self.walk(ty, offset, pointee, &mut d)
        {
            out.push_str(&d);
            return;
        }
        if offset != 0 {
            let _ = write!(out, "+{offset}");
        }
    }

    fn walk(
        &self,
        mut ty: QualType,
        mut offset: u64,
        pointee: Option<QualType>,
        d: &mut String,
    ) -> bool {
        let same = |a: QualType, b: QualType| a.without_quals() == b.without_quals();
        // Bounded by the nesting depth of the type; 4096 guards a cyclic
        // (buggy) environment.
        for _ in 0..4096 {
            if offset == 0 && pointee.is_none_or(|p| same(p, ty)) {
                return true;
            }
            match self.env.shape(ty) {
                Shape::Array { elem, stride, len } if stride != 0 => {
                    let i = offset / stride;
                    if i > len || (i == len && !offset.is_multiple_of(stride)) {
                        return false;
                    }
                    let _ = write!(d, "[{i}]");
                    if i == len {
                        return true; // one past the end
                    }
                    offset -= i * stride;
                    ty = elem;
                }
                Shape::Record { members, .. } => {
                    let hit = members.iter().find(|m| {
                        let at = m.offset_bits / 8;
                        m.bit_width.is_none()
                            && m.offset_bits % 8 == 0
                            && at <= offset
                            && (offset < at + m.size || (offset == at && m.size == 0))
                    });
                    let Some(m) = hit else { return false };
                    if !m.name.is_empty() {
                        d.push('.');
                        d.push_str(&m.name);
                    }
                    offset -= m.offset_bits / 8;
                    ty = m.ty;
                }
                _ => return offset == 0,
            }
        }
        false
    }

    /// Prints the object of type `ty` at byte `at` of `img`.
    fn object(&self, img: &ByteImage<'_>, ty: QualType, at: u64, depth: u32, out: &mut String) {
        if let Ok(i) = img.relocs.binary_search_by_key(&at, |r| u64::from(r.at)) {
            // A pointer-like subobject (or an aggregate starting with one).
            if !matches!(self.env.shape(ty), Shape::Array { .. } | Shape::Record { .. }) {
                self.value(img.relocs[i].value, depth, out);
                return;
            }
        }
        match self.env.shape(ty) {
            Shape::Unknown => {
                if depth == 0 && at == 0 && ty == img.ty {
                    out.push_str("{...}");
                } else {
                    out.push('?');
                }
            }
            Shape::Bool => {
                out.push_str(if read_le(img.bytes, at, 1) != 0 { "true" } else { "false" });
            }
            Shape::Int { width, signed, .. } => {
                let width = width.clamp(1, IntBits::MAX_WIDTH);
                let w = read_bits(img.bytes, at * 8, width);
                self.image_int(ty, signed, width, &w, out);
            }
            Shape::Float(kind) => {
                format_float(kind, read_le(img.bytes, at, kind.value_bytes()), out);
            }
            Shape::NullPtr => out.push_str("nullptr"),
            Shape::Pointer { .. } | Shape::MemberPointer | Shape::Reflection => out.push('?'),
            Shape::Array { elem, stride, len } => {
                if depth >= self.opts.max_depth {
                    out.push_str("{...}");
                    return;
                }
                out.push('{');
                let shown = len.min(u64::from(self.opts.max_elems));
                for i in 0..shown {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    self.object(img, elem, at + i * stride, depth + 1, out);
                }
                if shown < len {
                    out.push_str(if shown > 0 { ", ..." } else { "..." });
                }
                out.push('}');
            }
            Shape::Record { members, union } => {
                if self.template() {
                    self.env.type_name(ty, out);
                }
                if depth >= self.opts.max_depth {
                    out.push_str("{...}");
                    return;
                }
                if union {
                    let active = img
                        .unions
                        .iter()
                        .filter(|u| u64::from(u.at) == at)
                        .find_map(|u| members.iter().find(|m| m.field == Some(u.field)));
                    out.push('{');
                    if let Some(m) = active {
                        let _ = write!(out, ".{} = ", m.name);
                        self.member(img, m, at, depth, out);
                    }
                    out.push('}');
                    return;
                }
                out.push('{');
                let shown =
                    members.iter().filter(|m| !(m.bit_width.is_some() && m.name.is_empty()));
                for (n, m) in shown.enumerate() {
                    if n == self.opts.max_elems as usize {
                        out.push_str(if n > 0 { ", ..." } else { "..." });
                        break;
                    }
                    if n > 0 {
                        out.push_str(", ");
                    }
                    self.member(img, m, at, depth, out);
                }
                out.push('}');
            }
        }
    }

    fn member(&self, img: &ByteImage<'_>, m: &Member, at: u64, depth: u32, out: &mut String) {
        match m.bit_width {
            Some(width) if width > 0 => {
                let signed = matches!(self.env.shape(m.ty), Shape::Int { signed: true, .. });
                let w = read_bits(img.bytes, at * 8 + m.offset_bits, width);
                if matches!(self.env.shape(m.ty), Shape::Bool) {
                    out.push_str(if w[0] != 0 { "true" } else { "false" });
                } else {
                    self.image_int(m.ty, signed, width, &w, out);
                }
            }
            Some(_) => out.push('0'),
            None => self.object(img, m.ty, at + m.offset_bits / 8, depth + 1, out),
        }
    }

    fn image_int(&self, ty: QualType, signed: bool, width: u32, words: &[u64], out: &mut String) {
        match IntBits::from_words(width, words) {
            Some(bits) => self.int(ty, signed, &bits, out),
            None => out.push('?'),
        }
    }
}
