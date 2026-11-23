//! The value of an evaluation session's run as the client reads it
//! (spec/eval-session.md §4.1): a tree owned by the report, decoded from
//! the frozen constant the Oven's freezer made of the run's result.
//!
//! The tree holds no id numbered by history: names are strings (as
//! `std::meta::display_string_of` spells them), `decl` is given only for
//! an entity that existed when the session was opened, and objects
//! without a name are entries of a constants table, indexed in
//! first-reached order. Decoding interns nothing: every table it uses is
//! local to one read.
//!
//! None of the enums is `#[non_exhaustive]`: a client matches them
//! exhaustively, so a new variant is a breaking change by design.

use std::rc::Rc;
use std::sync::Arc;

pub use kiln_ast::consts::FloatKind;
use kiln_ast::consts::{ByteImage, ConstValue, IntBits, PrintEnv, PtrBase, Scalar, Shape};
use kiln_ast::decl::DeclKind;
use kiln_ast::dump::AstNames;
use kiln_ast::reflect::Reflectee;
use kiln_ast::strings::StringEncoding;
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, QualType, ReflectionId, StringLitId};
use kiln_oven::ResultReadError;
use kiln_source::fx::{FxHashMap, FxHashSet};

use crate::Sema;

/// A run's value (spec/eval-session.md §4.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunValue {
    /// A `void` entry.
    Void,
    /// A `bool`.
    Bool(bool),
    /// A signed integral value (`_BitInt(N)` with N <= 128 included).
    Signed(i128),
    /// An unsigned integral value (the character types by their
    /// signedness).
    Unsigned(u128),
    /// A floating-point value: its format and its exact encoding
    /// (zero-extended), so `-0.0` and a NaN's sign and payload survive.
    Float {
        /// The format.
        kind: FloatKind,
        /// The encoding.
        bits: u128,
    },
    /// An enumeration value.
    Enum(Box<EnumValue>),
    /// A null pointer, a null pointer to member, or `nullptr`.
    Null,
    /// A non-null pointer or a reference.
    Pointer(Box<PointerValue>),
    /// A non-null pointer to member: the member it designates.
    MemberPointer(Box<Entity>),
    /// A `std::meta::info`.
    Info(Box<InfoValue>),
    /// An array of a character type: every element (NULs included) as a
    /// code unit, little-endian at the unit's width.
    Text {
        /// The element type.
        unit: CharUnit,
        /// The code units' bytes.
        data: Box<[u8]>,
    },
    /// Any other array (`_Complex T` as `[real, imag]`).
    Array(Box<[RunValue]>),
    /// A class or union.
    Record(Box<RecordValue>),
}

const _: () = assert!(size_of::<RunValue>() == 32);

impl RunValue {
    /// The field named `name` of a record (a member by its name, a base by
    /// its type's spelling); the first one when names repeat.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&RunValue> {
        match self {
            RunValue::Record(r) => r.fields.iter().find(|f| &*f.name == name).map(|f| &f.value),
            _ => None,
        }
    }

    /// An integral or enumeration value as `i128`, if it fits.
    #[must_use]
    pub fn as_i128(&self) -> Option<i128> {
        match self {
            RunValue::Bool(b) => Some(i128::from(*b)),
            RunValue::Signed(v) => Some(*v),
            RunValue::Unsigned(v) => i128::try_from(*v).ok(),
            RunValue::Enum(e) => match e.value {
                IntValue::Signed(v) => Some(v),
                IntValue::Unsigned(v) => i128::try_from(v).ok(),
            },
            _ => None,
        }
    }

    /// A `Text`'s code units, widened.
    #[must_use]
    pub fn text_units(&self) -> Option<Vec<u32>> {
        let RunValue::Text { unit, data } = self else { return None };
        let w = unit.width(4);
        Some(
            data.chunks(w)
                .map(|c| c.iter().rev().fold(0u32, |acc, &b| (acc << 8) | u32::from(b)))
                .collect(),
        )
    }
}

/// An entity by its spelling, and its declaration when it existed at
/// open (spec/eval-session.md §4.1: `decl` is `Some` if and only if the
/// entity existed when the session was opened).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entity {
    /// As `display_string_of` spells it (a spelling, not an identity).
    pub display: Arc<str>,
    /// The first declaration, if the entity existed at open.
    pub decl: Option<DeclId>,
}

