//! Kind-2 module files, part 2a: the declaration and type image
//! (spec/modules.md §11.11 "Part 2a").
//!
//! The writer serialises the declarations of a module interface unit's
//! purview and the types they use, with every id rewritten to an index in
//! the image (one index space per arena: names, types, declarations). An
//! importer that is given the image builds the unit from it through the
//! [`kiln_ast::AstBuilder`] instead of replaying the unit's source: one
//! builder call per record, no lexing, parsing or semantic analysis.
//!
//! Coverage. Only what can be rebuilt exactly is written: namespaces,
//! export declarations, non-inline non-constexpr functions (the body is
//! not needed by an importer, which neither emits nor evaluates it: only
//! its existence and braces are kept), variables without initializers,
//! enumerations and their enumerators (with their values), and typedefs;
//! types built from builtins, pointers, references, arrays, function
//! types, enumerations and typedefs. Anything else (a class, a template,
//! an `inline` or `constexpr` function, an initializer, an attribute, a
//! global module fragment, an import, a declaration outside the unit's
//! main file, ...) makes the writer emit no image, and the importer
//! replays the source as before (the fallback; correctness first).
//!
//! Reading is total: [`decode`] validates every index, offset and length
//! before anything is built; a malformed image is the module file's
//! corrupt-file error and builds nothing.

use kiln_ast::consts::Scalar;
use kiln_ast::decl::{
    Access, ConstexprKind, CtorInitKind, DeclFlags, DeclKind, Defaulted, DefinitionBits, EnumData,
    FnSpec, FnStorage, FriendKind, InitStyle, ModuleOwnership, ParamData, TagKind, VarSpec,
    VarStorage,
};
use kiln_ast::modules::Owner;
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::stmt::StmtKind;
use kiln_ast::types::{BuiltinType, ExcSpec, FuncInfo, Quals, TagKeyword, TypeView};
use kiln_ast::{DeclHead, DeclId, QualType, TypeId};
use kiln_source::fx::{FxHashMap, FxHashSet};
use kiln_source::{FileId, Loc};

use crate::Sema;
use crate::modules::Directive;

mod body;

use body::{Bodies, BodyState, Root};

/// The image format's version (the first field); bumped on any change.
pub(crate) const IMAGE_VERSION: u32 = 3;

/// "None" in an index or offset field.
const NONE: u32 = u32::MAX;
/// The translation unit as a lexical context.
const LEX_TU: u32 = u32::MAX - 1;
/// Type references `1..=BUILTINS` are the builtins; `BUILTINS + 1 + i`
/// is the image's type `i`.
const BUILTINS: u32 = BuiltinType::ALL.len() as u32;
/// Special-name words in a record's name field: the constructors and the
/// destructor of the enclosing class, a conversion function (to its
/// return type), `operator@` (`N_OP + op`). The importer recomputes the
/// name.
const N_CTOR: u32 = 0xFFFF_FF00;
const N_DTOR: u32 = 0xFFFF_FF01;
const N_CONV: u32 = 0xFFFF_FF02;
const N_OP: u32 = 0xFFFF_FE00;

/// What the writer hands the driver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleImage {
    /// The [`kiln_pp::bmi::SECTION_IMAGE`] bytes; empty when there is no
    /// image.
    pub bytes: Vec<u8>,
    /// Declaration records in the image (parameters not counted).
    pub decls: usize,
    /// Why there is no image (the first unsupported construct).
    pub fallback: Option<&'static str>,
}

// ---------------------------------------------------------------------------
// The image model
// ---------------------------------------------------------------------------

/// A qualified type reference: builtin or image type, plus cv bits.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct QRef {
    ty: u32,
    quals: u8,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum TyRec {
    Pointer(QRef),
    LRef(QRef),
    RRef(QRef),
    CArray(QRef, u64),
    IArray(QRef),
    Paren(QRef),
    Elab(u8, QRef),
    Func {
        ret: QRef,
        params: Vec<QRef>,
        info: u16,
        exc: u8,
    },
    Enum(u32),
    Typedef(u32),
    /// cv on array sugar (`const Quad` for `using Quad = int[4]`): the
    /// base, qualified through `TypeContext::qualify`.
    QArray(u8, QRef),
    /// One of the implicit builtin typedefs every translation unit
    /// declares (`__size_t` 0, `__signed_size_t` 1, `__ptrdiff_t` 2:
    /// the types of `sizeof` and pointer subtraction); the importer
    /// names its own.
    Sugar(u8),
    /// The class declared by a record.
    Record(u32),
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct ParamRec {
    /// Index into the parameter names, or [`NONE`].
    name: u32,
    loc: u32,
    ty: QRef,
    /// `ParamData.flags` (scope depth only).
    pflags: u16,
    /// `USED` / `REFERENCED`.
    flags: u8,
    /// The default argument's expression record, or [`NONE`].
    default: u32,
}

/// A function's body.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FnBody {
    None,
    /// The braces of a body the image does not carry (a non-inline
    /// function: importers only need to know it is defined).
    Braces(u32, u32),
    /// The body's compound statement record (inline, `constexpr` and
    /// `consteval` functions).
    Stmt(u32),
}

/// A base-specifier of a class definition (never virtual).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct BaseRec {
    ty: QRef,
    loc: u32,
    /// [`Access`] as written (0 when defaulted).
    access: u8,
}

/// A class definition: its frozen `DefinitionBits` (without
/// `BEING_DEFINED` / `COMPLETE`), the first record after its members, and
/// its bases.
#[derive(Clone, PartialEq, Eq, Debug)]
struct RecDef {
    bits: u64,
    end: u32,
    bases: Vec<BaseRec>,
}

/// A ctor-initializer: `kind` is `CtorInitKind` (`Base` 0: `target` a
/// type reference; `Field` 2: a field record; `Delegating` 4: 0).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct InitRec {
    kind: u8,
    target: u32,
    written: u8,
    /// The initializer's expression record.
    init: u32,
    loc: u32,
}

/// `Function.kind`: the declaration kind of a function record.
const FN_KINDS: [DeclKind; 5] = [
    DeclKind::Function,
    DeclKind::CXXMethod,
    DeclKind::CXXConstructor,
    DeclKind::CXXDestructor,
    DeclKind::CXXConversion,
];

