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
//! Coverage. Only what can be rebuilt exactly is written: the list is
//! spec/modules.md §11.11 "Part 2a" (declarations, function bodies,
//! initializers, since S3 non-template classes, and since WP-1120
//! polymorphic ones with virtual bases). Anything else (a polymorphic
//! class template, a global module fragment, an import, a declaration
//! outside the unit's main file, ...)
//! makes the writer emit no image, and the importer replays the source as
//! before (the fallback; correctness first; `-v` prints the reason).
//!
//! Reading is total: [`decode`] validates every index, offset and length
//! before anything is built; a malformed image is the module file's
//! corrupt-file error and builds nothing.

use kiln_ast::consts::Scalar;
use kiln_ast::decl::{
    Access, ConstexprKind, CtorInitKind, DeclFlags, DeclKind, Defaulted, DefinitionBits, EnumData,
    FnSpec, FnStorage, FriendKind, InitStyle, ModuleOwnership, ParamData, TagKind, TemplLinkKind,
    VarSpec, VarStorage,
};
use kiln_ast::modules::{Owner, UnitKind};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::stmt::StmtKind;
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{
    AutoKeyword, BuiltinType, ExcSpec, FuncInfo, Nns, NoexceptValue, Quals, TagKeyword,
    TemplateArg, TypeView,
};
use kiln_ast::{DeclHead, DeclId, NnsId, QualType, TypeId};
use kiln_source::fx::{FxHashMap, FxHashSet};
use kiln_source::{FileId, Loc};

use crate::Sema;
use crate::modules::Directive;

mod attrs;
mod body;
mod classtempl;
mod declocs;
mod locs;
mod pch;
pub use pch::PchDecls;
mod scope;
mod spec;
mod templ;

use body::{Bodies, BodyState, Root};
pub(crate) use locs::Locs;
use locs::{FileRec, LocTable, SlocRec};

/// The image format's version (the first field); bumped on any change.
pub(crate) const IMAGE_VERSION: u32 = 15;

/// "None" in an index or offset field.
const NONE: u32 = u32::MAX;
/// `DeclFlags::DEPENDENT_CTX`: a template parameter, a pattern, or a
/// local of a pattern (S4).
const DEP: u8 = DeclFlags::DEPENDENT_CTX.bits();
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

/// `-fkiln-bmi-image-census` (spec/modules.md §11.11 "Fallback
/// census"): why the image cannot carry the unit's top-level
/// declarations.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageCensus {
    /// The module's name.
    pub module: String,
    /// Top-level declarations the writer recorded or refused.
    pub top_level: usize,
    /// Per first fallback reason: the top-level declarations it stops,
    /// and the presumed location of the first one, most frequent first.
    pub rows: Vec<CensusRow>,
    /// Why the writer could not start at all (no row is counted then).
    pub unit: Option<&'static str>,
    /// Per attribute name: the declarations of the attributed-declaration
    /// row that carry it, most frequent first.
    pub attrs: Vec<(String, usize)>,
}

/// One reason of an [`ImageCensus`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CensusRow {
    /// The fallback reason; prefixed `cascade: ` when it names a
    /// declaration a refused one made (an earlier row's).
    pub reason: String,
    /// Top-level declarations whose first fallback it is.
    pub count: usize,
    /// `file:line:col` of the first of them.
    pub example: String,
}

/// The census writer's state.
#[derive(Default)]
struct Census {
    /// Top-level declarations refused while recording: what they contain
    /// and their templates are skipped.
    dead: FxHashSet<DeclId>,
    /// Top-level declarations with a counted reason.
    failed: FxHashSet<DeclId>,
    /// Every top-level declaration recorded or refused.
    seen: FxHashSet<DeclId>,
    /// Reason, count, example, in first-seen order.
    rows: Vec<(String, usize, String)>,
    by_reason: FxHashMap<String, usize>,
    attrs: FxHashMap<String, usize>,
}

/// The lengths [`Writer::rollback`] restores after a census failure.
#[derive(Clone, Copy)]
struct Marks {
    decls: usize,
    types: usize,
    roots: usize,
    overrides: usize,
    keys: usize,
    inits: usize,
}

/// [`Writer::collect`]'s state across declarations.
#[derive(Default)]
struct Pass {
    open_enum: Option<u32>,
    /// The open class definitions, innermost last.
    open: Vec<u32>,
    /// The implicitly declared special members, recorded after every
    /// class closed.
    implicit: Vec<(DeclId, u32, u8)>,
    /// The implicit instantiations the unit made, and everything
    /// declared in them (members of a class specialization).
    skipped: FxHashSet<DeclId>,
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
    /// A type named through a nested-name-specifier (`ns::S`,
    /// `Outer::Inner`): keyword, the qualifier's steps outermost first,
    /// and the named type.
    ElabQ(u8, Vec<NnsRec>, QRef),
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
    /// The type of a template type parameter record (S4).
    Ttp(u32),
    /// `X<args>` (S4b): the template record, the arguments as written
    /// and the underlying type (a [`TyRec::Record`] naming a
    /// specialization reference), `None` when dependent.
    Tst {
        template: u32,
        args: Vec<ArgRec>,
        under: Option<QRef>,
    },
    /// A substituted template type parameter (S4b): the replacement, the
    /// template record whose parameter it replaced, the parameter index
    /// and the pack element plus one (0: not a pack element).
    Subst {
        repl: QRef,
        assoc: u32,
        index: u32,
        pack: u32,
    },
    /// The injected-class-name type of a class template's pattern (S4b).
    Injected(u32),
    /// A pack expansion `P...` in a pattern (WP-1279).
    Pack(QRef),
    /// An unconstrained `auto` or `decltype(auto)` (WP-1279): bit 0 the
    /// keyword (`decltype(auto)`), bit 1 deduced as the dependent type (a
    /// placeholder in a pattern).
    Auto(u8),
}

/// A template argument (S4b): a type, or an integral or `bool` value of
/// a type.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ArgRec {
    Type(QRef),
    Value(QRef, i128),
}

impl ArgRec {
    fn q(self) -> QRef {
        match self {
            ArgRec::Type(q) | ArgRec::Value(q, _) => q,
        }
    }
}

/// One step of a type's nested-name-specifier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NnsRec {
    /// `::` (only as the first step).
    Global,
    /// A namespace record.
    Ns(u32),
    /// A type, and whether `template` preceded it.
    Ty(QRef, bool),
    /// A namespace alias record (S6).
    Alias(u32),
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

/// A base-specifier of a class definition.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct BaseRec {
    ty: QRef,
    loc: u32,
    /// [`Access`] as written (0 when defaulted).
    access: u8,
    /// `virtual` (WP-1120).
    virt: bool,
}

/// A class definition: its frozen `DefinitionBits` (without
/// `BEING_DEFINED` / `COMPLETE`), the first record after its members, its
/// key function (a member function record of the class, or [`NONE`];
/// Itanium C++ ABI §5.2.3, WP-1120) and its bases.
#[derive(Clone, PartialEq, Eq, Debug)]
struct RecDef {
    bits: u64,
    end: u32,
    key: u32,
    bases: Vec<BaseRec>,
}

/// A ctor-initializer: `kind` is `CtorInitKind` (`Base` 0: `target` the
/// base's position in its class's base list; `VirtualBase` 1: the type
/// reference (unqualified) of the virtual base as written, direct or
/// indirect; `Field` 2: a field record of the class, possibly after the
/// constructor; `Delegating` 4: 0).
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
        /// The virtual functions a first declaration overrides
        /// ([class.virtual]/2, sema's `cold.overrides`): function or
        /// implicit member records, possibly after this one (WP-1120).
        overrides: Vec<u32>,
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
    /// A template type parameter (S4): `begin` is its first token when
    /// that is not its name (`class` in `class T`), or [`NONE`].
    Ttp {
        depth: u16,
        index: u16,
        typename_kw: bool,
        /// A template parameter pack (WP-1279).
        pack: bool,
        begin: u32,
        /// Its default template argument, a type (S4b).
        default: Option<QRef>,
    },
    /// A non-type template parameter (S4); a pack (WP-1279); its default
    /// template argument's expression record or [`NONE`], and whether
    /// that argument carries its canonical profile (WP-1279).
    Nttp {
        ty: QRef,
        depth: u16,
        index: u16,
        pack: bool,
        default: u32,
        profile: bool,
    },
    /// A template (S4): `kind` 0 a function template; its pattern record
    /// (earlier, same name and context) and its template-head (the
    /// `template`, `<` and `>` locations, depth, parameter records);
    /// kind 1 a class template, kind 2 an alias template, whose `using`
    /// keyword is `using` ([`NONE`] for the other kinds; WP-1279).
    Template {
        kind: u8,
        pattern: u32,
        tloc: u32,
        using: u32,
        langle: u32,
        rangle: u32,
        depth: u16,
        params: Vec<u32>,
    },
    /// A specialization reference (S4b): a specialization of template
    /// record `template` the unit made implicitly, `complete` when its
    /// class definition was instantiated, and its converted arguments.
    Spec {
        template: u32,
        complete: bool,
        args: Vec<ArgRec>,
    },
    /// A member reference (S4b): the member of class specialization
    /// reference `spec` instantiated from pattern member `pattern`.
    Member {
        spec: u32,
        pattern: u32,
    },
    /// A using-declaration (S6): its qualifier; its shadows follow it.
    Using {
        steps: Vec<NnsRec>,
    },
    /// The shadow a using-declarator introduces (S6) for `target`.
    Shadow {
        target: u32,
    },
    /// A using-directive (S6) nominating namespace record `ns` (a first
    /// declaration).
    UsingDir {
        ns: u32,
    },
    /// A namespace alias (S6) of a namespace or alias record.
    NsAlias {
        target: u32,
    },
    /// A using-enum-declaration (S6) of an enumeration's definition; the
    /// shadows of its enumerators follow it.
    UsingEnum {
        e: u32,
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
    /// S6: the main-file offset just past the unit's last top-level
    /// module or import directive, which an importer replays before it
    /// builds the records; [`NONE`] for a primary interface without
    /// imports (the reader then applies the module directive itself).
    preamble: u32,
    /// The unit's declared names.
    names: Vec<String>,
    /// The names (indices into `names`, ascending) of its declarations at
    /// global namespace scope: the preprocessor's gate ([`gated_names`]).
    gated: Vec<u32>,
    /// S7: the buffers the image's locations are in (index 0: the main
    /// file) and the location records ([`locs`]).
    files: Vec<FileRec>,
    slocs: Vec<SlocRec>,
    /// Parameter and local names (not gated).
    pnames: Vec<String>,
    types: Vec<TyRec>,
    decls: Vec<DeclRec>,
    bodies: Bodies,
    /// WP-1278: the declarations' attributes and builtin identity.
    attrs: attrs::Attrs,
    /// WP-1278: inline namespaces and linkage specifications.
    scopes: scope::Scopes,
    /// WP-1283: explicit and partial class template specializations.
    cspecs: classtempl::ClassSpecs,
    /// WP-1283: declarator locations of dependent declarations.
    declocs: declocs::DeclLocs,
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
const K_TTP: u8 = 15;
const K_NTTP: u8 = 16;
const K_TEMPLATE: u8 = 17;
const K_SPEC: u8 = 18;
const K_MEMBER: u8 = 19;
const K_USING: u8 = 20;
const K_SHADOW: u8 = 21;
const K_USING_DIR: u8 = 22;
const K_NS_ALIAS: u8 = 23;
const K_USING_ENUM: u8 = 24;

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
            Payload::Ttp { .. } => K_TTP,
            Payload::Nttp { .. } => K_NTTP,
            Payload::Template { .. } => K_TEMPLATE,
            Payload::Spec { .. } => K_SPEC,
            Payload::Member { .. } => K_MEMBER,
            Payload::Using { .. } => K_USING,
            Payload::Shadow { .. } => K_SHADOW,
            Payload::UsingDir { .. } => K_USING_DIR,
            Payload::NsAlias { .. } => K_NS_ALIAS,
            Payload::UsingEnum { .. } => K_USING_ENUM,
        }
    }
}

const DECL_FLAGS: u8 = DeclFlags::USED.bits() | DeclFlags::REFERENCED.bits();

/// The fallback reason of a declaration with attributes (the census
/// names them).
const ATTRIBUTED: &str = attrs::UNCARRIED;

/// `FieldData.flags` carried: `mutable`, the initializer style, a
/// default member initializer (`[[no_unique_address]]` is an attribute).
const FIELD_FLAGS: u32 =
    kiln_ast::decl::FieldData::MUTABLE | (3 << 2) | kiln_ast::decl::FieldData::HAS_DEFAULT_INIT;

/// `DefinitionBits` a carried class may have: every defined bit (through
/// WP-1109's `NONTRIVIAL_*`, bits 57-61) except a lambda, anonymous, with
/// inherited members, or `trivial_abi` (spec/modules.md §11.11
/// "Classes"; polymorphic, abstract and virtual bases since WP-1120).
const DEF_BITS: u64 = {
    use kiln_ast::decl::DefinitionBits as B;
    ((B::NONTRIVIAL_MOVE_ASSIGN.bits() << 1) - 1)
        & !(B::BEING_DEFINED.bits()
            | B::COMPLETE.bits()
            | B::ANONYMOUS.bits()
            | B::LAMBDA.bits()
            | B::HAS_INHERITED_CTOR.bits()
            | B::HAS_INHERITED_ASSIGN.bits()
            | B::LAMBDA_CAPTURES.bits()
            | B::TRIVIAL_ABI.bits())
};

/// Function specifier bits carried: storage, inline (written and
/// implicit), `constexpr`/`consteval`, explicitly defaulted, deleted,
/// `explicit`, trivial, immediate-escalating, escalated, `...`, inherited
/// storage, not selected ([special]/6, WP-1109), deduced return type;
/// since WP-1120 virtual (semantic and written), pure, `override` and
/// `final`. Anything else (implicitly defaulted, coroutine, contracts,
/// explicit object parameter, ...) is not carried yet.
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
        .with_virtual(bit(6))
        .with_pure(bit(7))
        .with_defaulted(if bit(8) { Defaulted::Explicit } else { Defaulted::No })
        .with_deleted(bit(10))
        .with_explicit(bit(11))
        .with_trivial(bit(12))
        .with_immediate_escalating(bit(13))
        .with_escalated(bit(16))
        .with_variadic(bit(17))
        .with_override(bit(18))
        .with_final(bit(19))
        .with_virtual_written(bit(21))
        .with_storage_inherited(bit(22))
        .with_not_selected(bit(23))
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
        4 => ExcSpec::NothrowAttr,
        _ => return None,
    })
}