/// An integer by the signedness of its type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntValue {
    /// Of a signed type.
    Signed(i128),
    /// Of an unsigned type (or `bool`).
    Unsigned(u128),
}

/// An enumeration value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumValue {
    /// The enumeration.
    pub ty: Entity,
    /// The value, by the underlying type's signedness.
    pub value: IntValue,
    /// The first enumerator in declaration order with this value.
    pub enumerator: Option<Arc<str>>,
}

/// A class or union value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordValue {
    /// The class.
    pub ty: Entity,
    /// A union (`fields` holds its active member, if any).
    pub union: bool,
    /// The direct non-virtual bases, then the non-static data members, in
    /// declaration order; unnamed bit-fields omitted.
    pub fields: Box<[Field]>,
}

/// A base or member of a record value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    /// What it is.
    pub kind: FieldKind,
    /// A member's name, a base's type, empty for an anonymous member.
    pub name: Arc<str>,
    /// Its value.
    pub value: RunValue,
}

/// What a [`Field`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    /// A direct base class subobject.
    Base,
    /// A named non-static data member.
    Member,
    /// An unnamed member (an anonymous union or struct, a capture).
    Anonymous,
}

/// A non-null pointer or reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PointerValue {
    /// The complete object it points into.
    pub base: PointerBase,
    /// The designator from the complete object.
    pub path: Box<[PathStep]>,
    /// One past the end of the object the path designates (not an array
    /// element: `&a[N]` is `Index(N)`).
    pub past_end: bool,
}

/// The complete object a pointer points into.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PointerBase {
    /// A variable with static storage duration.
    Variable(Entity),
    /// A function.
    Function(Entity),
    /// A lifetime-extended temporary, by the variable that extends it.
    Temporary(Entity),
    /// A `std::type_info` object, by its type's spelling.
    TypeInfo(Arc<str>),
    /// An object without a name: its index in the constants table.
    Constant(u32),
}

/// One step of a pointer's designator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathStep {
    /// A base class subobject, by its type's spelling.
    Base(Arc<str>),
    /// A named member.
    Member(Arc<str>),
    /// An unnamed member: its position among its record's bases and
    /// members, unnamed bit-fields omitted.
    Anonymous(u32),
    /// An array element (the array's length: one past the last).
    Index(u64),
}

/// A `std::meta::info` value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoValue {
    /// What it reflects.
    pub kind: InfoKind,
    /// Its `display_string_of` text.
    pub display: Arc<str>,
    /// The declaration it reflects, if that existed at open.
    pub decl: Option<DeclId>,
    /// A reflected value's value, or a `Pointer` to a reflected object.
    pub value: Option<RunValue>,
}

/// What a reflection reflects (the kinds of `Reflectee`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfoKind {
    /// The null reflection.
    Null,
    /// A type.
    Type,
    /// A variable, function, data member, enumerator, structured binding.
    Declaration,
    /// A namespace.
    Namespace,
    /// A namespace alias.
    NamespaceAlias,
    /// A template.
    Template,
    /// A value (`reflect_constant`).
    Value,
    /// An object (`reflect_object`).
    Object,
    /// A base specifier.
    BaseSpecifier,
    /// A data member description.
    DataMemberSpec,
    /// An annotation.
    Annotation,
    /// A function parameter.
    Parameter,
}

/// The element type of a [`RunValue::Text`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharUnit {
    /// `char`.
    Char,
    /// `signed char`.
    SignedChar,
    /// `unsigned char`.
    UnsignedChar,
    /// `char8_t`.
    Char8,
    /// `char16_t`.
    Char16,
    /// `char32_t`.
    Char32,
    /// `wchar_t`.
    WChar,
}

impl CharUnit {
    /// Bytes per unit (`wchar_t`'s is the target's, given).
    #[must_use]
    pub fn width(self, wchar: usize) -> usize {
        match self {
            CharUnit::Char | CharUnit::SignedChar | CharUnit::UnsignedChar | CharUnit::Char8 => 1,
            CharUnit::Char16 => 2,
            CharUnit::Char32 => 4,
            CharUnit::WChar => wchar,
        }
    }
}

/// A value with its constants table ([`crate::session::EvalSession::variable_value`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueTree {
    /// The value.
    pub root: RunValue,
    /// The objects without a name its pointers reach, by index.
    pub constants: Vec<RunValue>,
}