#[derive(Clone, PartialEq, Eq, Debug)]
enum Payload {
    Namespace,
    Export,
    Function {
        /// Index into [`FN_KINDS`].
        kind: u8,
        ty: QRef,
        /// `FnSpec` bits (the subset [`fn_spec`] accepts).
        spec: u32,
        /// The type's exception specification is deferred to the
        /// function itself (`ExcSpec::Deferred`; `ty` is built with none).
        deferred: bool,
        params: Vec<ParamRec>,
        body: FnBody,
        /// A constructor's ctor-initializers, in initialization order.
        inits: Vec<InitRec>,
    },
    Var {
        ty: QRef,
        /// `VarSpec` bits (the subset [`var_spec`] accepts).
        spec: u32,
        /// The initializer's expression record, or [`NONE`].
        init: u32,
        /// The evaluated value: [`NONE`] not evaluated (or a value the
        /// image does not carry), [`body::NOT_CONST`], or a constant.
        eval: u32,
    },
    /// A label (a local declaration only).
    Label,
    /// A `LifetimeExtendedTemporary` (a local declaration only): the
    /// temporary's type, the local variable record whose reference
    /// binding extends it, and its mangling index.
    Let {
        ty: QRef,
        extending: u32,
        index: u32,
    },
    /// `scoped` is `EnumData` `SCOPED | SCOPED_STRUCT`; `def` is present
    /// for a definition, holding the computed (underlying, promotion)
    /// types of one without a fixed underlying type.
    Enum {
        scoped: u8,
        fixed: Option<(QRef, QRef)>,
        // Three states: a declaration only, a definition with a fixed
        // type, a definition with computed types (spec/modules.md §11.11
        // "Encoding", the enumeration payload).
        #[allow(clippy::option_option)]
        def: Option<Option<(QRef, QRef)>>,
    },
    Enumerator {
        ty: QRef,
        value: i128,
    },
    Typedef {
        alias: bool,
        underlying: QRef,
    },
    /// A class: `tag` is `TagKind`; `def` is present for a definition.
    Record {
        tag: u8,
        def: Option<RecDef>,
    },
    /// A non-static data member: `FieldData.flags` (the subset
    /// [`FIELD_FLAGS`]), the bit-field width and default member
    /// initializer expression records (or [`NONE`]).
    Field {
        ty: QRef,
        flags: u32,
        width: u32,
        init: u32,
    },
    /// An access specifier (its access is the header's).
    Access,
    /// An implicitly declared special member (`SpecialKind`) of the
    /// complete class `ctx`: the importer declares it the way sema does
    /// lazily (spec/modules.md §11.11 "Classes").
    Implicit {
        kind: u8,
    },
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct DeclRec {
    /// `USED` / `REFERENCED`.
    flags: u8,
    /// Bit 0 purview, bit 1 exported.
    owner: u8,
    /// [`Access`] of a class member (0 elsewhere).
    access: u8,
    /// Index into the names, or [`NONE`].
    name: u32,
    /// Record index, or [`NONE`] for the translation unit.
    ctx: u32,
    /// Record index, [`LEX_TU`], or [`NONE`] when it is `ctx`.
    lexical: u32,
    loc: u32,
    /// Record index of the previous declaration, or [`NONE`].
    prev: u32,
    payload: Payload,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
struct Image {
    main: String,
    module: String,
    kw: u32,
    name_loc: u32,
    /// The unit's declared names (the preprocessor's gate).
    names: Vec<String>,
    /// Parameter and local names (not gated).
    pnames: Vec<String>,
    types: Vec<TyRec>,
    decls: Vec<DeclRec>,
    bodies: Bodies,
}

const K_NAMESPACE: u8 = 1;
const K_EXPORT: u8 = 2;
const K_FUNCTION: u8 = 3;
const K_VAR: u8 = 4;
const K_ENUM: u8 = 5;
const K_ENUMERATOR: u8 = 6;
const K_TYPEDEF: u8 = 7;
const K_ALIAS: u8 = 8;
const K_LABEL: u8 = 9;
const K_LET: u8 = 10;
const K_RECORD: u8 = 11;
const K_FIELD: u8 = 12;
const K_ACCESS: u8 = 13;
const K_IMPLICIT: u8 = 14;

impl Payload {
    fn code(&self) -> u8 {
        match self {
            Payload::Namespace => K_NAMESPACE,
            Payload::Export => K_EXPORT,
            Payload::Function { .. } => K_FUNCTION,
            Payload::Var { .. } => K_VAR,
            Payload::Enum { .. } => K_ENUM,
            Payload::Enumerator { .. } => K_ENUMERATOR,
            Payload::Typedef { alias: false, .. } => K_TYPEDEF,
            Payload::Typedef { alias: true, .. } => K_ALIAS,
            Payload::Label => K_LABEL,
            Payload::Let { .. } => K_LET,
            Payload::Record { .. } => K_RECORD,
            Payload::Field { .. } => K_FIELD,
            Payload::Access => K_ACCESS,
            Payload::Implicit { .. } => K_IMPLICIT,
        }
    }
}

const DECL_FLAGS: u8 = DeclFlags::USED.bits() | DeclFlags::REFERENCED.bits();

/// `FieldData.flags` carried: `mutable`, the initializer style, a
/// default member initializer (`[[no_unique_address]]` is an attribute).
const FIELD_FLAGS: u32 = kiln_ast::decl::FieldData::MUTABLE
    | (3 << 2)
    | kiln_ast::decl::FieldData::HAS_DEFAULT_INIT;

/// `DefinitionBits` a carried class may have: not a lambda, polymorphic,
/// abstract, anonymous, with virtual or inherited members, or
/// `trivial_abi` (spec/modules.md §11.11 "Classes").
const DEF_BITS: u64 = {
    use kiln_ast::decl::DefinitionBits as B;
    ((1u64 << 57) - 1)
        & !(B::BEING_DEFINED.bits()
            | B::COMPLETE.bits()
            | B::ANONYMOUS.bits()
            | B::LAMBDA.bits()
            | B::POLYMORPHIC.bits()
            | B::ABSTRACT.bits()
            | B::HAS_VIRTUAL_BASES.bits()
            | B::HAS_INHERITED_CTOR.bits()
            | B::HAS_INHERITED_ASSIGN.bits()
            | B::LAMBDA_CAPTURES.bits()
            | B::TRIVIAL_ABI.bits())
};

/// Function specifier bits carried: storage, inline (written and
/// implicit), `constexpr`/`consteval`, explicitly defaulted, deleted,
/// `explicit`, trivial, immediate-escalating, escalated, `...`, inherited
/// storage, deduced return type. Anything else (virtual, implicitly
/// defaulted, coroutine, contracts, ...) is not carried yet.
fn fn_spec(bits: u32) -> Option<FnSpec> {
    let storage = match bits & 3 {
        0 => FnStorage::None,
        1 => FnStorage::Static,
        2 => FnStorage::Extern,
        _ => return None,
    };
    let ck = match (bits >> 4) & 3 {
        0 => ConstexprKind::None,
        1 => ConstexprKind::Constexpr,
        2 => ConstexprKind::Consteval,
        _ => return None,
    };
    let bit = |b: u32| bits & (1 << b) != 0;
    let spec = FnSpec::new()
        .with_storage(storage)
        .with_inline(bit(2))
        .with_implicit_inline(bit(3))
        .with_constexpr_kind(ck)
        .with_defaulted(if bit(8) { Defaulted::Explicit } else { Defaulted::No })
        .with_deleted(bit(10))
        .with_explicit(bit(11))
        .with_trivial(bit(12))
        .with_immediate_escalating(bit(13))
        .with_escalated(bit(16))
        .with_variadic(bit(17))
        .with_storage_inherited(bit(22))
        .with_return_deduced(bit(24));
    (spec.bits() == bits).then_some(spec)
}

/// `Access` from its `u8` (validated `<= 3`).
fn access_of(a: u8) -> Access {
    match a {
        1 => Access::Public,
        2 => Access::Protected,
        3 => Access::Private,
        _ => Access::None,
    }
}

/// `TagKind` from its `u8` (validated `<= 2`).
fn tag_of(t: u8) -> TagKind {
    match t {
        1 => TagKind::Class,
        2 => TagKind::Union,
        _ => TagKind::Struct,
    }
}

/// `CtorInitKind` from a carried initializer's kind (0, 2 or 4).
fn init_kind_of(k: u8) -> CtorInitKind {
    match k {
        0 => CtorInitKind::Base,
        2 => CtorInitKind::Field,
        _ => CtorInitKind::Delegating,
    }
}

/// Variable specifier bits carried: storage (none, static, extern), the
/// initialization style, `constexpr`, `constinit`, `inline`, NRVO and
/// range-`for` variables. Thread-local storage is not carried yet.
fn var_spec(bits: u32) -> Option<VarSpec> {
    let storage = match bits & 7 {
        0 => VarStorage::None,
        1 => VarStorage::Static,
        2 => VarStorage::Extern,
        _ => return None,
    };
    let style = match (bits >> 5) & 3 {
        0 => InitStyle::C,
        1 => InitStyle::Call,
        2 => InitStyle::List,
        _ => InitStyle::ParenList,
    };
    let bit = |b: u32| bits & (1 << b) != 0;
    let spec = VarSpec::new()
        .with_storage(storage)
        .with_init_style(style)
        .with_constexpr(bit(7))
        .with_constinit(bit(8))
        .with_inline(bit(9))
        .with_nrvo(bit(10))
        .with_cxx_for_range(bit(12));
    (spec.bits() == bits).then_some(spec)
}

fn tag_keyword(k: u8) -> Option<TagKeyword> {
    Some(match k {
        0 => TagKeyword::None,
        1 => TagKeyword::Struct,
        2 => TagKeyword::Class,
        3 => TagKeyword::Union,
        4 => TagKeyword::Enum,
        5 => TagKeyword::Typename,
        6 => TagKeyword::Interface,
        _ => return None,
    })
}

fn exc_spec(e: u8) -> Option<ExcSpec> {
    Some(match e {
        0 => ExcSpec::None,
        1 => ExcSpec::BasicNoexcept,
        2 => ExcSpec::DynamicNone,
        3 => ExcSpec::Nothrow,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

struct Wr(Vec<u8>);

impl Wr {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn len(&mut self, n: usize) {
        self.u32(u32::try_from(n).unwrap_or(u32::MAX));
    }
    fn str(&mut self, s: &str) {
        self.len(s.len());
        self.0.extend_from_slice(s.as_bytes());
    }
    fn q(&mut self, q: QRef) {
        self.u32(q.ty);
        self.u8(q.quals);
    }
    fn pair(&mut self, p: Option<(QRef, QRef)>) {
        match p {
            None => self.u8(0),
            Some((a, b)) => {
                self.u8(1);
                self.q(a);
                self.q(b);
            }
        }
    }
}

fn encode_decl(w: &mut Wr, d: &DeclRec) {
    w.u8(d.payload.code());
    w.u8(d.flags);
    w.u8(d.owner);
    w.u8(d.access);
    for v in [d.name, d.ctx, d.lexical, d.loc, d.prev] {
        w.u32(v);
    }
    match &d.payload {
        Payload::Namespace | Payload::Export | Payload::Label | Payload::Access => {}
        Payload::Function { kind, ty, spec, deferred, params, body, inits } => {
            w.u8(*kind);
            w.q(*ty);
            w.u32(*spec);
            w.u8(u8::from(*deferred));
            w.len(params.len());
            for p in params {
                w.u32(p.name);
                w.u32(p.loc);
                w.q(p.ty);
                w.u16(p.pflags);
                w.u8(p.flags);
                w.u32(p.default);
            }
            match body {
                FnBody::None => w.u8(0),
                FnBody::Braces(lb, rb) => {
                    w.u8(1);
                    w.u32(*lb);
                    w.u32(*rb);
                }
                FnBody::Stmt(s) => {
                    w.u8(2);
                    w.u32(*s);
                }
            }
            w.len(inits.len());
            for i in inits {
                w.u8(i.kind);
                w.u32(i.target);
                w.u8(i.written);
                w.u32(i.init);
                w.u32(i.loc);
            }
        }
        Payload::Var { ty, spec, init, eval } => {
            w.q(*ty);
            w.u32(*spec);
            w.u32(*init);
            w.u32(*eval);
        }
        Payload::Let { ty, extending, index } => {
            w.q(*ty);
            w.u32(*extending);
            w.u32(*index);
        }
        Payload::Enum { scoped, fixed, def } => {
            w.u8(*scoped);
            w.pair(*fixed);
            match def {
                None => w.u8(0),
                Some(c) => {
                    w.u8(1);
                    w.pair(*c);
                }
            }
        }
        Payload::Enumerator { ty, value } => {
            w.q(*ty);
            w.0.extend_from_slice(&value.to_le_bytes());
        }
        Payload::Typedef { underlying, .. } => w.q(*underlying),
        Payload::Record { tag, def } => {
            w.u8(*tag);
            match def {
                None => w.u8(0),
                Some(d) => {
                    w.u8(1);
                    w.u64(d.bits);
                    w.u32(d.end);
                    w.len(d.bases.len());
                    for b in &d.bases {
                        w.q(b.ty);
                        w.u32(b.loc);
                        w.u8(b.access);
                    }
                }
            }
        }
        Payload::Field { ty, flags, width, init } => {
            w.q(*ty);
            w.u32(*flags);
            w.u32(*width);
            w.u32(*init);
        }
        Payload::Implicit { kind } => w.u8(*kind),
    }
}

fn encode(img: &Image) -> Vec<u8> {
    let mut w = Wr(Vec::new());
    w.u32(IMAGE_VERSION);
    w.str(&img.main);
    w.str(&img.module);
    w.u32(img.kw);
    w.u32(img.name_loc);
    for list in [&img.names, &img.pnames] {
        w.len(list.len());
        for s in list {
            w.str(s);
        }
    }
    w.len(img.types.len());
    for t in &img.types {
        match t {
            TyRec::Pointer(q) => (w.u8(1), w.q(*q)).1,
            TyRec::LRef(q) => (w.u8(2), w.q(*q)).1,
            TyRec::RRef(q) => (w.u8(3), w.q(*q)).1,
            TyRec::CArray(q, n) => {
                w.u8(4);
                w.q(*q);
                w.0.extend_from_slice(&n.to_le_bytes());
            }
            TyRec::IArray(q) => (w.u8(5), w.q(*q)).1,
            TyRec::Paren(q) => (w.u8(6), w.q(*q)).1,
            TyRec::Elab(k, q) => {
                w.u8(7);
                w.u8(*k);
                w.q(*q);
            }
            TyRec::Func { ret, params, info, exc } => {
                w.u8(8);
                w.q(*ret);
                w.len(params.len());
                for p in params {
                    w.q(*p);
                }
                w.u16(*info);
                w.u8(*exc);
            }
            TyRec::Enum(d) => (w.u8(9), w.u32(*d)).1,
            TyRec::Typedef(d) => (w.u8(10), w.u32(*d)).1,
            TyRec::QArray(c, q) => {
                w.u8(11);
                w.u8(*c);
                w.q(*q);
            }
            TyRec::Sugar(k) => (w.u8(12), w.u8(*k)).1,
            TyRec::Record(d) => (w.u8(13), w.u32(*d)).1,
        }
    }
    // Declarations: an offset table into the record blob, so a reader
    // can find record `i` without decoding the ones before it.
    let mut blob = Wr(Vec::new());
    let mut offsets = Vec::with_capacity(img.decls.len());
    for d in &img.decls {
        offsets.push(u32::try_from(blob.0.len()).unwrap_or(u32::MAX));
        encode_decl(&mut blob, d);
    }
    w.len(img.decls.len());
    for o in offsets {
        w.u32(o);
    }
    w.len(blob.0.len());
    w.0.extend_from_slice(&blob.0);
    // The top-level lookup table: (context, name, record), sorted.
    let mut lookup: Vec<(u32, u32, u32)> = img
        .decls
        .iter()
        .enumerate()
        .filter(|(_, d)| d.name != NONE && !body::is_local(img, d))
        .map(|(i, d)| (d.ctx, d.name, i as u32))
        .collect();
    lookup.sort_unstable();
    w.len(lookup.len());
    for (c, n, d) in lookup {
        w.u32(c);
        w.u32(n);
        w.u32(d);
    }
    body::encode_bodies(&mut w, &img.bodies);
    w.0
}

// ---------------------------------------------------------------------------
// Decoding (total)
// ---------------------------------------------------------------------------

/// A malformed image; the text is the corrupt-file diagnostic's reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImageError(pub String);

type Res<T> = Result<T, ImageError>;

fn bad<T>(why: &str) -> Res<T> {
    Err(ImageError(format!("declaration image: {why}")))
}

struct Rd<'a>(&'a [u8]);

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Res<&'a [u8]> {
        if n > self.0.len() {
            return bad("truncated");
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u8(&mut self) -> Res<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Res<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Res<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u64(&mut self) -> Res<u64> {
        let mut a = [0u8; 8];
        a.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(a))
    }
    fn i128(&mut self) -> Res<i128> {
        let mut a = [0u8; 16];
        a.copy_from_slice(self.take(16)?);
        Ok(i128::from_le_bytes(a))
    }
    /// A count whose elements take at least `min` bytes each, checked
    /// against the bytes left before anything is sized by it.
    fn count(&mut self, min: usize) -> Res<usize> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min.max(1)) > self.0.len() {
            return bad("length exceeds the section");
        }
        Ok(n)
    }
    fn str(&mut self) -> Res<String> {
        let n = self.count(1)?;
        match std::str::from_utf8(self.take(n)?) {
            Ok(s) => Ok(s.to_owned()),
            Err(_) => bad("invalid UTF-8 in a name"),
        }
    }
    fn q(&mut self) -> Res<QRef> {
        let ty = self.u32()?;
        let quals = self.u8()?;
        if quals > 7 {
            return bad("bad qualifier bits");
        }
        Ok(QRef { ty, quals })
    }
    fn pair(&mut self) -> Res<Option<(QRef, QRef)>> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some((self.q()?, self.q()?))),
            _ => bad("bad flag byte"),
        }
    }
}

/// The smallest encoded type and declaration record.
const MIN_TYPE: usize = 5;
const MIN_DECL: usize = 4 + 20;

fn decode_decl(r: &mut Rd<'_>) -> Res<DeclRec> {
    let code = r.u8()?;
    let flags = r.u8()?;
    let owner = r.u8()?;
    let access = r.u8()?;
    let (name, ctx, lexical, loc, prev) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?);
    let payload = match code {
        K_NAMESPACE => Payload::Namespace,
        K_EXPORT => Payload::Export,
        K_FUNCTION => {
            let kind = r.u8()?;
            let ty = r.q()?;
            let spec = r.u32()?;
            let deferred = match r.u8()? {
                0 => false,
                1 => true,
                _ => return bad("bad flag byte"),
            };
            let n = r.count(20)?;
            let mut params = Vec::with_capacity(n);
            for _ in 0..n {
                params.push(ParamRec {
                    name: r.u32()?,
                    loc: r.u32()?,
                    ty: r.q()?,
                    pflags: r.u16()?,
                    flags: r.u8()?,
                    default: r.u32()?,
                });
            }
            let body = match r.u8()? {
                0 => FnBody::None,
                1 => FnBody::Braces(r.u32()?, r.u32()?),
                2 => FnBody::Stmt(r.u32()?),
                _ => return bad("bad flag byte"),
            };
            let n = r.count(14)?;
            let mut inits = Vec::with_capacity(n);
            for _ in 0..n {
                inits.push(InitRec {
                    kind: r.u8()?,
                    target: r.u32()?,
                    written: r.u8()?,
                    init: r.u32()?,
                    loc: r.u32()?,
                });
            }
            Payload::Function { kind, ty, spec, deferred, params, body, inits }
        }
        K_VAR => Payload::Var { ty: r.q()?, spec: r.u32()?, init: r.u32()?, eval: r.u32()? },
        K_LABEL => Payload::Label,
        K_LET => Payload::Let { ty: r.q()?, extending: r.u32()?, index: r.u32()? },
        K_ENUM => {
            let scoped = r.u8()?;
            let fixed = r.pair()?;
            let def = match r.u8()? {
                0 => None,
                1 => Some(r.pair()?),
                _ => return bad("bad flag byte"),
            };
            Payload::Enum { scoped, fixed, def }
        }
        K_ENUMERATOR => Payload::Enumerator { ty: r.q()?, value: r.i128()? },
        K_TYPEDEF | K_ALIAS => Payload::Typedef { alias: code == K_ALIAS, underlying: r.q()? },
        K_RECORD => {
            let tag = r.u8()?;
            let def = match r.u8()? {
                0 => None,
                1 => {
                    let bits = r.u64()?;
                    let end = r.u32()?;
                    let n = r.count(10)?;
                    let mut bases = Vec::with_capacity(n);
                    for _ in 0..n {
                        bases.push(BaseRec { ty: r.q()?, loc: r.u32()?, access: r.u8()? });
                    }
                    Some(RecDef { bits, end, bases })
                }
                _ => return bad("bad flag byte"),
            };
            Payload::Record { tag, def }
        }
        K_FIELD => {
            Payload::Field { ty: r.q()?, flags: r.u32()?, width: r.u32()?, init: r.u32()? }
        }
        K_ACCESS => Payload::Access,
        K_IMPLICIT => Payload::Implicit { kind: r.u8()? },
        _ => return bad("unknown declaration kind"),
    };
    Ok(DeclRec { flags, owner, access, name, ctx, lexical, loc, prev, payload })
}