/// The exception specification words 5 (`noexcept(true)`) and 6
/// (`noexcept(false)`): a `noexcept` whose operand is a `bool` literal (the
/// C library's `__THROW`), which the importer builds again.
fn noexcept_literal(ast: &kiln_ast::Ast, e: kiln_ast::ExprId, v: NoexceptValue) -> W<u8> {
    use kiln_ast::expr::ExprKind;
    let n = ast.exprs.get(e);
    let lit = n.kind() == ExprKind::ConstantExpr && {
        let l = ast.exprs.get(ast.exprs.operand(e));
        l.kind() == ExprKind::CXXBoolLiteralExpr && l.ty() == QualType::BOOL
    };
    match v {
        NoexceptValue::True if lit => Ok(5),
        NoexceptValue::False if lit => Ok(6),
        _ => Err("an exception specification the image does not carry"),
    }
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
    fn args(&mut self, args: &[ArgRec]) {
        self.len(args.len());
        for a in args {
            match *a {
                ArgRec::Type(q) => (self.u8(0), self.q(q)).1,
                ArgRec::Value(q, v) => {
                    self.u8(1);
                    self.q(q);
                    self.0.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
    }
    fn q(&mut self, q: QRef) {
        self.u32(q.ty);
        self.u8(q.quals);
    }
    /// A qualifier: `n`, then the steps.
    fn steps(&mut self, steps: &[NnsRec]) {
        self.len(steps.len());
        for st in steps {
            match st {
                NnsRec::Global => self.u8(0),
                NnsRec::Ns(d) => (self.u8(1), self.u32(*d)).1,
                NnsRec::Ty(q, t) => {
                    self.u8(2);
                    self.q(*q);
                    self.u8(u8::from(*t));
                }
                NnsRec::Alias(d) => (self.u8(3), self.u32(*d)).1,
            }
        }
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

#[allow(clippy::too_many_lines)] // one arm per record kind, kept together with the format
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
        Payload::Function { kind, ty, spec, deferred, params, body, inits, overrides } => {
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
            w.len(overrides.len());
            for &o in overrides {
                w.u32(o);
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
                    w.u32(d.key);
                    w.len(d.bases.len());
                    for b in &d.bases {
                        w.q(b.ty);
                        w.u32(b.loc);
                        w.u8(b.access);
                        w.u8(u8::from(b.virt));
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
        Payload::Ttp { depth, index, typename_kw, pack, begin, default } => {
            w.u16(*depth);
            w.u16(*index);
            w.u8(u8::from(*typename_kw) | u8::from(*pack) << 1);
            w.u32(*begin);
            w.u32(default.map_or(0, |q| q.ty));
            w.u8(default.map_or(0, |q| q.quals));
        }
        Payload::Nttp { ty, depth, index, pack, default, profile } => {
            w.q(*ty);
            w.u16(*depth);
            w.u16(*index);
            w.u8(u8::from(*pack) | u8::from(*profile) << 1);
            w.u32(*default);
        }
        Payload::Template { kind, pattern, tloc, using, langle, rangle, depth, params } => {
            w.u8(*kind);
            for v in [*pattern, *tloc, *using, *langle, *rangle] {
                w.u32(v);
            }
            w.u16(*depth);
            w.len(params.len());
            for &p in params {
                w.u32(p);
            }
        }
        Payload::Spec { template, complete, args } => {
            w.u32(*template);
            w.u8(u8::from(*complete));
            w.args(args);
        }
        Payload::Member { spec, pattern } => {
            w.u32(*spec);
            w.u32(*pattern);
        }
        Payload::Using { steps } => w.steps(steps),
        Payload::Shadow { target: x }
        | Payload::UsingDir { ns: x }
        | Payload::NsAlias { target: x }
        | Payload::UsingEnum { e: x } => w.u32(*x),
    }
}

#[allow(clippy::too_many_lines)] // the sections in format order, kept together with the format
fn encode(img: &Image) -> Vec<u8> {
    let mut w = Wr(Vec::new());
    w.u32(IMAGE_VERSION);
    w.str(&img.main);
    w.str(&img.module);
    w.u32(img.kw);
    w.u32(img.name_loc);
    w.u32(img.preamble);
    w.len(img.names.len());
    for s in &img.names {
        w.str(s);
    }
    w.len(img.gated.len());
    for &g in &img.gated {
        w.u32(g);
    }
    w.len(img.files.len());
    for f in &img.files {
        w.str(&f.name);
        w.str(&f.path);
        w.u32(f.len);
        w.u64(f.hash);
        w.u8(u8::from(f.system) | (u8::from(f.text.is_some()) << 1));
        if let Some(t) = &f.text {
            w.str(t);
        }
    }
    w.len(img.slocs.len());
    for s in &img.slocs {
        match s {
            SlocRec::File { file, from, marks } => {
                w.u8(0);
                w.u32(*file);
                w.u32(*from);
                w.len(marks.len());
                for &(o, k) in marks {
                    w.u32(o);
                    w.u8(k);
                }
            }
            SlocRec::Macro { spelling, len, exp, name } => {
                w.u8(1);
                for v in [*spelling, *len, exp[0], exp[1], name[0], name[1]] {
                    w.u32(v);
                }
            }
            SlocRec::Arg { spelling, len, exp } => {
                w.u8(2);
                for v in [*spelling, *len, exp[0], exp[1]] {
                    w.u32(v);
                }
            }
        }
    }
    w.len(img.pnames.len());
    for s in &img.pnames {
        w.str(s);
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
            TyRec::Ttp(d) => (w.u8(15), w.u32(*d)).1,
            TyRec::Tst { template, args, under } => {
                w.u8(16);
                w.u32(*template);
                w.args(args);
                w.u32(under.map_or(0, |q| q.ty));
                w.u8(under.map_or(0, |q| q.quals));
            }
            TyRec::Injected(d) => (w.u8(17), w.u32(*d)).1,
            TyRec::Pack(q) => (w.u8(19), w.q(*q)).1,
            TyRec::Auto(k) => (w.u8(20), w.u8(*k)).1,
            TyRec::Subst { repl, assoc, index, pack } => {
                w.u8(18);
                w.q(*repl);
                w.u32(*assoc);
                w.u32(*index);
                w.u32(*pack);
            }
            TyRec::ElabQ(k, steps, q) => {
                w.u8(14);
                w.u8(*k);
                w.steps(steps);
                w.q(*q);
            }
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
        .filter(|(i, d)| d.name != NONE && !unlisted(img, *i, d))
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
    attrs::encode_attrs(&mut w, &img.attrs);
    scope::encode_scopes(&mut w, &img.scopes);
    classtempl::encode_class_specs(&mut w, &img.cspecs);
    declocs::encode_decl_locs(&mut w, &img.declocs);
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
    fn args(&mut self) -> Res<Vec<ArgRec>> {
        let n = self.count(6)?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(match self.u8()? {
                0 => ArgRec::Type(self.q()?),
                1 => ArgRec::Value(self.q()?, self.i128()?),
                _ => return bad("bad template argument"),
            });
        }
        Ok(v)
    }
    /// A qualifier ([`Wr::steps`]): never empty, `::` only first.
    fn steps(&mut self) -> Res<Vec<NnsRec>> {
        let n = self.count(1)?;
        if n == 0 {
            return bad("an empty qualifier");
        }
        let mut steps = Vec::with_capacity(n);
        for i in 0..n {
            steps.push(match self.u8()? {
                0 if i == 0 => NnsRec::Global,
                1 => NnsRec::Ns(self.u32()?),
                2 => {
                    let q = self.q()?;
                    match self.u8()? {
                        t @ 0..=1 => NnsRec::Ty(q, t == 1),
                        _ => return bad("bad qualifier step"),
                    }
                }
                3 => NnsRec::Alias(self.u32()?),
                _ => return bad("bad qualifier step"),
            });
        }
        Ok(steps)
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

#[allow(clippy::too_many_lines)] // one arm per record kind, kept together with the format
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
            let n = r.count(4)?;
            let mut overrides = Vec::with_capacity(n);
            for _ in 0..n {
                overrides.push(r.u32()?);
            }
            Payload::Function { kind, ty, spec, deferred, params, body, inits, overrides }
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
                    let key = r.u32()?;
                    let n = r.count(11)?;
                    let mut bases = Vec::with_capacity(n);
                    for _ in 0..n {
                        let (ty, loc, access) = (r.q()?, r.u32()?, r.u8()?);
                        let virt = match r.u8()? {
                            0 => false,
                            1 => true,
                            _ => return bad("bad flag byte"),
                        };
                        bases.push(BaseRec { ty, loc, access, virt });
                    }
                    Some(RecDef { bits, end, key, bases })
                }
                _ => return bad("bad flag byte"),
            };
            Payload::Record { tag, def }
        }
        K_FIELD => Payload::Field { ty: r.q()?, flags: r.u32()?, width: r.u32()?, init: r.u32()? },
        K_ACCESS => Payload::Access,
        K_IMPLICIT => Payload::Implicit { kind: r.u8()? },
        K_TTP => {
            let (depth, index) = (r.u16()?, r.u16()?);
            let bits = r.u8()?;
            if bits > 3 {
                return bad("bad flag byte");
            }
            let (typename_kw, pack) = (bits & 1 != 0, bits & 2 != 0);
            let begin = r.u32()?;
            let default = match r.q()? {
                QRef { ty: 0, quals: 0 } => None,
                QRef { ty: 0, .. } => return bad("bad type reference"),
                q => Some(q),
            };
            Payload::Ttp { depth, index, typename_kw, pack, begin, default }
        }
        K_NTTP => {
            let (ty, depth, index, bits) = (r.q()?, r.u16()?, r.u16()?, r.u8()?);
            if bits > 3 {
                return bad("bad flag byte");
            }
            let (pack, profile, default) = (bits & 1 != 0, bits & 2 != 0, r.u32()?);
            Payload::Nttp { ty, depth, index, pack, default, profile }
        }
        K_TEMPLATE => {
            let kind = r.u8()?;
            let (pattern, tloc, using) = (r.u32()?, r.u32()?, r.u32()?);
            let (langle, rangle) = (r.u32()?, r.u32()?);
            let depth = r.u16()?;
            let n = r.count(4)?;
            let mut params = Vec::with_capacity(n);
            for _ in 0..n {
                params.push(r.u32()?);
            }
            Payload::Template { kind, pattern, tloc, using, langle, rangle, depth, params }
        }
        K_SPEC => {
            let template = r.u32()?;
            let complete = match r.u8()? {
                0 => false,
                1 => true,
                _ => return bad("bad flag byte"),
            };
            Payload::Spec { template, complete, args: r.args()? }
        }
        K_MEMBER => Payload::Member { spec: r.u32()?, pattern: r.u32()? },
        K_USING => Payload::Using { steps: r.steps()? },
        K_SHADOW => Payload::Shadow { target: r.u32()? },
        K_USING_DIR => Payload::UsingDir { ns: r.u32()? },
        K_NS_ALIAS => Payload::NsAlias { target: r.u32()? },
        K_USING_ENUM => Payload::UsingEnum { e: r.u32()? },
        _ => return bad("unknown declaration kind"),
    };
    Ok(DeclRec { flags, owner, access, name, ctx, lexical, loc, prev, payload })
}

/// The type references a type record uses, and the records it names.
fn ty_refs(t: &TyRec) -> (Vec<QRef>, Vec<u32>) {
    match t {
        TyRec::Pointer(q)
        | TyRec::LRef(q)
        | TyRec::RRef(q)
        | TyRec::CArray(q, _)
        | TyRec::IArray(q)
        | TyRec::Paren(q)
        | TyRec::Elab(_, q)
        | TyRec::QArray(_, q)
        | TyRec::Pack(q) => (vec![*q], Vec::new()),
        TyRec::ElabQ(_, steps, q) => {
            let (mut qs, mut ds) = (vec![*q], Vec::new());
            for st in steps {
                match st {
                    NnsRec::Global => {}
                    NnsRec::Ns(d) | NnsRec::Alias(d) => ds.push(*d),
                    NnsRec::Ty(q, _) => qs.push(*q),
                }
            }
            (qs, ds)
        }
        TyRec::Func { ret, params, .. } => {
            let mut v = Vec::with_capacity(params.len() + 1);
            v.push(*ret);
            v.extend_from_slice(params);
            (v, Vec::new())
        }
        TyRec::Enum(d)
        | TyRec::Typedef(d)
        | TyRec::Record(d)
        | TyRec::Ttp(d)
        | TyRec::Injected(d) => (Vec::new(), vec![*d]),
        TyRec::Tst { template, args, under } => {
            (args.iter().map(|a| a.q()).chain(*under).collect(), vec![*template])
        }
        TyRec::Subst { repl, assoc, .. } => (vec![*repl], vec![*assoc]),
        TyRec::Sugar(_) | TyRec::Auto(_) => (Vec::new(), Vec::new()),
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
        | Payload::Template { .. }
        | Payload::Member { .. }
        | Payload::Shadow { .. }
        | Payload::UsingDir { .. }
        | Payload::NsAlias { .. }
        | Payload::UsingEnum { .. }
        | Payload::Record { def: None, .. } => Vec::new(),
        Payload::Using { steps } => steps
            .iter()
            .filter_map(|s| match s {
                NnsRec::Ty(q, _) => Some(*q),
                _ => None,
            })
            .collect(),
        Payload::Ttp { default, .. } => default.iter().copied().collect(),
        Payload::Spec { args, .. } => args.iter().map(|a| a.q()).collect(),
        Payload::Record { def: Some(d), .. } => d.bases.iter().map(|b| b.ty).collect(),
        Payload::Function { ty, params, inits, .. } => std::iter::once(*ty)
            .chain(params.iter().map(|p| p.ty))
            .chain(inits.iter().filter(|x| x.kind == 1).map(|x| QRef { ty: x.target, quals: 0 }))
            .collect(),
        Payload::Field { ty, .. }
        | Payload::Var { ty, .. }
        | Payload::Enumerator { ty, .. }
        | Payload::Let { ty, .. }
        | Payload::Nttp { ty, .. } => vec![*ty],
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
        (
            Payload::Field { .. }
            | Payload::Access
            | Payload::Implicit { .. }
            | Payload::Export
            | Payload::Ttp { .. }
            | Payload::Nttp { .. }
            | Payload::Template { .. }
            | Payload::Spec { .. }
            | Payload::Member { .. }
            | Payload::Using { .. }
            | Payload::UsingDir { .. }
            | Payload::UsingEnum { .. },
            _,
        ) => false,
        _ => a.code() == b.code(),
    }
}

/// Decodes the file table and the location records (S7), and checks
/// them ([`locs::validate`]).
fn decode_locs(r: &mut Rd<'_>, img: &mut Image) -> Res<()> {
    let n = r.count(21)?;
    img.files = Vec::with_capacity(n);
    for _ in 0..n {
        let (name, path, len, hash) = (r.str()?, r.str()?, r.u32()?, r.u64()?);
        let flags = r.u8()?;
        if flags > 3 {
            return bad("bad file flags");
        }
        let text = if flags & 2 != 0 { Some(r.str()?) } else { None };
        img.files.push(FileRec { name, path, len, hash, system: flags & 1 != 0, text });
    }
    let n = r.count(13)?;
    img.slocs = Vec::with_capacity(n);
    for _ in 0..n {
        let s = match r.u8()? {
            0 => {
                let (file, from) = (r.u32()?, r.u32()?);
                let m = r.count(5)?;
                let mut marks = Vec::with_capacity(m);
                for _ in 0..m {
                    marks.push((r.u32()?, r.u8()?));
                }
                SlocRec::File { file, from, marks }
            }
            1 => SlocRec::Macro {
                spelling: r.u32()?,
                len: r.u32()?,
                exp: [r.u32()?, r.u32()?],
                name: [r.u32()?, r.u32()?],
            },
            2 => SlocRec::Arg { spelling: r.u32()?, len: r.u32()?, exp: [r.u32()?, r.u32()?] },
            _ => return bad("bad location record"),
        };
        img.slocs.push(s);
    }
    match locs::validate(&img.files, &img.slocs) {
        Ok(_) => Ok(()),
        Err(why) => bad(why),
    }
}

/// Decodes and validates an image. Total: any malformed input is an
/// [`ImageError`]; an `Ok` image can be built without further checks
/// other than its offsets against the main file's length.
#[allow(clippy::too_many_lines)] // the sections in format order, kept together with the format
fn decode(bytes: &[u8]) -> Res<Image> {
    let mut r = Rd(bytes);
    if r.u32()? != IMAGE_VERSION {
        return bad("unsupported version");
    }
    let mut img = Image { main: r.str()?, module: r.str()?, ..Image::default() };
    img.kw = r.u32()?;
    img.name_loc = r.u32()?;
    img.preamble = r.u32()?;
    for which in 0..2 {
        let n = r.count(4)?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(r.str()?);
        }
        if which == 0 {
            img.names = v;
            let n = r.count(4)?;
            img.gated = Vec::with_capacity(n);
            for _ in 0..n {
                match r.u32()? {
                    g if (g as usize) < img.names.len() => img.gated.push(g),
                    _ => return bad("a gated name out of range"),
                }
            }
            decode_locs(&mut r, &mut img)?;
        } else {
            img.pnames = v;
        }
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
                if exc_spec(exc).is_none() && !matches!(exc, 5 | 6) {
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
            15 => TyRec::Ttp(r.u32()?),
            16 => {
                let template = r.u32()?;
                let args = r.args()?;
                let under = match r.q()? {
                    QRef { ty: 0, quals: 0 } => None,
                    QRef { ty: 0, .. } => return bad("bad type reference"),
                    q => Some(q),
                };
                TyRec::Tst { template, args, under }
            }
            17 => TyRec::Injected(r.u32()?),
            19 => TyRec::Pack(r.q()?),
            20 => match r.u8()? {
                k @ 0..=3 => TyRec::Auto(k),
                _ => return bad("bad placeholder type"),
            },
            18 => {
                let repl = r.q()?;
                let (assoc, index, pack) = (r.u32()?, r.u32()?, r.u32()?);
                if pack > 0xffff {
                    return bad("bad pack index");
                }
                TyRec::Subst { repl, assoc, index, pack }
            }
            14 => {
                let k = r.u8()?;
                if tag_keyword(k).is_none() {
                    return bad("bad tag keyword");
                }
                let steps = r.steps()?;
                TyRec::ElabQ(k, steps, r.q()?)
            }
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
    img.attrs = attrs::decode_attrs(&mut r)?;
    img.scopes = scope::decode_scopes(&mut r)?;
    img.cspecs = classtempl::decode_class_specs(&mut r)?;
    img.declocs = declocs::decode_decl_locs(&mut r)?;
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
            TyRec::Elab(_, q)
            | TyRec::ElabQ(_, _, q)
            | TyRec::Paren(q)
            | TyRec::Tst { under: Some(q), .. } => q.ty,
            TyRec::Typedef(d) => match img.decls.get(*d as usize)?.payload {
                Payload::Typedef { underlying, .. } => underlying.ty,
                _ => return None,
            },
            TyRec::Record(d) => {
                let ok = match img.decls.get(*d as usize)?.payload {
                    Payload::Record { .. } => complete(*d),
                    // A specialization the importer completes (S4b).
                    Payload::Spec { complete, .. } => complete,
                    _ => false,
                };
                return ok.then_some(*d);
            }
            _ => return None,
        };
    }
    None
}

/// Whether `q` is a dependent base class type of a class template's
/// pattern (S4b): a type parameter or a dependent specialization.
fn dependent_base(img: &Image, q: QRef) -> bool {
    q.quals == 0
        && q.ty > BUILTINS
        && matches!(
            img.types.get((q.ty - BUILTINS - 1) as usize),
            Some(TyRec::Ttp(_) | TyRec::Tst { under: None, .. })
        )
}

/// Whether record `d` (index `i`) stays out of the lookup table: a
/// local, a template parameter, a pattern (its template is looked up in
/// its place) or a specialization (WP-1283).
fn unlisted(img: &Image, i: usize, d: &DeclRec) -> bool {
    body::is_local(img, d)
        || d.flags & DEP != 0
        || classtempl::explicit_of(img, i as u32).is_some()
        // Found through their shadows (S6).
        || matches!(d.payload, Payload::Using { .. } | Payload::UsingEnum { .. })
}

/// Whether every step of a qualifier names a record below `below` of its
/// kind: a namespace step a namespace, an alias step an alias (S6).
fn steps_ok(img: &Image, steps: &[NnsRec], below: usize) -> bool {
    steps.iter().all(|s| match *s {
        NnsRec::Global | NnsRec::Ty(..) => true,
        NnsRec::Ns(x) => {
            (x as usize) < below
                && matches!(img.decls.get(x as usize).map(|r| &r.payload), Some(Payload::Namespace))
        }
        NnsRec::Alias(x) => {
            (x as usize) < below
                && matches!(
                    img.decls.get(x as usize).map(|r| &r.payload),
                    Some(Payload::NsAlias { .. })
                )
        }
    })
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
        for d in decl {
            let class_tmpl = |x: u32| {
                matches!(
                    img.decls.get(x as usize).map(|r| &r.payload),
                    Some(Payload::Template { kind: 1, .. })
                )
            };
            let ok = img.decls.get(d as usize).is_some_and(|r| {
                matches!(
                    (t, &r.payload),
                    (TyRec::Enum(_), Payload::Enum { .. })
                        | (TyRec::Typedef(_), Payload::Typedef { .. })
                        | (TyRec::Record(_), Payload::Record { .. })
                        | (TyRec::ElabQ(..), Payload::Namespace | Payload::NsAlias { .. })
                        | (TyRec::Ttp(_), Payload::Ttp { .. })
                        | (
                            TyRec::Injected(_) | TyRec::Tst { .. },
                            Payload::Template { kind: 1, .. }
                        )
                        | (TyRec::Tst { under: Some(_), .. }, Payload::Template { kind: 2, .. })
                        | (TyRec::Subst { .. }, Payload::Template { .. })
                ) || matches!((t, &r.payload),
                    (TyRec::Record(_), Payload::Spec { template, .. }) if class_tmpl(*template))
                    // A partial specialization's injected-class-name (WP-1283).
                    || matches!(t, TyRec::Injected(_) | TyRec::Subst { .. })
                        && classtempl::partial_of(img, d).is_some()
            });
            if !ok {
                return bad("bad declaration reference in a type");
            }
            acc = acc.max(d as usize + 1);
        }
        if let TyRec::ElabQ(_, steps, _) = t
            && !steps_ok(img, steps, nd)
        {
            return bad("bad qualifier step");
        }
        if let TyRec::Func { info, .. } = t
            && FuncInfo::from_bits(*info).bits() != *info
        {
            return bad("bad function type bits");
        }
        // A specialization type's underlying type is the record type of
        // a specialization reference of the same template.
        if let TyRec::Tst { template, under: Some(u), .. } = t
            && !matches!(
                img.decls.get(*template as usize).map(|r| &r.payload),
                Some(Payload::Template { kind: 2, .. })
            )
        {
            let ok = u.quals == 0
                && u.ty > BUILTINS
                && matches!(img.types.get((u.ty - BUILTINS - 1) as usize),
                    Some(TyRec::Record(r)) if matches!(img.decls.get(*r as usize).map(|x| &x.payload),
                        Some(Payload::Spec { template: st, .. }) if st == template)
                        || classtempl::explicit_of(img, *r) == Some(*template));
            if !ok {
                return bad("bad specialization type");
            }
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
    // S4: the dependent non-local records; per record, whether a
    // template claimed it as its pattern or a parameter.
    let mut dependent: Vec<usize> = Vec::new();
    let mut claimed = vec![false; nd];
    // S4b: per class template pattern, its parameter count.
    let mut tmpl_params: FxHashMap<usize, usize> = FxHashMap::default();
    classtempl::validate_class_specs(img, &qref_at, &mut claimed)?;
    declocs::validate_decl_locs(img)?;
    for (i, d) in img.decls.iter().enumerate() {
        let kind_of = |x: u32| img.decls.get(x as usize).filter(|_| (x as usize) < i);
        while stack.last().is_some_and(|&(_, end)| end <= i) {
            stack.pop();
        }
        chain.push(if (d.prev as usize) < i { chain[d.prev as usize] } else { i });
        // S4b: a specialization or member reference is no declaration of
        // the unit: it has no name or context and may lie anywhere after
        // what it names (inside a class definition too).
        if matches!(d.payload, Payload::Spec { .. } | Payload::Member { .. }) {
            if d.flags & !DECL_FLAGS != 0
                || d.owner > 3
                || d.access != 0
                || d.name != NONE
                || d.ctx != NONE
                || d.lexical != NONE
                || d.prev != NONE
            {
                return bad("bad specialization reference");
            }
            let ok = match &d.payload {
                Payload::Spec { template, complete, args } => {
                    kind_of(*template).is_some_and(|t| match &t.payload {
                        Payload::Template { kind, params, .. } => {
                            (*kind == 1 || !*complete)
                                && params.len() == args.len()
                                && params.iter().zip(args).all(|(&p, a)| {
                                    match (&img.decls[p as usize].payload, a) {
                                        (Payload::Ttp { .. }, ArgRec::Type(q)) => qref_at(*q, i),
                                        (Payload::Nttp { .. }, ArgRec::Value(q, _)) => {
                                            q.ty <= BUILTINS && q.quals == 0
                                        }
                                        _ => false,
                                    }
                                })
                        }
                        _ => false,
                    })
                }
                Payload::Member { spec, pattern } => {
                    kind_of(*spec).is_some_and(|sr| match sr.payload {
                        Payload::Spec { template, complete: true, .. } => {
                            match img.decls[template as usize].payload {
                                Payload::Template { kind: 1, pattern: p, .. } => kind_of(*pattern)
                                    .is_some_and(|m| {
                                        (m.ctx == p
                                            || classtempl::partial_of(img, m.ctx) == Some(template))
                                            && m.lexical == NONE
                                            && matches!(
                                                m.payload,
                                                Payload::Function { .. }
                                                    | Payload::Var { .. }
                                                    | Payload::Field { .. }
                                            )
                                    }),
                                _ => false,
                            }
                        }
                        _ => false,
                    })
                }
                _ => false,
            };
            if !ok {
                return bad("bad specialization reference");
            }
            continue;
        }
        // A local declaration (a body's variable or label): its context
        // is an earlier function record; it is never looked up by name.
        if let Some(f) = kind_of(d.ctx).filter(|c| matches!(c.payload, Payload::Function { .. })) {
            // A local of a template's pattern is dependent, like it.
            if d.flags & !(body::LOCAL_FLAGS | DEP) != 0
                || (d.flags & DEP) != (f.flags & DEP)
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
        if d.flags & !(DECL_FLAGS | DEP) != 0
            || d.owner > 3
            || matches!(d.payload, Payload::Label | Payload::Let { .. })
        {
            return bad("bad declaration flags");
        }
        // A member of a class template's pattern (S4b) is claimed by its
        // class, which must be claimed itself.
        let dep_class = kind_of(d.ctx)
            .is_some_and(|c| c.flags & DEP != 0 && matches!(c.payload, Payload::Record { .. }));
        if dep_class {
            if d.flags & DEP == 0 || !claimed[d.ctx as usize] {
                return bad("a member of an unclaimed dependent class");
            }
            claimed[i] = true;
            if d.lexical != NONE {
                // An out-of-line member definition claims the parameters
                // of its own template-head, just before it.
                let n = tmpl_params.get(&(d.ctx as usize)).copied().unwrap_or(usize::MAX);
                if n == usize::MAX || n > i {
                    return bad("an out-of-line member of a nested dependent class");
                }
                // The parameters belong to the definition's own scope.
                let scope = if d.lexical == LEX_TU { NONE } else { d.lexical };
                for (k, j) in (i - n..i).enumerate() {
                    let ok = match img.decls[j].payload {
                        Payload::Ttp { depth: 0, index, .. }
                        | Payload::Nttp { depth: 0, index, .. } => {
                            usize::from(index) == k && img.decls[j].ctx == scope
                        }
                        _ => false,
                    };
                    if !ok || claimed[j] {
                        return bad("bad template parameter list");
                    }
                    claimed[j] = true;
                }
            }
        } else if d.flags & DEP != 0 {
            // Only template parameters and patterns (claimed below).
            dependent.push(i);
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
                // A builtin function, at global scope (WP-1278).
                (Payload::Implicit { kind: scope::BUILTIN_KIND }, _) => {
                    top.is_none() && d.ctx == NONE && d.lexical == NONE
                }
                (Payload::Implicit { .. }, _) => {
                    top.is_none() && closed(d.ctx) && d.lexical == NONE
                }
                // A class template's record follows its pattern's record,
                // inside the pattern's definition (S4b).
                (Payload::Template { kind: 1, pattern, .. }, NONE) if top == Some(*pattern) => {
                    stack.len() == 1 && scope(d.ctx)
                }
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
            Payload::Export
            | Payload::Access
            | Payload::Implicit { .. }
            | Payload::UsingDir { .. } => d.name == NONE,
            // The target's, the enumeration's or the pattern's name
            // (checked below); a using-declarator may name an operator.
            Payload::Shadow { .. } | Payload::UsingEnum { .. } | Payload::Template { .. } => true,
            Payload::Using { .. } => match d.name {
                n if n >= N_OP => {
                    n < N_CTOR
                        && u8::try_from(n - N_OP)
                            .ok()
                            .and_then(OverloadedOperator::from_u8)
                            .is_some()
                }
                n => ident(n),
            },
            // Template parameters name `pnames`, like function parameters.
            Payload::Ttp { .. } | Payload::Nttp { .. } => {
                d.name == NONE || (d.name as usize) < img.pnames.len()
            }
            // An unnamed namespace has no name.
            Payload::Field { .. } | Payload::Namespace => d.name == NONE || ident(d.name),
            // An unnamed class or enumeration a typedef names (WP-1278).
            Payload::Record { .. } | Payload::Enum { .. } if d.name == NONE => {
                scope::anon_tag(img, i as u32)
            }
            Payload::Function { kind, .. } => match d.name {
                N_CTOR => *kind == 2,
                N_DTOR => *kind == 3,
                N_CONV => *kind == 4,
                n if (N_OP..N_CTOR).contains(&n) => {
                    *kind < 2
                        && u8::try_from(n - N_OP)
                            .ok()
                            .and_then(OverloadedOperator::from_u8)
                            .is_some()
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
            Payload::Function { kind, ty, spec, params, inits, overrides, .. } => {
                if *kind as usize >= FN_KINDS.len() || (*kind == 0) == in_class {
                    return bad("bad function kind");
                }
                // A first declaration's overridden functions: other
                // member functions (records anywhere in the image).
                let overrider = |x: u32| {
                    x as usize != i
                        && img.decls.get(x as usize).is_some_and(|o| match o.payload {
                            Payload::Function { kind: 1 | 3 | 4, .. } => o.prev == NONE,
                            // An implicit assignment operator or destructor.
                            Payload::Implicit { kind } => (3..=5).contains(&kind),
                            _ => false,
                        })
                };
                if (!overrides.is_empty() && (d.prev != NONE || !matches!(*kind, 1 | 3 | 4)))
                    || !overrides.iter().all(|&x| overrider(x))
                {
                    return bad("bad overridden functions");
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
                        // The field may follow the constructor in its
                        // class: inits are applied after every record.
                        2 => img.decls.get(x.target as usize).is_some_and(|f| {
                            matches!(f.payload, Payload::Field { .. }) && f.ctx == d.ctx
                        }),
                        4 => x.target == 0,
                        // A virtual base (its type checked with the
                        // record's type references).
                        1 => base_class(img, QRef { ty: x.target, quals: 0 }, &|_| true).is_some(),
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
                let pack = if d.flags & DEP != 0 { ParamData::PACK } else { 0 };
                for p in params {
                    if (p.name != NONE && p.name as usize >= img.pnames.len())
                        || p.flags & !(DECL_FLAGS | (d.flags & DEP)) != 0
                        || p.pflags & !(ParamData::SCOPE_DEPTH_MASK | pack) != 0
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
                    // The key function: a member function declared in the
                    // class (not an out-of-line definition).
                    if df.key != NONE
                        && (!(i + 1..end).contains(&(df.key as usize))
                            || img.decls.get(df.key as usize).is_none_or(|k| {
                                !matches!(k.payload, Payload::Function { kind: 1 | 3 | 4, .. })
                                    || k.ctx as usize != i
                                    || k.lexical != NONE
                            }))
                    {
                        return bad("bad key function");
                    }
                    if def_end[chain[i]] != usize::MAX {
                        return bad("a class defined twice");
                    }
                    def_end[chain[i]] = end;
                    let complete = |x: u32| chain.get(x as usize).is_some_and(|&c| def_end[c] <= i);
                    for b in &df.bases {
                        let ok = base_class(img, b.ty, &complete).is_some()
                            || (d.flags & DEP != 0 && dependent_base(img, b.ty));
                        if b.access > 3 || !ok {
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
            Payload::Implicit { kind: scope::BUILTIN_KIND } => {
                if d.prev != NONE {
                    return bad("bad builtin function record");
                }
            }
            Payload::Implicit { kind } => {
                if *kind > 5 || d.prev != NONE || !implicit.insert((d.ctx, *kind)) {
                    return bad("bad implicit member");
                }
            }
            Payload::Ttp { .. } | Payload::Nttp { .. } => {
                if d.flags & DEP == 0 || !scope(d.ctx) || d.lexical != NONE || d.prev != NONE {
                    return bad("bad template parameter");
                }
            }
            Payload::Template { kind, pattern, depth, params, .. } => {
                // S4 slice: a function template at namespace scope, its
                // pattern the namespace-scope function just declared.
                // S4b: or a class template at namespace scope, its
                // pattern the class record just before it.
                let p = *pattern as usize;
                let ok = *kind <= 3
                    && *depth == 0
                    && d.flags & DEP == 0
                    && scope(d.ctx)
                    && d.lexical == NONE
                    && d.prev == NONE
                    && (*kind == 0 || p + 1 == i)
                    && kind_of(*pattern).is_some_and(|r| {
                        matches!(
                            (*kind, &r.payload),
                            (0, Payload::Function { kind: 0, .. })
                                | (1, Payload::Record { .. })
                                | (2, Payload::Typedef { alias: true, .. })
                                | (3, Payload::Var { .. })
                        ) && r.flags & DEP != 0
                            && r.name == d.name
                            && r.ctx == d.ctx
                            && r.owner == d.owner
                            && r.lexical == NONE
                            && r.prev == NONE
                    })
                    && !claimed[p];
                if !ok || params.len() > usize::from(u16::MAX) {
                    return bad("bad template");
                }
                claimed[p] = true;
                if *kind == 1 {
                    tmpl_params.insert(p, params.len());
                }
                for (k, &x) in params.iter().enumerate() {
                    let ok = kind_of(x).is_some_and(|r| {
                        r.ctx == d.ctx
                            && match r.payload {
                                Payload::Ttp { depth: pd, index, .. }
                                | Payload::Nttp { depth: pd, index, .. } => {
                                    pd == *depth && usize::from(index) == k
                                }
                                _ => false,
                            }
                    });
                    if !ok || claimed[x as usize] {
                        return bad("bad template parameter list");
                    }
                    claimed[x as usize] = true;
                }
            }
            // S6: the using family, at namespace scope only.
            Payload::Using { steps } => {
                if in_class || !steps_ok(img, steps, i) {
                    return bad("bad using-declaration");
                }
            }
            Payload::Shadow { target } => {
                let ok = kind_of(*target).is_some_and(|t| {
                    t.flags & DEP == 0
                        && t.name == d.name
                        && matches!(
                            t.payload,
                            Payload::Function { kind: 0, .. }
                                | Payload::Var { .. }
                                | Payload::Enum { .. }
                                | Payload::Enumerator { .. }
                                | Payload::Typedef { .. }
                                | Payload::Record { .. }
                                | Payload::Template { .. }
                                | Payload::Shadow { .. }
                        )
                });
                if in_class || !ok {
                    return bad("bad using-shadow");
                }
            }
            Payload::UsingDir { ns } => {
                let ok = kind_of(*ns)
                    .is_some_and(|n| matches!(n.payload, Payload::Namespace) && n.prev == NONE);
                if in_class || !ok {
                    return bad("bad using-directive");
                }
            }
            Payload::NsAlias { target } => {
                let ok = kind_of(*target).is_some_and(|t| {
                    matches!(t.payload, Payload::Namespace | Payload::NsAlias { .. })
                });
                if in_class || !ok {
                    return bad("bad namespace alias");
                }
            }
            Payload::UsingEnum { e } => {
                let ok = kind_of(*e).is_some_and(|t| {
                    t.name == d.name && matches!(t.payload, Payload::Enum { def: Some(_), .. })
                });
                if in_class || !ok {
                    return bad("bad using-enum-declaration");
                }
            }
            Payload::Namespace
            | Payload::Export
            | Payload::Typedef { .. }
            | Payload::Access
            | Payload::Label
            | Payload::Let { .. }
            | Payload::Spec { .. }
            | Payload::Member { .. } => {}
        }
    }
    if dependent.iter().any(|&i| !claimed[i]) {
        return bad("a dependent declaration outside a template");
    }
    // The lookup table: exactly the named listed records, sorted.
    let named = img
        .decls
        .iter()
        .enumerate()
        .filter(|(i, d)| d.name != NONE && !unlisted(img, *i, d))
        .count();
    if lookup.len() != named || lookup.windows(2).any(|w| w[0] >= w[1]) {
        return bad("bad lookup table");
    }
    for &(c, n, d) in lookup {
        if !img.decls.get(d as usize).is_some_and(|r| r.ctx == c && r.name == n) {
            return bad("bad lookup table");
        }
    }
    let _ = nd;
    attrs::validate_attrs(img)?;
    scope::validate_scopes(img)?;
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
    /// S7: the file table and location records made so far.
    lt: std::cell::RefCell<LocTable>,
    recs: FxHashMap<DeclId, u32>,
    names: FxHashMap<String, u32>,
    pnames: FxHashMap<String, u32>,
    tmemo: FxHashMap<TypeId, u32>,
    img: Image,
    bs: BodyState,
    /// Function records and the functions they override, class records
    /// and their key functions: resolved once every record is written
    /// (the targets may follow, implicit members last; WP-1120).
    overrides: Vec<(u32, Vec<DeclId>)>,
    keys: Vec<(u32, DeclId)>,
    /// S6: per lookup context, name and entity, the latest shadow or
    /// namespace alias record (what a redeclaration replaces in lookup).
    usings: FxHashMap<(DeclId, DeclName, DeclId), u32>,
    /// `-fkiln-bmi-image-census`: record every refusal and go on.
    census: Option<Census>,
    /// WP-1036: the image of a precompiled header ([`pch`]).
    pch: bool,
}

/// A reserved identifier ([lex.name]/3): `__x` or `_X`.
/// The names of `img`'s declarations at global namespace scope, which
/// the importer's preprocessor checks before it uses the image
/// (spec/modules.md §11.11 "Part 2a", "ODR merging"): those whose
/// context is the translation unit, seen through export blocks, unnamed
/// namespaces and unscoped enumerations (whose names are visible in the
/// enclosing scope). Any other declaration sits in a class, a named
/// namespace, a function or a template parameter list of the image, so
/// an importer's declaration can meet it only through a gated name.
fn gated_names(img: &Image) -> Vec<u32> {
    let mut on = vec![false; img.names.len()];
    for r in &img.decls {
        if (r.name as usize) >= on.len()
            || body::is_local(img, r)
            || matches!(r.payload, Payload::Ttp { .. } | Payload::Nttp { .. })
        {
            continue;
        }
        let mut ctx = r.ctx;
        let mut steps = 0;
        let top = loop {
            if ctx == NONE {
                break true;
            }
            let Some(c) = img.decls.get(ctx as usize).filter(|_| steps < img.decls.len()) else {
                break false;
            };
            let transparent = match &c.payload {
                Payload::Export => true,
                Payload::Namespace => c.name == NONE,
                Payload::Enum { scoped, .. } => *scoped == 0,
                _ => false,
            };
            if !transparent {
                break false;
            }
            ctx = c.ctx;
            steps += 1;
        };
        if top {
            on[r.name as usize] = true;
        }
    }
    (0u32..).zip(on).filter_map(|(i, on)| on.then_some(i)).collect()
}

fn reserved(s: &str) -> bool {
    let b = s.as_bytes();
    b.first() == Some(&b'_') && b.get(1).is_some_and(|&c| c == b'_' || c.is_ascii_uppercase())
}

impl Writer<'_, '_, '_> {
    /// The image location of `l` (S7: [`locs`]).
    fn loc(&self, l: Loc) -> W<u32> {
        self.lt.borrow_mut().loc(self.s.diags.sm(), l)
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

    /// A type's qualifier, outermost step first.
    fn nns_steps(&mut self, n: NnsId) -> W<Vec<NnsRec>> {
        let mut chain = Vec::new();
        let mut cur = Some(n);
        while let Some(id) = cur {
            if chain.len() > 1024 {
                return Err("a malformed qualifier");
            }
            let v = self.s.ast.types.nns_view(id);
            cur = match v {
                Nns::Global => None,
                Nns::Namespace { prefix, .. }
                | Nns::Type { prefix, .. }
                | Nns::NamespaceAlias { prefix, .. } => prefix,
                _ => return Err("a qualifier naming a dependent name or splice"),
            };
            chain.push(v);
        }
        let mut steps = Vec::with_capacity(chain.len());
        for v in chain.into_iter().rev() {
            steps.push(match v {
                Nns::Global => NnsRec::Global,
                Nns::Namespace { ns, .. } => match self.recs.get(&ns) {
                    Some(&r)
                        if matches!(self.img.decls[r as usize].payload, Payload::Namespace) =>
                    {
                        NnsRec::Ns(r)
                    }
                    _ => return Err("a qualifier naming a namespace outside the image"),
                },
                Nns::Type { ty, template_kw, .. } => NnsRec::Ty(self.qref(ty)?, template_kw),
                // S6: the importer resolves the alias again.
                Nns::NamespaceAlias { alias, ns, .. } => match self.recs.get(&alias) {
                    Some(&r)
                        if matches!(
                            self.img.decls[r as usize].payload,
                            Payload::NsAlias { .. }
                        ) && self.s.ast.resolve_alias(alias) == ns =>
                    {
                        NnsRec::Alias(r)
                    }
                    _ => return Err("a qualifier naming a namespace alias outside the image"),
                },
                _ => return Err("a malformed qualifier"),
            });
        }
        Ok(steps)
    }

    fn ty(&mut self, t: TypeId) -> W<u32> {
        if let Some(&i) = self.tmemo.get(&t) {
            return Ok(i);
        }
        if let Some(rec) = self.template_ty(t)? {
            let i = BUILTINS + 1 + self.img.types.len() as u32;
            self.img.types.push(rec);
            self.tmemo.insert(t, i);
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
            TypeView::Elaborated { keyword, nns: Some(n), named } => {
                let steps = self.nns_steps(n)?;
                TyRec::ElabQ(keyword as u8, steps, self.qref(named)?)
            }
            TypeView::Function(p) => {
                let exc = match types.exc_spec(&p) {
                    ExcSpec::None => 0,
                    ExcSpec::BasicNoexcept => 1,
                    ExcSpec::DynamicNone => 2,
                    ExcSpec::Nothrow => 3,
                    ExcSpec::NothrowAttr => 4,
                    ExcSpec::NoexceptExpr { expr, value } => {
                        noexcept_literal(&self.s.ast, expr, value)?
                    }
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
            TypeView::TemplateTypeParm(d) => TyRec::Ttp(self.type_decl(d, t)?),
            TypeView::PackExpansion { pattern, num_expansions: None } => {
                TyRec::Pack(self.qref(pattern)?)
            }
            TypeView::Auto { keyword, deduced, concept: None, args: None, pack: false }
                if keyword != AutoKeyword::GnuAutoType
                    && deduced.is_none_or(|q| q == QualType::DEPENDENT) =>
            {
                TyRec::Auto(keyword as u8 | u8::from(deduced.is_some()) << 1)
            }
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
    /// S6: the payload of a using-declaration, using-shadow,
    /// using-directive, namespace alias or using-enum-declaration at
    /// namespace scope, whose target must be a record of the image.
    fn using_record(&mut self, d: DeclId) -> W<Payload> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        let in_class = n
            .ctx()
            .and_then(|c| self.recs.get(&c))
            .is_some_and(|&c| matches!(self.img.decls[c as usize].payload, Payload::Record { .. }));
        if in_class {
            return Err("a using-declaration in a class");
        }
        let rec = |x: Option<DeclId>| x.and_then(|x| self.recs.get(&x).copied());
        let payload_of = |r: u32| &self.img.decls[r as usize].payload;
        Ok(match n.kind() {
            DeclKind::Using => {
                let q = ast.using_qualifier(d).ok_or("a using-declaration without a qualifier")?;
                Payload::Using { steps: self.nns_steps(q)? }
            }
            DeclKind::UsingShadow => {
                let t = rec(ast.using_target(d))
                    .ok_or("a using-declaration naming a declaration outside the image")?;
                if self.img.decls[t as usize].flags & DEP != 0 {
                    return Err("a using-declaration naming a template pattern");
                }
                Payload::Shadow { target: t }
            }
            DeclKind::UsingDirective => {
                let ns = rec(ast.using_target(d).map(|x| ast.first(x)))
                    .filter(|&r| matches!(payload_of(r), Payload::Namespace))
                    .ok_or("a using-directive or alias naming a namespace outside the image")?;
                Payload::UsingDir { ns }
            }
            DeclKind::NamespaceAlias => {
                let target = rec(ast.using_target(d))
                    .filter(|&r| {
                        matches!(payload_of(r), Payload::Namespace | Payload::NsAlias { .. })
                    })
                    .ok_or("a using-directive or alias naming a namespace outside the image")?;
                Payload::NsAlias { target }
            }
            _ => {
                let e = rec(ast.using_target(d))
                    .filter(|&r| matches!(payload_of(r), Payload::Enum { def: Some(_), .. }))
                    .ok_or("a using-declaration naming a declaration outside the image")?;
                Payload::UsingEnum { e }
            }
        })
    }

    /// S6: the lookup context, name and entity of shadow or namespace
    /// alias `d`, which a later one of the same entity replaces in lookup
    /// (sema's `introduce_shadow` and `act_on_namespace_alias`).
    fn using_key(&self, d: DeclId) -> Option<(DeclId, DeclName, DeclId)> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        let target = ast.using_target(d)?;
        let entity = match n.kind() {
            DeclKind::UsingShadow => ast.first(ast.strip_shadow(target)),
            DeclKind::NamespaceAlias => ast.resolve_alias(target),
            _ => return None,
        };
        let mut c = n.ctx()?;
        for _ in 0..1024 {
            if !matches!(ast.decls.get(c).kind(), DeclKind::Export | DeclKind::LinkageSpec) {
                break;
            }
            c = ast.decls.get(c).ctx()?;
        }
        Some((ast.primary_context(ast.redecl_context(c)), n.name(), entity))
    }

    /// S6: the earlier shadow or alias record `d` replaces in lookup, or
    /// [`NONE`]. Its entity is a record of the image, so an earlier one
    /// can only be a record of the image too (an import cannot name it,
    /// and a global module fragment is refused).
    fn using_prev(&self, d: DeclId) -> W<u32> {
        let k = self.using_key(d).ok_or("a malformed using-declaration")?;
        Ok(self.usings.get(&k).copied().unwrap_or(NONE))
    }

    fn name_word(&mut self, d: DeclId) -> W<u32> {
        use kiln_ast::names::DeclNameView as V;
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        let class_canon =
            || n.ctx().and_then(|c| ast.decls.get(c).ty()).map(|q| ast.types.canonical(q).ty());
        Ok(match ast.names.view(n.name()) {
            V::Empty => NONE,
            V::Identifier(sym) => {
                let s = ast.symbols().as_str(sym).to_owned();
                self.check_name(d, &s)?;
                Self::name_ix(&mut self.names, &mut self.img.names, &s)
            }
            V::Operator(op) => N_OP + u32::from(op as u8),
            V::Constructor(t)
                if n.kind() == DeclKind::CXXConstructor && class_canon() == Some(t) =>
            {
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
        let mut extra = n.flags().bits() & !(DECL_FLAGS | DEP | DeclFlags::HAS_LEXICAL_CTX.bits());
        if n.kind() == DeclKind::UsingShadow {
            // Every shadow is implicit; the reader's builder marks it.
            extra &= !DeclFlags::IMPLICIT.bits();
        }
        if extra & DeclFlags::HAS_ATTRS.bits() != 0 && self.attrs_carried(d) {
            // Carried by `attr_recs` (WP-1278).
            extra &= !DeclFlags::HAS_ATTRS.bits();
        }
        if extra != 0 {
            return Err(if extra & DeclFlags::INVALID.bits() != 0 {
                "an invalid declaration"
            } else if extra & DeclFlags::IMPLICIT.bits() != 0 {
                "an implicit declaration of a kind the image does not carry"
            } else if extra & DeclFlags::INJECTED.bits() != 0 {
                "a declaration injected by define_aggregate"
            } else if extra & DeclFlags::HAS_ATTRS.bits() != 0 {
                ATTRIBUTED
            } else {
                "a declaration with flags the image does not carry"
            });
        }
        let parm = matches!(n.kind(), DeclKind::TemplateTypeParm | DeclKind::NonTypeTemplateParm);
        if n.has(DeclFlags::DEPENDENT_CTX)
            && !parm
            && !matches!(
                self.s.ast.templ_link(d).get(),
                Some((TemplLinkKind::Pattern | TemplLinkKind::PartialSpecialization, _))
            )
            && !self.dep_member(d)
        {
            return Err(self.in_template_reason(d));
        }
        if n.has(DeclFlags::HAS_LEXICAL_CTX)
            && self.dep_member(d)
            && n.ctx().and_then(|c| ast.described_template(ast.first(c))).is_none()
        {
            return Err("an out-of-line member of a class nested in a template");
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
            None if matches!(n.kind(), DeclKind::UsingShadow | DeclKind::NamespaceAlias) => {
                self.using_prev(d)?
            }
            None => NONE,
            Some(p) if self.implicit_prev(d, p) => NONE,
            Some(p) => self
                .recs
                .get(&p)
                .copied()
                .ok_or("a redeclaration of an entity outside the image")?,
        };
        let flags = n.flags().bits() & (DECL_FLAGS | DEP);
        let name = if parm {
            // Not a declared name of the unit: not gated, like a
            // function parameter's.
            match n.name() {
                x if x == DeclName::EMPTY => NONE,
                x => {
                    let s =
                        self.ident(x).ok_or("a template parameter not named by an identifier")?;
                    Self::name_ix(&mut self.pnames, &mut self.img.pnames, &s)
                }
            }
        } else {
            self.name_word(d)?
        };
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
                self.namespace_flags(i, nd.flags)?;
                Payload::Namespace
            }
            DeclKind::Export => Payload::Export,
            DeclKind::LinkageSpec => self.linkage_record(d, i)?,
            DeclKind::Using
            | DeclKind::UsingShadow
            | DeclKind::UsingDirective
            | DeclKind::NamespaceAlias
            | DeclKind::UsingEnum => self.using_record(d)?,
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
                if !td.templ.is_none() && !self.pattern_of(d, DeclKind::TypeAliasTemplate) {
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
            DeclKind::TemplateTypeParm | DeclKind::NonTypeTemplateParm => self.template_parm(d)?,
            DeclKind::FunctionTemplate
            | DeclKind::ClassTemplate
            | DeclKind::TypeAliasTemplate
            | DeclKind::VarTemplate => self.template(d)?,
            _ => return Err("a declaration the image does not carry yet"),
        })
    }

    /// Rejects declarations whose cold side data the image does not carry.
    fn cold_gate(&self, d: DeclId) -> W<()> {
        let c = &self.s.ast.decls.cold;
        if c.friends.get(d).is_some_and(|&l| !self.s.ast.lists.is_empty(l)) {
            return Err("a friend declaration");
        }
        // The overridden functions, key function and deleting
        // destructor's `operator delete` are carried or recomputed
        // (WP-1120); a destroying `operator delete` is not.
        if c.dtor_delete_this.contains(d) {
            return Err("a destroying operator delete");
        }
        if c.inheriting_ctor.contains(d) || c.inherited_ctor.contains(d) {
            return Err("an inherited constructor");
        }
        if c.delete_message.contains(d)
            || c.explicit_expr.contains(d)
            || c.record_pack.contains(d)
            || c.requires_clause.contains(d)
            || c.contracts.contains(d)
            || (c.asm_label.contains(d) && !self.own_asm_label(d))
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
        // The pattern of a function template (S4): its body is always
        // carried, as dependent records, for importers to instantiate.
        let pattern = match fd.templ.get() {
            None => self.dep_member(d),
            Some((TemplLinkKind::Pattern, _)) if kind == 0 => true,
            Some((TemplLinkKind::Pattern, _)) => return Err("a member function template"),
            Some(_) => return Err("a template specialization"),
        };
        // A pattern constructor's mem-initializers live in the template
        // state as parsed (`InstState::pattern_mem_inits`), not in the
        // declaration: not carried yet (S4b).
        if pattern
            && (s.inst.pattern_mem_inits.contains_key(&d) || s.inst.failed_mem_inits.contains(&d))
        {
            return Err("mem-initializers of a constructor in a class template");
        }
        let spec = fd.spec;
        let bits = spec.bits();
        let virt = spec.is_virtual() || spec.is_pure() || spec.is_override() || spec.is_final();
        if virt && pattern {
            return Err("a virtual function in a class template");
        }
        if fn_spec(bits) != Some(spec) {
            return Err("a coroutine, contract-checked or implicitly defaulted function");
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
        // A destructor written without one keeps its implicit
        // specification deferred even once computed (WP-1036).
        let deferred = ast.types.exc_spec(&proto) == ExcSpec::Deferred(ast.first(d))
            || (first && s.implicit_exc_dtors.contains(&d));
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
                // A precompiled header's every body: each consumer emits
                // the header's definitions (WP-1036).
                if pattern || self.pch || Self::carries_body(spec) {
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
            && ast.decls.ctor_inits(d, &ast.lists).len() != 0
        {
            return Err("ctor-initializers of a constructor without a carried body");
        }
        // The functions a first declaration overrides (resolved in
        // `collect`); a redeclaration shares its first declaration's.
        let ov = |x: DeclId| {
            let l = ast.decls.cold.overrides.get(x).copied();
            l.map(|l| ast.lists.iter::<DeclId>(l).collect::<Vec<_>>()).unwrap_or_default()
        };
        let mine = ov(d);
        if first {
            if !mine.is_empty() {
                self.overrides.push((i, mine));
            }
        } else if !mine.is_empty() && mine != ov(ast.first(d)) {
            return Err("a redeclared virtual function with other overridden functions");
        }
        let overrides = Vec::new();
        Ok(Payload::Function {
            kind: kind as u8,
            ty,
            spec: bits,
            deferred,
            params,
            body,
            inits,
            overrides,
        })
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
                    // The target is the type as written in the
                    // mem-initializer (`B<int>`, an alias): match it
                    // canonically.
                    let tq = QualType::unqual(TypeId::new(x.target));
                    let want = ast.types.canonical(tq).ty();
                    let pos = bases.iter().position(|&b| {
                        let q = ast.decls.base_spec(b).ty;
                        ast.types.canonical(q).ty() == want
                    });
                    pos.ok_or("a ctor-initializer of an indirect base")? as u32
                }
                CtorInitKind::Field => {
                    let f = DeclId::try_new(x.target).ok_or("a malformed ctor-initializer")?;
                    if ast.decls.get(f).ctx() != Some(class) {
                        return Err("a ctor-initializer of an anonymous union member");
                    }
                    // The field may follow the constructor: resolved when
                    // the declarations are all recorded.
                    self.bs.init_fields.push((i, k as u32, f));
                    NONE
                }
                CtorInitKind::Delegating => 0,
                CtorInitKind::VirtualBase => {
                    // The virtual base's type as written (direct or
                    // indirect).
                    let q = self.qref(QualType::unqual(TypeId::new(x.target)))?;
                    if q.quals != 0 {
                        return Err("a malformed ctor-initializer");
                    }
                    q.ty
                }
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
        if rd.lambda.is_some() {
            return Err("a closure type");
        }
        match rd.templ.get() {
            None => {}
            Some((TemplLinkKind::Pattern, _))
                if ast
                    .described_template(ast.first(d))
                    .is_some_and(|t| ast.decls.get(t).kind() == DeclKind::ClassTemplate) => {}
            // WP-1283: listed with its template and arguments.
            Some((TemplLinkKind::Specialization | TemplLinkKind::PartialSpecialization, _)) => {
                self.class_spec_entry(d, i)?;
            }
            _ => return Err("a member template or a template in a template"),
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
        if ast.decls.get(d).has(DeclFlags::DEPENDENT_CTX)
            && (b.contains(DefinitionBits::POLYMORPHIC)
                || b.contains(DefinitionBits::HAS_VIRTUAL_BASES))
        {
            return Err("a polymorphic class template");
        }
        if let Some(&k) = ast.decls.cold.key_function.get(d) {
            self.keys.push((i, k));
        }
        if b.contains(DefinitionBits::HAS_INHERITED_CTOR)
            || b.contains(DefinitionBits::HAS_INHERITED_ASSIGN)
        {
            return Err("an inherited constructor or assignment operator");
        }
        if b.contains(DefinitionBits::ANONYMOUS) {
            return Err("an anonymous union or struct member");
        }
        // A dynamic class declares its implicit members eagerly anyway
        // (`finish_special_bits`), and they are carried as implicit
        // records in id order; a non-dynamic one still falls back
        // (WP-1121).
        let dynamic = b.contains(DefinitionBits::POLYMORPHIC)
            || b.contains(DefinitionBits::HAS_VIRTUAL_BASES);
        if !dynamic && crate::class::SpecialKind::any_needs_overload(b) {
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
            if bs.pack_expansion {
                return Err("a pack expansion");
            }
            let _ = self.bs.bspec.insert(g, (i, pos as u32));
            bases.push(BaseRec {
                ty: self.qref(bs.ty)?,
                loc: self.loc(bs.loc)?,
                access: bs.access as u8,
                virt: bs.is_virtual,
            });
        }
        Ok(Payload::Record { tag, def: Some(RecDef { bits, end: NONE, key: NONE, bases }) })
    }

    /// Parameter `p`, the image's parameter `g`.
    fn param(&mut self, p: DeclId, g: u32) -> W<ParamRec> {
        let ast = &self.s.ast;
        let pd = *ast.decls.param(p).ok_or("a malformed parameter")?;
        let n = ast.decls.get(p);
        // A function parameter pack (WP-1279) only in a pattern.
        let pack = if n.has(DeclFlags::DEPENDENT_CTX) { ParamData::PACK } else { 0 };
        if pd.original_ty.is_some() || pd.flags & !(ParamData::SCOPE_DEPTH_MASK | pack) != 0 {
            return Err("a parameter with an adjusted type or a pack");
        }
        if let Some(e) = pd.default_arg {
            // Patched by `walk_roots`.
            self.bs.roots.push(Root::Default(g, e));
        }
        if n.flags().bits() & !(DECL_FLAGS | DEP | DeclFlags::HAS_LEXICAL_CTX.bits()) != 0
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
            flags: n.flags().bits() & (DECL_FLAGS | DEP),
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

    #[allow(clippy::too_many_lines)] // one pass over the purview in id order, with the class nesting
    fn collect(&mut self) -> W<()> {
        let s = self.s;
        let ast = &s.ast;
        let mt = &ast.modules;
        let sm = s.diags.sm();
        if !self.pch {
            self.module_header()?;
        }
        self.img.main = sm.file_name(self.main).to_owned();
        let mut st = Pass::default();
        for d in ast.decls.ids() {
            if self.census.as_ref().is_some_and(|c| !c.dead.is_empty()) && self.census_dead(d) {
                continue;
            }
            let before = self.marks();
            match self.one(d, &mut st) {
                Ok(()) => {
                    if self.census.is_some() && self.img.decls.len() > before.decls {
                        let root = self.census_root(d);
                        // A carried scope is not counted itself; what it
                        // holds is.
                        let scope = matches!(
                            self.s.ast.decls.get(root).kind(),
                            DeclKind::Export | DeclKind::LinkageSpec | DeclKind::Namespace
                        );
                        if let Some(c) = self.census.as_mut().filter(|_| !scope) {
                            let _ = c.seen.insert(root);
                        }
                    }
                }
                Err(why) if self.census.is_some() => {
                    self.rollback(before);
                    let ns = self.s.ast.decls.get(d).kind() == DeclKind::Namespace;
                    self.census_fail(d, why, !ns);
                    if ns {
                        // What the namespace holds is counted on its own:
                        // a placeholder stands for it.
                        let ctx = self.s.ast.decls.get(d).ctx();
                        let ctx = ctx.and_then(|c| self.recs.get(&c).copied()).unwrap_or(NONE);
                        let i = self.img.decls.len() as u32;
                        self.img.decls.push(DeclRec {
                            flags: 0,
                            owner: 1,
                            access: 0,
                            name: NONE,
                            ctx,
                            lexical: NONE,
                            loc: NONE,
                            prev: NONE,
                            payload: Payload::Namespace,
                        });
                        self.recs.insert(d, i);
                    }
                }
                Err(why) => return Err(why),
            }
        }
        let Pass { mut open, implicit, .. } = st;
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
        for (i, list) in std::mem::take(&mut self.overrides) {
            let mut out = Vec::with_capacity(list.len());
            for o in list {
                match self.recs.get(&o) {
                    Some(&r) => out.push(r),
                    None => self.miss(i, "an override of a function outside the image")?,
                }
            }
            if let Payload::Function { overrides, .. } = &mut self.img.decls[i as usize].payload {
                *overrides = out;
            }
        }
        for (i, k) in std::mem::take(&mut self.keys) {
            let Some(&r) = self.recs.get(&k) else {
                self.miss(i, "a key function outside the image")?;
                continue;
            };
            if let Payload::Record { def: Some(df), .. } = &mut self.img.decls[i as usize].payload {
                df.key = r;
            }
        }
        self.anon_typedefs()?;
        self.decl_locs()?;
        for (i, k, f) in std::mem::take(&mut self.bs.init_fields) {
            let Some(&r) = self.recs.get(&f) else {
                self.miss(i, "a ctor-initializer of a field outside the image")?;
                continue;
            };
            if let Payload::Function { inits, .. } = &mut self.img.decls[i as usize].payload
                && let Some(x) = inits.get_mut(k as usize)
            {
                x.target = r;
            }
        }
        self.walk_roots()
    }

    /// The module unit's name, keyword and preamble words of the image.
    fn module_header(&mut self) -> W<()> {
        let s = self.s;
        let mt = &s.ast.modules;
        let unit = mt.units.first().ok_or("no module unit")?;
        let partition = match unit.kind {
            UnitKind::Interface => false,
            UnitKind::PartitionInterface | UnitKind::PartitionImplementation => true,
            _ => return Err("a unit that is not a module interface or partition"),
        };
        let module = unit.module.clone().ok_or("no module declaration")?;
        // S6: a unit with imports, or a partition, is entered by replaying
        // its preamble; the records follow it (spec/modules.md §11.11
        // "Imports, partitions and using-declarations").
        if partition || !unit.imports.is_empty() || mt.units.len() != 1 {
            let (end, bad) = s.module_preamble();
            if bad {
                return Err("an import outside the module's main file, or code after its `;`");
            }
            self.img.preamble = end.ok_or("an import outside the module's main file")?;
        }
        self.img.module = match &unit.partition {
            Some(p) if partition => format!("{module}:{p}"),
            _ => module,
        };
        let (kw, kw_loc, private) = s.module_main_frame().ok_or("no module declaration")?;
        if private {
            return Err("a private module fragment");
        }
        self.img.kw = self.loc(kw)?;
        self.img.name_loc = self.loc(kw_loc)?;
        if self.img.kw == NONE || self.img.name_loc == NONE {
            return Err("no module declaration");
        }
        Ok(())
    }

    fn marks(&self) -> Marks {
        Marks {
            decls: self.img.decls.len(),
            types: self.img.types.len(),
            roots: self.bs.roots.len(),
            overrides: self.overrides.len(),
            keys: self.keys.len(),
            inits: self.bs.init_fields.len(),
        }
    }

    /// The census writer drops what a refused declaration recorded.
    fn rollback(&mut self, m: Marks) {
        self.img.decls.truncate(m.decls);
        self.img.types.truncate(m.types);
        self.bs.roots.truncate(m.roots);
        self.overrides.truncate(m.overrides);
        self.keys.truncate(m.keys);
        self.bs.init_fields.truncate(m.inits);
        let (n, t) = (m.decls as u32, BUILTINS + 1 + m.types as u32);
        self.recs.retain(|_, v| *v < n);
        self.usings.retain(|_, v| *v < n);
        self.tmemo.retain(|_, v| *v < t);
        self.img.cspecs.truncate(n);
    }

    /// The top-level declaration `d` belongs to: the outermost enclosing
    /// declaration below a namespace, an export block, a linkage
    /// specification or the translation unit, seen as its template when
    /// it is a template's pattern.
    fn census_root(&self, d: DeclId) -> DeclId {
        let ast = &self.s.ast;
        let mut cur = d;
        loop {
            let n = ast.decls.get(cur);
            let up = if n.has(DeclFlags::HAS_LEXICAL_CTX) {
                ast.decls.lexical_ctx(cur)
            } else {
                n.ctx()
            };
            match up {
                Some(p)
                    if !matches!(
                        ast.decls.get(p).kind(),
                        DeclKind::TranslationUnit
                            | DeclKind::Namespace
                            | DeclKind::Export
                            | DeclKind::LinkageSpec
                    ) =>
                {
                    cur = p;
                }
                _ => break,
            }
        }
        ast.described_template(cur).unwrap_or(cur)
    }

    /// `d` is in (or is) a top-level declaration the census refused.
    fn census_dead(&self, d: DeclId) -> bool {
        let Some(c) = &self.census else { return false };
        let ast = &self.s.ast;
        let mut cur = Some(d);
        while let Some(x) = cur {
            if c.dead.contains(&x) || ast.described_template(x).is_some_and(|t| c.dead.contains(&t))
            {
                return true;
            }
            let n = ast.decls.get(x);
            cur =
                if n.has(DeclFlags::HAS_LEXICAL_CTX) { ast.decls.lexical_ctx(x) } else { n.ctx() };
        }
        false
    }

    /// Counts `why` as the first fallback reason of `d`'s top-level
    /// declaration; `kill` skips the rest of it.
    fn census_fail(&mut self, d: DeclId, why: &'static str, kill: bool) {
        let root = self.census_root(d);
        let sm = self.s.diags.sm();
        let ast = &self.s.ast;
        let at = Some(ast.decls.get(d).loc())
            .filter(|l| l.is_valid())
            .unwrap_or_else(|| ast.decls.get(root).loc());
        let mut attrs: Vec<String> = Vec::new();
        if why == ATTRIBUTED {
            attrs = ast
                .lists
                .iter::<kiln_ast::AttrId>(ast.attrs.of_decl(d))
                .map(|a| ast.attrs.get(a))
                .filter(|a| !attrs::carried(a, ast.symbols().len()))
                .map(|a| match crate::decl_attrs::attr_name(a) {
                    Some(n) => n.to_owned(),
                    None => format!("(kind {})", a.kind.0),
                })
                .collect();
            attrs.sort_unstable();
            attrs.dedup();
        }
        let kind = format!("{why} ({:?})", ast.decls.get(d).kind());
        let Some(c) = self.census.as_mut() else { return };
        let cascade = !c.dead.is_empty() && why.contains("outside the image");
        if kill {
            let _ = c.dead.insert(root);
        }
        let _ = c.seen.insert(root);
        if !c.failed.insert(root) {
            return;
        }
        for a in attrs {
            *c.attrs.entry(a).or_default() += 1;
        }
        let reason = match () {
            () if cascade => format!("cascade: {why}"),
            () if why.starts_with("an implicit declaration") => kind,
            () => why.to_owned(),
        };
        if let Some(&i) = c.by_reason.get(&reason) {
            c.rows[i].1 += 1;
            return;
        }
        let example = sm.presumed_line_col(at).map_or_else(
            || format!("{:?} '{}'", ast.decls.get(d).kind(), self.s.qualified_decl_name(d)),
            |p| format!("{}:{}:{}", p.file_name, p.line, p.col),
        );
        c.by_reason.insert(reason.clone(), c.rows.len());
        c.rows.push((reason, 1, example));
    }

    /// A fix-up of record `rec` names a declaration outside the image:
    /// the census counts it, the writer falls back.
    fn miss(&mut self, rec: u32, why: &'static str) -> W<()> {
        if self.census.is_none() {
            return Err(why);
        }
        if let Some(d) = self.recs.iter().find(|&(_, &v)| v == rec).map(|(&d, _)| d) {
            self.census_fail(d, why, false);
        }
        Ok(())
    }

    /// One declaration of the purview, in id order ([`Writer::collect`]).
    #[allow(clippy::too_many_lines)] // the purview's declaration kinds, with the class nesting
    fn one(&mut self, d: DeclId, st: &mut Pass) -> W<()> {
        let s = self.s;
        let ast = &s.ast;
        let mt = &ast.modules;
        let sm = s.diags.sm();
        let n = ast.decls.get(d);
        let in_skipped = |c: Option<DeclId>| c.is_some_and(|c| st.skipped.contains(&c));
        if in_skipped(n.ctx())
            || (n.has(DeclFlags::HAS_LEXICAL_CTX) && in_skipped(ast.decls.lexical_ctx(d)))
        {
            let _ = st.skipped.insert(d);
            return Ok(());
        }
        let o = mt.owner(d);
        if self.pch && (o.unit != 0 || o.purview) {
            return Err("a module import or declaration in the header");
        }
        if o.unit != 0 {
            // Imported, or replayed for an import of the preamble.
            return Ok(());
        }
        if !(o.purview || self.pch && self.pch_own(d)) {
            // Predefined declarations have no location in a real
            // file; anything else is a global module fragment.
            let real = sm
                .offset_of(sm.expansion_loc(n.loc()))
                .is_some_and(|(f, _)| !crate::bmi_ast::unstable_file(sm.file_name(f)));
            if real && !n.has(DeclFlags::IMPLICIT) {
                return Err("a global module fragment");
            }
            return Ok(());
        }
        if n.kind() == DeclKind::ParmVar || self.local(d) {
            return Ok(());
        }
        if n.kind() == DeclKind::TranslationUnit {
            return Ok(());
        }
        if n.kind() == DeclKind::StaticAssert {
            // Checked when the module was compiled; nothing to import.
            // In a template, each instantiation checks it again: not
            // carried yet (WP-1283).
            if n.has(DeclFlags::DEPENDENT_CTX) {
                return Err("a static assertion in a template");
            }
            return Ok(());
        }
        if ast.spec_info(d).is_some_and(|(_, si)| {
            matches!(si.kind, SpecKind::ImplicitInstantiation | SpecKind::Undeclared)
        }) {
            // An implicit instantiation the unit made: an importer
            // instantiates the carried template again on use (S4); a
            // carried body or type naming one names it through a
            // specialization reference (S4b).
            let _ = st.skipped.insert(d);
            return Ok(());
        }
        if n.has(DeclFlags::IMPLICIT) && self.implicit_scope_decl(d) {
            // Declared again by the importer with its unnamed namespace.
            return Ok(());
        }
        if n.has(DeclFlags::IMPLICIT) && self.builtin_record(d)? {
            return Ok(());
        }
        if n.has(DeclFlags::IMPLICIT) && n.kind() != DeclKind::UsingShadow {
            // An implicitly declared special member: the importer
            // declares it again (spec/modules.md §11.11 "Classes").
            if n.kind() == DeclKind::IndirectField {
                return Err("an anonymous union or struct member");
            }
            let k = s
                .special_kind_of(d)
                .ok_or("an implicit declaration that is not a special member")?;
            let implicit_default =
                ast.decls.function(d).is_some_and(|f| f.spec.defaulted() == Defaulted::Implicit);
            let class = n.ctx().and_then(|c| self.recs.get(&c).copied());
            if class.is_some_and(|c| self.img.decls[c as usize].flags & DEP != 0) {
                return Err("an implicit member of a class template");
            }
            match class {
                Some(c)
                    if implicit_default
                        && n.flags().bits() & !DECL_FLAGS == DeclFlags::IMPLICIT.bits() =>
                {
                    st.implicit.push((d, c, k as u8));
                }
                None => return Err("an implicit special member of a class outside the image"),
                Some(_) => {
                    return Err(
                        "an implicit special member that is deleted, not defaulted, or flagged",
                    );
                }
            }
            return Ok(());
        }
        kind_gate(n.kind())?;
        self.cold_gate(d)?;
        let at = self.loc(n.loc())?;
        let main_at = locs::main_pos(sm, self.main, n.loc());
        if self.img.preamble != NONE && at != NONE && main_at.is_none_or(|p| p < self.img.preamble)
        {
            // Before the preamble's end: the importer replays it.
            return Err("a declaration before the module's last import");
        }
        let h = self.head(d)?;
        // The specialization references its types name come first
        // (S4b): `record` must not add records before its own.
        self.prescan(d);
        let i = self.img.decls.len() as u32;
        let payload = self.record(d, i)?;
        if self.img.decls.len() as u32 != i {
            return Err("a specialization named while its user is recorded");
        }
        let tmpl_of_open = matches!(payload,
            Payload::Template { kind: 1, pattern, .. } if st.open.last() == Some(&pattern));
        match &payload {
            Payload::Enumerator { .. } => {
                if st.open_enum != Some(h.ctx) {
                    return Err("an enumerator outside its enumeration's definition");
                }
            }
            Payload::Enum { def: Some(_), .. } => st.open_enum = Some(i),
            _ => st.open_enum = None,
        }
        if !matches!(payload, Payload::Enumerator { .. }) && !tmpl_of_open {
            // Close the classes this declaration is not a member of.
            while let Some(&top) = st.open.last() {
                if h.lexical == NONE && h.ctx == top {
                    break;
                }
                self.close_class(top);
                let _ = st.open.pop();
            }
            let class_ctx = h.ctx != NONE
                && matches!(self.img.decls[h.ctx as usize].payload, Payload::Record { .. });
            if class_ctx && st.open.is_empty() && (h.lexical == NONE || h.prev == NONE) {
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
        let attrs = self.attr_recs(d, i)?;
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
        self.attr_commit(i, attrs);
        if let Some(k) = self.using_key(d) {
            self.usings.insert(k, i);
        }
        if class_def {
            st.open.push(i);
        }
        Ok(())
    }
}

impl<'s, 'a, 'b> Writer<'s, 'a, 'b> {
    fn new(s: &'s Sema<'a, 'b>, main: FileId, census: bool) -> W<Self> {
        let lt = LocTable::new(s.diags.sm(), main)?;
        Ok(Writer {
            s,
            main,
            lt: std::cell::RefCell::new(lt),
            recs: FxHashMap::default(),
            names: FxHashMap::default(),
            pnames: FxHashMap::default(),
            tmemo: FxHashMap::default(),
            img: Image { preamble: NONE, ..Image::default() },
            bs: BodyState::default(),
            overrides: Vec::new(),
            keys: Vec::new(),
            usings: FxHashMap::default(),
            census: census.then(Census::default),
            pch: false,
        })
    }
}

/// The declaration kinds the image carries; the fallback reason for the
/// others.
fn kind_gate(k: DeclKind) -> W<()> {
    use DeclKind as K;
    match k {
        K::Namespace
        | K::Export
        | K::LinkageSpec
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
        | K::CXXConversion
        | K::FunctionTemplate
        | K::ClassTemplate
        | K::TypeAliasTemplate
        | K::VarTemplate
        | K::TemplateTypeParm
        | K::NonTypeTemplateParm
        | K::NamespaceAlias
        | K::UsingDirective
        | K::Using
        | K::UsingEnum
        | K::UsingShadow => Ok(()),
        K::IndirectField => Err("an anonymous union or struct member"),
        K::Friend => Err("a friend declaration"),
        K::ClassTemplateSpecialization
        | K::ClassTemplatePartialSpecialization
        | K::Concept
        | K::VarTemplateSpecialization
        | K::VarTemplatePartialSpecialization
        | K::FriendTemplate
        | K::CXXDeductionGuide
        | K::TemplateTemplateParm => Err("a template"),
        K::ConstructorUsingShadow => Err("an inheriting constructor"),
        K::UsingPack | K::UnresolvedUsingValue | K::UnresolvedUsingTypename => {
            Err("a dependent using-declaration")
        }
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
        let mut w = match Writer::new(self, main, false) {
            Ok(w) => w,
            Err(why) => return none(why),
        };
        if let Err(why) = w.collect() {
            return none(why);
        }
        (w.img.files, w.img.slocs) = w.lt.into_inner().finish();
        w.img.gated = gated_names(&w.img);
        let decls = w.img.decls.len();
        let bytes = encode(&w.img);
        // The writer's own output must pass the reader's checks (a body
        // shape the schema and the AST disagree on is a fallback, never
        // a corrupt module file).
        if decode(&bytes).is_err() {
            return none("an internal image inconsistency");
        }
        Some(ModuleImage { bytes, decls, fallback: None })
    }

    /// `-fkiln-bmi-image-census`: runs the writer over the main unit and
    /// counts, per top-level declaration it cannot carry, the first
    /// fallback reason (spec/modules.md §11.11 "Fallback census"). No
    /// module file changes. `None` outside a module unit.
    pub fn image_census(&mut self) -> Option<ImageCensus> {
        let sm = self.diags.sm();
        let (_, kw_loc, _) = self.module_main_frame()?;
        let (main, _) = sm.offset_of(kw_loc)?;
        let mut out = ImageCensus::default();
        let mut w = match Writer::new(self, main, true) {
            Ok(w) => w,
            Err(why) => {
                out.unit = Some(why);
                return Some(out);
            }
        };
        if let Err(why) = w.collect() {
            out.unit = Some(why);
        }
        out.module.clone_from(&w.img.module);
        let c = w.census.take().unwrap_or_default();
        out.top_level = c.seen.len();
        out.rows = c
            .rows
            .into_iter()
            .map(|(reason, count, example)| CensusRow { reason, count, example })
            .collect();
        out.rows.sort_by_key(|r| std::cmp::Reverse(r.count));
        out.attrs = c.attrs.into_iter().collect();
        out.attrs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        Some(out)
    }

    /// Builds the unit just begun from the declaration image `bytes` of
    /// module file `path`, whose file table the preprocessor loaded as
    /// `files` (index 0: the main file; spec/modules.md §11.11 "Part 2a",
    /// "File table"). A malformed image is the module file's corrupt-file
    /// fatal error at `at`, and nothing is built.
    pub fn load_module_image(
        &mut self,
        path: &str,
        bytes: &[u8],
        files: &[FileId],
        at: Loc,
        after_preamble: bool,
    ) {
        let Some(&main) = files.first() else { return };
        let len = self.diags.sm().text(main).len();
        let key = self.module_current_key().to_owned();
        let checked = decode(bytes).and_then(|img| {
            // S6: an image with a preamble is built at its end, in the
            // unit the replayed preamble entered, which must be the
            // image's; one without is built in place of the unit.
            if after_preamble != (img.preamble != NONE) {
                return bad("a preamble that does not match the import");
            }
            if after_preamble && (key != img.module || img.preamble as usize > len) {
                return bad("a preamble that does not enter the image's unit");
            }
            let Ok(bases) = locs::validate(&img.files, &img.slocs) else {
                return bad("bad location records");
            };
            if files.len() != img.files.len() {
                return bad("a file table the importer did not load");
            }
            if !self.image_builtins_ok(&img) {
                return bad("a builtin function the importer does not declare");
            }
            let off_ok = |o: u32| Locs::in_space(&bases, &img.files, &img.slocs, o);
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
                    Payload::Ttp { begin, .. } => offs.push(*begin),
                    Payload::Template { tloc, using, langle, rangle, .. } => {
                        offs.extend([*tloc, *using, *langle, *rangle]);
                    }
                    _ => {}
                }
            }
            body::body_locs(&img, &mut offs);
            classtempl::class_spec_locs(&img, &mut offs);
            declocs::decl_loc_offsets(&img, &mut offs);
            if img.kw == NONE || img.name_loc == NONE || !offs.into_iter().all(off_ok) {
                return bad("an offset past the end of the module's source");
            }
            match Locs::build(self.diags.sm(), &img.files, &img.slocs, &bases, files) {
                Ok(m) => Ok((img, m)),
                Err(why) => bad(why),
            }
        });
        match checked {
            Ok((img, m)) => self.build_image(&img, &m),
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
    fn build_image(&mut self, img: &Image, main: &Locs) {
        let lo = |_: &Self, o: u32| main.get(o);
        let (kw, name_loc) = (lo(self, img.kw), lo(self, img.name_loc));
        // A precompiled header's image (no module name) is built in the
        // translation unit's global module (WP-1036).
        if img.preamble == NONE && !img.module.is_empty() {
            self.act_on_module_directive(
                kw,
                true,
                Directive::Module { name: &img.module, partition: None, kw_loc: name_loc },
            );
        }
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
        let mut nttp_defaults: Vec<(DeclId, u32, bool)> = Vec::new();
        // S4: the pattern records, built as the parser builds a pattern.
        let patterns: FxHashSet<u32> = img
            .decls
            .iter()
            .filter_map(|d| match d.payload {
                Payload::Template { pattern, .. } => Some(pattern),
                _ => None,
            })
            .collect();
        let mut attr_exprs = attrs::AttrExprs::new();
        let saved_depth = self.ast.templates.dependent_depth;
        for (i, r) in img.decls.iter().enumerate() {
            // A dependent record (template parameter, pattern, local of a
            // pattern) is built inside a template parameter scope, so the
            // builder marks it (and a pattern's parameters) dependent.
            self.ast.templates.dependent_depth = saved_depth + u32::from(r.flags & DEP != 0);
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
                n if body::is_local(img, r)
                    || matches!(r.payload, Payload::Ttp { .. } | Payload::Nttp { .. }) =>
                {
                    pnames[n as usize]
                }
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
            let mut prev = at(r.prev);
            if prev.is_none() && matches!(r.payload, Payload::Enum { .. }) {
                prev = self.image_enum_prev(&h);
            }
            let d = match &r.payload {
                Payload::Namespace => self.image_namespace(img, i as u32, h, prev),
                Payload::Export => self.image_scope(img, i as u32, ctx, h),
                Payload::Function { kind, ty, spec, deferred, params, body, inits: ci, .. } => {
                    let mut fty = self.image_ty(img, &mut tys, &ids, *ty);
                    let spec = fn_spec(*spec).unwrap_or_default();
                    let kind = FN_KINDS[usize::from(*kind)];
                    h.name = self.image_special_name(r.name, kind, ctx, fty).unwrap_or(h.name);
                    if *deferred {
                        // The exception specification names the function
                        // itself (its first declaration).
                        let next = self.ast.decls.next_id();
                        fty = self.with_exc_spec(
                            fty,
                            ExcSpec::Deferred(prev.map_or(next, |p| self.ast.first(p))),
                        );
                    }
                    if patterns.contains(&(i as u32)) {
                        self.ast.templates.pattern_next = true;
                    }
                    let f = self.builder().function(h, kind, fty, spec, prev);
                    self.ast.templates.pattern_next = false;
                    if *deferred && kind == DeclKind::CXXDestructor && prev.is_none() {
                        // Written without one ([except.spec]/8): a
                        // redeclaration may omit it too (WP-1036).
                        self.implicit_exc_dtors.insert(f);
                    }
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
                    if patterns.contains(&(i as u32)) {
                        self.ast.templates.pattern_next = true;
                    }
                    let v = self.builder().var(h, vty, spec, prev);
                    self.ast.templates.pattern_next = false;
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
                    if patterns.contains(&(i as u32)) {
                        self.ast.templates.pattern_next = true;
                    }
                    let t = self.builder().typedef(h, kind, u, prev);
                    self.ast.templates.pattern_next = false;
                    t
                }
                Payload::Record { tag, def } => {
                    let tag = tag_of(*tag);
                    if patterns.contains(&(i as u32)) {
                        self.ast.templates.pattern_next = true;
                    }
                    let c = if classtempl::partial_of(img, i as u32).is_some()
                        || classtempl::explicit_of(img, i as u32).is_some()
                    {
                        self.image_class_spec(img, &mut tys, &ids, main, i as u32, h, tag)
                    } else {
                        self.builder().record(h, tag, prev)
                    };
                    self.ast.templates.pattern_next = false;
                    if let Some(df) = def {
                        let _ = self.builder().begin_record_definition(c);
                        let bases: Vec<kiln_ast::decl::BaseSpec> = df
                            .bases
                            .iter()
                            .map(|b| kiln_ast::decl::BaseSpec {
                                ty: self.image_ty(img, &mut tys, &ids, b.ty),
                                loc: lo(self, b.loc),
                                is_virtual: b.virt,
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
                Payload::Implicit { kind: scope::BUILTIN_KIND } => {
                    self.image_builtin(img, i as u32)
                }
                Payload::Implicit { kind } => {
                    let k = crate::class::SpecialKind::ALL[usize::from(*kind).min(5)];
                    // Declaring one implicit member can declare another
                    // that a later record names (a derived class's
                    // default constructor declares its base's, WP-1120):
                    // that record takes the member already declared.
                    let f = if self.implicit_bits(ctx) & k.bit() != 0
                        && let Some(f) = self.ast.members(self.class_def(ctx)).find(|&m| {
                            self.ast.decls.get(m).flags().contains(DeclFlags::IMPLICIT)
                                && self.special_kind_of(m) == Some(k)
                        }) {
                        f
                    } else {
                        self.declare_implicit(ctx, k)
                    };
                    defaulted.push((f, r.flags));
                    f
                }
                Payload::Ttp { depth, index, typename_kw, pack, begin, default } => {
                    let t =
                        self.builder().template_type_parm(h, *depth, *index, *pack, *typename_kw);
                    if *begin != NONE {
                        let _ = self.ast.decls.cold.begin_loc.insert(t, lo(self, *begin));
                    }
                    if let Some(q) = default {
                        let dq = self.image_ty(img, &mut tys, &ids, *q);
                        let _ = self.builder().set_template_default(t, TemplateArg::ty(dq));
                    }
                    t
                }
                Payload::Nttp { ty, depth, index, pack, default, profile } => {
                    let pty = self.image_ty(img, &mut tys, &ids, *ty);
                    let t = self.builder().non_type_template_parm(h, pty, *depth, *index, *pack);
                    if *default != NONE {
                        nttp_defaults.push((t, *default, *profile));
                    }
                    t
                }
                Payload::Template { kind, pattern, tloc, using, langle, rangle, depth, params } => {
                    let ps: Vec<DeclId> = params.iter().map(|&p| ids[p as usize]).collect();
                    let (t, l, rr) = (lo(self, *tloc), lo(self, *langle), lo(self, *rangle));
                    let list = self.builder().template_param_list(t, l, rr, *depth, &ps, None);
                    let pat = ids[*pattern as usize];
                    let k = match kind {
                        1 => DeclKind::ClassTemplate,
                        2 => DeclKind::TypeAliasTemplate,
                        3 => DeclKind::VarTemplate,
                        _ => DeclKind::FunctionTemplate,
                    };
                    let t = self.builder().template_decl(k, pat, list, None);
                    if *using != NONE {
                        let _ = self.templ.alias_using_loc.insert(pat, lo(self, *using));
                    }
                    t
                }
                Payload::Spec { .. } => {
                    let loc = lo(self, r.loc);
                    self.image_spec(img, &mut tys, &ids, r, loc).unwrap_or(self.ast.tu)
                }
                Payload::Member { .. } => self.image_member(&ids, r),
                Payload::Using { steps } => {
                    if let Some(op) = r
                        .name
                        .checked_sub(N_OP)
                        .and_then(|o| u8::try_from(o).ok())
                        .and_then(OverloadedOperator::from_u8)
                    {
                        h.name = DeclName::operator(op);
                    }
                    let nns =
                        self.image_nns(steps, &ids, &mut |s, q| s.image_ty(img, &mut tys, &ids, q));
                    self.builder().using_decl(h, nns)
                }
                Payload::Shadow { target } => {
                    let t = ids[*target as usize];
                    h.name = self.ast.decls.get(t).name();
                    self.builder().using_shadow(h, t, prev)
                }
                Payload::UsingDir { ns } => self.image_using_directive(h, ids[*ns as usize]),
                Payload::NsAlias { target } => {
                    self.builder().namespace_alias(h, ids[*target as usize], prev)
                }
                Payload::UsingEnum { e } => self.builder().using_enum(h, ids[*e as usize]),
            };
            // A builtin function is the importer's own declaration.
            let builtin = r.payload == Payload::Implicit { kind: scope::BUILTIN_KIND };
            if !builtin && !matches!(r.payload, Payload::Spec { .. } | Payload::Member { .. }) {
                image_flags(self, d, r.flags);
            }
            if !builtin {
                self.image_attrs(img, i as u32, d, main, &mut attr_exprs);
            }
            debug_assert_eq!(ids.len(), i);
            ids.push(d);
            self.image_anon_typedef(img, i as u32, &ids);
        }
        self.ast.templates.dependent_depth = saved_depth;
        if let Some((e, computed)) = open.take() {
            self.image_complete_enum(img, &mut tys, &ids, e, computed);
        }
        while let Some((c, _, bits)) = classes.pop() {
            self.builder().complete_record_definition(c, DefinitionBits::from_bits(bits));
        }
        self.image_virtuals(img, &ids);
        self.image_decl_locs(img, &ids, |o| main.get(o));
        let built = self.image_bodies(img, &mut tys, &ids, &params_all, main);
        self.image_attr_exprs(&attr_exprs, &built);
        self.image_nttp_defaults(&nttp_defaults, &built);
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
                        CtorInitKind::VirtualBase => {
                            let q = QRef { ty: x.target, quals: 0 };
                            self.image_ty(img, &mut tys, &ids, q).ty().index()
                        }
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
        // A virtual destructor defined in the module: the `operator
        // delete` its deleting destructor calls ([class.dtor]/16), as
        // sema finds it at the definition (WP-1120).
        for (i, r) in img.decls.iter().enumerate() {
            if let Payload::Function { kind: 3, body, .. } = r.payload
                && body != FnBody::None
                && let Some(class) = self.ast.decls.get(ids[i]).ctx()
            {
                self.check_destructor_delete(ids[i], class);
            }
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
        self.image_mark_specs(img, &ids, main);
        // Defaulted and implicit special members the module used are
        // defined again (spec/modules.md §11.11 "Classes").
        for (f, flags) in defaulted {
            let loc =
                self.ast.decls.get(f).ctx().map_or(Loc::INVALID, |c| self.ast.decls.get(c).loc());
            if flags & DeclFlags::USED.bits() != 0 {
                self.mark_function_referenced(f, loc);
            } else if flags & DeclFlags::REFERENCED.bits() != 0 {
                self.exc_spec_needed(f, loc);
            }
        }
        self.module_owner_changed();
    }

    /// The virtual function relations of the image's classes (WP-1120):
    /// the functions each first declaration overrides (a redeclaration
    /// shares its first declaration's list, as `finish_virtual_decl`
    /// does) and each class's key function. Implicit members computed
    /// their own when declared.
    fn image_virtuals(&mut self, img: &Image, ids: &[DeclId]) {
        for (i, r) in img.decls.iter().enumerate() {
            match &r.payload {
                Payload::Function { overrides, .. } if !overrides.is_empty() => {
                    let os: Vec<DeclId> =
                        overrides.iter().map(|&o| self.ast.first(ids[o as usize])).collect();
                    let _ = self.builder().set_overrides(ids[i], &os);
                }
                Payload::Record { def: Some(df), .. } if df.key != NONE => {
                    let _ = self.ast.decls.cold.key_function.insert(ids[i], ids[df.key as usize]);
                }
                _ => {}
            }
        }
        for (i, r) in img.decls.iter().enumerate() {
            if let Payload::Function { .. } = r.payload
                && r.prev != NONE
            {
                let f = ids[i];
                let first = self.ast.first(f);
                if let Some(&l) = self.ast.decls.cold.overrides.get(first)
                    && !self.ast.decls.cold.overrides.contains(f)
                {
                    let _ = self.ast.decls.cold.overrides.insert(f, l);
                }
            }
        }
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
            N_CTOR if kind == DeclKind::CXXConstructor => {
                Some(self.ast.names.constructor(class()?))
            }
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

    /// S6: a using-directive nominating namespace `ns` (a first
    /// declaration), as [`Sema::using_directive`] makes one at namespace
    /// scope: in its context's member list and in the lookup tables of
    /// the namespace (or translation unit) it appears in.
    fn image_using_directive(&mut self, h: DeclHead, ns: DeclId) -> DeclId {
        let d = self.ast.decls.push(
            kiln_ast::decl::NewDecl {
                kind: DeclKind::UsingDirective,
                flags: h.flags,
                misc: kiln_ast::decl::DeclMisc::default(),
                name: DeclName::EMPTY,
                ctx: Some(h.ctx),
                loc: h.loc,
                ty: None,
            },
            kiln_ast::decl::Payload::None(kiln_ast::ids::RawId::raw(ns)),
        );
        if self.ast.decls.member_list(h.ctx).is_some() {
            self.ast.decls.append_member(h.ctx, d);
        }
        // An `export` block is transparent: the directive is the
        // enclosing namespace's.
        let mut at = h.ctx;
        for _ in 0..1024 {
            if self.ast.decls.get(at).kind() != DeclKind::Export {
                break;
            }
            match self.ast.decls.get(at).ctx() {
                Some(c) => at = c,
                None => break,
            }
        }
        self.ast.add_using_directive(at, ns);
        d
    }

    /// The nested-name-specifier of qualifier `steps`; `ty` builds a
    /// type step's type.
    fn image_nns(
        &mut self,
        steps: &[NnsRec],
        ids: &[DeclId],
        ty: &mut dyn FnMut(&mut Self, QRef) -> QualType,
    ) -> Option<NnsId> {
        let mut cur: Option<NnsId> = None;
        for st in steps {
            let n = match *st {
                NnsRec::Global => Nns::Global,
                NnsRec::Ns(d) => match ids.get(d as usize) {
                    Some(&ns) => Nns::Namespace { prefix: cur, ns },
                    None => Nns::Global,
                },
                NnsRec::Ty(q, template_kw) => {
                    Nns::Type { prefix: cur, ty: ty(self, q), template_kw }
                }
                // S6: an alias step, over the namespace it denotes.
                NnsRec::Alias(d) => match ids.get(d as usize) {
                    Some(&alias) => Nns::NamespaceAlias {
                        prefix: cur,
                        alias,
                        ns: self.ast.resolve_alias(alias),
                    },
                    None => Nns::Global,
                },
            };
            cur = Some(self.ast.types.nns(n));
        }
        cur
    }

    /// An exception-specification byte: 5 and 6 are `noexcept(true)` and
    /// `noexcept(false)` with a bool literal, the rest [`exc_spec`]'s.
    fn image_exc_spec(&mut self, exc: u8) -> ExcSpec {
        match exc {
            5 | 6 => {
                let v = exc == 5;
                let lit = self.builder().bool_literal(QualType::BOOL, v, Loc::INVALID);
                let cv = self.int_const(QualType::BOOL, i128::from(v));
                let expr = self.builder().constant_expr(lit, Some(cv));
                let value = if v { NoexceptValue::True } else { NoexceptValue::False };
                ExcSpec::NoexceptExpr { expr, value }
            }
            e => exc_spec(e).unwrap_or_default(),
        }
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
            TyRec::ElabQ(k, steps, q) => {
                let cur = self.image_nns(steps, ids, &mut |s, q| qt(s, q));
                let q = qt(self, *q);
                let k = tag_keyword(*k).unwrap_or_default();
                self.ast.types.elaborated(k, cur, q)
            }
            TyRec::Func { ret, params, info, exc } => {
                let r = qt(self, *ret);
                let ps: Vec<QualType> = params.iter().map(|&p| qt(self, p)).collect();
                let exc = self.image_exc_spec(*exc);
                self.ast.types.function(r, &ps, FuncInfo::from_bits(*info), exc)
            }
            TyRec::Enum(d) | TyRec::Typedef(d) | TyRec::Record(d) | TyRec::Ttp(d) => {
                decl_ty(self, *d)
            }
            TyRec::QArray(c, q) => {
                let q = qt(self, *q);
                self.ast.types.qualify(q, Quals::from_bits(*c))
            }
            TyRec::Sugar(k) => self.builtin_sugar()[usize::from(*k).min(2)],
            TyRec::Tst { template, args, under } => {
                let args: Vec<TemplateArg> = args
                    .iter()
                    .map(|a| {
                        let ty = qt(self, a.q());
                        self.image_arg(ty, a)
                    })
                    .collect();
                let under = under.map(|u| qt(self, u));
                let tmpl = ids.get(*template as usize).map_or(self.ast.tu, |&t| self.ast.first(t));
                let alias = self.ast.decls.get(tmpl).kind() == DeclKind::TypeAliasTemplate;
                let name = self.ast.types.template_name_of(tmpl);
                let list = self.ast.types.arg_list(&args);
                self.ast.types.template_specialization(name, list, under, alias)
            }
            TyRec::Injected(t) => match ids.get(*t as usize) {
                // A partial specialization's (WP-1283).
                Some(&p) if self.ast.partial_spec_info(p).is_some() => {
                    self.partial_injected_type(p).unwrap_or(QualType::INT)
                }
                Some(&t) => self.injected_type_of(t).unwrap_or(QualType::INT),
                None => QualType::INT,
            },
            TyRec::Pack(q) => {
                let q = qt(self, *q);
                self.ast.types.pack_expansion(q, None)
            }
            TyRec::Auto(k) => {
                let kw = if *k & 1 != 0 { AutoKeyword::DecltypeAuto } else { AutoKeyword::Auto };
                let deduced = (*k & 2 != 0).then_some(QualType::DEPENDENT);
                self.ast.types.auto_type(kw, deduced, None, None, false)
            }
            TyRec::Subst { repl, assoc, index, pack } => {
                let r = qt(self, *repl);
                let a = ids.get(*assoc as usize).copied().unwrap_or(self.ast.tu);
                let pack = pack.checked_sub(1);
                self.ast.types.subst_template_type_parm(r, a, *index, pack)
            }
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
    pub(super) fn sample() -> Image {
        let q = |ty| QRef { ty, quals: 0 };
        let int = BuiltinType::Int as u32;
        Image {
            main: "m.cppm".into(),
            files: vec![FileRec {
                name: "m.cppm".into(),
                path: "/m.cppm".into(),
                len: 4096,
                hash: 0,
                system: false,
                text: None,
            }],
            slocs: vec![SlocRec::File { file: 0, from: NONE, marks: Vec::new() }],
            module: "m".into(),
            kw: 0,
            name_loc: 7,
            preamble: NONE,
            names: ["ns", "f", "E", "a", "T", "v", "g"].map(String::from).to_vec(),
            gated: Vec::new(),
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
                        overrides: vec![],
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
                        overrides: vec![],
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
                        deps: 0,
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
                        deps: 0,
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
                        deps: 0,
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
            attrs: attrs::Attrs::default(),
            scopes: scope::Scopes::default(),
            cspecs: classtempl::ClassSpecs::default(),
            declocs: declocs::DeclLocs::default(),
        }
    }

    #[test]
    fn round_trip() {
        let mut img = sample();
        img.gated = gated_names(&img);
        assert!(!img.gated.is_empty());
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

    /// Stage S3: `sample` plus classes. `struct B {}; struct S : B { int m;
    /// S() : B(), m(7) {} };` with S's implicit destructor, and a
    /// variable of type `ns::S`.
    fn class_sample() -> Image {
        let q = |ty| QRef { ty, quals: 0 };
        let int = BuiltinType::Int as u32;
        let void = BuiltinType::Void as u32;
        let mut img = sample();
        let base = img.names.len() as u32;
        img.names.extend(["B", "S", "m", "w"].map(String::from));
        let t = BUILTINS + 1 + img.types.len() as u32;
        img.types.extend([
            TyRec::Record(10),
            TyRec::Record(11),
            TyRec::Func { ret: q(void), params: vec![], info: 0, exc: 0 },
            TyRec::ElabQ(0, vec![NnsRec::Ns(0)], q(t + 1)),
        ]);
        let head = |name, ctx, access, loc, payload| DeclRec {
            flags: 0,
            owner: 3,
            access,
            name,
            ctx,
            lexical: NONE,
            loc,
            prev: NONE,
            payload,
        };
        let def = |end, bases| Payload::Record {
            tag: 0,
            def: Some(RecDef { bits: 0, end, key: NONE, bases }),
        };
        img.decls.extend([
            head(base, 1, 0, 200, def(11, vec![])),
            head(
                base + 1,
                1,
                0,
                210,
                def(14, vec![BaseRec { ty: q(t), loc: 212, access: 0, virt: false }]),
            ),
            head(
                base + 2,
                11,
                1,
                214,
                Payload::Field { ty: q(int), flags: 0, width: NONE, init: NONE },
            ),
            head(
                N_CTOR,
                11,
                1,
                216,
                Payload::Function {
                    kind: 2,
                    deferred: false,
                    overrides: vec![],
                    inits: vec![
                        InitRec { kind: 0, target: 0, written: 0, init: 0, loc: 220 },
                        InitRec { kind: 2, target: 12, written: 1, init: 0, loc: 224 },
                    ],
                    ty: q(t + 2),
                    spec: 0,
                    params: vec![],
                    body: FnBody::Braces(230, 231),
                },
            ),
            head(NONE, 11, 1, 210, Payload::Implicit { kind: 5 }),
            head(
                base + 3,
                1,
                0,
                240,
                Payload::Var { ty: q(t + 3), spec: 0, init: NONE, eval: NONE },
            ),
        ]);
        img
    }

    #[test]
    fn class_round_trip() {
        let img = class_sample();
        assert_eq!(decode(&encode(&img)), Ok(img));
    }

    /// [special]/6 (WP-1109, WP-1119): a class's `NONTRIVIAL_*` bits
    /// (57-61) and a special member's `not_selected` round-trip; a class
    /// bit past them is still an error.
    #[test]
    fn carries_eligibility_bits() {
        use kiln_ast::decl::DefinitionBits as B;
        let nontrivial = B::NONTRIVIAL_DEFAULT_CTOR.bits()
            | B::NONTRIVIAL_COPY_CTOR.bits()
            | B::NONTRIVIAL_MOVE_CTOR.bits()
            | B::NONTRIVIAL_COPY_ASSIGN.bits()
            | B::NONTRIVIAL_MOVE_ASSIGN.bits();
        assert_eq!(DEF_BITS & nontrivial, nontrivial);
        let set_bits = |i: &mut Image, v: u64| {
            if let Payload::Record { def: Some(d), .. } = &mut i.decls[11].payload {
                d.bits = v;
            }
        };
        let spec = FnSpec::new().with_not_selected(true);
        assert_eq!(fn_spec(spec.bits()), Some(spec));
        let mut img = class_sample();
        set_bits(&mut img, nontrivial | B::TRIVIAL_DEFAULT_CTOR.bits());
        if let Payload::Function { spec: s, .. } = &mut img.decls[13].payload {
            *s = spec.bits();
        }
        assert_eq!(decode(&encode(&img)), Ok(img));
        let mut img = class_sample();
        set_bits(&mut img, B::NONTRIVIAL_MOVE_ASSIGN.bits() << 1);
        assert!(decode(&encode(&img)).is_err());
    }

    /// spec/modules.md §11.11 "Part 2a" (stage S3): a class record that
    /// is malformed or inconsistent with its class is an error.
    #[test]
    fn rejects_corrupt_class_records() {
        type Corrupt = (&'static str, fn(&mut Image));
        let corrupt: [Corrupt; 13] = [
            ("implicit kind", |i| i.decls[14].payload = Payload::Implicit { kind: 7 }),
            ("duplicate implicit", |i| {
                let d = i.decls[14].clone();
                i.decls.insert(15, d);
            }),
            ("implicit inside its class", |i| i.decls[13].payload = Payload::Implicit { kind: 0 }),
            ("field outside a class", |i| i.decls[12].ctx = 1),
            ("namespace-scope access", |i| i.decls[10].access = 1),
            ("end before the record", |i| set_end(i, 11, 11)),
            ("end past the parent", |i| set_end(i, 11, 99)),
            ("base access", |i| set_base(i, |b| b.access = 4)),
            ("base not a class", |i| set_base(i, |b| b.ty.ty = BuiltinType::Int as u32)),
            ("base of itself", |i| set_base(i, |b| b.ty.ty = BUILTINS + 1 + 4)),
            ("base init position", |i| set_init(i, 0, 1)),
            ("field init target", |i| set_init(i, 1, 10)),
            ("constructor name", |i| i.decls[13].name = i.names.len() as u32 - 1),
        ];
        for (what, f) in corrupt {
            let mut img = class_sample();
            f(&mut img);
            assert!(decode(&encode(&img)).is_err(), "{what}");
        }
        // A qualifier naming a class as a namespace, and `::` after
        // another step.
        let mut img = class_sample();
        img.types[6] = TyRec::ElabQ(0, vec![NnsRec::Ns(10)], QRef { ty: BUILTINS + 5, quals: 0 });
        assert!(decode(&encode(&img)).is_err());
        let mut img = class_sample();
        img.types[6] = TyRec::ElabQ(
            0,
            vec![NnsRec::Ns(0), NnsRec::Global],
            QRef { ty: BUILTINS + 5, quals: 0 },
        );
        assert!(decode(&encode(&img)).is_err());
    }

    /// `class_sample` with S's constructor turned into a virtual member
    /// function `w` (WP-1120): S's key function, overriding the implicit
    /// destructor record (any first declaration of a method, destructor
    /// or implicit destructor is a valid target), over a virtual base.
    fn virtual_sample() -> Image {
        let mut img = class_sample();
        let w = img.names.len() as u32 - 1;
        img.decls[13].name = w;
        if let Payload::Function { kind, inits, overrides, spec, .. } = &mut img.decls[13].payload {
            *kind = 1;
            inits.clear();
            *overrides = vec![14];
            *spec = FnSpec::new().with_virtual(true).with_override(true).bits();
        }
        if let Payload::Record { def: Some(df), .. } = &mut img.decls[11].payload {
            df.key = 13;
            df.bases[0].virt = true;
        }
        img
    }

    /// [class.virtual], [class.mi]/4 (WP-1120): virtual functions, the
    /// functions they override, a key function and a virtual base
    /// round-trip, with the virt-specifier and `pure` bits; a malformed
    /// relation is an error.
    #[test]
    fn carries_virtual_records() {
        type Corrupt = (&'static str, fn(&mut Image));
        fn set_key(i: &mut Image, k: u32) {
            if let Payload::Record { def: Some(df), .. } = &mut i.decls[11].payload {
                df.key = k;
            }
        }
        fn set_ovr(i: &mut Image, o: Vec<u32>) {
            if let Payload::Function { overrides, .. } = &mut i.decls[13].payload {
                *overrides = o;
            }
        }
        let spec = FnSpec::new()
            .with_virtual(true)
            .with_pure(true)
            .with_override(true)
            .with_final(true)
            .with_virtual_written(true);
        assert_eq!(fn_spec(spec.bits()), Some(spec));
        let img = virtual_sample();
        assert_eq!(decode(&encode(&img)), Ok(img));
        let corrupt: [Corrupt; 6] = [
            ("key outside the class", |i| set_key(i, 14)),
            ("key not a function", |i| set_key(i, 12)),
            ("overrides itself", |i| set_ovr(i, vec![13])),
            ("overrides a field", |i| set_ovr(i, vec![12])),
            ("overrides past the image", |i| set_ovr(i, vec![99])),
            ("overrides on a redeclaration", |i| i.decls[13].prev = 0),
        ];
        for (what, f) in corrupt {
            let mut img = virtual_sample();
            f(&mut img);
            assert!(decode(&encode(&img)).is_err(), "{what}");
        }
        // A virtual base flag byte other than 0 or 1.
        let mut bytes = encode(&virtual_sample());
        let good = bytes.clone();
        let mut img = virtual_sample();
        set_base(&mut img, |b| b.virt = false);
        let plain = encode(&img);
        let at = good.iter().zip(&plain).position(|(a, b)| a != b).unwrap();
        bytes[at] = 2;
        assert!(decode(&bytes).is_err());
    }

    fn set_end(img: &mut Image, r: usize, end: u32) {
        if let Payload::Record { def: Some(df), .. } = &mut img.decls[r].payload {
            df.end = end;
        }
    }

    fn set_base(img: &mut Image, f: fn(&mut BaseRec)) {
        if let Payload::Record { def: Some(df), .. } = &mut img.decls[11].payload {
            f(&mut df.bases[0]);
        }
    }

    fn set_init(img: &mut Image, k: usize, target: u32) {
        if let Payload::Function { inits, .. } = &mut img.decls[13].payload {
            inits[k].target = target;
        }
    }

    /// [module.import] / spec/modules.md §11.11 "Part 2a": a damaged image
    /// (truncated, flipped bytes, corrupt lengths) is an error, never a
    /// panic.
    #[test]
    fn fuzz_never_panics() {
        fuzz(&encode(&sample()));
        fuzz(&encode(&class_sample()));
        fuzz(&encode(&template_sample()));
    }

    /// Stage S6: `sample` plus, at global scope, `export using ns::f;`
    /// (with its shadow), `export namespace na = ns;`, `export using
    /// enum ns::E;`, `using namespace ns;`, `export using na::v;` (an
    /// alias step) and a shadow of the first shadow.
    fn using_sample() -> Image {
        let mut img = sample();
        let na = img.names.len() as u32;
        img.names.push("na".into());
        let rec = |name, payload| DeclRec {
            flags: 0,
            owner: 3,
            access: 0,
            name,
            ctx: NONE,
            lexical: NONE,
            loc: 200,
            prev: NONE,
            payload,
        };
        img.decls.extend([
            rec(1, Payload::Using { steps: vec![NnsRec::Ns(0)] }),
            rec(1, Payload::Shadow { target: 2 }),
            rec(na, Payload::NsAlias { target: 0 }),
            rec(2, Payload::UsingEnum { e: 3 }),
            rec(NONE, Payload::UsingDir { ns: 0 }),
            rec(5, Payload::Using { steps: vec![NnsRec::Global, NnsRec::Alias(12)] }),
            rec(5, Payload::Shadow { target: 6 }),
            rec(1, Payload::Shadow { target: 11 }),
        ]);
        img.decls[14].owner = 1;
        img.gated = gated_names(&img);
        img
    }

    /// [namespace.udecl], [enum.udecl], [namespace.alias],
    /// [namespace.udir] (S6, WP-1068): the using family round-trips; a
    /// record naming the wrong kind of target, a later one, or carrying
    /// the wrong name is an error.
    #[test]
    fn carries_a_preamble() {
        // S6: the preamble word and a partition's key round-trip (the
        // preprocessor's read of the header is checked by the driver's
        // kind2_declaration_image_imports; spec/modules.md §11.11
        // "Imports, partitions and using-declarations").
        let mut img = using_sample();
        img.module = "m:p".to_owned();
        img.preamble = 40;
        let bytes = encode(&img);
        assert_eq!(decode(&bytes), Ok(img.clone()));
        // A truncated header is corrupt, not a panic.
        for n in [0, 4, 12, 20] {
            assert!(decode(&bytes[..n.min(bytes.len())]).is_err());
        }
    }

    #[test]
    fn carries_using_records() {
        type Corrupt = (&'static str, fn(&mut Image));
        let img = using_sample();
        assert!(img.gated.contains(&(img.names.len() as u32 - 1)));
        assert_eq!(decode(&encode(&img)), Ok(img));
        let corrupt: [Corrupt; 12] = [
            ("shadow of a later declaration", |i| {
                i.decls[11].payload = Payload::Shadow { target: 16 }
            }),
            ("shadow of itself", |i| i.decls[11].payload = Payload::Shadow { target: 11 }),
            ("shadow of a namespace", |i| i.decls[11].payload = Payload::Shadow { target: 0 }),
            ("shadow under another name", |i| i.decls[11].name = 2),
            ("directive naming an export block", |i| {
                i.decls[14].payload = Payload::UsingDir { ns: 1 };
            }),
            ("directive with a name", |i| i.decls[14].name = 0),
            ("alias of a function", |i| i.decls[12].payload = Payload::NsAlias { target: 2 }),
            ("alias of a later alias", |i| i.decls[12].payload = Payload::NsAlias { target: 12 }),
            ("using-enum of a typedef", |i| i.decls[13].payload = Payload::UsingEnum { e: 5 }),
            ("using-enum under another name", |i| i.decls[13].name = 0),
            ("alias step naming a namespace", |i| {
                i.decls[15].payload = Payload::Using { steps: vec![NnsRec::Alias(0)] };
            }),
            ("namespace step naming an alias", |i| {
                i.decls[15].payload = Payload::Using { steps: vec![NnsRec::Ns(12)] };
            }),
        ];
        for (what, f) in corrupt {
            let mut img = using_sample();
            f(&mut img);
            assert!(decode(&encode(&img)).is_err(), "{what}");
        }
        // A global step after the first is not a qualifier.
        let mut img = using_sample();
        img.decls[15].payload = Payload::Using { steps: vec![NnsRec::Alias(12), NnsRec::Global] };
        assert!(decode(&encode(&img)).is_err());
        fuzz(&encode(&using_sample()));
    }

    /// Stage S4: `sample` plus `namespace ns { template <class T, int N>
    /// T tf(T x); }`: the two parameter records, the pattern (dependent,
    /// unlisted) and the template record claiming them.
    fn template_sample() -> Image {
        let q = |ty| QRef { ty, quals: 0 };
        let mut img = sample();
        let name = img.names.len() as u32;
        img.names.push("tf".into());
        let pname = img.pnames.len() as u32;
        img.pnames.extend(["T", "N"].map(String::from));
        let t = BUILTINS + 1 + img.types.len() as u32;
        img.types.extend([
            TyRec::Ttp(10),
            TyRec::Func { ret: q(t), params: vec![q(t)], info: 0, exc: 0 },
        ]);
        let head = |flags, name, loc, payload| DeclRec {
            flags,
            owner: 3,
            access: 0,
            name,
            ctx: 1,
            lexical: NONE,
            loc,
            prev: NONE,
            payload,
        };
        img.decls.extend([
            head(
                DEP,
                pname,
                302,
                Payload::Ttp {
                    depth: 0,
                    index: 0,
                    typename_kw: false,
                    pack: false,
                    begin: 296,
                    default: None,
                },
            ),
            head(
                DEP,
                pname + 1,
                308,
                Payload::Nttp {
                    ty: q(BuiltinType::Int as u32),
                    depth: 0,
                    index: 1,
                    pack: false,
                    default: NONE,
                    profile: false,
                },
            ),
            head(
                DEP,
                name,
                314,
                Payload::Function {
                    kind: 0,
                    deferred: false,
                    inits: vec![],
                    overrides: vec![],
                    ty: q(t + 1),
                    spec: 0,
                    params: vec![ParamRec {
                        name: 0,
                        loc: 318,
                        ty: q(t),
                        pflags: 0x100,
                        flags: DEP,
                        default: NONE,
                    }],
                    body: FnBody::Braces(320, 321),
                },
            ),
            head(
                0,
                name,
                314,
                Payload::Template {
                    kind: 0,
                    pattern: 12,
                    tloc: 290,
                    using: NONE,
                    langle: 294,
                    rangle: 310,
                    depth: 0,
                    params: vec![10, 11],
                },
            ),
        ]);
        img
    }

    #[test]
    fn template_round_trip() {
        let img = template_sample();
        assert_eq!(decode(&encode(&img)), Ok(img));
    }

    /// spec/modules.md §11.11 "Templates as patterns" (stage S4): every
    /// dependent record belongs to exactly one template, whose pattern
    /// and parameters are well formed; anything else is an error.
    #[test]
    fn rejects_corrupt_template_records() {
        fn set_tpl(i: &mut Image, f: fn(&mut u32, &mut Vec<u32>)) {
            if let Payload::Template { pattern, params, .. } = &mut i.decls[13].payload {
                f(pattern, params);
            }
        }
        type Corrupt = (&'static str, fn(&mut Image));
        let corrupt: [Corrupt; 11] = [
            ("unclaimed parameter", |i| set_tpl(i, |_, p| p.truncate(1))),
            ("parameter order", |i| set_tpl(i, |_, p| p.swap(0, 1))),
            ("parameter twice", |i| set_tpl(i, |_, p| p[1] = 10)),
            ("pattern not dependent", |i| set_tpl(i, |p, _| *p = 7)),
            ("pattern is a parameter", |i| set_tpl(i, |p, _| *p = 11)),
            ("no template", |i| {
                i.decls.pop();
            }),
            ("dependent template", |i| i.decls[13].flags = DEP),
            ("independent parameter", |i| i.decls[10].flags = 0),
            ("pattern name", |i| i.decls[12].name = 0),
            ("pattern claimed twice", |i| {
                let t = i.decls[13].clone();
                i.decls.push(t);
            }),
            ("parameter depth", |i| {
                i.decls[11].payload = Payload::Nttp {
                    ty: QRef { ty: 1, quals: 0 },
                    depth: 1,
                    index: 1,
                    pack: false,
                    default: NONE,
                    profile: false,
                };
            }),
        ];
        for (what, f) in corrupt {
            let mut img = template_sample();
            f(&mut img);
            assert!(decode(&encode(&img)).is_err(), "{what}");
        }
    }

    /// Stage S4b: `template_sample` plus `namespace ns { template <class
    /// T = int> struct Box { T v; T w; }; Box<int> bx; }` and a use of the
    /// specialization's `v`: the pattern, the class template record
    /// just after it, the specialization and member reference records,
    /// and the specialization, injected-class-name and substituted
    /// parameter types.
    #[allow(clippy::too_many_lines)] // one sample image, its records in format order
    fn class_template_sample() -> Image {
        let q = |ty| QRef { ty, quals: 0 };
        let int = BuiltinType::Int as u32;
        let mut img = template_sample();
        let name = img.names.len() as u32;
        img.names.extend(["Box", "v", "w", "bx"].map(String::from));
        let pname = img.pnames.len() as u32;
        img.pnames.push("T".into());
        let t = BUILTINS + 1 + img.types.len() as u32;
        img.types.extend([
            TyRec::Ttp(14),
            TyRec::Injected(16),
            TyRec::Pointer(q(t + 1)),
            TyRec::Record(19),
            TyRec::Tst { template: 16, args: vec![ArgRec::Type(q(int))], under: Some(q(t + 3)) },
            TyRec::Subst { repl: q(int), assoc: 16, index: 0, pack: 0 },
        ]);
        let head = |flags, name, ctx, access, loc, payload| DeclRec {
            flags,
            owner: 3,
            access,
            name,
            ctx,
            lexical: NONE,
            loc,
            prev: NONE,
            payload,
        };
        let field = |ty| Payload::Field { ty, flags: 0, width: NONE, init: NONE };
        img.decls.extend([
            head(
                DEP,
                pname,
                1,
                0,
                402,
                Payload::Ttp {
                    depth: 0,
                    index: 0,
                    typename_kw: false,
                    pack: false,
                    begin: 396,
                    default: Some(q(int)),
                },
            ),
            head(
                DEP,
                name,
                1,
                0,
                420,
                Payload::Record {
                    tag: 0,
                    def: Some(RecDef { bits: 0, end: 19, key: NONE, bases: vec![] }),
                },
            ),
            head(
                0,
                name,
                1,
                0,
                420,
                Payload::Template {
                    kind: 1,
                    pattern: 15,
                    tloc: 390,
                    using: NONE,
                    langle: 394,
                    rangle: 410,
                    depth: 0,
                    params: vec![14],
                },
            ),
            head(DEP, name + 1, 15, 1, 430, field(q(t))),
            head(DEP, name + 2, 15, 1, 440, field(q(t + 2))),
            head(
                0,
                NONE,
                NONE,
                0,
                450,
                Payload::Spec { template: 16, complete: true, args: vec![ArgRec::Type(q(int))] },
            ),
            head(0, NONE, NONE, 0, NONE, Payload::Member { spec: 19, pattern: 17 }),
            head(
                0,
                name + 3,
                1,
                0,
                450,
                Payload::Var { ty: q(t + 4), spec: 0, init: NONE, eval: NONE },
            ),
            head(
                0,
                name + 3,
                1,
                0,
                460,
                Payload::Var { ty: q(t + 5), spec: 0, init: NONE, eval: NONE },
            ),
        ]);
        // A pattern body's `T{}.member`: a dependent construction and a
        // dependent member access (its name in `pnames`, its operator's
        // location a cold annotation).
        let e = img.bodies.exprs.len() as u32;
        img.pnames.push("member".into());
        img.bodies.exprs.extend([
            body::ExprRec {
                kind: 47,
                bits: 0,
                aux: 1,
                cold: 0,
                deps: 7,
                ty: q(t),
                loc: 470,
                words: vec![0],
            },
            body::ExprRec {
                kind: 48,
                bits: 1,
                aux: 0,
                cold: 16,
                deps: 7,
                ty: q(t),
                loc: 474,
                words: vec![e, pname + 1, 473],
            },
        ]);
        img
    }

    #[test]
    fn class_template_round_trip() {
        let img = class_template_sample();
        assert_eq!(decode(&encode(&img)), Ok(img.clone()));
        fuzz(&encode(&img));
    }

    /// spec/modules.md §11.11 "Class templates" and "Specialization
    /// references" (S4b): the class template follows its pattern, which
    /// claims its members; a specialization names a carried class
    /// template; a member reference names a specialization and a member
    /// of its template's pattern; anything else is an error.
    #[test]
    fn rejects_corrupt_class_template_records() {
        fn payload(i: &mut Image, r: usize, p: Payload) {
            i.decls[r].payload = p;
        }
        type Corrupt = (&'static str, fn(&mut Image));
        let corrupt: [Corrupt; 17] = [
            ("template not after its pattern", |i| i.decls.swap(16, 17)),
            ("member name out of range", |i| {
                let n = i.bodies.exprs.len();
                i.bodies.exprs[n - 1].words[1] = i.pnames.len() as u32;
            }),
            ("member access without its operator location", |i| {
                let n = i.bodies.exprs.len();
                i.bodies.exprs[n - 1].cold = 0;
            }),
            ("dependent member template keyword", |i| {
                let n = i.bodies.exprs.len();
                i.bodies.exprs[n - 1].aux = 2;
            }),
            ("construction argument out of range", |i| {
                let n = i.bodies.exprs.len();
                i.bodies.exprs[n - 2].words = vec![1, n as u32];
            }),
            ("independent member", |i| i.decls[17].flags = 0),
            ("spec of a function template", |i| {
                let p = Payload::Spec { template: 13, complete: true, args: vec![] };
                payload(i, 19, p);
            }),
            ("spec of a non-template", |i| {
                let p = Payload::Spec { template: 15, complete: false, args: vec![] };
                payload(i, 19, p);
            }),
            ("spec after its use", |i| {
                let p = Payload::Spec { template: 21, complete: false, args: vec![] };
                payload(i, 19, p);
            }),
            ("member of a non-spec", |i| payload(i, 20, Payload::Member { spec: 16, pattern: 17 })),
            ("member of a non-pattern", |i| {
                payload(i, 20, Payload::Member { spec: 19, pattern: 12 });
            }),
            ("member pattern of another template", |i| {
                payload(i, 20, Payload::Member { spec: 19, pattern: 11 });
            }),
            ("spec type naming a function template", |i| {
                let n = i.types.len();
                if let TyRec::Tst { template, .. } = &mut i.types[n - 2] {
                    *template = 13;
                }
            }),
            ("injected type of a function template", |i| {
                let n = i.types.len();
                i.types[n - 5] = TyRec::Injected(13);
            }),
            ("subst type naming a non-template", |i| {
                let n = i.types.len();
                i.types[n - 1] =
                    TyRec::Subst { repl: QRef { ty: 1, quals: 0 }, assoc: 15, index: 0, pack: 0 };
            }),
            ("default argument above its parameter", |i| {
                if let Payload::Ttp { default, .. } = &mut i.decls[14].payload {
                    *default = Some(QRef { ty: BUILTINS + 1 + i.types.len() as u32 - 1, quals: 0 });
                }
            }),
            ("spec argument out of range", |i| {
                if let Payload::Spec { args, .. } = &mut i.decls[19].payload {
                    args.push(ArgRec::Type(QRef { ty: u32::MAX - 1, quals: 0 }));
                }
            }),
        ];
        for (what, f) in corrupt {
            let mut img = class_template_sample();
            f(&mut img);
            assert!(decode(&encode(&img)).is_err(), "{what}");
        }
    }

    fn fuzz(bytes: &[u8]) {
        for n in 0..bytes.len() {
            assert!(decode(&bytes[..n]).is_err(), "prefix {n} decoded");
        }
        for i in 0..bytes.len() {
            for x in [0x01u8, 0x80, 0xff, 0x7f, 0x10] {
                let mut b = bytes.to_vec();
                b[i] ^= x;
                let _ = decode(&b);
            }
        }
        for i in 0..bytes.len().saturating_sub(3) {
            for v in [u32::MAX, u32::MAX / 2, 0x0100_0000, 0, NONE - 1] {
                let mut b = bytes.to_vec();
                b[i..i + 4].copy_from_slice(&v.to_le_bytes());
                let _ = decode(&b);
            }
        }
        let mut s: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..4000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let mut b = bytes.to_vec();
            let i = (s as usize) % b.len();
            b[i] = (s >> 32) as u8;
            let j = ((s >> 16) as usize) % b.len();
            b.truncate(j.max(i));
            let _ = decode(&b);
        }
    }
}