/// An object without a name, as the constant model tells objects apart.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum CKey {
    /// A template parameter object.
    Decl(DeclId),
    /// A string literal object: its contents and the expression whose
    /// evaluation created it.
    Str(StringLitId, ExprId),
    /// A frozen object of the store (`define_static_*`,
    /// `reflect_constant`).
    Const(ConstValueId),
}

type R<T> = Result<T, ResultReadError>;

fn unreadable<T>(what: &str) -> R<T> {
    Err(ResultReadError::Unreadable(format!("{what} cannot be read by the host")))
}

/// Decodes frozen value `v` of `sema`'s store (spec/eval-session.md
/// §4.1): `open` is the declaration count when the session was opened,
/// `cap` the tree's size limit.
pub(crate) fn decode(sema: &Sema<'_, '_>, v: ConstValueId, open: usize, cap: u64) -> R<ValueTree> {
    let mut d = Decoder {
        sema,
        ast: &sema.ast,
        names: AstNames::new(&sema.ast),
        open,
        cap,
        used: 0,
        strings: FxHashSet::default(),
        decl_display: FxHashMap::default(),
        type_display: FxHashMap::default(),
        idents: FxHashMap::default(),
        shapes: FxHashMap::default(),
        keys: FxHashMap::default(),
        queue: Vec::new(),
    };
    d.charge(size_of::<RunValue>() as u64)?;
    let root = d.value(v)?;
    let mut constants = Vec::new();
    let mut i = 0;
    while i < d.queue.len() {
        let c = d.constant_value(d.queue[i])?;
        constants.push(c);
        i += 1;
    }
    Ok(ValueTree { root, constants })
}

/// Whether `q` is a result type (spec/eval-session.md §3), `top` for an
/// entry's return type (which may also be `void`; never a reference).
pub(crate) fn result_type_ok(sema: &Sema<'_, '_>, q: QualType, top: bool) -> bool {
    let types = &sema.ast.types;
    if top {
        if types.is_void(q) {
            return true;
        }
        if types.is_reference(q) || !sema.is_literal_type(q) {
            return false;
        }
    }
    type_ok(&sema.ast, q, 0)
}

fn type_ok(ast: &Ast, q: QualType, depth: u32) -> bool {
    if depth > 256 {
        return false;
    }
    let types = &ast.types;
    match types.canonical_view(q) {
        // As the constant printer's shapes: `bool`, the character and
        // integer types, the floating-point types, `nullptr_t`, `info`.
        TypeView::Builtin(b) => matches!(b as u8, 2..=28),
        TypeView::BitInt { width, .. } => width <= 128,
        TypeView::Pointer(_)
        | TypeView::LValueRef(_)
        | TypeView::RValueRef(_)
        | TypeView::MemberPointer { .. } => true,
        TypeView::ConstantArray(a) => type_ok(ast, a.elem, depth + 1),
        TypeView::Complex(e) => type_ok(ast, e, depth + 1),
        TypeView::Enum(d) => ast.decls.enumeration(d).is_some_and(|e| e.int_type.is_some()),
        TypeView::Record(r) => {
            let Some(def) = ast.definition(r) else { return false };
            for b in ast.decls.bases(def, &ast.lists) {
                if b.is_virtual || !type_ok(ast, b.ty, depth + 1) {
                    return false;
                }
            }
            ast.members(def)
                .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
                .all(|f| ast.decls.get(f).ty().is_some_and(|t| type_ok(ast, t, depth + 1)))
        }
        _ => false,
    }
}

struct Decoder<'s, 'a, 'sm> {
    sema: &'s Sema<'a, 'sm>,
    ast: &'s Ast,
    names: AstNames<'s>,
    open: usize,
    cap: u64,
    used: u64,
    /// Every distinct string of the tree (charged once each).
    strings: FxHashSet<Arc<str>>,
    decl_display: FxHashMap<DeclId, Arc<str>>,
    type_display: FxHashMap<QualType, Arc<str>>,
    idents: FxHashMap<DeclId, Arc<str>>,
    shapes: FxHashMap<QualType, Rc<Shape>>,
    /// The constants table's index of each object reached.
    keys: FxHashMap<CKey, u32>,
    queue: Vec<CKey>,
}