/// The type references a type record uses, and the record it names.
fn ty_refs(t: &TyRec) -> (Vec<QRef>, Option<u32>) {
    match t {
        TyRec::Pointer(q)
        | TyRec::LRef(q)
        | TyRec::RRef(q)
        | TyRec::CArray(q, _)
        | TyRec::IArray(q)
        | TyRec::Paren(q)
        | TyRec::Elab(_, q)
        | TyRec::QArray(_, q) => (vec![*q], None),
        TyRec::Func { ret, params, .. } => {
            let mut v = Vec::with_capacity(params.len() + 1);
            v.push(*ret);
            v.extend_from_slice(params);
            (v, None)
        }
        TyRec::Enum(d) | TyRec::Typedef(d) | TyRec::Record(d) => (Vec::new(), Some(*d)),
        TyRec::Sugar(_) => (Vec::new(), None),
    }
}

/// The type references a declaration record uses.
fn decl_refs(d: &DeclRec) -> Vec<QRef> {
    match &d.payload {
        Payload::Namespace
        | Payload::Export
        | Payload::Label
        | Payload::Access
        | Payload::Implicit { .. }
        | Payload::Record { def: None, .. } => Vec::new(),
        Payload::Record { def: Some(d), .. } => d.bases.iter().map(|b| b.ty).collect(),
        Payload::Field { ty, .. } => vec![*ty],
        Payload::Function { ty, params, .. } => {
            std::iter::once(*ty).chain(params.iter().map(|p| p.ty)).collect()
        }
        Payload::Var { ty, .. } | Payload::Enumerator { ty, .. } | Payload::Let { ty, .. } => {
            vec![*ty]
        }
        Payload::Enum { fixed, def, .. } => {
            let mut v = Vec::new();
            for (a, b) in fixed.iter().chain(def.iter().flatten()) {
                v.push(*a);
                v.push(*b);
            }
            v
        }
        Payload::Typedef { underlying, .. } => vec![*underlying],
    }
}

fn same_kind(a: &Payload, b: &Payload) -> bool {
    match (a, b) {
        (Payload::Function { kind: x, .. }, Payload::Function { kind: y, .. }) => x == y,
        (Payload::Field { .. } | Payload::Access | Payload::Implicit { .. } | Payload::Export, _) => {
            false
        }
        _ => a.code() == b.code(),
    }
}

/// Decodes and validates an image. Total: any malformed input is an
/// [`ImageError`]; an `Ok` image can be built without further checks
/// other than its offsets against the main file's length.
fn decode(bytes: &[u8]) -> Res<Image> {
    let mut r = Rd(bytes);
    if r.u32()? != IMAGE_VERSION {
        return bad("unsupported version");
    }
    let mut img = Image { main: r.str()?, module: r.str()?, ..Image::default() };
    img.kw = r.u32()?;
    img.name_loc = r.u32()?;
    for which in 0..2 {
        let n = r.count(4)?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(r.str()?);
        }
        if which == 0 { img.names = v } else { img.pnames = v }
    }
    let nt = r.count(MIN_TYPE)?;
    img.types = Vec::with_capacity(nt);
    for _ in 0..nt {
        let t = match r.u8()? {
            1 => TyRec::Pointer(r.q()?),
            2 => TyRec::LRef(r.q()?),
            3 => TyRec::RRef(r.q()?),
            4 => TyRec::CArray(r.q()?, r.u64()?),
            5 => TyRec::IArray(r.q()?),
            6 => TyRec::Paren(r.q()?),
            7 => {
                let k = r.u8()?;
                if tag_keyword(k).is_none() {
                    return bad("bad tag keyword");
                }
                TyRec::Elab(k, r.q()?)
            }
            8 => {
                let ret = r.q()?;
                let n = r.count(5)?;
                let mut params = Vec::with_capacity(n);
                for _ in 0..n {
                    params.push(r.q()?);
                }
                let info = r.u16()?;
                let exc = r.u8()?;
                if exc_spec(exc).is_none() {
                    return bad("bad exception specification");
                }
                TyRec::Func { ret, params, info, exc }
            }
            9 => TyRec::Enum(r.u32()?),
            10 => TyRec::Typedef(r.u32()?),
            11 => {
                let c = r.u8()?;
                let q = r.q()?;
                if c == 0 || c > 7 || q.quals != 0 {
                    return bad("bad qualified array");
                }
                TyRec::QArray(c, q)
            }
            12 => match r.u8()? {
                k @ 0..=2 => TyRec::Sugar(k),
                _ => return bad("unknown builtin typedef"),
            },
            13 => TyRec::Record(r.u32()?),
            _ => return bad("unknown type kind"),
        };
        img.types.push(t);
    }
    let nd = r.count(4 + MIN_DECL)?;
    let mut offsets = Vec::with_capacity(nd);
    for _ in 0..nd {
        offsets.push(r.u32()?);
    }
    let blob_len = r.count(1)?;
    let blob = r.take(blob_len)?;
    let mut br = Rd(blob);
    img.decls = Vec::with_capacity(nd);
    for &o in &offsets {
        if o as usize != blob_len - br.0.len() {
            return bad("bad declaration offset");
        }
        img.decls.push(decode_decl(&mut br)?);
    }
    if !br.0.is_empty() {
        return bad("trailing declaration bytes");
    }
    let nl = r.count(12)?;
    let mut lookup = Vec::with_capacity(nl);
    for _ in 0..nl {
        lookup.push((r.u32()?, r.u32()?, r.u32()?));
    }
    img.bodies = body::decode_bodies(&mut r)?;
    if !r.0.is_empty() {
        return bad("trailing bytes");
    }
    validate(&img, &lookup)?;
    Ok(img)
}

/// The class a base-specifier's type `q` names: a class definition
/// `complete` accepts, through elaborated, parenthesized and
/// typedef sugar.
fn base_class(img: &Image, q: QRef, complete: &dyn Fn(u32) -> bool) -> Option<u32> {
    let mut t = q.ty;
    for _ in 0..64 {
        let j = t.checked_sub(BUILTINS + 1)? as usize;
        t = match img.types.get(j)? {
            TyRec::Elab(_, q) | TyRec::Paren(q) => q.ty,
            TyRec::Typedef(d) => match img.decls.get(*d as usize)?.payload {
                Payload::Typedef { underlying, .. } => underlying.ty,
                _ => return None,
            },
            TyRec::Record(d) => {
                let ok = matches!(img.decls.get(*d as usize)?.payload, Payload::Record { .. });
                return (ok && complete(*d)).then_some(*d);
            }
            _ => return None,
        };
    }
    None
}

/// The structural checks of [`decode`].
#[allow(clippy::too_many_lines)] // one arm per record or type kind, kept together with the format
fn validate(img: &Image, lookup: &[(u32, u32, u32)]) -> Res<()> {
    let nt = img.types.len() as u64;
    let nd = img.decls.len();
    let qref_ok = |q: QRef, below: u64| -> bool {
        let t = u64::from(q.ty);
        (1..=u64::from(BUILTINS)).contains(&t)
            || (t > u64::from(BUILTINS) && t - u64::from(BUILTINS) - 1 < below)
    };
    // Per type, the highest record (plus one) it or any earlier type
    // names: types are built in order, so a record using type `j` needs
    // every record named by types `0..=j` built before it.
    let mut pdep = Vec::with_capacity(img.types.len());
    let mut acc = 0usize;
    for (j, t) in img.types.iter().enumerate() {
        let (refs, decl) = ty_refs(t);
        if refs.iter().any(|&q| !qref_ok(q, j as u64)) {
            return bad("bad type reference");
        }
        if let Some(d) = decl {
            let ok = img.decls.get(d as usize).is_some_and(|r| {
                matches!(
                    (t, &r.payload),
                    (TyRec::Enum(_), Payload::Enum { .. })
                        | (TyRec::Typedef(_), Payload::Typedef { .. })
                        | (TyRec::Record(_), Payload::Record { .. })
                )
            });
            if !ok {
                return bad("bad declaration reference in a type");
            }
            acc = acc.max(d as usize + 1);
        }
        if let TyRec::Func { info, .. } = t
            && FuncInfo::from_bits(*info).bits() != *info
        {
            return bad("bad function type bits");
        }
        pdep.push(acc);
    }
    let qref_at = |q: QRef, i: usize| -> bool {
        if !qref_ok(q, nt) {
            return false;
        }
        let t = u64::from(q.ty);
        t <= u64::from(BUILTINS) || pdep[(t - u64::from(BUILTINS) - 1) as usize] <= i
    };
    let mut open_enum: Option<usize> = None;
    // The open class definitions (record, end), innermost last; per
    // record, the first declaration of its entity; per entity, whether
    // a definition was seen; the implicit members declared.
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut chain: Vec<usize> = Vec::with_capacity(nd);
    // Per entity (its first record), the end of its definition.
    let mut def_end = vec![usize::MAX; nd];
    let mut implicit: FxHashSet<(u32, u8)> = FxHashSet::default();
    for (i, d) in img.decls.iter().enumerate() {
        let kind_of = |x: u32| img.decls.get(x as usize).filter(|_| (x as usize) < i);
        while stack.last().is_some_and(|&(_, end)| end <= i) {
            stack.pop();
        }
        chain.push(if (d.prev as usize) < i { chain[d.prev as usize] } else { i });
        // A local declaration (a body's variable or label): its context
        // is an earlier function record; it is never looked up by name.
        if kind_of(d.ctx).is_some_and(|c| matches!(c.payload, Payload::Function { .. })) {
            if d.flags & !body::LOCAL_FLAGS != 0
                || d.owner > 3
                || d.access != 0
                || d.lexical != NONE
                || d.prev != NONE
                || (d.name != NONE && d.name as usize >= img.pnames.len())
            {
                return bad("bad local declaration");
            }
            open_enum = None;
            match &d.payload {
                Payload::Label => {}
                Payload::Var { ty, spec, .. } => {
                    if !qref_at(*ty, i) || var_spec(*spec).is_none() {
                        return bad("bad local variable");
                    }
                }
                Payload::Let { ty, extending, .. } => {
                    let ext = kind_of(*extending);
                    if !qref_at(*ty, i)
                        || !ext.is_some_and(|x| {
                            matches!(x.payload, Payload::Var { .. }) && x.ctx == d.ctx
                        })
                    {
                        return bad("bad lifetime-extended temporary");
                    }
                }
                _ => return bad("bad local declaration"),
            }
            continue;
        }
        if d.flags & !DECL_FLAGS != 0
            || d.owner > 3
            || matches!(d.payload, Payload::Label | Payload::Let { .. })
        {
            return bad("bad declaration flags");
        }
        let scope = |x: u32| {
            x == NONE
                || kind_of(x)
                    .is_some_and(|r| matches!(r.payload, Payload::Namespace | Payload::Export))
        };
        let closed = |x: u32| {
            kind_of(x).is_some_and(|r| {
                matches!(&r.payload, Payload::Record { def: Some(df), .. } if df.end as usize <= i)
            })
        };
        let in_class = kind_of(d.ctx).is_some_and(|r| matches!(r.payload, Payload::Record { .. }));
        let top = stack.last().map(|&(r, _)| r as u32);
        let is_enumerator = matches!(d.payload, Payload::Enumerator { .. });
        if is_enumerator {
            if open_enum.is_none() || d.ctx as usize != open_enum.unwrap_or(usize::MAX) {
                return bad("an enumerator outside its enumeration");
            }
        } else {
            open_enum = None;
            // Class members follow their class's definition record and
            // precede its end; an out-of-line member definition names a
            // complete class and a previous declaration; an implicit
            // member names a complete class.
            let ok = match (&d.payload, d.lexical) {
                (Payload::Implicit { .. }, _) => top.is_none() && closed(d.ctx) && d.lexical == NONE,
                (_, NONE) => top.map_or_else(|| scope(d.ctx), |t| d.ctx == t),
                _ => top.is_none() && (scope(d.ctx) || (closed(d.ctx) && d.prev != NONE)),
            };
            if !ok || (matches!(d.payload, Payload::Field { .. } | Payload::Access) && !in_class) {
                return bad("bad declaration context");
            }
        }
        // Members carry their access; so do the enumerators of a member
        // enumeration (which share its access).
        let member_enum = is_enumerator && kind_of(d.ctx).is_some_and(|e| e.access != 0);
        if d.access > 3 || (!in_class && !member_enum && d.access != 0) {
            return bad("bad member access");
        }
        if d.lexical != NONE && d.lexical != LEX_TU && !scope(d.lexical) {
            return bad("bad lexical context");
        }
        if d.prev != NONE && !kind_of(d.prev).is_some_and(|p| same_kind(&p.payload, &d.payload)) {
            return bad("bad previous declaration");
        }
        let ident = |n: u32| (n as usize) < img.names.len();
        let name_ok = match &d.payload {
            Payload::Export | Payload::Access | Payload::Implicit { .. } => d.name == NONE,
            Payload::Field { .. } => d.name == NONE || ident(d.name),
            Payload::Function { kind, .. } => match d.name {
                N_CTOR => *kind == 2,
                N_DTOR => *kind == 3,
                N_CONV => *kind == 4,
                n if (N_OP..N_CTOR).contains(&n) => {
                    *kind < 2 && u8::try_from(n - N_OP).ok().and_then(OverloadedOperator::from_u8).is_some()
                }
                n => *kind < 2 && ident(n),
            },
            _ => ident(d.name),
        };
        if !name_ok {
            return bad("bad declaration name");
        }
        if decl_refs(d).into_iter().any(|q| !qref_at(q, i)) {
            return bad("bad type reference");
        }
        match &d.payload {
            Payload::Function { kind, ty, spec, params, inits, .. } => {
                if *kind as usize >= FN_KINDS.len() || (*kind == 0) == in_class {
                    return bad("bad function kind");
                }
                if !inits.is_empty() && *kind != 2 {
                    return bad("ctor-initializers of a function that is not a constructor");
                }
                let nbases = kind_of(d.ctx).map_or(0, |r| match &r.payload {
                    Payload::Record { def: Some(df), .. } => df.bases.len(),
                    _ => 0,
                });
                for x in inits {
                    let ok = match x.kind {
                        0 => (x.target as usize) < nbases,
                        2 => kind_of(x.target).is_some_and(|f| {
                            matches!(f.payload, Payload::Field { .. }) && f.ctx == d.ctx
                        }),
                        4 => x.target == 0,
                        _ => false,
                    };
                    if !ok {
                        return bad("bad ctor-initializer");
                    }
                }
                let proto = (ty.ty > BUILTINS)
                    .then(|| img.types.get((ty.ty - BUILTINS - 1) as usize))
                    .flatten();
                let Some(TyRec::Func { params: pt, .. }) = proto else {
                    return bad("a function without a function type");
                };
                if ty.quals != 0 || pt.len() != params.len() || params.len() > usize::from(u16::MAX)
                {
                    return bad("bad function type");
                }
                if fn_spec(*spec).is_none() {
                    return bad("bad function specifiers");
                }
                for p in params {
                    if (p.name != NONE && p.name as usize >= img.pnames.len())
                        || p.flags & !DECL_FLAGS != 0
                        || p.pflags & !ParamData::SCOPE_DEPTH_MASK != 0
                    {
                        return bad("bad parameter");
                    }
                }
            }
            Payload::Var { spec, .. } => {
                if var_spec(*spec).is_none() {
                    return bad("bad variable specifiers");
                }
            }
            Payload::Enum { scoped, def, .. } => {
                if u32::from(*scoped) & !(EnumData::SCOPED | EnumData::SCOPED_STRUCT) != 0 {
                    return bad("bad enumeration flags");
                }
                if def.is_some() {
                    open_enum = Some(i);
                }
            }
            Payload::Enumerator { ty, .. } => {
                if ty.ty > BUILTINS || ty.quals != 0 {
                    return bad("bad enumerator type");
                }
            }
            Payload::Record { tag, def } => {
                if *tag > TagKind::Union as u8 {
                    return bad("bad class tag");
                }
                if let Some(df) = def {
                    let parent_end = stack.last().map_or(nd, |&(_, e)| e);
                    let end = df.end as usize;
                    if end <= i || end > parent_end || df.bits & !DEF_BITS != 0 {
                        return bad("bad class definition");
                    }
                    if def_end[chain[i]] != usize::MAX {
                        return bad("a class defined twice");
                    }
                    def_end[chain[i]] = end;
                    let complete = |x: u32| {
                        chain.get(x as usize).is_some_and(|&c| def_end[c] <= i)
                    };
                    for b in &df.bases {
                        if b.access > 3 || base_class(img, b.ty, &complete).is_none() {
                            return bad("bad base class");
                        }
                    }
                    stack.push((i, end));
                }
            }
            Payload::Field { flags, .. } => {
                if flags & !FIELD_FLAGS != 0 {
                    return bad("bad field flags");
                }
            }
            Payload::Implicit { kind } => {
                if *kind > 5 || d.prev != NONE || !implicit.insert((d.ctx, *kind)) {
                    return bad("bad implicit member");
                }
            }
            Payload::Namespace
            | Payload::Export
            | Payload::Typedef { .. }
            | Payload::Access
            | Payload::Label
            | Payload::Let { .. } => {}
        }
    }
    // The lookup table: exactly the named non-local records, sorted.
    let named = img.decls.iter().filter(|d| d.name != NONE && !body::is_local(img, d)).count();
    if lookup.len() != named || lookup.windows(2).any(|w| w[0] >= w[1]) {
        return bad("bad lookup table");
    }
    for &(c, n, d) in lookup {
        if !img.decls.get(d as usize).is_some_and(|r| r.ctx == c && r.name == n) {
            return bad("bad lookup table");
        }
    }
    let _ = nd;
    body::validate_bodies(img, &|q| qref_ok(q, nt))
}

