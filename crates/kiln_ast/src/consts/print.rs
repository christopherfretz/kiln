//! Printing constant values (spec/ast.md §9.3): the `value=` fields of the
//! AST dump, diagnostics, and template arguments in type names.
//!
//! The store knows bytes, not types: an aggregate image is decoded through
//! a [`PrintEnv`], which describes each type's [`Shape`] (array element
//! stride, record member offsets, integer width/signedness/style) and names
//! declarations and types. The AST core implements it over `TypeContext`
//! and the record layouts; [`NoEnv`] is a context-free fallback (opaque
//! aggregates, `#d17`-style names).

use std::cell::Cell;
use std::fmt::Write as _;

use super::ConstStore;
use super::float::format_float;
use super::value::{ByteImage, ConstValue, FloatKind, IntBits, PtrBase, Scalar};
use crate::ids::{ConstValueId, DeclId, QualType, ReflectionId};
use crate::strings::{StringEncoding, StringPool, push_char_literal};

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
    /// [`PrintStyle::TemplateArg`]: whether an integer shows its type
    /// (suffix or cast; clang's `IncludeType`). False for the argument of
    /// a known non-`auto` parameter: a specialization's name prints
    /// `A<5, 'x'>`, not `A<5UL, (unsigned char)'x'>`.
    pub include_type: bool,
    /// [`PrintStyle::TemplateArg`] without [`Self::include_type`]: an
    /// enumeration value that names no enumerator still prints as a cast
    /// (`(E)5`, not `5`), so the spelling re-parses as a value of the
    /// enumeration (`display_string_of`; spec/reflection.md §3.9).
    pub enum_cast: bool,
    /// [`PrintStyle::TemplateArg`] as GCC 16.1's diagnostics print a
    /// constant (`dump_expr`; GCC-style `display_string_of`,
    /// spec/reflection.md §3.9): a `char` array as a string literal
    /// (`"ab"`), any other array with its type (`int [2]{1, 2}`), an empty
    /// class as `T()`, plain `char` with octal escapes (`'\000'`), other
    /// character types as integers, floating point in `%e` form with the
    /// type's `max_digits10` digits (`1.5e+0`), a null pointer as `0`.
    pub gcc: bool,
}

impl PrintOptions {
    /// The default limits: depth 10, 32 elements.
    pub const DEFAULT: PrintOptions = PrintOptions {
        style: PrintStyle::Dump,
        max_depth: 10,
        max_elems: 32,
        include_type: true,
        enum_cast: false,
        gcc: false,
    };

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
    /// `signed char` / `unsigned char`: `(unsigned char)'a'`.
    SmallChar,
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
    /// Whether `a` and `b` are the same type, ignoring top-level
    /// qualifiers, including those of array elements.
    fn same_unqualified(&self, a: QualType, b: QualType) -> bool {
        a.without_quals() == b.without_quals()
    }
    /// Appends a type's name.
    fn type_name(&self, ty: QualType, out: &mut String) {
        let _ = write!(out, "#t{}", ty.ty().index());
    }
    /// Appends a template argument's integral type as its cast prints
    /// it (clang `printIntegral` with `IncludeType`): the canonical type,
    /// fully qualified (`(n::E)5`).
    fn arg_type_name(&self, ty: QualType, out: &mut String) {
        self.type_name(ty, out);
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
    /// Set when a part printed as a placeholder rather than a source
    /// spelling ([`ConstStore::print_exact`]).
    lossy: Cell<bool>,
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
        Printer { store: self, strings, env, opts, lossy: Cell::new(false) }.value(id, 0, out);
    }