impl Decoder<'_, '_, '_> {
    fn charge(&mut self, n: u64) -> R<()> {
        self.used = self.used.saturating_add(n);
        if self.used > self.cap {
            return Err(ResultReadError::TooLarge(self.cap));
        }
        Ok(())
    }

    fn string(&mut self, s: String) -> R<Arc<str>> {
        if let Some(a) = self.strings.get(s.as_str()) {
            return Ok(Arc::clone(a));
        }
        self.charge(s.len() as u64)?;
        let a: Arc<str> = Arc::from(s);
        self.strings.insert(Arc::clone(&a));
        Ok(a)
    }

    /// `d`'s first declaration, if it existed at open.
    fn decl_at_open(&self, d: DeclId) -> Option<DeclId> {
        let f = self.ast.first(d);
        (f.index() as usize <= self.open).then_some(f)
    }

    fn decl_display(&mut self, d: DeclId) -> R<Arc<str>> {
        if let Some(s) = self.decl_display.get(&d) {
            return Ok(Arc::clone(s));
        }
        let s = self.string(self.sema.gcc_decl_string(d))?;
        self.decl_display.insert(d, Arc::clone(&s));
        Ok(s)
    }

    fn type_display(&mut self, t: QualType) -> R<Arc<str>> {
        let t = self.ast.types.canonical(t).without_quals();
        if let Some(s) = self.type_display.get(&t) {
            return Ok(Arc::clone(s));
        }
        let s = self.string(self.sema.gcc_type_string(t))?;
        self.type_display.insert(t, Arc::clone(&s));
        Ok(s)
    }

    /// A member's identifier (empty for an unnamed one).
    fn ident(&mut self, d: DeclId) -> R<Arc<str>> {
        if let Some(s) = self.idents.get(&d) {
            return Ok(Arc::clone(s));
        }
        let text = self
            .ast
            .decls
            .get(d)
            .name()
            .as_ident()
            .map(|n| self.ast.symbols().as_str(n).to_owned())
            .unwrap_or_default();
        let s = self.string(text)?;
        self.idents.insert(d, Arc::clone(&s));
        Ok(s)
    }

    fn is_unnamed(&self, d: DeclId) -> bool {
        self.ast
            .decls
            .get(d)
            .name()
            .as_ident()
            .is_none_or(|n| self.ast.symbols().as_str(n).is_empty())
    }

    fn entity(&mut self, d: DeclId) -> R<Entity> {
        Ok(Entity { display: self.decl_display(d)?, decl: self.decl_at_open(d) })
    }

    fn type_entity(&mut self, t: QualType) -> R<Entity> {
        let d = self.ast.types.as_record(t).or_else(|| self.ast.types.as_enum(t));
        Ok(Entity { display: self.type_display(t)?, decl: d.and_then(|d| self.decl_at_open(d)) })
    }

    fn shape(&mut self, t: QualType) -> Rc<Shape> {
        let t = self.ast.types.canonical(t).without_quals();
        if let Some(s) = self.shapes.get(&t) {
            return Rc::clone(s);
        }
        let s = Rc::new(self.names.shape(t));
        self.shapes.insert(t, Rc::clone(&s));
        s
    }

    fn same(&self, a: QualType, b: QualType) -> bool {
        self.names.same_unqualified(a, b)
    }

    // ---- values -------------------------------------------------------

    /// The value of store value `v`.
    fn value(&mut self, v: ConstValueId) -> R<RunValue> {
        let ast = self.ast;
        match ast.consts.get(v) {
            ConstValue::Scalar(s) => self.scalar(&s),
            ConstValue::Aggregate(img) => self.object(&img, img.ty, 0),
        }
    }

    fn scalar(&mut self, s: &Scalar<'_>) -> R<RunValue> {
        Ok(match *s {
            Scalar::Int { ty, signed, ref bits } => {
                let raw = raw_bits(bits);
                self.int(ty, raw, bits.width(), signed)?
            }
            Scalar::Bool { ty, value } => self.int(ty, u128::from(value), 1, false)?,
            Scalar::Float { kind, bits, .. } => RunValue::Float { kind, bits },
            Scalar::Null { .. } => RunValue::Null,
            Scalar::Pointer { ty, base, offset, designated, past } => {
                self.pointer(ty, base, offset, designated, past)?
            }
            Scalar::MemberPtr { member, .. } => {
                self.charge(size_of::<Entity>() as u64)?;
                RunValue::MemberPointer(Box::new(self.entity(member)?))
            }
            Scalar::Reflection { id, .. } => self.info(id)?,
        })
    }

    /// An integer, `bool` or enumeration of type `ty` with value bits
    /// `raw` (`width` bits).
    fn int(&mut self, ty: QualType, raw: u128, width: u32, signed: bool) -> R<RunValue> {
        let value = if signed {
            IntValue::Signed(sign_extend(raw, width))
        } else {
            IntValue::Unsigned(mask(raw, width))
        };
        if let Some(e) = self.ast.types.as_enum(ty) {
            self.charge(size_of::<EnumValue>() as u64)?;
            let enumerator = self.enumerator(e, mask(raw, width))?;
            let ty = self.type_entity(ty)?;
            return Ok(RunValue::Enum(Box::new(EnumValue { ty, value, enumerator })));
        }
        if self.ast.types.canonical(ty).without_quals() == QualType::BOOL {
            return Ok(RunValue::Bool(raw & 1 != 0));
        }
        Ok(match value {
            IntValue::Signed(v) => RunValue::Signed(v),
            IntValue::Unsigned(v) => RunValue::Unsigned(v),
        })
    }

    /// The first enumerator of enumeration `e` with value bits `raw`.
    fn enumerator(&mut self, e: DeclId, raw: u128) -> R<Option<Arc<str>>> {
        let ast = self.ast;
        let e = ast.definition(e).unwrap_or(e);
        for m in ast.members(e) {
            let Some(v) = ast.decls.enumerator(m).and_then(|d| d.value) else { continue };
            let bits = match ast.consts.scalar(v) {
                Some(Scalar::Int { bits, .. }) => mask(raw_bits(&bits), bits.width()),
                Some(Scalar::Bool { value, .. }) => u128::from(value),
                _ => continue,
            };
            if bits == raw {
                return Ok(Some(self.ident(m)?));
            }
        }
        Ok(None)
    }

    fn info(&mut self, id: ReflectionId) -> R<RunValue> {
        self.charge(size_of::<InfoValue>() as u64)?;
        let ast = self.ast;
        let r = ast.reflections.get(id);
        let kind = match r {
            Reflectee::Null => InfoKind::Null,
            Reflectee::Type(_) => InfoKind::Type,
            Reflectee::Decl(_) => InfoKind::Declaration,
            Reflectee::Namespace(_) => InfoKind::Namespace,
            Reflectee::NamespaceAlias(_) => InfoKind::NamespaceAlias,
            Reflectee::Template(_) => InfoKind::Template,
            Reflectee::Value(_) => InfoKind::Value,
            Reflectee::Object(_) => InfoKind::Object,
            Reflectee::BaseSpec(..) => InfoKind::BaseSpecifier,
            Reflectee::DataMemberSpec(_) => InfoKind::DataMemberSpec,
            Reflectee::Annotation(_) => InfoKind::Annotation,
            Reflectee::FnParam(..) => InfoKind::Parameter,
        };
        let decl = match r {
            Reflectee::Decl(d) | Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => {
                self.decl_at_open(d)
            }
            Reflectee::Template(n) => ast.types.template_decl(n).and_then(|d| self.decl_at_open(d)),
            Reflectee::Type(t) => ast
                .types
                .as_record(t)
                .or_else(|| ast.types.as_enum(t))
                .and_then(|d| self.decl_at_open(d)),
            _ => None,
        };
        let display = self.string(self.sema.display_string(id))?;
        let value = match r {
            Reflectee::Value(v) | Reflectee::Object(v) => Some(self.value(v)?),
            _ => None,
        };
        Ok(RunValue::Info(Box::new(InfoValue { kind, display, decl, value })))
    }

    /// The object of type `ty` at byte `at` of image `img`.
    fn object(&mut self, img: &ByteImage<'_>, ty: QualType, at: u64) -> R<RunValue> {
        let shape = self.shape(ty);
        // A pointer-like subobject (an aggregate may start with one).
        if !matches!(&*shape, Shape::Array { .. } | Shape::Record { .. })
            && let Ok(i) = img.relocs.binary_search_by_key(&at, |r| u64::from(r.at))
        {
            return self.value(img.relocs[i].value);
        }
        match &*shape {
            Shape::Bool => {
                let b = img.bytes.get(at as usize).copied().unwrap_or(0);
                self.int(ty, u128::from(b & 1), 1, false)
            }
            &Shape::Int { width, signed, .. } => {
                let n = width.div_ceil(8);
                let raw = read_le(img.bytes, at, n);
                self.int(ty, raw, width, signed)
            }
            &Shape::Float(kind) => {
                Ok(RunValue::Float { kind, bits: read_le(img.bytes, at, kind.value_bytes()) })
            }
            Shape::NullPtr => Ok(RunValue::Null),
            &Shape::Array { elem, stride, len } => {
                if let Some(unit) = char_unit(self.ast, elem) {
                    let n = stride.saturating_mul(len);
                    self.charge(n)?;
                    let a = at as usize;
                    let Some(data) = img.bytes.get(a..a + n as usize) else {
                        return unreadable("a character array");
                    };
                    return Ok(RunValue::Text { unit, data: data.into() });
                }
                self.charge(len.saturating_mul(size_of::<RunValue>() as u64))?;
                let mut out = Vec::with_capacity(len as usize);
                for i in 0..len {
                    out.push(self.object(img, elem, at + i * stride)?);
                }
                Ok(RunValue::Array(out.into_boxed_slice()))
            }
            Shape::Record { members, union } => {
                self.charge(size_of::<RecordValue>() as u64)?;
                let active = if *union {
                    img.unions
                        .iter()
                        .find(|u| {
                            u64::from(u.at) == at
                                && members.iter().any(|m| m.field == Some(u.field))
                        })
                        .map(|u| u.field)
                } else {
                    None
                };
                let mut fields = Vec::new();
                for m in members {
                    if *union && (m.field.is_none() || m.field != active) {
                        continue;
                    }
                    let Some(f) = m.field else {
                        let name = self.type_display(m.ty)?;
                        self.charge(size_of::<Field>() as u64)?;
                        let value = self.object(img, m.ty, at + m.offset_bits / 8)?;
                        fields.push(Field { kind: FieldKind::Base, name, value });
                        continue;
                    };
                    let unnamed = self.is_unnamed(f);
                    if let Some(w) = m.bit_width {
                        if unnamed {
                            continue;
                        }
                        let name = self.ident(f)?;
                        self.charge(size_of::<Field>() as u64)?;
                        let signed = matches!(&*self.shape(m.ty), Shape::Int { signed: true, .. });
                        let raw = read_bits(img.bytes, at * 8 + m.offset_bits, w);
                        let value = self.int(m.ty, raw, w, signed)?;
                        fields.push(Field { kind: FieldKind::Member, name, value });
                        continue;
                    }
                    let (kind, name) = if unnamed {
                        (FieldKind::Anonymous, self.string(String::new())?)
                    } else {
                        (FieldKind::Member, self.ident(f)?)
                    };
                    self.charge(size_of::<Field>() as u64)?;
                    let value = self.object(img, m.ty, at + m.offset_bits / 8)?;
                    fields.push(Field { kind, name, value });
                }
                let ty = self.type_entity(ty)?;
                Ok(RunValue::Record(Box::new(RecordValue {
                    ty,
                    union: *union,
                    fields: fields.into_boxed_slice(),
                })))
            }
            Shape::Pointer { .. } | Shape::MemberPointer | Shape::Reflection => {
                unreadable("a pointer without a relocation")
            }
            Shape::Unknown => unreadable("a value of this type"),
        }
    }

    // ---- pointers -----------------------------------------------------

    fn pointer(
        &mut self,
        ty: QualType,
        base: PtrBase,
        offset: u64,
        designated: Option<QualType>,
        past: bool,
    ) -> R<RunValue> {
        self.charge(size_of::<PointerValue>() as u64)?;
        let ast = self.ast;
        let pointee = designated.or_else(|| match &*self.shape(ty) {
            Shape::Pointer { pointee, .. } => *pointee,
            _ => None,
        });
        let (pbase, whole) = match base {
            PtrBase::Decl(d) if ast.decls.get(d).kind() == DeclKind::TemplateParamObject => {
                (PointerBase::Constant(self.constant(CKey::Decl(d))?), ast.decls.get(d).ty())
            }
            PtrBase::Decl(d) => (PointerBase::Variable(self.entity(d)?), ast.decls.get(d).ty()),
            PtrBase::Function(d) => (PointerBase::Function(self.entity(d)?), None),
            PtrBase::Temporary(t) => {
                let ext = ast.decls.lifetime_extended(t).map_or(t, |l| l.extending);
                (PointerBase::Temporary(self.entity(ext)?), ast.decls.get(t).ty())
            }
            PtrBase::TypeInfo(t) => (PointerBase::TypeInfo(self.type_display(t)?), None),
            PtrBase::Const(c) => {
                (PointerBase::Constant(self.constant(CKey::Const(c))?), Some(ast.consts.ty(c)))
            }
            PtrBase::StringLit { lit, origin } => {
                let base = PointerBase::Constant(self.constant(CKey::Str(lit, origin))?);
                let unit = u64::from(ast.strings.encoding(lit).unit_bytes());
                let whole = pointee.is_some_and(|p| ast.types.is_array(p));
                let path: Box<[PathStep]> = if whole && offset == 0 {
                    Box::new([])
                } else if offset.is_multiple_of(unit) {
                    self.charge(size_of::<PathStep>() as u64)?;
                    Box::new([PathStep::Index(offset / unit)])
                } else {
                    return unreadable("a pointer into the middle of a code unit");
                };
                return Ok(RunValue::Pointer(Box::new(PointerValue {
                    base,
                    path,
                    past_end: false,
                })));
            }
            PtrBase::Absolute | PtrBase::Label { .. } | PtrBase::LabelDiff { .. } => {
                return unreadable("an address that designates no object");
            }
        };
        let (path, past_end) = match whole {
            None if offset == 0 => (Vec::new(), false),
            None => return unreadable("a pointer past a function or type_info object"),
            Some(whole) => self.designator(whole, offset, pointee, past)?,
        };
        self.charge((path.len() * size_of::<PathStep>()) as u64)?;
        Ok(RunValue::Pointer(Box::new(PointerValue {
            base: pbase,
            path: path.into_boxed_slice(),
            past_end,
        })))
    }

    /// The designator of byte `offset` of an object of type `whole` for
    /// a pointer to `pointee` (`past`: one past the end of a subobject):
    /// the path and whether the pointer is one past the end of what it
    /// names.
    fn designator(
        &mut self,
        whole: QualType,
        offset: u64,
        pointee: Option<QualType>,
        past: bool,
    ) -> R<(Vec<PathStep>, bool)> {
        let mut steps = Vec::new();
        let size = pointee.and_then(|p| self.names.size_of(p)).filter(|&n| n != 0 && n <= offset);
        // A designator whose object has the pointee's type first (the
        // pointer itself, then one past the end of the subobject before
        // it), then one whose last step is a scalar of another type (a
        // `void*`).
        for strict in [true, false] {
            if !past && self.walk(whole, offset, pointee, strict, &mut steps, 0)? {
                return Ok((steps, false));
            }
            steps.clear();
            if let Some(size) = size
                && self.walk(whole, offset - size, pointee, strict, &mut steps, 0)?
            {
                return Ok((steps, true));
            }
            steps.clear();
        }
        unreadable("the designator of a pointer")
    }

    /// The constant printer's designator recovery (`walk_at`), as steps.
    /// Several members can share an offset (an empty base, a
    /// `[[no_unique_address]]` member, a union's members): each is tried
    /// until one holds `pointee` (`strict`), or a scalar at the offset.
    fn walk(
        &mut self,
        mut ty: QualType,
        mut offset: u64,
        pointee: Option<QualType>,
        strict: bool,
        steps: &mut Vec<PathStep>,
        depth: u32,
    ) -> R<bool> {
        for _ in depth..4096 {
            if offset == 0 && pointee.is_none_or(|p| self.same(p, ty)) {
                return Ok(true);
            }
            let shape = self.shape(ty);
            match &*shape {
                &Shape::Array { elem, stride, len } if stride != 0 => {
                    let i = offset / stride;
                    if i > len || (i == len && !offset.is_multiple_of(stride)) {
                        return Ok(false);
                    }
                    if i == len {
                        if pointee.is_none_or(|p| self.same(p, elem)) {
                            steps.push(PathStep::Index(i));
                            return Ok(true);
                        }
                        return Ok(false);
                    }
                    steps.push(PathStep::Index(i));
                    offset -= i * stride;
                    ty = elem;
                }
                Shape::Record { members, .. } => {
                    let mut pos = 0u32;
                    for m in members {
                        let unnamed_bits =
                            m.bit_width.is_some() && m.field.is_none_or(|f| self.is_unnamed(f));
                        if unnamed_bits {
                            continue;
                        }
                        let here = pos;
                        pos += 1;
                        let at = m.offset_bits / 8;
                        let hit = m.bit_width.is_none()
                            && m.offset_bits % 8 == 0
                            && at <= offset
                            && (offset < at + m.size || (offset == at && m.size == 0));
                        if !hit {
                            continue;
                        }
                        let step = match m.field {
                            None => PathStep::Base(self.type_display(m.ty)?),
                            Some(f) if self.is_unnamed(f) => PathStep::Anonymous(here),
                            Some(f) => PathStep::Member(self.ident(f)?),
                        };
                        let len = steps.len();
                        steps.push(step);
                        if self.walk(m.ty, offset - at, pointee, strict, steps, depth + 1)? {
                            return Ok(true);
                        }
                        steps.truncate(len);
                    }
                    return Ok(false);
                }
                _ => return Ok(offset == 0 && !strict),
            }
        }
        Ok(false)
    }

    // ---- the constants table -----------------------------------------

    /// The index of object `k` in the constants table, given on first
    /// reach.
    fn constant(&mut self, k: CKey) -> R<u32> {
        if let Some(&i) = self.keys.get(&k) {
            return Ok(i);
        }
        self.charge(size_of::<RunValue>() as u64)?;
        let Ok(i) = u32::try_from(self.queue.len()) else {
            return Err(ResultReadError::TooLarge(self.cap));
        };
        self.keys.insert(k, i);
        self.queue.push(k);
        Ok(i)
    }

    fn constant_value(&mut self, k: CKey) -> R<RunValue> {
        let ast = self.ast;
        match k {
            CKey::Decl(d) => match ast.templates.param_object_value(d) {
                Some(v) => self.value(v),
                None => unreadable("a template parameter object"),
            },
            CKey::Const(c) => self.value(c),
            CKey::Str(lit, _) => {
                let (enc, bytes) = ast.strings.get(lit);
                let w = enc.unit_bytes() as usize;
                self.charge((bytes.len() + w) as u64)?;
                let mut data = Vec::with_capacity(bytes.len() + w);
                data.extend_from_slice(bytes);
                data.resize(bytes.len() + w, 0);
                let unit = match enc {
                    StringEncoding::Ordinary => CharUnit::Char,
                    StringEncoding::Utf8 => CharUnit::Char8,
                    StringEncoding::Utf16 => CharUnit::Char16,
                    StringEncoding::Utf32 => CharUnit::Char32,
                    StringEncoding::Wide16 | StringEncoding::Wide32 => CharUnit::WChar,
                };
                Ok(RunValue::Text { unit, data: data.into_boxed_slice() })
            }
        }
    }
}