// ---------------------------------------------------------------------------
// The writer
// ---------------------------------------------------------------------------

type W<T> = Result<T, &'static str>;

/// The header fields every record shares (see [`DeclRec`]).
struct Head {
    flags: u8,
    access: u8,
    name: u32,
    ctx: u32,
    lexical: u32,
    loc: u32,
    prev: u32,
}

struct Writer<'s, 'a, 'b> {
    s: &'s Sema<'a, 'b>,
    main: FileId,
    recs: FxHashMap<DeclId, u32>,
    names: FxHashMap<String, u32>,
    pnames: FxHashMap<String, u32>,
    tmemo: FxHashMap<TypeId, u32>,
    img: Image,
    bs: BodyState,
}

/// A reserved identifier ([lex.name]/3): `__x` or `_X`.
fn reserved(s: &str) -> bool {
    let b = s.as_bytes();
    b.first() == Some(&b'_') && b.get(1).is_some_and(|&c| c == b'_' || c.is_ascii_uppercase())
}

impl Writer<'_, '_, '_> {
    fn loc(&self, l: Loc) -> W<u32> {
        if !l.is_valid() {
            return Ok(NONE);
        }
        let sm = self.s.diags.sm();
        match sm.offset_of(l) {
            Some((f, o)) if f == self.main && sm.loc(f, o) == l && o < LEX_TU => Ok(o),
            _ => Err("a declaration outside the module's main file or in a macro expansion"),
        }
    }

    fn name_ix(map: &mut FxHashMap<String, u32>, list: &mut Vec<String>, s: &str) -> u32 {
        if let Some(&i) = map.get(s) {
            return i;
        }
        let i = list.len() as u32;
        list.push(s.to_owned());
        map.insert(s.to_owned(), i);
        i
    }

    fn ident(&self, n: DeclName) -> Option<String> {
        n.as_ident().map(|sym| self.s.ast.symbols().as_str(sym).to_owned())
    }

    fn qref(&mut self, q: QualType) -> W<QRef> {
        let ty = self.ty(q.ty())?;
        let quals = q.quals_bits();
        if quals != 0
            && ty > BUILTINS
            && matches!(
                self.img.types[(ty - BUILTINS - 1) as usize],
                TyRec::LRef(_)
                    | TyRec::RRef(_)
                    | TyRec::CArray(..)
                    | TyRec::IArray(_)
                    | TyRec::Func { .. }
            )
        {
            return Err("a qualified array, reference or function type");
        }
        Ok(QRef { ty, quals })
    }

    /// The record of `d` naming type `t` (its own type, unqualified).
    fn type_decl(&self, d: DeclId, t: TypeId) -> W<u32> {
        let r = self.recs.get(&d).copied().ok_or("a type declared outside the image")?;
        match self.s.ast.decls.get(d).ty() {
            Some(q) if q.ty() == t && q.quals_bits() == 0 => Ok(r),
            _ => Err("a type the image does not carry"),
        }
    }

    fn ty(&mut self, t: TypeId) -> W<u32> {
        if let Some(&i) = self.tmemo.get(&t) {
            return Ok(i);
        }
        let types = &self.s.ast.types;
        let rec = match types.view(t) {
            TypeView::Builtin(b) => {
                let i = b as u32;
                if i == 0 || i > BUILTINS || b.type_id() != t {
                    return Err("a builtin type the image does not carry");
                }
                return Ok(i);
            }
            TypeView::Pointer(q) => TyRec::Pointer(self.qref(q)?),
            TypeView::LValueRef(q) => TyRec::LRef(self.qref(q)?),
            TypeView::RValueRef(q) => TyRec::RRef(self.qref(q)?),
            TypeView::ConstantArray(a) => TyRec::CArray(self.qref(a.elem)?, a.size),
            TypeView::IncompleteArray(q) => TyRec::IArray(self.qref(q)?),
            TypeView::Paren(q) => TyRec::Paren(self.qref(q)?),
            TypeView::Elaborated { keyword, nns: None, named } => {
                TyRec::Elab(keyword as u8, self.qref(named)?)
            }
            TypeView::Function(p) => {
                let exc = match types.exc_spec(&p) {
                    ExcSpec::None => 0,
                    ExcSpec::BasicNoexcept => 1,
                    ExcSpec::DynamicNone => 2,
                    ExcSpec::Nothrow => 3,
                    _ => return Err("an exception specification the image does not carry"),
                };
                let ps: Vec<QualType> = types.params(&p).to_vec();
                let info = p.info().bits();
                let ret = self.qref(p.ret())?;
                let params = ps.into_iter().map(|q| self.qref(q)).collect::<W<Vec<_>>>()?;
                TyRec::Func { ret, params, info, exc }
            }
            TypeView::Enum(d) => TyRec::Enum(self.type_decl(d, t)?),
            TypeView::Record(d) => TyRec::Record(self.type_decl(self.s.ast.first(d), t)?),
            TypeView::Typedef { .. } if self.s.builtin_sugar().iter().any(|q| q.ty() == t) => {
                let k = self.s.builtin_sugar().iter().position(|q| q.ty() == t).unwrap_or(0);
                TyRec::Sugar(k as u8)
            }
            TypeView::Typedef { decl, .. } => TyRec::Typedef(self.type_decl(decl, t)?),
            TypeView::QualifiedArray { quals, base } => {
                let sugar = matches!(
                    types.view(base),
                    TypeView::Typedef { .. } | TypeView::Paren(_) | TypeView::Elaborated { .. }
                );
                if !sugar || quals.bits() == 0 {
                    return Err("a qualified array type the image does not carry");
                }
                TyRec::QArray(quals.bits(), self.qref(QualType::unqual(base))?)
            }
            _ => return Err("a type the image does not carry"),
        };
        let i = BUILTINS + 1 + self.img.types.len() as u32;
        self.img.types.push(rec);
        self.tmemo.insert(t, i);
        Ok(i)
    }

    /// The name word of `d`: a name index, [`NONE`], or a special name
    /// (constructor, destructor, conversion, operator) the importer
    /// recomputes from the class and the function type.
    fn name_word(&mut self, d: DeclId) -> W<u32> {
        use kiln_ast::names::DeclNameView as V;
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        let class_canon = || {
            n.ctx()
                .and_then(|c| ast.decls.get(c).ty())
                .map(|q| ast.types.canonical(q).ty())
        };
        Ok(match ast.names.view(n.name()) {
            V::Empty => NONE,
            V::Identifier(sym) => {
                let s = ast.symbols().as_str(sym).to_owned();
                if reserved(&s) || s == "std" || s == "main" {
                    return Err("a reserved or special name");
                }
                Self::name_ix(&mut self.names, &mut self.img.names, &s)
            }
            V::Operator(op) => N_OP + u32::from(op as u8),
            V::Constructor(t) if n.kind() == DeclKind::CXXConstructor && class_canon() == Some(t) => {
                N_CTOR
            }
            V::Destructor(t) if n.kind() == DeclKind::CXXDestructor && class_canon() == Some(t) => {
                N_DTOR
            }
            V::Conversion(q) if n.kind() == DeclKind::CXXConversion => {
                let ret = n.ty().and_then(|f| match ast.types.view(f.ty()) {
                    TypeView::Function(p) => Some(ast.types.canonical(p.ret())),
                    _ => None,
                });
                if ret != Some(q) {
                    return Err("a conversion function declared through a typedef");
                }
                N_CONV
            }
            _ => return Err("a declaration not named by an identifier"),
        })
    }

    /// Checks the header fields every record shares.
    fn head(&mut self, d: DeclId) -> W<Head> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        let extra = n.flags().bits() & !(DECL_FLAGS | DeclFlags::HAS_LEXICAL_CTX.bits());
        if extra != 0 {
            return Err("an implicit, invalid or attributed declaration");
        }
        let m = n.misc();
        if m.friend() != FriendKind::None {
            return Err("a friend declaration");
        }
        if m.local_extern()
            || m.name_independent()
            || m.module_ownership() != ModuleOwnership::Unowned
        {
            return Err("a declaration with local scope");
        }
        let access = m.access() as u8;
        let ctx = match n.ctx() {
            Some(c) if c == ast.tu => NONE,
            Some(c) => {
                self.recs.get(&c).copied().ok_or("a declaration in an unsupported context")?
            }
            None => return Err("a declaration in an unsupported context"),
        };
        let lexical = if n.has(DeclFlags::HAS_LEXICAL_CTX) {
            match ast.decls.lexical_ctx(d) {
                Some(c) if c == ast.tu => LEX_TU,
                Some(c) => self.recs.get(&c).copied().ok_or("an unsupported lexical context")?,
                None => return Err("an unsupported lexical context"),
            }
        } else {
            NONE
        };
        let loc = self.loc(n.loc())?;
        let prev = match ast.decls.redecl(d).and_then(|r| r.prev) {
            None => NONE,
            Some(p) => self
                .recs
                .get(&p)
                .copied()
                .ok_or("a redeclaration of an entity outside the image")?,
        };
        let flags = n.flags().bits() & DECL_FLAGS;
        let name = self.name_word(d)?;
        Ok(Head { flags, access, name, ctx, lexical, loc, prev })
    }

    #[allow(clippy::too_many_lines)] // one arm per record or type kind, kept together with the format
    /// The payload of record `i`, declaration `d`.
    fn record(&mut self, d: DeclId, i: u32) -> W<Payload> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        Ok(match n.kind() {
            DeclKind::Namespace => {
                let nd = ast.decls.namespace(d).ok_or("a malformed namespace")?;
                if nd.flags != 0 {
                    return Err("an inline or unnamed namespace");
                }
                Payload::Namespace
            }
            DeclKind::Export => Payload::Export,
            DeclKind::Function
            | DeclKind::CXXMethod
            | DeclKind::CXXConstructor
            | DeclKind::CXXDestructor
            | DeclKind::CXXConversion => self.function(d, i)?,
            DeclKind::Var => self.var_payload(d, i)?,
            DeclKind::Enum => {
                let ed = *ast.decls.enumeration(d).ok_or("a malformed enumeration")?;
                if !ed.templ.is_none() {
                    return Err("a template");
                }
                let defined = ast.definition(d) == Some(d);
                let complete = ed.flags & EnumData::COMPLETE != 0;
                if ed.flags & EnumData::BEING_DEFINED != 0 || complete != defined {
                    return Err("an opaque enumeration declaration");
                }
                let pair = |w: &mut Self| -> W<Option<(QRef, QRef)>> {
                    match (ed.int_type, ed.promotion_type) {
                        (Some(a), Some(b)) => Ok(Some((w.qref(a)?, w.qref(b)?))),
                        _ => Err("an enumeration without an underlying type"),
                    }
                };
                let fixed = if ed.is_fixed() { pair(self)? } else { None };
                let def = match (defined, ed.is_fixed()) {
                    (false, _) => None,
                    (true, true) => Some(None),
                    (true, false) => Some(pair(self)?),
                };
                let scoped = (ed.flags & (EnumData::SCOPED | EnumData::SCOPED_STRUCT)) as u8;
                Payload::Enum { scoped, fixed, def }
            }
            DeclKind::EnumConstant => {
                let ec = *ast.decls.enumerator(d).ok_or("a malformed enumerator")?;
                let parent = n.ctx().ok_or("a malformed enumerator")?;
                if n.ty() != ast.decls.get(parent).ty() {
                    return Err("an enumerator whose type is not its enumeration");
                }
                let v = ec.value.ok_or("an enumerator without a value")?;
                let Some(Scalar::Int { ty, signed, bits }) = ast.consts.scalar(v) else {
                    return Err("an enumerator value the image does not carry");
                };
                let value = if signed {
                    bits.to_i128()
                } else {
                    bits.to_u128().and_then(|u| i128::try_from(u).ok())
                }
                .ok_or("an enumerator value the image does not carry")?;
                let q = self.qref(ty)?;
                if q.ty > BUILTINS || q.quals != 0 {
                    return Err("an enumerator value the image does not carry");
                }
                Payload::Enumerator { ty: q, value }
            }
            DeclKind::Typedef | DeclKind::TypeAlias => {
                let td = *ast.decls.typedef(d).ok_or("a malformed typedef")?;
                if !td.templ.is_none() {
                    return Err("a template");
                }
                Payload::Typedef {
                    alias: n.kind() == DeclKind::TypeAlias,
                    underlying: self.qref(td.underlying)?,
                }
            }
            DeclKind::CXXRecord => self.class(d, i)?,
            DeclKind::Field => {
                let fd = *ast.decls.field(d).ok_or("a malformed field")?;
                if fd.flags & !FIELD_FLAGS != 0 {
                    return Err("a field with [[no_unique_address]]");
                }
                if n.name() == DeclName::EMPTY && fd.bit_width.is_none() {
                    return Err("an anonymous union or struct member");
                }
                // Patched by `walk_roots`.
                if let Some(e) = fd.bit_width {
                    self.bs.roots.push(Root::FieldWidth(i, e));
                }
                if let Some(e) = fd.init {
                    self.bs.roots.push(Root::FieldInit(i, e));
                }
                let ty = self.qref(n.ty().ok_or("a malformed field")?)?;
                Payload::Field { ty, flags: fd.flags, width: NONE, init: NONE }
            }
            DeclKind::AccessSpec => Payload::Access,
            _ => return Err("a declaration the image does not carry yet"),
        })
    }

    /// Rejects declarations whose cold side data the image does not carry.
    fn cold_gate(&self, d: DeclId) -> W<()> {
        let c = &self.s.ast.decls.cold;
        if c.friends.get(d).is_some_and(|&l| !self.s.ast.lists.is_empty(l)) {
            return Err("a friend declaration");
        }
        if c.overrides.contains(d) || c.key_function.contains(d) || c.dtor_delete.contains(d)
            || c.dtor_delete_this.contains(d)
        {
            return Err("a virtual function");
        }
        if c.inheriting_ctor.contains(d) || c.inherited_ctor.contains(d) {
            return Err("an inherited constructor");
        }
        if c.typedef_for_anon.contains(d) {
            return Err("a typedef naming an unnamed class");
        }
        if c.delete_message.contains(d)
            || c.explicit_expr.contains(d)
            || c.record_pack.contains(d)
            || c.requires_clause.contains(d)
            || c.contracts.contains(d)
            || c.asm_label.contains(d)
            || c.alignment.contains(d)
            || c.deprecation.contains(d)
            || c.instantiated_from.contains(d)
            || c.injection_point.contains(d)
            || c.conversion_spec_target.contains(d)
        {
            return Err("a declaration with side data the image does not carry");
        }
        Ok(())
    }

    /// A function record (namespace scope or a member), record `i`.
    #[allow(clippy::too_many_lines)] // the checks, the type, parameters, body and initializers
    fn function(&mut self, d: DeclId, i: u32) -> W<Payload> {
        let s = self.s;
        let ast = &s.ast;
        let n = ast.decls.get(d);
        let kind = FN_KINDS.iter().position(|&k| k == n.kind()).ok_or("a malformed function")?;
        let fd = *ast.decls.function(d).ok_or("a malformed function")?;
        if !fd.templ.is_none() {
            return Err("a template");
        }
        let spec = fd.spec;
        let bits = spec.bits();
        if spec.is_virtual() || spec.is_pure() || spec.is_override() || spec.is_final() {
            return Err("a virtual function");
        }
        if fn_spec(bits) != Some(spec) {
            return Err("a coroutine, contract-checked or implicitly defaulted function");
        }
        if ast.decls.builtin_id(d).is_some() || ast.decls.lib_builtin(d).is_some() {
            return Err("a builtin or library function");
        }
        if s.inh_ctor_base.contains_key(&d) {
            return Err("an inherited constructor");
        }
        if kind == 1
            && matches!(
                ast.names.view(n.name()),
                kiln_ast::names::DeclNameView::Operator(
                    OverloadedOperator::New
                        | OverloadedOperator::Delete
                        | OverloadedOperator::ArrayNew
                        | OverloadedOperator::ArrayDelete
                )
            )
        {
            return Err("a class-specific allocation function");
        }
        let first = ast.first(d) == d;
        // An explicitly defaulted special member on its first
        // declaration: the importer defines it again (its body and
        // ctor-initializers are synthesized, spec/modules.md §11.11).
        let defaulted = spec.defaulted() == Defaulted::Explicit;
        if defaulted && (!first || s.special_kind_of(d).is_none()) {
            return Err("a defaulted comparison or an out-of-line defaulted function");
        }
        let fty = n.ty().ok_or("a malformed function")?;
        let TypeView::Function(proto) = ast.types.view(fty.ty()) else {
            return Err("a function declared through a typedef");
        };
        let deferred = ast.types.exc_spec(&proto) == ExcSpec::Deferred(ast.first(d));
        let ty = if deferred {
            // The exception specification names the function itself:
            // build the type without it (the importer defers it again).
            let ps: Vec<QualType> = ast.types.params(&proto).to_vec();
            let info = proto.info().bits();
            let ret = self.qref(proto.ret())?;
            let params = ps.into_iter().map(|q| self.qref(q)).collect::<W<Vec<_>>>()?;
            let t = BUILTINS + 1 + self.img.types.len() as u32;
            self.img.types.push(TyRec::Func { ret, params, info, exc: 0 });
            QRef { ty: t, quals: fty.quals_bits() }
        } else {
            self.qref(fty)?
        };
        if ty.quals != 0 {
            return Err("a function declared through a typedef");
        }
        let plist: Vec<DeclId> = ast.lists.iter::<DeclId>(fd.params).collect();
        let mut params = Vec::with_capacity(plist.len());
        for p in plist {
            let g = self.bs.param_pos.len() as u32;
            self.bs.param_pos.push((i, params.len() as u32));
            if self.bs.params.insert(p, g).is_some() {
                return Err("a parameter shared between declarations");
            }
            params.push(self.param(p, g)?);
        }
        let nproto = match self.img.types.get((ty.ty - BUILTINS - 1) as usize) {
            Some(TyRec::Func { params, .. }) => params.len(),
            _ => usize::MAX,
        };
        if nproto != params.len() {
            return Err("a function whose parameters do not match its type");
        }
        let mut inits = Vec::new();
        let body = match fd.body {
            _ if defaulted => FnBody::None,
            None => FnBody::None,
            Some(b) => {
                let st = &s.ast.stmts;
                if st.get(b).kind() != StmtKind::CompoundStmt {
                    return Err("a function-try-block");
                }
                if Self::carries_body(spec) {
                    // Patched by `walk_roots`.
                    self.bs.roots.push(Root::Body(i, b));
                    if kind == 2 {
                        inits = self.ctor_inits(d, i)?;
                    }
                    FnBody::None
                } else {
                    // A non-inline body (a member defined in a named
                    // module's class is not implicitly inline,
                    // [class.mfct]/1) is emitted by the module's own object;
                    // importers only need to know it is defined.
                    FnBody::Braces(self.loc(st.get(b).loc())?, self.loc(st.rbrace(b))?)
                }
            }
        };
        if inits.is_empty()
            && !defaulted
            && !matches!(body, FnBody::Braces(..))
            && ast.decls.ctor_inits(d, &ast.lists).len() != 0 {
            return Err("ctor-initializers of a constructor without a carried body");
        }
        Ok(Payload::Function { kind: kind as u8, ty, spec: bits, deferred, params, body, inits })
    }

    /// The ctor-initializers of constructor `d`, record `i`.
    fn ctor_inits(&mut self, d: DeclId, i: u32) -> W<Vec<InitRec>> {
        let ast = &self.s.ast;
        let class = ast.decls.get(d).ctx().ok_or("a malformed constructor")?;
        let bases: Vec<u32> =
            ast.decls.record(class).map(|r| ast.lists.get(r.bases).to_vec()).unwrap_or_default();
        let list: Vec<kiln_ast::decl::CtorInit> =
            ast.decls.ctor_inits(d, &ast.lists).copied().collect();
        let mut out = Vec::with_capacity(list.len());
        for (k, x) in list.into_iter().enumerate() {
            let target = match x.kind {
                CtorInitKind::Base => {
                    let pos = bases.iter().position(|&b| {
                        let q = ast.decls.base_spec(b).ty;
                        ast.types.canonical(q).ty().index() == x.target
                    });
                    pos.ok_or("a ctor-initializer of an indirect base")? as u32
                }
                CtorInitKind::Field => {
                    let f = DeclId::try_new(x.target).ok_or("a malformed ctor-initializer")?;
                    if ast.decls.get(f).ctx() != Some(class) {
                        return Err("a ctor-initializer of an anonymous union member");
                    }
                    *self.recs.get(&f).ok_or("a ctor-initializer of a field outside the image")?
                }
                CtorInitKind::Delegating => 0,
                CtorInitKind::VirtualBase => return Err("a virtual base class"),
                CtorInitKind::IndirectField => return Err("an anonymous union member"),
            };
            // Patched by `walk_roots`.
            self.bs.roots.push(Root::CtorInit(i, k as u32, x.init));
            out.push(InitRec {
                kind: x.kind as u8,
                target,
                written: x.written_index,
                init: NONE,
                loc: self.loc(x.loc)?,
            });
        }
        Ok(out)
    }

    /// Ends the definition of class record `r` before the next record.
    fn close_class(&mut self, r: u32) {
        let end = self.img.decls.len() as u32;
        if let Payload::Record { def: Some(df), .. } = &mut self.img.decls[r as usize].payload {
            df.end = end;
        }
    }

    /// A class record (`CXXRecord`), record `i`. A definition's `end` is
    /// patched by `collect` when the class closes.
    fn class(&mut self, d: DeclId, i: u32) -> W<Payload> {
        let ast = &self.s.ast;
        let rd = ast.decls.record(d).ok_or("a malformed class")?;
        if !rd.templ.is_none() || rd.lambda.is_some() {
            return Err("a template or closure type");
        }
        self.cold_gate(d)?;
        let tag = rd.tag as u8;
        if ast.definition(d) != Some(d) {
            return Ok(Payload::Record { tag, def: None });
        }
        let b = rd.defn;
        if !b.contains(DefinitionBits::COMPLETE) || b.contains(DefinitionBits::BEING_DEFINED) {
            return Err("an incomplete class definition");
        }
        if b.contains(DefinitionBits::POLYMORPHIC) {
            return Err("a polymorphic class");
        }
        if b.contains(DefinitionBits::HAS_VIRTUAL_BASES) {
            return Err("a virtual base class");
        }
        if b.contains(DefinitionBits::HAS_INHERITED_CTOR)
            || b.contains(DefinitionBits::HAS_INHERITED_ASSIGN)
        {
            return Err("an inherited constructor or assignment operator");
        }
        if b.contains(DefinitionBits::ANONYMOUS) {
            return Err("an anonymous union or struct member");
        }
        if crate::class::SpecialKind::any_needs_overload(b) {
            return Err("a class whose implicit special members need overload resolution");
        }
        let bits = b.without(DefinitionBits::COMPLETE).bits();
        if bits & !DEF_BITS != 0 {
            return Err("a class with properties the image does not carry yet");
        }
        let list: Vec<u32> = ast.lists.get(rd.bases).to_vec();
        let mut bases = Vec::with_capacity(list.len());
        for (pos, g) in list.into_iter().enumerate() {
            let bs = *ast.decls.base_spec(g);
            if bs.is_virtual {
                return Err("a virtual base class");
            }
            if bs.pack_expansion {
                return Err("a pack expansion");
            }
            let _ = self.bs.bspec.insert(g, (i, pos as u32));
            bases.push(BaseRec { ty: self.qref(bs.ty)?, loc: self.loc(bs.loc)?, access: bs.access as u8 });
        }
        Ok(Payload::Record { tag, def: Some(RecDef { bits, end: NONE, bases }) })
    }

    /// Parameter `p`, the image's parameter `g`.
    fn param(&mut self, p: DeclId, g: u32) -> W<ParamRec> {
        let ast = &self.s.ast;
        let pd = *ast.decls.param(p).ok_or("a malformed parameter")?;
        let n = ast.decls.get(p);
        if pd.original_ty.is_some() || pd.flags & !ParamData::SCOPE_DEPTH_MASK != 0 {
            return Err("a parameter with an adjusted type or a pack");
        }
        if let Some(e) = pd.default_arg {
            // Patched by `walk_roots`.
            self.bs.roots.push(Root::Default(g, e));
        }
        if n.flags().bits() & !(DECL_FLAGS | DeclFlags::HAS_LEXICAL_CTX.bits()) != 0
            || n.misc().name_independent()
        {
            return Err("an implicit, invalid or attributed parameter");
        }
        let name = match n.name() {
            x if x == DeclName::EMPTY => NONE,
            x => {
                let s = self.ident(x).ok_or("a parameter not named by an identifier")?;
                Self::name_ix(&mut self.pnames, &mut self.img.pnames, &s)
            }
        };
        let loc = self.loc(n.loc())?;
        let ty = self.qref(n.ty().ok_or("a malformed parameter")?)?;
        Ok(ParamRec {
            name,
            loc,
            ty,
            pflags: pd.flags,
            flags: n.flags().bits() & DECL_FLAGS,
            default: NONE,
        })
    }

    /// Whether `d` lies inside a function (a local declaration: it gets a
    /// record only when a carried body names it).
    fn local(&self, d: DeclId) -> bool {
        let decls = &self.s.ast.decls;
        let mut c = decls.get(d).ctx();
        let mut steps = 0u32;
        while let Some(x) = c {
            if decls.function(x).is_some() {
                return true;
            }
            steps += 1;
            if steps > 4096 {
                return false;
            }
            c = decls.get(x).ctx();
        }
        false
    }

    fn collect(&mut self) -> W<()> {
        let s = self.s;
        let ast = &s.ast;
        let mt = &ast.modules;
        let sm = s.diags.sm();
        let unit = mt.units.first().ok_or("no module unit")?;
        if mt.units.len() != 1 || !unit.imports.is_empty() {
            return Err("an import");
        }
        if unit.kind != kiln_ast::modules::UnitKind::Interface || unit.partition.is_some() {
            return Err("a unit that is not a primary module interface");
        }
        self.img.module = unit.module.clone().ok_or("no module declaration")?;
        let (kw, kw_loc, private) = s.module_main_frame().ok_or("no module declaration")?;
        if private {
            return Err("a private module fragment");
        }
        self.img.kw = self.loc(kw)?;
        self.img.name_loc = self.loc(kw_loc)?;
        if self.img.kw == NONE || self.img.name_loc == NONE {
            return Err("no module declaration");
        }
        self.img.main = sm.file_name(self.main).to_owned();
        let mut open_enum: Option<u32> = None;
        // The open class definitions, innermost last; the implicitly
        // declared special members, recorded after every class closed.
        let mut open: Vec<u32> = Vec::new();
        let mut implicit: Vec<(DeclId, u32, u8)> = Vec::new();
        for d in ast.decls.ids() {
            let n = ast.decls.get(d);
            let o = mt.owner(d);
            if !o.purview {
                // Predefined declarations have no location in a real
                // file; anything else is a global module fragment.
                let real = sm
                    .offset_of(sm.expansion_loc(n.loc()))
                    .is_some_and(|(f, _)| !crate::bmi_ast::unstable_file(sm.file_name(f)));
                if real && !n.has(DeclFlags::IMPLICIT) {
                    return Err("a global module fragment");
                }
                continue;
            }
            if n.kind() == DeclKind::ParmVar || self.local(d) {
                continue;
            }
            if n.kind() == DeclKind::TranslationUnit {
                continue;
            }
            if n.kind() == DeclKind::StaticAssert {
                // Checked when the module was compiled; nothing to import.
                continue;
            }
            if n.has(DeclFlags::IMPLICIT) {
                // An implicitly declared special member: the importer
                // declares it again (spec/modules.md §11.11 "Classes").
                if n.kind() == DeclKind::IndirectField {
                    return Err("an anonymous union or struct member");
                }
                let k = s.special_kind_of(d).ok_or("an implicit declaration")?;
                let implicit_default = ast
                    .decls
                    .function(d)
                    .is_some_and(|f| f.spec.defaulted() == Defaulted::Implicit);
                let class = n.ctx().and_then(|c| self.recs.get(&c).copied());
                match class {
                    Some(c) if implicit_default && n.flags().bits() & !DECL_FLAGS == DeclFlags::IMPLICIT.bits() => {
                        implicit.push((d, c, k as u8));
                    }
                    _ => return Err("an implicit declaration"),
                }
                continue;
            }
            kind_gate(n.kind())?;
            self.cold_gate(d)?;
            let h = self.head(d)?;
            let i = self.img.decls.len() as u32;
            let payload = self.record(d, i)?;
            match &payload {
                Payload::Enumerator { .. } => {
                    if open_enum != Some(h.ctx) {
                        return Err("an enumerator outside its enumeration's definition");
                    }
                }
                Payload::Enum { def: Some(_), .. } => open_enum = Some(i),
                _ => open_enum = None,
            }
            if !matches!(payload, Payload::Enumerator { .. }) {
                // Close the classes this declaration is not a member of.
                while let Some(&top) = open.last() {
                    if h.lexical == NONE && h.ctx == top {
                        break;
                    }
                    self.close_class(top);
                    let _ = open.pop();
                }
                let class_ctx = h.ctx != NONE
                    && matches!(self.img.decls[h.ctx as usize].payload, Payload::Record { .. });
                if class_ctx && open.is_empty() && (h.lexical == NONE || h.prev == NONE) {
                    return Err("a class member outside its class definition");
                }
                if h.ctx != NONE
                    && !matches!(
                        self.img.decls[h.ctx as usize].payload,
                        Payload::Namespace | Payload::Export | Payload::Record { .. }
                    )
                {
                    return Err("a declaration in an unsupported context");
                }
            }
            let class_def = matches!(payload, Payload::Record { def: Some(_), .. });
            let owner = u8::from(o.purview) | (u8::from(o.exported) << 1);
            self.img.decls.push(DeclRec {
                flags: h.flags,
                owner,
                access: h.access,
                name: h.name,
                ctx: h.ctx,
                lexical: h.lexical,
                loc: h.loc,
                prev: h.prev,
                payload,
            });
            self.recs.insert(d, i);
            if class_def {
                open.push(i);
            }
        }
        while let Some(top) = open.pop() {
            self.close_class(top);
        }
        for (d, ctx, kind) in implicit {
            let o = mt.owner(d);
            let i = self.img.decls.len() as u32;
            self.img.decls.push(DeclRec {
                flags: ast.decls.get(d).flags().bits() & DECL_FLAGS,
                owner: u8::from(o.purview) | (u8::from(o.exported) << 1),
                access: 0,
                name: NONE,
                ctx,
                lexical: NONE,
                loc: NONE,
                prev: NONE,
                payload: Payload::Implicit { kind },
            });
            self.recs.insert(d, i);
        }
        self.walk_roots()
    }
}