    /// [`ConstStore::print`], returning whether the text is exact: false
    /// when some part printed as a placeholder (`?`, `{...}`, a `...`
    /// truncation, `#c3`, a `+N` offset no designator recovers, a
    /// temporary), so it is no source spelling of the value.
    pub fn print_exact(
        &self,
        id: ConstValueId,
        strings: &StringPool,
        env: &dyn PrintEnv,
        opts: &PrintOptions,
        out: &mut String,
    ) -> bool {
        let p = Printer { store: self, strings, env, opts, lossy: Cell::new(false) };
        p.value(id, 0, out);
        !p.lossy.get()
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

    /// Notes that a placeholder was printed ([`ConstStore::print_exact`]).
    fn lost(&self) {
        self.lossy.set(true);
    }

    /// A floating-point value ([`format_float`]); an infinity or NaN has
    /// no literal, so is no exact spelling.
    fn float(&self, kind: FloatKind, bits: u128, out: &mut String) {
        if self.opts.gcc && gcc_float(kind, bits, out) {
            return;
        }
        let at = out.len();
        format_float(kind, bits, out);
        if out[at..].ends_with("inf") || out[at..].ends_with("nan") {
            self.lost();
        }
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
            Scalar::Float { kind, bits, .. } => self.float(kind, bits, out),
            Scalar::Null { ty, .. } if self.opts.gcc && self.env.shape(ty) != Shape::NullPtr => {
                out.push('0');
            }
            Scalar::Null { .. } => out.push_str("nullptr"),
            Scalar::Pointer { ty, base, offset, designated } => {
                self.pointer(ty, base, offset, designated, out);
            }
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
        let typed = self.opts.include_type;
        match style {
            IntStyle::Plain { suffix } => {
                bits.write_decimal(signed, out);
                if typed {
                    out.push_str(suffix);
                }
            }
            IntStyle::SmallChar | IntStyle::Char(_)
                if self.opts.gcc && style != IntStyle::Char(StringEncoding::Ordinary) =>
            {
                bits.write_decimal(signed, out);
            }
            IntStyle::Char(_) if self.opts.gcc => {
                out.push('\'');
                push_gcc_char(bits.to_u128().map_or(0, |v| v as u32 & 0xff), out);
                out.push('\'');
            }
            IntStyle::SmallChar => {
                if typed {
                    out.push_str(if signed { "(signed char)" } else { "(unsigned char)" });
                }
                out.push('\'');
                let u = bits.to_u128().map_or(0, |v| v as u32 & 0xff);
                push_char_literal(u, true, out);
                out.push('\'');
            }
            IntStyle::Char(enc) => {
                out.push_str(enc.prefix());
                out.push('\'');
                let u = bits.to_u128().map_or(0, |v| v as u32);
                push_char_literal(u, enc == StringEncoding::Ordinary, out);
                out.push('\'');
            }
            IntStyle::Enum if self.env.enumerator(ty, bits, out) => {}
            IntStyle::Enum if !typed && !self.opts.enum_cast => bits.write_decimal(signed, out),
            IntStyle::Cast if !typed => bits.write_decimal(signed, out),
            IntStyle::Enum | IntStyle::Cast => {
                out.push('(');
                self.env.arg_type_name(ty, out);
                out.push(')');
                bits.write_decimal(signed, out);
            }
        }
    }

    /// A `void *` prints the designator of the object it `designated`:
    /// the whole object one past its end as `&x + 1`, and (as a template
    /// argument) an array variable's address as the array (`arr`; clang
    /// keys the argument by the declaration, ignoring the decay).
    fn pointer(
        &self,
        ty: QualType,
        base: PtrBase,
        offset: u64,
        designated: Option<QualType>,
        out: &mut String,
    ) {
        let (pointee, reference) = match self.env.shape(ty) {
            Shape::Pointer { pointee, reference } => (pointee, reference),
            _ => (None, false),
        };
        let pointee = designated.or(pointee);
        let mut amp = !(reference && self.template());
        if let (PtrBase::Decl(d), Some(t)) = (base, designated)
            && let Some(whole) = self.env.decl_type(d)
            && self.env.same_unqualified(whole, t)
        {
            if offset != 0 {
                out.push('&');
                self.env.decl_name(d, out);
                out.push_str(" + 1");
                return;
            }
            if self.template() && matches!(self.env.shape(whole), Shape::Array { .. }) {
                amp = false;
            }
        }
        match base {
            PtrBase::StringLit { lit, .. } => {
                let unit = u64::from(self.strings.encoding(lit).unit_bytes());
                if offset == 0 {
                    self.strings.write_literal(lit, out);
                } else {
                    out.push('&');
                    self.strings.write_literal(lit, out);
                    if offset.is_multiple_of(unit) {
                        let _ = write!(out, "[{}]", offset / unit);
                    } else {
                        self.lost();
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
                    self.lost();
                    let _ = write!(out, "+{offset}");
                }
            }
            // clang prints an `AddrLabelExpr` base as the expression.
            PtrBase::Label { label, .. } => {
                self.lost();
                out.push_str("&&");
                self.env.decl_name(label, out);
                if offset != 0 {
                    let _ = write!(out, " + {offset}");
                }
            }
            PtrBase::Decl(d) | PtrBase::Temporary(d) => {
                if matches!(base, PtrBase::Temporary(_)) {
                    self.lost();
                }
                if amp {
                    out.push('&');
                }
                self.env.decl_name(d, out);
                self.designator(self.env.decl_type(d), offset, pointee, out);
            }
            PtrBase::Const(c) => {
                self.lost();
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
            self.lost();
            let _ = write!(out, "+{offset}");
        }
    }

    fn walk(&self, ty: QualType, offset: u64, pointee: Option<QualType>, d: &mut String) -> bool {
        self.walk_at(ty, offset, pointee, d, 0)
    }

    /// [`Self::walk`] at nesting `depth`. Several members can share an
    /// offset (an empty base, a `[[no_unique_address]]` member): each is
    /// tried until one holds `pointee`.
    fn walk_at(
        &self,
        mut ty: QualType,
        mut offset: u64,
        pointee: Option<QualType>,
        d: &mut String,
        depth: u32,
    ) -> bool {
        // Bounded by the nesting depth of the type; 4096 guards a cyclic
        // (buggy) environment.
        for _ in depth..4096 {
            if offset == 0 && pointee.is_none_or(|p| self.env.same_unqualified(p, ty)) {
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
                    let hits = members.iter().filter(|m| {
                        let at = m.offset_bits / 8;
                        m.bit_width.is_none()
                            && m.offset_bits % 8 == 0
                            && at <= offset
                            && (offset < at + m.size || (offset == at && m.size == 0))
                    });
                    let len = d.len();
                    for m in hits {
                        if !m.name.is_empty() {
                            d.push('.');
                            d.push_str(&m.name);
                        }
                        if self.walk_at(m.ty, offset - m.offset_bits / 8, pointee, d, depth + 1) {
                            return true;
                        }
                        d.truncate(len);
                    }
                    return false;
                }
                _ => return offset == 0,
            }
        }
        false
    }

    /// Prints the object of type `ty` at byte `at` of `img`.
    #[allow(clippy::too_many_lines, reason = "one arm per object kind")]
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
                self.lost();
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
                self.float(kind, read_le(img.bytes, at, kind.value_bytes()), out);
            }
            Shape::NullPtr => out.push_str("nullptr"),
            Shape::Pointer { .. } | Shape::MemberPointer | Shape::Reflection => {
                self.lost();
                out.push('?');
            }
            Shape::Array { elem, stride, len } => {
                if depth >= self.opts.max_depth {
                    self.lost();
                    out.push_str("{...}");
                    return;
                }
                if self.opts.gcc {
                    let plain_char = matches!(
                        self.env.shape(elem),
                        Shape::Int { style: IntStyle::Char(StringEncoding::Ordinary), .. }
                    );
                    if plain_char && stride == 1 {
                        // GCC keeps a `char` array constant as a string
                        // literal, its trailing NULs not printed.
                        let n = usize::try_from(len).unwrap_or(0);
                        let start = usize::try_from(at).unwrap_or(0);
                        let bytes = img.bytes.get(start..start + n).unwrap_or(&[]);
                        let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
                        out.push('"');
                        for &b in &bytes[..end] {
                            push_gcc_char(u32::from(b), out);
                        }
                        out.push('"');
                        return;
                    }
                    self.env.type_name(ty, out);
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
                    self.lost();
                    out.push_str(if shown > 0 { ", ..." } else { "..." });
                }
                out.push('}');
            }
            Shape::Record { members, union } => {
                if self.template() {
                    self.env.type_name(ty, out);
                }
                if depth >= self.opts.max_depth {
                    self.lost();
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
                let shown =
                    members.iter().filter(|m| !(m.bit_width.is_some() && m.name.is_empty()));
                if self.opts.gcc && shown.clone().next().is_none() {
                    // GCC prints an empty CONSTRUCTOR as `T()`.
                    out.push_str("()");
                    return;
                }
                out.push('{');
                for (n, m) in shown.enumerate() {
                    if n == self.opts.max_elems as usize {
                        self.lost();
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
        if let Some(bits) = IntBits::from_words(width, words) {
            self.int(ty, signed, &bits, out);
        } else {
            self.lost();
            out.push('?');
        }
    }
}

/// GCC's `pp_c_char`: a printable character as itself (`\\`, `\'` and
/// `\"` escaped), any other as a three-digit octal escape (`\012`).
fn push_gcc_char(c: u32, out: &mut String) {
    match char::from_u32(c) {
        Some('\\') => out.push_str("\\\\"),
        Some('\'') => out.push_str("\\'"),
        Some('"') => out.push_str("\\\""),
        Some(ch) if (0x20..0x7f).contains(&c) => out.push(ch),
        _ => {
            let _ = write!(out, "\\{c:03o}");
        }
    }
}

/// GCC's `pp_c_floating_constant` for `float` and `double`
/// (`real_to_decimal` with the type's `max_digits10` significant digits,
/// trailing zeros cropped, one kept after the point; `1.5e+0`,
/// `1.0000000000000001e-1`). False for other formats and for an infinity
/// or NaN (the caller prints those).
fn gcc_float(kind: FloatKind, bits: u128, out: &mut String) -> bool {
    let (v, digits) = match kind {
        FloatKind::Single => (f64::from(f32::from_bits(bits as u32)), 9),
        FloatKind::Double => (f64::from_bits(bits as u64), 17),
        _ => return false,
    };
    if !v.is_finite() {
        return false;
    }
    let s = format!("{:.*e}", digits - 1, v);
    let Some((mant, exp)) = s.split_once('e') else { return false };
    let mut mant = mant.to_owned();
    if mant.contains('.') {
        while mant.ends_with('0') {
            mant.pop();
        }
        if mant.ends_with('.') {
            mant.push('0');
        }
    } else {
        mant.push_str(".0");
    }
    let exp: i32 = exp.parse().unwrap_or(0);
    let _ = write!(out, "{mant}e{}{}", if exp < 0 { '-' } else { '+' }, exp.unsigned_abs());
    true
}