/// The element type of an array decoded as [`RunValue::Text`].
fn char_unit(ast: &Ast, elem: QualType) -> Option<CharUnit> {
    Some(match ast.types.builtin(ast.types.canonical(elem))? {
        BuiltinType::Char => CharUnit::Char,
        BuiltinType::SChar => CharUnit::SignedChar,
        BuiltinType::UChar => CharUnit::UnsignedChar,
        BuiltinType::Char8 => CharUnit::Char8,
        BuiltinType::Char16 => CharUnit::Char16,
        BuiltinType::Char32 => CharUnit::Char32,
        BuiltinType::WChar => CharUnit::WChar,
        _ => return None,
    })
}

/// An integer's bits, zero-extended (at most 128 are kept).
fn raw_bits(b: &IntBits<'_>) -> u128 {
    let w = b.words();
    let lo = w.first().copied().unwrap_or(0);
    let hi = w.get(1).copied().unwrap_or(0);
    u128::from(lo) | (u128::from(hi) << 64)
}

fn mask(v: u128, width: u32) -> u128 {
    if width >= 128 { v } else { v & ((1u128 << width) - 1) }
}

fn sign_extend(v: u128, width: u32) -> i128 {
    if width == 0 || width >= 128 {
        return v as i128;
    }
    let shift = 128 - width;
    ((v << shift) as i128) >> shift
}

/// `n` little-endian bytes at `at` (missing bytes read as 0).
fn read_le(bytes: &[u8], at: u64, n: u32) -> u128 {
    let mut v = 0u128;
    for i in (0..n.min(16)).rev() {
        let b = bytes.get(at as usize + i as usize).copied().unwrap_or(0);
        v = (v << 8) | u128::from(b);
    }
    v
}

/// `width` bits from bit `bit` (little-endian bit order).
fn read_bits(bytes: &[u8], bit: u64, width: u32) -> u128 {
    let mut v = 0u128;
    for i in 0..u64::from(width.min(128)) {
        let p = bit + i;
        let b = bytes.get((p / 8) as usize).copied().unwrap_or(0);
        v |= u128::from((b >> (p % 8)) & 1) << i;
    }
    v
}