/// The declaration kinds the image carries; the fallback reason for the
/// others.
fn kind_gate(k: DeclKind) -> W<()> {
    use DeclKind as K;
    match k {
        K::Namespace
        | K::Export
        | K::Function
        | K::Var
        | K::Enum
        | K::EnumConstant
        | K::Typedef
        | K::TypeAlias
        | K::CXXRecord
        | K::Field
        | K::AccessSpec
        | K::CXXMethod
        | K::CXXConstructor
        | K::CXXDestructor
        | K::CXXConversion => Ok(()),
        K::IndirectField => Err("an anonymous union or struct member"),
        K::Friend => Err("a friend declaration"),
        K::ClassTemplateSpecialization | K::ClassTemplatePartialSpecialization => {
            Err("a template")
        }
        K::ClassTemplate
        | K::FunctionTemplate
        | K::VarTemplate
        | K::TypeAliasTemplate
        | K::Concept
        | K::VarTemplateSpecialization
        | K::VarTemplatePartialSpecialization
        | K::FriendTemplate
        | K::CXXDeductionGuide
        | K::TemplateTypeParm
        | K::NonTypeTemplateParm
        | K::TemplateTemplateParm => Err("a template"),
        K::NamespaceAlias
        | K::UsingDirective
        | K::Using
        | K::UsingEnum
        | K::UsingShadow
        | K::ConstructorUsingShadow
        | K::UsingPack => Err("a using-declaration, using-directive or namespace alias"),
        K::LinkageSpec => Err("a linkage specification"),
        _ => Err("a declaration the image does not carry yet"),
    }
}

impl Sema<'_, '_> {
    /// Records the main unit's declaration image for a kind-2 module file
    /// (spec/modules.md §11.11 "Part 2a"); set by the driver for
    /// `-fkiln-bmi-image`.
    pub fn record_module_image(&mut self) {
        self.bmi.record_image = true;
    }

    /// The main unit's declaration image ([`Sema::record_module_image`]),
    /// once: the section bytes, or why the unit has none.
    pub fn take_module_image(&mut self) -> Option<ModuleImage> {
        if !std::mem::take(&mut self.bmi.record_image) {
            return None;
        }
        let none = |why| Some(ModuleImage { bytes: Vec::new(), decls: 0, fallback: Some(why) });
        let sm = self.diags.sm();
        let Some((_, kw_loc, _)) = self.module_main_frame() else {
            return none("no module declaration");
        };
        let Some((main, _)) = sm.offset_of(kw_loc) else {
            return none("a module declaration outside a file");
        };
        let mut w = Writer {
            s: self,
            main,
            recs: FxHashMap::default(),
            names: FxHashMap::default(),
            pnames: FxHashMap::default(),
            tmemo: FxHashMap::default(),
            img: Image::default(),
            bs: BodyState::default(),
        };
        if let Err(why) = w.collect() {
            return none(why);
        }
        let decls = w.img.decls.len();
        let bytes = encode(&w.img);
        // The writer's own output must pass the reader's checks (a body
        // shape the schema and the AST disagree on is a fallback, never
        // a corrupt module file).
        if let Err(e) = decode(&bytes) {
            eprintln!("DEBUG1065 {e:?}");
            return none("an internal image inconsistency");
        }
        Some(ModuleImage { bytes, decls, fallback: None })
    }

    /// Builds the unit just begun from the declaration image `bytes` of
    /// module file `path`, whose main file the preprocessor loaded as
    /// `main` (spec/modules.md §11.11 "Part 2a"). A malformed image is
    /// the module file's corrupt-file fatal error at `at`, and nothing is
    /// built.
    pub fn load_module_image(&mut self, path: &str, bytes: &[u8], main: FileId, at: Loc) {
        let len = self.diags.sm().text(main).len();
        let checked = decode(bytes).and_then(|img| {
            let off_ok = |o: u32| o == NONE || (o as usize) <= len;
            let mut offs = vec![img.kw, img.name_loc];
            for d in &img.decls {
                offs.push(d.loc);
                match &d.payload {
                    Payload::Function { params, body, inits, .. } => {
                        offs.extend(params.iter().map(|p| p.loc));
                        offs.extend(inits.iter().map(|x| x.loc));
                        if let FnBody::Braces(l, r) = body {
                            offs.extend([*l, *r]);
                        }
                    }
                    Payload::Record { def: Some(df), .. } => {
                        offs.extend(df.bases.iter().map(|b| b.loc));
                    }
                    _ => {}
                }
            }
            body::body_locs(&img, &mut offs);
            if img.kw == NONE || img.name_loc == NONE || !offs.into_iter().all(off_ok) {
                return bad("an offset past the end of the module's source");
            }
            Ok(img)
        });
        match checked {
            Ok(img) => self.build_image(&img, main),
            Err(ImageError(why)) => {
                kiln_diag::Diag::new(
                    kiln_diag::DiagId::pp_module_file_corrupt,
                    kiln_source::Span::new(at, at),
                )
                .arg(path)
                .arg(why)
                .emit(self.diags);
            }
        }
    }

    #[allow(clippy::too_many_lines)] // one arm per record or type kind, kept together with the format
    fn build_image(&mut self, img: &Image, main: FileId) {
        let lo =
            |s: &Self, o: u32| if o == NONE { Loc::INVALID } else { s.diags.sm().loc(main, o) };
        let (kw, name_loc) = (lo(self, img.kw), lo(self, img.name_loc));
        self.act_on_module_directive(
            kw,
            true,
            Directive::Module { name: &img.module, partition: None, kw_loc: name_loc },
        );
        let unit = self.module_current_unit();
        let names: Vec<DeclName> =
            img.names.iter().map(|n| DeclName::ident(self.intern(n))).collect();
        let pnames: Vec<DeclName> =
            img.pnames.iter().map(|n| DeclName::ident(self.intern(n))).collect();
        let mut ids: Vec<DeclId> = Vec::with_capacity(img.decls.len());
        let mut tys: Vec<QualType> = Vec::with_capacity(img.types.len());
        let mut open: Option<(DeclId, Option<(QRef, QRef)>)> = None;
        // Parameters in image order; what is attached once the bodies
        // are built: function bodies, variable initializers and values.
        let mut params_all: Vec<DeclId> = Vec::new();
        let mut fbodies: Vec<(DeclId, u32)> = Vec::new();
        let mut inits: Vec<(DeclId, u32, u32)> = Vec::new();
        // Open class definitions (class, end, frozen bits); fields with a
        // width or initializer; constructors with ctor-initializers;
        // defaulted and implicit members to define again once used.
        let mut classes: Vec<(DeclId, usize, u64)> = Vec::new();
        let mut fields: Vec<(DeclId, u32, u32)> = Vec::new();
        let mut ctors: Vec<(DeclId, usize)> = Vec::new();
        let mut defaulted: Vec<(DeclId, u8)> = Vec::new();
        for (i, r) in img.decls.iter().enumerate() {
            if !matches!(r.payload, Payload::Enumerator { .. })
                && let Some((e, computed)) = open.take()
            {
                self.image_complete_enum(img, &mut tys, &ids, e, computed);
            }
            while let Some(&(c, end, bits)) = classes.last()
                && end <= i
            {
                classes.pop();
                self.builder().complete_record_definition(c, DefinitionBits::from_bits(bits));
            }
            let owner = Owner { unit, purview: r.owner & 1 != 0, exported: r.owner & 2 != 0 };
            let next = self.ast.decls.next_id();
            self.ast.modules.set_owner_from(next, owner);
            let at = |x: u32| if x == NONE { None } else { ids.get(x as usize).copied() };
            let ctx = at(r.ctx).unwrap_or(self.ast.tu);
            let name = match r.name {
                NONE => DeclName::EMPTY,
                // Locals name `pnames` (they are not gated, "ODR merging").
                n if body::is_local(img, r) => pnames[n as usize],
                // A special member's name: recomputed below.
                n if n >= N_OP => DeclName::EMPTY,
                n => names[n as usize],
            };
            let mut h = DeclHead::new(ctx, name, lo(self, r.loc));
            if r.flags & DeclFlags::IMPLICIT.bits() != 0 {
                h.flags = DeclFlags::IMPLICIT;
            }
            if r.access != 0 {
                h = h.access(access_of(r.access));
            }
            h.lexical = match r.lexical {
                NONE => None,
                LEX_TU => Some(self.ast.tu),
                x => at(x),
            };
            let prev = at(r.prev);
            let d = match &r.payload {
                Payload::Namespace => self.builder().namespace(h, false, prev),
                Payload::Export => self.builder().export_decl(ctx, h.loc),
                Payload::Function { kind, ty, spec, deferred, params, body, inits: ci } => {
                    let mut fty = self.image_ty(img, &mut tys, &ids, *ty);
                    let spec = fn_spec(*spec).unwrap_or_default();
                    let kind = FN_KINDS[usize::from(*kind)];
                    h.name = self.image_special_name(r.name, kind, ctx, fty).unwrap_or(h.name);
                    if *deferred {
                        // The exception specification names the function
                        // itself (its first declaration).
                        let next = self.ast.decls.next_id();
                        fty = self.with_exc_spec(fty, ExcSpec::Deferred(prev.map_or(next, |p| self.ast.first(p))));
                    }
                    let f = self.builder().function(h, kind, fty, spec, prev);
                    if !ci.is_empty() {
                        ctors.push((f, i));
                    }
                    if spec.defaulted() == Defaulted::Explicit {
                        defaulted.push((f, r.flags));
                    }
                    let mut ps = Vec::with_capacity(params.len());
                    for (k, p) in params.iter().enumerate() {
                        let pty = self.image_ty(img, &mut tys, &ids, p.ty);
                        let pname =
                            if p.name == NONE { DeclName::EMPTY } else { pnames[p.name as usize] };
                        let ploc = lo(self, p.loc);
                        let index = u16::try_from(k).unwrap_or(u16::MAX);
                        let pd = self.builder().param(f, pname, ploc, pty, index, p.pflags);
                        image_flags(self, pd, p.flags);
                        ps.push(pd);
                    }
                    if !ps.is_empty() {
                        let _ = self.builder().set_params(f, &ps);
                    }
                    params_all.extend_from_slice(&ps);
                    match *body {
                        FnBody::None => {}
                        FnBody::Braces(lb, rb) => {
                            let (lb, rb) = (lo(self, lb), lo(self, rb));
                            let b = self.builder().compound(&[], lb, rb);
                            let _ = self.builder().set_body(f, b);
                        }
                        FnBody::Stmt(s) => fbodies.push((f, s)),
                    }
                    f
                }
                Payload::Var { ty, spec, init, eval } => {
                    let vty = self.image_ty(img, &mut tys, &ids, *ty);
                    let spec = var_spec(*spec).unwrap_or_default();
                    let v = self.builder().var(h, vty, spec, prev);
                    inits.push((v, *init, *eval));
                    v
                }
                Payload::Label => self.image_label(ctx, name, h.loc),
                Payload::Let { ty, extending, index } => {
                    let lty = self.image_ty(img, &mut tys, &ids, *ty);
                    let ext = ids[*extending as usize];
                    self.builder().lifetime_extended_temporary(ext, *index, lty, h.loc)
                }
                Payload::Enum { scoped, fixed, def } => {
                    let fixed = fixed.map(|(a, b)| {
                        (
                            self.image_ty(img, &mut tys, &ids, a),
                            self.image_ty(img, &mut tys, &ids, b),
                        )
                    });
                    let e = self.builder().enumeration(h, u32::from(*scoped), fixed, prev);
                    if let Some(computed) = def {
                        let _ = self.builder().begin_enum_definition(e);
                        open = Some((e, *computed));
                    }
                    e
                }
                Payload::Enumerator { ty, value } => {
                    let vty = self.image_ty(img, &mut tys, &ids, *ty);
                    let d = self.builder().enumerator(h, None);
                    let cv = self.int_const(vty, *value);
                    let _ = self.ast.decls.set_enumerator_value(d, cv);
                    d
                }
                Payload::Typedef { alias, underlying } => {
                    let u = self.image_ty(img, &mut tys, &ids, *underlying);
                    let kind = if *alias { DeclKind::TypeAlias } else { DeclKind::Typedef };
                    self.builder().typedef(h, kind, u, prev)
                }
                Payload::Record { tag, def } => {
                    let tag = tag_of(*tag);
                    let c = self.builder().record(h, tag, prev);
                    if let Some(df) = def {
                        let _ = self.builder().begin_record_definition(c);
                        let bases: Vec<kiln_ast::decl::BaseSpec> = df
                            .bases
                            .iter()
                            .map(|b| kiln_ast::decl::BaseSpec {
                                ty: self.image_ty(img, &mut tys, &ids, b.ty),
                                loc: lo(self, b.loc),
                                is_virtual: false,
                                access: access_of(b.access),
                                pack_expansion: false,
                            })
                            .collect();
                        let _ = self.ast.decls.set_bases(c, &bases, &mut self.ast.lists);
                        classes.push((c, df.end as usize, df.bits));
                    }
                    c
                }
                Payload::Field { ty, flags, width, init } => {
                    let fty = self.image_ty(img, &mut tys, &ids, *ty);
                    let f = self.builder().field(h, fty, None, *flags);
                    if *init != NONE {
                        let _ = self.special.nsdmi.insert(f);
                    }
                    if *width != NONE || *init != NONE {
                        fields.push((f, *width, *init));
                    }
                    f
                }
                Payload::Access => {
                    let a = access_of(r.access);
                    self.builder().access_spec(ctx, a, h.loc)
                }
                Payload::Implicit { kind } => {
                    let k = crate::class::SpecialKind::ALL[usize::from(*kind).min(5)];
                    let f = self.declare_implicit(ctx, k);
                    defaulted.push((f, r.flags));
                    f
                }
            };
            image_flags(self, d, r.flags);
            debug_assert_eq!(ids.len(), i);
            ids.push(d);
        }
        if let Some((e, computed)) = open.take() {
            self.image_complete_enum(img, &mut tys, &ids, e, computed);
        }
        while let Some((c, _, bits)) = classes.pop() {
            self.builder().complete_record_definition(c, DefinitionBits::from_bits(bits));
        }
        let built = self.image_bodies(img, &mut tys, &ids, &params_all, main);
        for (f, width, init) in fields {
            if width != NONE {
                let _ = self.ast.decls.set_field_bit_width(f, built.expr(width));
            }
            if init != NONE {
                let _ = self.ast.decls.set_field_init(f, built.expr(init));
            }
        }
        for (f, i) in ctors {
            let Payload::Function { inits: ci, .. } = &img.decls[i].payload else { continue };
            let class = self.ast.decls.get(f).ctx();
            let bases: Vec<u32> = class
                .and_then(|c| self.ast.decls.record(c))
                .map(|r| self.ast.lists.get(r.bases).to_vec())
                .unwrap_or_default();
            let list: Vec<kiln_ast::decl::CtorInit> = ci
                .iter()
                .map(|x| {
                    let kind = init_kind_of(x.kind);
                    let target = match kind {
                        CtorInitKind::Base => bases.get(x.target as usize).map_or(0, |&g| {
                            let q = self.ast.decls.base_spec(g).ty;
                            q.ty().index()
                        }),
                        CtorInitKind::Field => ids[x.target as usize].index(),
                        _ => 0,
                    };
                    kiln_ast::decl::CtorInit {
                        target,
                        kind,
                        written_index: x.written,
                        init: built.expr(x.init),
                        loc: lo(self, x.loc),
                    }
                })
                .collect();
            let _ = self.builder().set_ctor_inits(f, &list);
        }
        for (f, s) in fbodies {
            let _ = self.builder().set_body(f, built.stmt(s));
        }
        for (v, init, eval) in inits {
            if init != NONE {
                let _ = self.builder().set_var_init(v, built.expr(init));
            }
            if eval != NONE && self.ast.decls.begin_eval(v).is_ok() {
                let value = (eval != body::NOT_CONST).then(|| built.consts[eval as usize]);
                self.ast.decls.finish_eval(v, value);
            }
        }
        for (g, p) in img
            .decls
            .iter()
            .flat_map(|d| match &d.payload {
                Payload::Function { params, .. } => params.as_slice(),
                _ => &[],
            })
            .enumerate()
        {
            if p.default != NONE {
                let _ = self.ast.decls.set_default_arg(params_all[g], built.expr(p.default));
            }
        }
        // Defaulted and implicit special members the module used are
        // defined again (spec/modules.md §11.11 "Classes").
        for (f, flags) in defaulted {
            let loc = self.ast.decls.get(f).ctx().map_or(Loc::INVALID, |c| self.ast.decls.get(c).loc());
            if flags & DeclFlags::USED.bits() != 0 {
                self.mark_function_referenced(f, loc);
            } else if flags & DeclFlags::REFERENCED.bits() != 0 {
                self.exc_spec_needed(f, loc);
            }
        }
        self.module_owner_changed();
    }

    /// The constructor, destructor or conversion function name of a
    /// function of kind `kind` in class `ctx` with type `fty`, or an
    /// operator's name, for the special name words; `None` otherwise.
    fn image_special_name(
        &mut self,
        word: u32,
        kind: DeclKind,
        ctx: DeclId,
        fty: QualType,
    ) -> Option<DeclName> {
        let class = || self.ast.decls.get(ctx).ty().map(|q| self.ast.types.canonical(q).ty());
        match word {
            N_CTOR if kind == DeclKind::CXXConstructor => Some(self.ast.names.constructor(class()?)),
            N_DTOR if kind == DeclKind::CXXDestructor => Some(self.ast.names.destructor(class()?)),
            N_CONV if kind == DeclKind::CXXConversion => {
                let ret = match self.ast.types.view(fty.ty()) {
                    TypeView::Function(p) => p.ret(),
                    _ => return None,
                };
                let ret = self.ast.types.canonical(ret);
                Some(self.ast.names.conversion(ret))
            }
            n if (N_OP..N_CTOR).contains(&n) => {
                let op = OverloadedOperator::from_u8(u8::try_from(n - N_OP).ok()?)?;
                Some(DeclName::operator(op))
            }
            _ => None,
        }
    }

    fn image_complete_enum(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        e: DeclId,
        computed: Option<(QRef, QRef)>,
    ) {
        let computed = computed
            .map(|(a, b)| (self.image_ty(img, tys, ids, a), self.image_ty(img, tys, ids, b)));
        let _ = self.builder().complete_enum_definition(e, computed);
    }

    /// The type `q` names, building image types in order up to it (each
    /// refers only to earlier ones and to records already built, which
    /// [`decode`] checked).
    fn image_ty(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        q: QRef,
    ) -> QualType {
        let base = if q.ty <= BUILTINS {
            BuiltinType::from_index(q.ty).map_or(QualType::INT, BuiltinType::qual_type)
        } else {
            let j = (q.ty - BUILTINS - 1) as usize;
            while tys.len() <= j {
                let t = self.image_build_ty(&img.types[tys.len()], tys, ids);
                tys.push(t);
            }
            tys[j]
        };
        if q.quals == 0 { base } else { self.ast.types.qualify(base, Quals::from_bits(q.quals)) }
    }

    fn image_build_ty(&mut self, t: &TyRec, tys: &[QualType], ids: &[DeclId]) -> QualType {
        let qt = |s: &mut Self, q: QRef| -> QualType {
            let base = if q.ty <= BUILTINS {
                BuiltinType::from_index(q.ty).map_or(QualType::INT, BuiltinType::qual_type)
            } else {
                tys[(q.ty - BUILTINS - 1) as usize]
            };
            if q.quals == 0 { base } else { s.ast.types.qualify(base, Quals::from_bits(q.quals)) }
        };
        let decl_ty = |s: &Self, d: u32| {
            ids.get(d as usize).and_then(|&x| s.ast.decls.get(x).ty()).unwrap_or(QualType::INT)
        };
        match t {
            TyRec::Pointer(q) => {
                let q = qt(self, *q);
                self.ast.types.pointer_to(q)
            }
            TyRec::LRef(q) => {
                let q = qt(self, *q);
                self.ast.types.lvalue_ref_to(q)
            }
            TyRec::RRef(q) => {
                let q = qt(self, *q);
                self.ast.types.rvalue_ref_to(q)
            }
            TyRec::CArray(q, n) => {
                let q = qt(self, *q);
                self.ast.types.constant_array(q, *n, None)
            }
            TyRec::IArray(q) => {
                let q = qt(self, *q);
                self.ast.types.incomplete_array(q)
            }
            TyRec::Paren(q) => {
                let q = qt(self, *q);
                self.ast.types.paren_type(q)
            }
            TyRec::Elab(k, q) => {
                let q = qt(self, *q);
                let k = tag_keyword(*k).unwrap_or_default();
                self.ast.types.elaborated(k, None, q)
            }
            TyRec::Func { ret, params, info, exc } => {
                let r = qt(self, *ret);
                let ps: Vec<QualType> = params.iter().map(|&p| qt(self, p)).collect();
                let exc = exc_spec(*exc).unwrap_or_default();
                self.ast.types.function(r, &ps, FuncInfo::from_bits(*info), exc)
            }
            TyRec::Enum(d) | TyRec::Typedef(d) | TyRec::Record(d) => decl_ty(self, *d),
            TyRec::QArray(c, q) => {
                let q = qt(self, *q);
                self.ast.types.qualify(q, Quals::from_bits(*c))
            }
            TyRec::Sugar(k) => self.builtin_sugar()[usize::from(*k).min(2)],
        }
    }
}

fn image_flags(s: &mut Sema<'_, '_>, d: DeclId, flags: u8) {
    for f in [DeclFlags::USED, DeclFlags::REFERENCED] {
        if flags & f.bits() != 0 {
            s.ast.decls.set_flags(d, f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_lines)] // a literal image covering every record and type kind
    fn sample() -> Image {
        let q = |ty| QRef { ty, quals: 0 };
        let int = BuiltinType::Int as u32;
        Image {
            main: "m.cppm".into(),
            module: "m".into(),
            kw: 0,
            name_loc: 7,
            names: ["ns", "f", "E", "a", "T", "v", "g"].map(String::from).to_vec(),
            pnames: ["x", "p", "y", "L"].map(String::from).to_vec(),
            types: vec![
                TyRec::Func { ret: q(int), params: vec![q(int)], info: 0, exc: 0 },
                TyRec::Enum(3),
                TyRec::Pointer(QRef { ty: BUILTINS + 2, quals: 1 }),
            ],
            decls: vec![
                DeclRec {
                    flags: 0,
                    owner: 1,
                    access: 0,
                    name: 0,
                    ctx: NONE,
                    lexical: NONE,
                    loc: 20,
                    prev: NONE,
                    payload: Payload::Namespace,
                },
                DeclRec {
                    flags: 0,
                    owner: 1,
                    access: 0,
                    name: NONE,
                    ctx: 0,
                    lexical: NONE,
                    loc: 30,
                    prev: NONE,
                    payload: Payload::Export,
                },
                DeclRec {
                    flags: 4,
                    owner: 3,
                    access: 0,
                    name: 1,
                    ctx: 1,
                    lexical: NONE,
                    loc: 40,
                    prev: NONE,
                    payload: Payload::Function {
                        kind: 0,
                        deferred: false,
                        inits: vec![],
                        ty: q(BUILTINS + 1),
                        spec: 0,
                        params: vec![ParamRec {
                            name: 0,
                            loc: 46,
                            ty: q(int),
                            pflags: 0x100,
                            flags: 0,
                            default: NONE,
                        }],
                        body: FnBody::Braces(50, 51),
                    },
                },
                DeclRec {
                    flags: 0,
                    owner: 3,
                    access: 0,
                    name: 2,
                    ctx: 1,
                    lexical: NONE,
                    loc: 60,
                    prev: NONE,
                    payload: Payload::Enum {
                        scoped: 1,
                        fixed: Some((q(int), q(int))),
                        def: Some(None),
                    },
                },
                DeclRec {
                    flags: 0,
                    owner: 3,
                    access: 0,
                    name: 3,
                    ctx: 3,
                    lexical: NONE,
                    loc: 70,
                    prev: NONE,
                    payload: Payload::Enumerator { ty: q(int), value: -3 },
                },
                DeclRec {
                    flags: 0,
                    owner: 3,
                    access: 0,
                    name: 4,
                    ctx: 1,
                    lexical: NONE,
                    loc: 80,
                    prev: NONE,
                    payload: Payload::Typedef { alias: true, underlying: q(BUILTINS + 3) },
                },
                DeclRec {
                    flags: 0,
                    owner: 1,
                    access: 0,
                    name: 5,
                    ctx: 0,
                    lexical: LEX_TU,
                    loc: 90,
                    prev: NONE,
                    payload: Payload::Var { ty: q(BUILTINS + 3), spec: 2, init: NONE, eval: NONE },
                },
                // constexpr int g(int p = 7) { int y = p; return 7 + y; } L:
                DeclRec {
                    flags: 0,
                    owner: 3,
                    access: 0,
                    name: 6,
                    ctx: 1,
                    lexical: NONE,
                    loc: 100,
                    prev: NONE,
                    payload: Payload::Function {
                        kind: 0,
                        deferred: false,
                        inits: vec![],
                        ty: q(BUILTINS + 1),
                        spec: 0x18,
                        params: vec![ParamRec {
                            name: 1,
                            loc: 102,
                            ty: q(int),
                            pflags: 0x100,
                            flags: 4,
                            default: 0,
                        }],
                        body: FnBody::Stmt(2),
                    },
                },
                DeclRec {
                    flags: 4,
                    owner: 3,
                    access: 0,
                    name: 2,
                    ctx: 7,
                    lexical: NONE,
                    loc: 106,
                    prev: NONE,
                    payload: Payload::Var { ty: q(int), spec: 0, init: 1, eval: NONE },
                },
                DeclRec {
                    flags: 0,
                    owner: 3,
                    access: 0,
                    name: 3,
                    ctx: 7,
                    lexical: NONE,
                    loc: 130,
                    prev: NONE,
                    payload: Payload::Label,
                },
            ],
            bodies: Bodies {
                consts: vec![body::ConstRec::Int {
                    ty: q(int),
                    signed: true,
                    width: 32,
                    words: vec![7],
                }],
                strings: vec![(0, b"hi".to_vec())],
                exprs: vec![
                    // 7 (a wide literal)
                    body::ExprRec {
                        kind: 0,
                        bits: 0x10,
                        aux: 0,
                        cold: 0,
                        ty: q(int),
                        loc: 101,
                        words: vec![0],
                    },
                    // p
                    body::ExprRec {
                        kind: 7,
                        bits: 1,
                        aux: 0,
                        cold: 1,
                        ty: q(int),
                        loc: 107,
                        words: vec![body::PARAM, 0, NONE, 108],
                    },
                    // 7 + y (the operands' shapes only matter to Sema)
                    body::ExprRec {
                        kind: 10,
                        bits: 0,
                        aux: 0,
                        cold: 0,
                        ty: q(int),
                        loc: 110,
                        words: vec![0, 1],
                    },
                ],
                stmts: vec![
                    body::StmtRec { kind: 1, aux: 0, loc: 105, words: vec![1, 8] },
                    body::StmtRec { kind: 14, aux: 0, loc: 109, words: vec![2, NONE] },
                    body::StmtRec { kind: 0, aux: 0, loc: 104, words: vec![2, 0, 1, 120] },
                ],
            },
        }
    }

    #[test]
    fn round_trip() {
        let img = sample();
        let bytes = encode(&img);
        assert_eq!(decode(&bytes), Ok(img));
    }

    #[test]
    fn rejects_out_of_order_references() {
        let mut img = sample();
        // A type naming a record that follows its first user.
        img.decls.swap(2, 3);
        assert!(decode(&encode(&img)).is_err());
        let mut img = sample();
        img.decls[4].ctx = 2; // an enumerator outside its enumeration
        assert!(decode(&encode(&img)).is_err());
        let mut img = sample();
        img.decls[2].prev = 1; // a function redeclaring an export
        assert!(decode(&encode(&img)).is_err());
        let mut img = sample();
        img.bodies.exprs[2].words = vec![2, 1]; // an operand naming itself
        assert!(decode(&encode(&img)).is_err());
        let mut img = sample();
        img.bodies.stmts[0].words = vec![1, 6]; // a declaration statement of a global
        assert!(decode(&encode(&img)).is_err());
        let mut img = sample();
        img.bodies.stmts[2].words = vec![2, 0, 2, 120]; // a block containing itself
        assert!(decode(&encode(&img)).is_err());
        let mut img = sample();
        if let Payload::Function { body, .. } = &mut img.decls[7].payload {
            *body = FnBody::Stmt(1); // not a compound statement
        }
        assert!(decode(&encode(&img)).is_err());
        let mut img = sample();
        img.decls[9].ctx = 0; // a label outside a function
        assert!(decode(&encode(&img)).is_err());
    }

    /// [module.import] / spec/modules.md §11.11 "Part 2a": a damaged image
    /// (truncated, flipped bytes, corrupt lengths) is an error, never a
    /// panic.
    #[test]
    fn fuzz_never_panics() {
        let bytes = encode(&sample());
        for n in 0..bytes.len() {
            assert!(decode(&bytes[..n]).is_err(), "prefix {n} decoded");
        }
        for i in 0..bytes.len() {
            for x in [0x01u8, 0x80, 0xff, 0x7f, 0x10] {
                let mut b = bytes.clone();
                b[i] ^= x;
                let _ = decode(&b);
            }
        }
        for i in 0..bytes.len().saturating_sub(3) {
            for v in [u32::MAX, u32::MAX / 2, 0x0100_0000, 0, NONE - 1] {
                let mut b = bytes.clone();
                b[i..i + 4].copy_from_slice(&v.to_le_bytes());
                let _ = decode(&b);
            }
        }
        let mut s: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..4000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let mut b = bytes.clone();
            let i = (s as usize) % b.len();
            b[i] = (s >> 32) as u8;
            let j = ((s >> 16) as usize) % b.len();
            b.truncate(j.max(i));
            let _ = decode(&b);
        }
    }
}
