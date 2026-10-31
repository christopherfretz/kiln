//! Declarations: the 28-byte [`DeclNode`] header, the per-kind payloads and
//! the cold side maps (spec/ast.md §4.1, §4.2, §2.4).
//!
//! Nodes are created through the `AstBuilder`; this module owns storage and
//! the write-once setters of the mutation discipline (§2.4).
#![warn(missing_docs)]

use kiln_source::Loc;

use crate::ids::{
    ConstValueId, DeclId, ExprId, LambdaId, ListId, QualType, RawId, RedeclGroupId, StmtId,
    StringLitId,
};
use crate::mem::{HeapSize, MemReport, MemRow};
use crate::names::DeclName;
use crate::slot::{AlreadySet, ColdMap, set_once};

// ---------------------------------------------------------------------------
// Kinds
// ---------------------------------------------------------------------------

/// The kind of a declaration (spec/ast.md §4.2). Names are clang's
/// (`<Kind>Decl` in the dump). The numbering is stable.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum DeclKind {
    // scopes
    /// The translation unit (★, payload [`ScopeData`]).
    TranslationUnit,
    /// A namespace (★, R, payload [`NamespaceData`]).
    Namespace,
    /// A namespace alias (R, M3+).
    NamespaceAlias,
    /// `extern "C"` / `extern "C++"` (★ transparent, payload [`ScopeData`],
    /// `aux` = [`LinkageLang`]).
    LinkageSpec,
    /// `export { ... }` (★ transparent, payload [`ScopeData`], M4).
    Export,
    /// An empty declaration `;`.
    Empty,
    /// A file-scope `asm(...)`.
    FileScopeAsm,
    // using
    /// `using namespace N;`.
    UsingDirective,
    /// A using-declaration.
    Using,
    /// `using enum E;`.
    UsingEnum,
    /// A shadow declaration introduced by a using-declaration.
    UsingShadow,
    /// A shadow constructor introduced by an inheriting using-declaration.
    ConstructorUsingShadow,
    /// A using-declaration pack expansion.
    UsingPack,
    /// A dependent value using-declaration (M3).
    UnresolvedUsingValue,
    /// A dependent `typename` using-declaration (M3).
    UnresolvedUsingTypename,
    /// A `__using_if_exists` placeholder (M3).
    UnresolvedUsingIfExists,
    // types
    /// `typedef` (R, payload [`TypedefData`]).
    Typedef,
    /// An alias-declaration `using A = T;` (R, T, payload [`TypedefData`]).
    TypeAlias,
    /// An enumeration (★, R, T, payload [`EnumData`]).
    Enum,
    /// An enumerator (payload [`EnumeratorData`]).
    EnumConstant,
    /// A class, struct or union (★, R, T, payload [`RecordData`]).
    CXXRecord,
    /// A class template specialization (★, R, payload [`RecordData`], M3).
    ClassTemplateSpecialization,
    /// A class template partial specialization (★, R, payload
    /// [`RecordData`], M3).
    ClassTemplatePartialSpecialization,
    // members
    /// A non-static data member (payload [`FieldData`]).
    Field,
    /// A member of an anonymous struct/union seen from the enclosing scope;
    /// `data` is the raw `ListId` of the chain of fields (outermost first).
    IndirectField,
    /// `public:` / `protected:` / `private:` (the access is
    /// `DeclNode.misc`'s; no payload).
    AccessSpec,
    /// A friend declaration: `data` is the raw `DeclId` of the befriended
    /// declaration (0 for a friend type, which is `DeclNode.ty`).
    Friend,
    /// A friend template declaration.
    FriendTemplate,
    // functions
    /// A function (R, T, payload [`FunctionData`]).
    Function,
    /// A member function (R, T, payload [`FunctionData`]).
    CXXMethod,
    /// A constructor (R, T, payload [`FunctionData`]).
    CXXConstructor,
    /// A destructor (R, T, payload [`FunctionData`]).
    CXXDestructor,
    /// A conversion function (R, T, payload [`FunctionData`]).
    CXXConversion,
    /// A deduction guide (T, payload [`FunctionData`]).
    CXXDeductionGuide,
    // variables
    /// A variable (R, T, payload [`VarData`]).
    Var,
    /// A function parameter (payload [`ParamData`]).
    ParmVar,
    /// An implicit parameter (`this`, coroutine frame).
    ImplicitParam,
    /// A structured-binding declaration.
    Decomposition,
    /// One structured binding.
    Binding,
    /// A variable template specialization (R, payload [`VarData`], M3).
    VarTemplateSpecialization,
    /// A variable template partial specialization (R, payload [`VarData`],
    /// M3).
    VarTemplatePartialSpecialization,
    // templates
    /// A class template (R, M3).
    ClassTemplate,
    /// A function template (R, M3).
    FunctionTemplate,
    /// A variable template (R, M3).
    VarTemplate,
    /// An alias template (R, M3).
    TypeAliasTemplate,
    /// A concept (M3).
    Concept,
    /// `__make_integer_seq` / `__type_pack_element` (M3).
    BuiltinTemplate,
    /// A type template parameter (M3).
    TemplateTypeParm,
    /// A non-type template parameter (M3; expressions naming one are
    /// value-dependent, [temp.dep.constexpr]).
    NonTypeTemplateParm,
    /// A template template parameter (M3).
    TemplateTemplateParm,
    /// An implicit concept specialization (M3).
    ImplicitConceptSpecialization,
    /// The parameter scope of a requires-expression (★, M3).
    RequiresExprBody,
    // synthesized
    /// A lifetime-extended temporary in a constant initializer.
    LifetimeExtendedTemporary,
    /// A class-type non-type template parameter object.
    TemplateParamObject,
    /// `static_assert` (payload [`StaticAssertData`]).
    StaticAssert,
    /// A label.
    Label,
    /// The result name of a postcondition (P2900).
    ContractResultName,
}

/// Which payload arena a kind's `data` indexes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PayloadClass {
    /// No payload (`data` is 0) or an inline payload documented on the kind.
    None,
    /// [`FunctionData`].
    Function,
    /// [`VarData`].
    Var,
    /// [`RecordData`].
    Record,
    /// [`FieldData`].
    Field,
    /// [`EnumData`].
    Enum,
    /// [`EnumeratorData`].
    Enumerator,
    /// [`ParamData`].
    Param,
    /// [`TypedefData`].
    Typedef,
    /// [`NamespaceData`].
    Namespace,
    /// [`ScopeData`].
    Scope,
    /// [`StaticAssertData`].
    StaticAssert,
    /// [`LetData`] (`LifetimeExtendedTemporary`).
    Let,
}

impl DeclKind {
    /// The payload arena of this kind.
    pub const fn payload(self) -> PayloadClass {
        use DeclKind as K;
        match self {
            K::Function
            | K::CXXMethod
            | K::CXXConstructor
            | K::CXXDestructor
            | K::CXXConversion
            | K::CXXDeductionGuide => PayloadClass::Function,
            K::Var | K::VarTemplateSpecialization | K::VarTemplatePartialSpecialization => {
                PayloadClass::Var
            }
            K::CXXRecord
            | K::ClassTemplateSpecialization
            | K::ClassTemplatePartialSpecialization => PayloadClass::Record,
            K::Field => PayloadClass::Field,
            K::Enum => PayloadClass::Enum,
            K::EnumConstant => PayloadClass::Enumerator,
            K::ParmVar => PayloadClass::Param,
            K::Typedef | K::TypeAlias => PayloadClass::Typedef,
            K::Namespace => PayloadClass::Namespace,
            K::TranslationUnit | K::LinkageSpec | K::Export | K::RequiresExprBody => {
                PayloadClass::Scope
            }
            K::StaticAssert => PayloadClass::StaticAssert,
            K::LifetimeExtendedTemporary => PayloadClass::Let,
            _ => PayloadClass::None,
        }
    }

    /// True for kinds that own a member list (★ in §4.2).
    pub const fn is_context(self) -> bool {
        matches!(
            self.payload(),
            PayloadClass::Record
                | PayloadClass::Enum
                | PayloadClass::Namespace
                | PayloadClass::Scope
        )
    }

    /// True for function kinds (semantic context of their parameters and
    /// locals, owning neither a member list nor a lookup table, §4.3).
    pub const fn is_function(self) -> bool {
        matches!(self.payload(), PayloadClass::Function)
    }

    /// True for kinds whose name is entered in a lookup table when it has
    /// one: everything but parameters (block-scope, §4.3), access
    /// specifiers, `static_assert`, empty declarations, using-directives,
    /// friends and contexts that only group declarations.
    pub const fn is_named_entity(self) -> bool {
        !matches!(
            self,
            DeclKind::TranslationUnit
                | DeclKind::LinkageSpec
                | DeclKind::Export
                | DeclKind::Empty
                | DeclKind::FileScopeAsm
                | DeclKind::UsingDirective
                | DeclKind::AccessSpec
                | DeclKind::Friend
                | DeclKind::FriendTemplate
                | DeclKind::StaticAssert
                | DeclKind::RequiresExprBody
                | DeclKind::ParmVar
                | DeclKind::ImplicitParam
                | DeclKind::LifetimeExtendedTemporary
                | DeclKind::TemplateParamObject
                | DeclKind::Label
        )
    }

    /// The dump name (`<name>Decl` in spec/ast-dump.md).
    pub const fn name(self) -> &'static str {
        use DeclKind as K;
        match self {
            K::TranslationUnit => "TranslationUnit",
            K::Namespace => "Namespace",
            K::NamespaceAlias => "NamespaceAlias",
            K::LinkageSpec => "LinkageSpec",
            K::Export => "Export",
            K::Empty => "Empty",
            K::FileScopeAsm => "FileScopeAsm",
            K::UsingDirective => "UsingDirective",
            K::Using => "Using",
            K::UsingEnum => "UsingEnum",
            K::UsingShadow => "UsingShadow",
            K::ConstructorUsingShadow => "ConstructorUsingShadow",
            K::UsingPack => "UsingPack",
            K::UnresolvedUsingValue => "UnresolvedUsingValue",
            K::UnresolvedUsingTypename => "UnresolvedUsingTypename",
            K::UnresolvedUsingIfExists => "UnresolvedUsingIfExists",
            K::Typedef => "Typedef",
            K::TypeAlias => "TypeAlias",
            K::Enum => "Enum",
            K::EnumConstant => "EnumConstant",
            K::CXXRecord => "CXXRecord",
            K::ClassTemplateSpecialization => "ClassTemplateSpecialization",
            K::ClassTemplatePartialSpecialization => "ClassTemplatePartialSpecialization",
            K::Field => "Field",
            K::IndirectField => "IndirectField",
            K::AccessSpec => "AccessSpec",
            K::Friend => "Friend",
            K::FriendTemplate => "FriendTemplate",
            K::Function => "Function",
            K::CXXMethod => "CXXMethod",
            K::CXXConstructor => "CXXConstructor",
            K::CXXDestructor => "CXXDestructor",
            K::CXXConversion => "CXXConversion",
            K::CXXDeductionGuide => "CXXDeductionGuide",
            K::Var => "Var",
            K::ParmVar => "ParmVar",
            K::ImplicitParam => "ImplicitParam",
            K::Decomposition => "Decomposition",
            K::Binding => "Binding",
            K::VarTemplateSpecialization => "VarTemplateSpecialization",
            K::VarTemplatePartialSpecialization => "VarTemplatePartialSpecialization",
            K::ClassTemplate => "ClassTemplate",
            K::FunctionTemplate => "FunctionTemplate",
            K::VarTemplate => "VarTemplate",
            K::TypeAliasTemplate => "TypeAliasTemplate",
            K::Concept => "Concept",
            K::BuiltinTemplate => "BuiltinTemplate",
            K::TemplateTypeParm => "TemplateTypeParm",
            K::NonTypeTemplateParm => "NonTypeTemplateParm",
            K::TemplateTemplateParm => "TemplateTemplateParm",
            K::ImplicitConceptSpecialization => "ImplicitConceptSpecialization",
            K::RequiresExprBody => "RequiresExprBody",
            K::LifetimeExtendedTemporary => "LifetimeExtendedTemporary",
            K::TemplateParamObject => "TemplateParamObject",
            K::StaticAssert => "StaticAssert",
            K::Label => "Label",
            K::ContractResultName => "ContractResultName",
        }
    }
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

/// `DeclNode.flags` (spec/ast.md §4.1).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct DeclFlags(u8);

impl DeclFlags {
    /// The declaration is invalid (an error was diagnosed). Monotone.
    pub const INVALID: DeclFlags = DeclFlags(1);
    /// Implicitly declared (special members, builtins, injected names).
    pub const IMPLICIT: DeclFlags = DeclFlags(2);
    /// Odr-used ([basic.def.odr]). Monotone.
    pub const USED: DeclFlags = DeclFlags(4);
    /// Referenced (for `-Wunused`). Monotone.
    pub const REFERENCED: DeclFlags = DeclFlags(8);
    /// Has an entry in `AttrArena::decl_attrs`.
    pub const HAS_ATTRS: DeclFlags = DeclFlags(16);
    /// The lexical context differs from `ctx` (entry in
    /// `DeclArena::lexical_ctx`).
    pub const HAS_LEXICAL_CTX: DeclFlags = DeclFlags(32);
    /// Declared in a dependent context ([temp.dep]).
    pub const DEPENDENT_CTX: DeclFlags = DeclFlags(64);
    /// Injected by `define_aggregate` (§10).
    pub const INJECTED: DeclFlags = DeclFlags(128);
    /// The flags sema may set after creation (§2.4).
    pub const MONOTONE: DeclFlags = DeclFlags(1 | 4 | 8);

    /// No flags.
    pub const fn empty() -> Self {
        DeclFlags(0)
    }
    /// The raw bits.
    pub const fn bits(self) -> u8 {
        self.0
    }
    /// True if every bit of `f` is set.
    pub const fn contains(self, f: DeclFlags) -> bool {
        self.0 & f.0 == f.0
    }
    /// The union.
    #[must_use]
    pub const fn with(self, f: DeclFlags) -> Self {
        DeclFlags(self.0 | f.0)
    }
    /// `self` with the bits of `f` cleared.
    #[must_use]
    pub const fn without(self, f: DeclFlags) -> Self {
        DeclFlags(self.0 & !f.0)
    }
}

/// Access of a member ([class.access]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[repr(u8)]
pub enum Access {
    /// Not a member (or not yet known).
    #[default]
    None = 0,
    /// `public`.
    Public = 1,
    /// `protected`.
    Protected = 2,
    /// `private`.
    Private = 3,
}

/// Friend status (clang's `FriendObjectKind`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[repr(u8)]
pub enum FriendKind {
    /// Not a friend.
    #[default]
    None = 0,
    /// A friend that was also declared elsewhere (visible to ordinary
    /// lookup).
    Declared = 1,
    /// A friend first declared by the friend declaration (found only by
    /// ADL, [namespace.memdef]).
    Undeclared = 2,
}

/// Module ownership of a declaration ([module.reach], M4).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[repr(u8)]
pub enum ModuleOwnership {
    /// Not owned by a named module.
    #[default]
    Unowned = 0,
    /// Visible.
    Visible = 1,
    /// Visible when imported.
    VisibleWhenImported = 2,
    /// Reachable when imported.
    ReachableWhenImported = 3,
    /// Module-private.
    ModulePrivate = 4,
}

/// `DeclNode.misc` (spec/ast.md §4.1): `access(2) friend(2)
/// local_extern(1) module_ownership(3) name_independent(1)
/// linkage_cache(3) visibility(2) spare(2)`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct DeclMisc(u16);

impl DeclMisc {
    /// All fields zero.
    pub const fn new() -> Self {
        DeclMisc(0)
    }
    const fn field(self, shift: u32, width: u32) -> u16 {
        (self.0 >> shift) & ((1 << width) - 1)
    }
    const fn with_field(self, shift: u32, width: u32, v: u16) -> Self {
        let mask = ((1u16 << width) - 1) << shift;
        DeclMisc((self.0 & !mask) | ((v << shift) & mask))
    }
    /// The raw bits.
    pub const fn bits(self) -> u16 {
        self.0
    }
    /// The access.
    pub const fn access(self) -> Access {
        match self.field(0, 2) {
            1 => Access::Public,
            2 => Access::Protected,
            3 => Access::Private,
            _ => Access::None,
        }
    }
    /// With access `a`.
    #[must_use]
    pub const fn with_access(self, a: Access) -> Self {
        self.with_field(0, 2, a as u16)
    }
    /// The friend status.
    pub const fn friend(self) -> FriendKind {
        match self.field(2, 2) {
            1 => FriendKind::Declared,
            2 => FriendKind::Undeclared,
            _ => FriendKind::None,
        }
    }
    /// With friend status `f`.
    #[must_use]
    pub const fn with_friend(self, f: FriendKind) -> Self {
        self.with_field(2, 2, f as u16)
    }
    /// A block-scope `extern` declaration.
    pub const fn local_extern(self) -> bool {
        self.field(4, 1) != 0
    }
    /// With the local-extern bit.
    #[must_use]
    pub const fn with_local_extern(self, v: bool) -> Self {
        self.with_field(4, 1, v as u16)
    }
    /// Module ownership.
    pub const fn module_ownership(self) -> ModuleOwnership {
        match self.field(5, 3) {
            1 => ModuleOwnership::Visible,
            2 => ModuleOwnership::VisibleWhenImported,
            3 => ModuleOwnership::ReachableWhenImported,
            4 => ModuleOwnership::ModulePrivate,
            _ => ModuleOwnership::Unowned,
        }
    }
    /// With module ownership `m`.
    #[must_use]
    pub const fn with_module_ownership(self, m: ModuleOwnership) -> Self {
        self.with_field(5, 3, m as u16)
    }
    /// A name-independent declaration (P2169 `_`, [basic.scope.scope]/5).
    pub const fn name_independent(self) -> bool {
        self.field(8, 1) != 0
    }
    /// With the name-independent bit.
    #[must_use]
    pub const fn with_name_independent(self, v: bool) -> Self {
        self.with_field(8, 1, v as u16)
    }
    /// The linkage cache: 0 = not computed, else sema's linkage code + 1.
    pub const fn linkage_cache(self) -> u8 {
        self.field(9, 3) as u8
    }
    /// The visibility cache (2 bits, sema-defined).
    pub const fn visibility_cache(self) -> u8 {
        self.field(12, 2) as u8
    }
}

/// The 28-byte declaration header (spec/ast.md §4.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DeclNode {
    kind: DeclKind,
    flags: DeclFlags,
    misc: DeclMisc,
    name: DeclName,
    ctx: Option<DeclId>,
    next: Option<DeclId>,
    loc: Loc,
    ty: Option<QualType>,
    data: u32,
}

impl DeclNode {
    /// The kind.
    pub const fn kind(&self) -> DeclKind {
        self.kind
    }
    /// The flags.
    pub const fn flags(&self) -> DeclFlags {
        self.flags
    }
    /// True if every bit of `f` is set.
    pub const fn has(&self, f: DeclFlags) -> bool {
        self.flags.contains(f)
    }
    /// The misc bits.
    pub const fn misc(&self) -> DeclMisc {
        self.misc
    }
    /// The name.
    pub const fn name(&self) -> DeclName {
        self.name
    }
    /// The semantic context (`None` only for the TU).
    pub const fn ctx(&self) -> Option<DeclId> {
        self.ctx
    }
    /// The next sibling in the lexical context's member list.
    pub const fn next(&self) -> Option<DeclId> {
        self.next
    }
    /// The name's location.
    pub const fn loc(&self) -> Loc {
        self.loc
    }
    /// Value decls: the declared type; type decls: the declared type.
    pub const fn ty(&self) -> Option<QualType> {
        self.ty
    }
    /// The payload index (0 = none) or inline payload.
    pub const fn data(&self) -> u32 {
        self.data
    }
}

// ---------------------------------------------------------------------------
// Payload pieces
// ---------------------------------------------------------------------------

/// The redeclaration link at the start of every redeclarable payload
/// (spec/ast.md §4.2, §4.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Redecl {
    /// The previous declaration of the same entity.
    pub prev: Option<DeclId>,
    /// The shared group (first / latest / definition).
    pub group: RedeclGroupId,
}

/// The member list of a context: appended in O(1) through
/// `DeclNode.next`.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Members {
    /// The first member.
    pub first: Option<DeclId>,
    /// The last member.
    pub last: Option<DeclId>,
}

/// `TemplLink` (spec/ast.md §8.2): 0 = none, else a 3-bit tag
/// (bits 29..=31) and a 29-bit index into the template tables (M3).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct TemplLink(u32);

/// The tag of a [`TemplLink`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum TemplLinkKind {
    /// The decl is the pattern of a template decl.
    Pattern = 1,
    /// A specialization or instantiation (index into `templates.specs`).
    Specialization = 2,
    /// A member of a class template specialization.
    MemberOfSpecialization = 3,
    /// A dependent friend function template specialization.
    DependentFriend = 4,
    /// A class template partial specialization (index into
    /// `templates.partials`, WP-070b).
    PartialSpecialization = 5,
}

impl TemplLink {
    /// Not templated.
    pub const NONE: TemplLink = TemplLink(0);

    /// A link of kind `kind` to `index` (< 2^29).
    pub const fn new(kind: TemplLinkKind, index: u32) -> Self {
        assert!(index < 1 << 29, "TemplLink index exceeds 29 bits");
        TemplLink(((kind as u32) << 29) | index)
    }
    /// True for [`TemplLink::NONE`].
    pub const fn is_none(self) -> bool {
        self.0 == 0
    }
    /// The kind and index, if any.
    pub const fn get(self) -> Option<(TemplLinkKind, u32)> {
        let idx = self.0 & ((1 << 29) - 1);
        match self.0 >> 29 {
            1 => Some((TemplLinkKind::Pattern, idx)),
            2 => Some((TemplLinkKind::Specialization, idx)),
            3 => Some((TemplLinkKind::MemberOfSpecialization, idx)),
            4 => Some((TemplLinkKind::DependentFriend, idx)),
            5 => Some((TemplLinkKind::PartialSpecialization, idx)),
            _ => None,
        }
    }
    /// The raw word.
    pub const fn to_raw(self) -> u32 {
        self.0
    }
}

/// Storage class of a function ([dcl.stc]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[repr(u8)]
pub enum FnStorage {
    /// None written.
    #[default]
    None = 0,
    /// `static`.
    Static = 1,
    /// `extern`.
    Extern = 2,
}

/// `constexpr` / `consteval` ([dcl.constexpr]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[repr(u8)]
pub enum ConstexprKind {
    /// Neither.
    #[default]
    None = 0,
    /// `constexpr` (or implicitly constexpr).
    Constexpr = 1,
    /// `consteval` (an immediate function).
    Consteval = 2,
}

/// Defaulted status ([dcl.fct.def.default]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[repr(u8)]
pub enum Defaulted {
    /// Not defaulted.
    #[default]
    No = 0,
    /// Explicitly `= default`.
    Explicit = 1,
    /// Implicitly declared and defaulted.
    Implicit = 2,
}

/// `FunctionData.spec` (spec/ast.md §4.2): `storage(2) inline(1)
/// implicit_inline(1) constexpr_kind(2) virtual(1) pure(1) defaulted(2)
/// deleted(1) explicit(1) trivial(1) immediate_escalating(1)
/// has_contracts(1) is_coroutine(1) escalated(1) variadic(1) override(1)
/// final(1) explicit_object(1) virtual_written(1) storage_inherited(1)`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct FnSpec(u32);

macro_rules! bit_accessors {
    ($ty:ident: $($(#[$m:meta])* $get:ident / $set:ident = $bit:expr;)*) => {
        impl $ty {
            $(
                $(#[$m])*
                pub const fn $get(self) -> bool {
                    self.0 & (1 << $bit) != 0
                }
                #[doc = concat!("With [`Self::", stringify!($get), "`] set to `v`.")]
                #[must_use]
                pub const fn $set(self, v: bool) -> Self {
                    if v { $ty(self.0 | (1 << $bit)) } else { $ty(self.0 & !(1 << $bit)) }
                }
            )*
        }
    };
}

impl FnSpec {
    /// Nothing set.
    pub const fn new() -> Self {
        FnSpec(0)
    }
    /// The raw bits.
    pub const fn bits(self) -> u32 {
        self.0
    }
    /// The storage class.
    pub const fn storage(self) -> FnStorage {
        match self.0 & 3 {
            1 => FnStorage::Static,
            2 => FnStorage::Extern,
            _ => FnStorage::None,
        }
    }
    /// With storage class `s`.
    #[must_use]
    pub const fn with_storage(self, s: FnStorage) -> Self {
        FnSpec((self.0 & !3) | s as u32)
    }
    /// `constexpr` / `consteval`.
    pub const fn constexpr_kind(self) -> ConstexprKind {
        match (self.0 >> 4) & 3 {
            1 => ConstexprKind::Constexpr,
            2 => ConstexprKind::Consteval,
            _ => ConstexprKind::None,
        }
    }
    /// With constexpr kind `k`.
    #[must_use]
    pub const fn with_constexpr_kind(self, k: ConstexprKind) -> Self {
        FnSpec((self.0 & !(3 << 4)) | ((k as u32) << 4))
    }
    /// Defaulted status.
    pub const fn defaulted(self) -> Defaulted {
        match (self.0 >> 8) & 3 {
            1 => Defaulted::Explicit,
            2 => Defaulted::Implicit,
            _ => Defaulted::No,
        }
    }
    /// With defaulted status `d`.
    #[must_use]
    pub const fn with_defaulted(self, d: Defaulted) -> Self {
        FnSpec((self.0 & !(3 << 8)) | ((d as u32) << 8))
    }
    /// The bits [`DeclArena::mark_function`] may set after creation
    /// (properties known only after the body or the class is complete:
    /// trivial, contracts, coroutine, escalated, `deleted` for
    /// `= default` members found deleted at `}`, spec/sema-class.md §8.2,
    /// and `implicit_inline` for a member function defined in its class,
    /// known only at its body, [class.mfct]/1; WP-067a: `defaulted`
    /// [`Defaulted::Explicit`] and `constexpr` [`ConstexprKind::Constexpr`],
    /// known only at `= default` after the declarator, [dcl.fct.def.default],
    /// [dcl.constexpr]/7). The two fields are OR-ed in, so sema sets them
    /// only from [`Defaulted::No`] / [`ConstexprKind::None`].
    pub const LATE: FnSpec = FnSpec(
        (1 << 3) | (1 << 4) | (1 << 8) | (1 << 10) | (1 << 12) | (1 << 14) | (1 << 15) | (1 << 16),
    );
}

bit_accessors!(FnSpec:
    /// `inline` written.
    is_inline / with_inline = 2;
    /// Implicitly inline (defined in class, constexpr, ...).
    implicit_inline / with_implicit_inline = 3;
    /// Virtual: `virtual` written or overriding a virtual function
    /// ([class.virtual]/2). Semantic; see [`Self::virtual_written`].
    is_virtual / with_virtual = 6;
    /// Pure virtual (`= 0`).
    is_pure / with_pure = 7;
    /// `= delete`.
    is_deleted / with_deleted = 10;
    /// `explicit` (constructors and conversion functions).
    is_explicit / with_explicit = 11;
    /// Trivial special member ([class.prop]); set at class completion.
    is_trivial / with_trivial = 12;
    /// Immediate-escalating ([expr.const]/17).
    immediate_escalating / with_immediate_escalating = 13;
    /// Has `pre`/`post` contract assertions (P2900).
    has_contracts / with_has_contracts = 14;
    /// A coroutine (body contains `co_await`/`co_yield`/`co_return`).
    is_coroutine / with_coroutine = 15;
    /// An immediate-escalating function that became an immediate function
    /// ([expr.const]/18).
    escalated / with_escalated = 16;
    /// C-style variadic (`...`).
    is_variadic / with_variadic = 17;
    /// `override` written (virt-specifier, [class.virtual]).
    is_override / with_override = 18;
    /// `final` written on a member function ([class.virtual]/4).
    is_final / with_final = 19;
    /// Has an explicit object parameter (P0847, [dcl.fct]/6).
    has_explicit_object / with_explicit_object = 20;
    /// The `virtual` function-specifier was written on this declaration
    /// (clang's dump prints `virtual` only when written; an implicit
    /// overrider has [`Self::is_virtual`] without this bit).
    virtual_written / with_virtual_written = 21;
    /// The storage class was inherited from a previous declaration
    /// ([dcl.stc]/4: `static void f(); void f() {}`, or an out-of-line
    /// static member function) rather than written here; clang's dump
    /// prints the written storage class only.
    storage_inherited / with_storage_inherited = 22;
);

/// Hot payload of function kinds (24 bytes, spec/ast.md §4.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FunctionData {
    /// Redeclaration link.
    pub redecl: Redecl,
    /// `ParmVar` decls.
    pub params: ListId,
    /// The body (write-once, §2.4).
    pub body: Option<StmtId>,
    /// Specifiers.
    pub spec: FnSpec,
    /// Template link.
    pub templ: TemplLink,
}

/// Storage class of a variable ([dcl.stc]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[repr(u8)]
pub enum VarStorage {
    /// None written.
    #[default]
    None = 0,
    /// `static`.
    Static = 1,
    /// `extern`.
    Extern = 2,
    /// `register` (C; ill-formed in C++17+, kept for diagnostics).
    Register = 3,
}

/// Thread-local kind.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[repr(u8)]
pub enum TlsKind {
    /// Not thread-local.
    #[default]
    None = 0,
    /// `thread_local` with static (constant) initialization.
    Static = 1,
    /// `thread_local` with dynamic initialization.
    Dynamic = 2,
}

/// Initialization syntax ([dcl.init]); the dump prints `cinit`,
/// `callinit`, `listinit`, `parenlistinit`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[repr(u8)]
pub enum InitStyle {
    /// `= e` (or no initializer).
    #[default]
    C = 0,
    /// `(args)`.
    Call = 1,
    /// `{args}`.
    List = 2,
    /// `(args)` aggregate initialization (P0960).
    ParenList = 3,
}

/// `VarData.spec`: `storage(3) tls(2) init_style(2) constexpr(1)
/// constinit(1) inline(1) nrvo(1) exception_var(1) cxx_for_range(1)`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct VarSpec(u32);

impl VarSpec {
    /// Nothing set.
    pub const fn new() -> Self {
        VarSpec(0)
    }
    /// The raw bits.
    pub const fn bits(self) -> u32 {
        self.0
    }
    /// The storage class.
    pub const fn storage(self) -> VarStorage {
        match self.0 & 7 {
            1 => VarStorage::Static,
            2 => VarStorage::Extern,
            3 => VarStorage::Register,
            _ => VarStorage::None,
        }
    }
    /// With storage class `s`.
    #[must_use]
    pub const fn with_storage(self, s: VarStorage) -> Self {
        VarSpec((self.0 & !7) | s as u32)
    }
    /// The thread-local kind.
    pub const fn tls(self) -> TlsKind {
        match (self.0 >> 3) & 3 {
            1 => TlsKind::Static,
            2 => TlsKind::Dynamic,
            _ => TlsKind::None,
        }
    }
    /// With thread-local kind `t`.
    #[must_use]
    pub const fn with_tls(self, t: TlsKind) -> Self {
        VarSpec((self.0 & !(3 << 3)) | ((t as u32) << 3))
    }
    /// The initialization syntax.
    pub const fn init_style(self) -> InitStyle {
        match (self.0 >> 5) & 3 {
            1 => InitStyle::Call,
            2 => InitStyle::List,
            3 => InitStyle::ParenList,
            _ => InitStyle::C,
        }
    }
    /// With initialization syntax `s`.
    #[must_use]
    pub const fn with_init_style(self, s: InitStyle) -> Self {
        VarSpec((self.0 & !(3 << 5)) | ((s as u32) << 5))
    }
    /// The bits [`DeclArena::mark_var`] may set after creation: `nrvo`,
    /// the `Call` init style of a default-initialized class variable
    /// (set over `C` only, with its `CXXConstructExpr`), `ParenList` (set
    /// over `Call` once the parenthesized initializer turns out to
    /// initialize an aggregate), and `inline` (an instantiated static data
    /// member becomes inline with its deferred initializer, [temp.inst]/4).
    pub const LATE: VarSpec = VarSpec((1 << 10) | (1 << 9) | (1 << 6) | (1 << 5));
}

bit_accessors!(VarSpec:
    /// `constexpr`.
    is_constexpr / with_constexpr = 7;
    /// `constinit`.
    is_constinit / with_constinit = 8;
    /// `inline` (written or implied by `constexpr` static members).
    is_inline / with_inline = 9;
    /// NRVO candidate ([class.copy.elision]); set after the body.
    is_nrvo / with_nrvo = 10;
    /// The exception object variable of a handler.
    is_exception_var / with_exception_var = 11;
    /// A range-based-for implicit variable.
    is_cxx_for_range / with_cxx_for_range = 12;
);

/// The state of a variable's constant evaluation (spec/ast.md §9.2), 4
/// bytes: `0` `NotEvaluated`, `u32::MAX` `Evaluating`, `u32::MAX − 1`
/// `NotConstant`, else a `ConstValueId`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct VarEval(u32);

/// A decoded [`VarEval`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VarEvalState {
    /// Not evaluated yet.
    NotEvaluated,
    /// Being evaluated (a re-entry is a self-reference).
    Evaluating,
    /// Evaluated; not a constant.
    NotConstant,
    /// Evaluated to a constant.
    Value(ConstValueId),
}

impl VarEval {
    const EVALUATING: u32 = u32::MAX;
    const NOT_CONSTANT: u32 = u32::MAX - 1;

    /// Decodes the state.
    pub fn state(self) -> VarEvalState {
        match self.0 {
            0 => VarEvalState::NotEvaluated,
            Self::EVALUATING => VarEvalState::Evaluating,
            Self::NOT_CONSTANT => VarEvalState::NotConstant,
            v => VarEvalState::Value(ConstValueId::new(v)),
        }
    }
    /// The raw word.
    pub const fn to_raw(self) -> u32 {
        self.0
    }
}

/// Hot payload of variable kinds (24 bytes, spec/ast.md §4.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VarData {
    /// Redeclaration link.
    pub redecl: Redecl,
    /// The initializer (write-once, §2.4).
    pub init: Option<ExprId>,
    /// Specifiers.
    pub spec: VarSpec,
    /// Evaluation state (§9.2).
    pub eval: VarEval,
    /// Template link.
    pub templ: TemplLink,
}

/// struct / class / union ([class.pre]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[repr(u8)]
pub enum TagKind {
    /// `struct`.
    #[default]
    Struct = 0,
    /// `class`.
    Class = 1,
    /// `union`.
    Union = 2,
}

impl TagKind {
    /// The keyword.
    pub const fn keyword(self) -> &'static str {
        match self {
            TagKind::Struct => "struct",
            TagKind::Class => "class",
            TagKind::Union => "union",
        }
    }
}

/// `RecordData.defn`: definition state plus clang's `DefinitionData`
/// property flags, computed by sema while the class is defined.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct DefinitionBits(u64);

impl DefinitionBits {
    /// `{` seen, `}` not yet.
    pub const BEING_DEFINED: DefinitionBits = DefinitionBits(1);
    /// `}` seen: the class is complete and its member list frozen.
    pub const COMPLETE: DefinitionBits = DefinitionBits(1 << 1);
    /// An anonymous struct/union member ([class.union.anon]).
    pub const ANONYMOUS: DefinitionBits = DefinitionBits(1 << 2);
    /// A lambda closure type.
    pub const LAMBDA: DefinitionBits = DefinitionBits(1 << 3);
    /// Aggregate ([dcl.init.aggr]).
    pub const AGGREGATE: DefinitionBits = DefinitionBits(1 << 4);
    /// Polymorphic ([class.virtual]).
    pub const POLYMORPHIC: DefinitionBits = DefinitionBits(1 << 5);
    /// Abstract ([class.abstract]).
    pub const ABSTRACT: DefinitionBits = DefinitionBits(1 << 6);
    /// Empty ([class.prop] `is_empty`).
    pub const EMPTY: DefinitionBits = DefinitionBits(1 << 7);
    /// Standard-layout ([class.prop]).
    pub const STANDARD_LAYOUT: DefinitionBits = DefinitionBits(1 << 8);
    /// Trivially copyable ([class.prop]).
    pub const TRIVIALLY_COPYABLE: DefinitionBits = DefinitionBits(1 << 9);
    /// Trivial default constructor.
    pub const TRIVIAL_DEFAULT_CTOR: DefinitionBits = DefinitionBits(1 << 10);
    /// Trivial copy constructor.
    pub const TRIVIAL_COPY_CTOR: DefinitionBits = DefinitionBits(1 << 11);
    /// Trivial move constructor.
    pub const TRIVIAL_MOVE_CTOR: DefinitionBits = DefinitionBits(1 << 12);
    /// Trivial copy assignment.
    pub const TRIVIAL_COPY_ASSIGN: DefinitionBits = DefinitionBits(1 << 13);
    /// Trivial move assignment.
    pub const TRIVIAL_MOVE_ASSIGN: DefinitionBits = DefinitionBits(1 << 14);
    /// Trivial destructor.
    pub const TRIVIAL_DTOR: DefinitionBits = DefinitionBits(1 << 15);
    /// Literal type ([basic.types.general]).
    pub const LITERAL: DefinitionBits = DefinitionBits(1 << 16);
    /// Structural type ([temp.param]/7).
    pub const STRUCTURAL: DefinitionBits = DefinitionBits(1 << 17);
    /// Has a user-declared constructor.
    pub const USER_DECLARED_CTOR: DefinitionBits = DefinitionBits(1 << 18);
    /// Has a `constexpr` (non-copy/move) constructor.
    pub const HAS_CONSTEXPR_CTOR: DefinitionBits = DefinitionBits(1 << 19);
    /// Has a variant member (unions and classes with anonymous unions).
    pub const HAS_VARIANT_MEMBERS: DefinitionBits = DefinitionBits(1 << 20);
    /// Has mutable fields (directly or in subobjects).
    pub const HAS_MUTABLE_FIELDS: DefinitionBits = DefinitionBits(1 << 21);
    /// Has a virtual base.
    pub const HAS_VIRTUAL_BASES: DefinitionBits = DefinitionBits(1 << 22);
    /// Trivially relocatable (P2786).
    pub const TRIVIALLY_RELOCATABLE: DefinitionBits = DefinitionBits(1 << 23);
    /// A user-declared copy constructor (clang `UserDeclaredSpecialMembers`).
    pub const USER_DECLARED_COPY_CTOR: DefinitionBits = DefinitionBits(1 << 24);
    /// A user-declared move constructor.
    pub const USER_DECLARED_MOVE_CTOR: DefinitionBits = DefinitionBits(1 << 25);
    /// A user-declared copy assignment operator.
    pub const USER_DECLARED_COPY_ASSIGN: DefinitionBits = DefinitionBits(1 << 26);
    /// A user-declared move assignment operator.
    pub const USER_DECLARED_MOVE_ASSIGN: DefinitionBits = DefinitionBits(1 << 27);
    /// A user-declared destructor.
    pub const USER_DECLARED_DTOR: DefinitionBits = DefinitionBits(1 << 28);
    /// A user-provided default constructor (clang `user_provided`).
    pub const USER_PROVIDED_DEFAULT_CTOR: DefinitionBits = DefinitionBits(1 << 29);
    /// Some non-static data member has a default member initializer.
    pub const HAS_IN_CLASS_INIT: DefinitionBits = DefinitionBits(1 << 30);
    /// Inherits constructors (`using B::B`).
    pub const HAS_INHERITED_CTOR: DefinitionBits = DefinitionBits(1 << 31);
    /// Inherits an assignment operator (`using B::operator=`).
    pub const HAS_INHERITED_ASSIGN: DefinitionBits = DefinitionBits(1 << 32);
    /// The implicit copy constructor takes `const C&` ([class.copy.ctor]/7).
    pub const IMPLICIT_COPY_CTOR_CONST_PARAM: DefinitionBits = DefinitionBits(1 << 33);
    /// The implicit copy assignment takes `const C&` ([class.copy.assign]/2).
    pub const IMPLICIT_COPY_ASSIGN_CONST_PARAM: DefinitionBits = DefinitionBits(1 << 34);
    /// A defaulted default constructor would be constexpr (clang `defaulted_is_constexpr`).
    pub const DEFAULTED_DEFAULT_CTOR_CONSTEXPR: DefinitionBits = DefinitionBits(1 << 35);
    /// Selecting a subobject's copy constructor needs overload resolution.
    pub const NEEDS_OVL_COPY_CTOR: DefinitionBits = DefinitionBits(1 << 36);
    /// Selecting a subobject's move constructor needs overload resolution.
    pub const NEEDS_OVL_MOVE_CTOR: DefinitionBits = DefinitionBits(1 << 37);
    /// Selecting a subobject's copy assignment needs overload resolution.
    pub const NEEDS_OVL_COPY_ASSIGN: DefinitionBits = DefinitionBits(1 << 38);
    /// Selecting a subobject's move assignment needs overload resolution.
    pub const NEEDS_OVL_MOVE_ASSIGN: DefinitionBits = DefinitionBits(1 << 39);
    /// Selecting a subobject's destructor needs overload resolution.
    pub const NEEDS_OVL_DTOR: DefinitionBits = DefinitionBits(1 << 40);
    /// A defaulted default constructor is known deleted (clang `DefaultedDefaultConstructorIsDeleted`).
    pub const DEFAULTED_DEFAULT_CTOR_DELETED: DefinitionBits = DefinitionBits(1 << 41);
    /// A defaulted copy constructor is known deleted.
    pub const DEFAULTED_COPY_CTOR_DELETED: DefinitionBits = DefinitionBits(1 << 42);
    /// A defaulted move constructor is known deleted.
    pub const DEFAULTED_MOVE_CTOR_DELETED: DefinitionBits = DefinitionBits(1 << 43);
    /// A defaulted destructor is known deleted.
    pub const DEFAULTED_DTOR_DELETED: DefinitionBits = DefinitionBits(1 << 44);
    /// A reference or const-without-default-init field without a default member initializer.
    pub const HAS_UNINIT_REF_OR_CONST: DefinitionBits = DefinitionBits(1 << 45);
    /// Has a private direct non-static data member.
    pub const HAS_PRIVATE_FIELDS: DefinitionBits = DefinitionBits(1 << 46);
    /// Has a protected direct non-static data member.
    pub const HAS_PROTECTED_FIELDS: DefinitionBits = DefinitionBits(1 << 47);
    /// POD class ([class.prop]/9; dump `pod`).
    pub const POD: DefinitionBits = DefinitionBits(1 << 48);
    /// Const-default-constructible ([dcl.init.general]/8).
    pub const CAN_CONST_DEFAULT_INIT: DefinitionBits = DefinitionBits(1 << 49);
    /// The class was declared with the `final` class-virt-specifier
    /// ([class.pre]; clang dumps a `FinalAttr final` child).
    pub const FINAL: DefinitionBits = DefinitionBits(1 << 50);

    /// No bits.
    pub const fn empty() -> Self {
        DefinitionBits(0)
    }
    /// The raw bits.
    pub const fn bits(self) -> u64 {
        self.0
    }
    /// True if every bit of `f` is set.
    pub const fn contains(self, f: DefinitionBits) -> bool {
        self.0 & f.0 == f.0
    }
    /// The union.
    #[must_use]
    pub const fn with(self, f: DefinitionBits) -> Self {
        DefinitionBits(self.0 | f.0)
    }
    /// The bits with `f` cleared.
    #[must_use]
    pub const fn without(self, f: DefinitionBits) -> Self {
        DefinitionBits(self.0 & !f.0)
    }
    /// A dynamic class: polymorphic or with virtual bases (clang
    /// `isDynamicClass`; not stored, spec/sema-class.md §2.9).
    pub const fn is_dynamic(self) -> bool {
        self.0 & (Self::POLYMORPHIC.0 | Self::HAS_VIRTUAL_BASES.0) != 0
    }
}

/// Hot payload of record kinds (40 bytes, spec/ast.md §4.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RecordData {
    /// Redeclaration link.
    pub redecl: Redecl,
    /// Member list (this declaration's lexical members).
    pub members: Members,
    /// Base specifiers: indices (1-based) into `DeclArena::base_specs`;
    /// [`ListId::EMPTY`] if none.
    pub bases: ListId,
    /// Definition state and properties.
    pub defn: DefinitionBits,
    /// Template link.
    pub templ: TemplLink,
    /// Closure data for lambda closure types (M3).
    pub lambda: Option<LambdaId>,
    /// struct / class / union.
    pub tag: TagKind,
}

/// One base-specifier ([class.derived]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BaseSpec {
    /// The base type as written.
    pub ty: QualType,
    /// Location of the base-specifier.
    pub loc: Loc,
    /// `virtual`.
    pub is_virtual: bool,
    /// The access as written (`None` if defaulted).
    pub access: Access,
    /// A pack expansion `Bases...`.
    pub pack_expansion: bool,
}

/// Payload of `Field` (12 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FieldData {
    /// The bit-field width (a `ConstantExpr` unless value-dependent).
    pub bit_width: Option<ExprId>,
    /// The default member initializer (write-once: parsed after the class
    /// is complete, [class.mem]/7).
    pub init: Option<ExprId>,
    /// [`FieldData::MUTABLE`], [`FieldData::NO_UNIQUE_ADDRESS`], and the
    /// in-class-initializer style in bits 2..=3 ([`InitStyle`]).
    pub flags: u32,
}

impl FieldData {
    /// `mutable`.
    pub const MUTABLE: u32 = 1;
    /// `[[no_unique_address]]`.
    pub const NO_UNIQUE_ADDRESS: u32 = 2;

    /// `mutable`.
    pub const fn is_mutable(&self) -> bool {
        self.flags & Self::MUTABLE != 0
    }
    /// `[[no_unique_address]]`.
    pub const fn no_unique_address(&self) -> bool {
        self.flags & Self::NO_UNIQUE_ADDRESS != 0
    }
}

/// Payload of `Enum` (32 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EnumData {
    /// Redeclaration link.
    pub redecl: Redecl,
    /// Enumerators (and nothing else).
    pub members: Members,
    /// The underlying type: set at declaration if fixed, else at `}`
    /// (write-once).
    pub int_type: Option<QualType>,
    /// The promotion type ([conv.prom]/4; write-once with `int_type`).
    pub promotion_type: Option<QualType>,
    /// [`EnumData::SCOPED`], [`EnumData::SCOPED_STRUCT`],
    /// [`EnumData::FIXED`], [`EnumData::BEING_DEFINED`],
    /// [`EnumData::COMPLETE`].
    pub flags: u32,
    /// Template link.
    pub templ: TemplLink,
}

impl EnumData {
    /// `enum class` / `enum struct`.
    pub const SCOPED: u32 = 1;
    /// Written with `struct` (dump prints `struct` instead of `class`).
    pub const SCOPED_STRUCT: u32 = 2;
    /// Fixed underlying type ([dcl.enum]/5).
    pub const FIXED: u32 = 4;
    /// `{` seen, `}` not yet.
    pub const BEING_DEFINED: u32 = 8;
    /// `}` seen.
    pub const COMPLETE: u32 = 16;

    /// A scoped enumeration.
    pub const fn is_scoped(&self) -> bool {
        self.flags & Self::SCOPED != 0
    }
    /// Has a fixed underlying type.
    pub const fn is_fixed(&self) -> bool {
        self.flags & Self::FIXED != 0
    }
}

/// Payload of `EnumConstant` (8 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EnumeratorData {
    /// The value (`None` while value-dependent).
    pub value: Option<ConstValueId>,
    /// The initializer as written.
    pub init: Option<ExprId>,
}

/// Payload of `ParmVar` (12 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ParamData {
    /// The default argument (write-once; in-class member functions'
    /// default arguments are parsed late).
    pub default_arg: Option<ExprId>,
    /// The type before adjustment ([dcl.fct]/5: array/function → pointer),
    /// if it differs.
    pub original_ty: Option<QualType>,
    /// The parameter's position.
    pub index: u16,
    /// Flags: [`ParamData::EXPLICIT_OBJECT`], [`ParamData::PACK`],
    /// [`ParamData::DEFAULT_ARG_PENDING`].
    pub flags: u16,
}

impl ParamData {
    /// The explicit object parameter (`this T self`, [dcl.fct]/6).
    pub const EXPLICIT_OBJECT: u16 = 1;
    /// A function parameter pack.
    pub const PACK: u16 = 2;
    /// The parameter has a default argument whose tokens are parsed late
    /// (in-class member functions, [class.mem]/7); `default_arg` is set
    /// when parsing finishes.
    pub const DEFAULT_ARG_PENDING: u16 = 4;
}

/// Payload of `Typedef` / `TypeAlias` (16 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TypedefData {
    /// Redeclaration link.
    pub redecl: Redecl,
    /// The aliased type.
    pub underlying: QualType,
    /// Template link (alias templates' patterns).
    pub templ: TemplLink,
}

/// Payload of `Namespace` (20 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NamespaceData {
    /// Redeclaration link (namespaces reopen: each `namespace N {}` is a
    /// redeclaration of the first).
    pub redecl: Redecl,
    /// This declaration's lexical members.
    pub members: Members,
    /// [`NamespaceData::INLINE`], [`NamespaceData::ANONYMOUS`].
    pub flags: u32,
}

impl NamespaceData {
    /// `inline namespace`.
    pub const INLINE: u32 = 1;
    /// `namespace { ... }`.
    pub const ANONYMOUS: u32 = 2;

    /// An inline namespace.
    pub const fn is_inline(&self) -> bool {
        self.flags & Self::INLINE != 0
    }
}

/// Language of a linkage specification.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum LinkageLang {
    /// `extern "C"`.
    C = 1,
    /// `extern "C++"`.
    Cxx = 2,
}

/// Payload of non-redeclarable contexts (TU, `LinkageSpec`, `Export`,
/// `RequiresExprBody`), 12 bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScopeData {
    /// Lexical members.
    pub members: Members,
    /// `LinkageSpec`: the [`LinkageLang`]; else 0.
    pub aux: u32,
}

/// Payload of `StaticAssert` (8 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaticAssertData {
    /// The condition (not wrapped in a `ConstantExpr`, §5.5).
    pub cond: ExprId,
    /// The message (a string literal or a P2741 constant expression).
    pub message: Option<ExprId>,
}

/// What a `Friend` declaration befriends ([class.friend]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FriendTarget {
    /// A function or function template (its declaration, whose lexical
    /// context is the class).
    Decl(DeclId),
    /// `friend T;` / `friend class C;` (a type, possibly not a class).
    Type {
        /// The befriended type as written (an `Elaborated` type for
        /// `friend class C;`).
        ty: QualType,
        /// The class declaration the elaborated-type-specifier introduced
        /// (`friend struct H;` with no prior `H`: semantic context the
        /// innermost enclosing namespace, lexical context the class,
        /// `FriendKind::Undeclared`); the dump prints it as the
        /// `FriendDecl`'s child, as clang.
        owned_tag: Option<DeclId>,
    },
}

/// Payload of `LifetimeExtendedTemporary` (12 bytes, spec/sema-class.md
/// §5.1): a temporary whose lifetime is extended by a reference
/// ([class.temporary]/6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LetData {
    /// The variable (or field) whose reference binding extends it.
    pub extending: DeclId,
    /// The Itanium `_ZGR` mangling index among the temporaries extended
    /// by `extending`.
    pub mangling_index: u32,
    /// The constant value (write-once, by the Oven for constant-initialized
    /// static-storage references, ast.md §9).
    pub value: Option<ConstValueId>,
}

/// What a ctor-initializer initializes (`CtorInit::kind`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum CtorInitKind {
    /// A direct non-virtual base: `target` is the base type's raw
    /// `TypeId` (canonical; dependent in a template pattern).
    Base = 0,
    /// A virtual base (direct or indirect): `target` is the base type's
    /// raw `TypeId`.
    VirtualBase = 1,
    /// A field: `target` is the raw `DeclId` of the `Field`.
    Field = 2,
    /// A member of an anonymous union/struct: `target` is the raw `DeclId`
    /// of the `IndirectField`.
    IndirectField = 3,
    /// A delegating constructor call: `target` is 0.
    Delegating = 4,
}

/// One ctor-initializer ([class.base.init], 16 bytes): written or implicit
/// (`written_index == CtorInit::IMPLICIT`), in initialization order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CtorInit {
    /// The initialized subobject (see [`CtorInitKind`]).
    pub target: u32,
    /// What `target` names.
    pub kind: CtorInitKind,
    /// The position among the written mem-initializers, or
    /// [`CtorInit::IMPLICIT`].
    pub written_index: u8,
    /// The initializer (a `CXXConstructExpr`, `ParenListExpr`-style
    /// conversion, `InitListExpr`, `CXXDefaultInitExpr`, ...).
    pub init: ExprId,
    /// The mem-initializer-id's location (the constructor's for implicit
    /// ones).
    pub loc: Loc,
}

impl CtorInit {
    /// `written_index` of an implicit initializer.
    pub const IMPLICIT: u8 = u8::MAX;

    /// True if written by the user.
    pub const fn is_written(&self) -> bool {
        self.written_index != Self::IMPLICIT
    }
}

// ---------------------------------------------------------------------------
// Cold data
// ---------------------------------------------------------------------------

/// Cold per-declaration data (spec/ast.md §4.2), each an insert-only
/// sparse map.
#[derive(Clone, Debug)]
pub struct ColdDeclData {
    /// The lexical context when it differs from `ctx` (`HAS_LEXICAL_CTX`).
    pub lexical_ctx: ColdMap<DeclId, DeclId>,
    /// The begin location (decl-specifiers start) when it differs from the
    /// name's location.
    pub begin_loc: ColdMap<DeclId, Loc>,
    /// `= delete("reason")` messages (P2573; spec/sema-class.md
    /// `deleted_msg`).
    pub delete_message: ColdMap<DeclId, ExprId>,
    /// `explicit(expr)` (spec/sema-class.md `explicit_spec`).
    pub explicit_expr: ColdMap<DeclId, ExprId>,
    /// Constructor → its ctor-initializers: a 1-based `ListId` of indices
    /// into `DeclArena::ctor_inits` (write-once, with the body).
    pub ctor_inits: ColdMap<DeclId, ListId>,
    /// Virtual member function → the methods it overrides (`ListId` of
    /// `DeclId`s; write-once at `}`).
    pub overrides: ColdMap<DeclId, ListId>,
    /// Class → its `Friend` declarations (`ListId` of `DeclId`s; write-once
    /// at `}`).
    pub friends: ColdMap<DeclId, ListId>,
    /// Dynamic class → its key function (Itanium §5.2.3).
    pub key_function: ColdMap<DeclId, DeclId>,
    /// Virtual destructor → the `operator delete` its deleting destructor
    /// calls ([class.dtor]/16).
    pub dtor_delete: ColdMap<DeclId, DeclId>,
    /// Class → the `#pragma pack` value in effect at its definition.
    pub record_pack: ColdMap<DeclId, u16>,
    /// Trailing requires-clauses.
    pub requires_clause: ColdMap<DeclId, ExprId>,
    /// Contract assertions (P2900): `ListId` of contract-spec indices.
    pub contracts: ColdMap<DeclId, ListId>,
    /// `asm("label")`.
    pub asm_label: ColdMap<DeclId, StringLitId>,
    /// Alignment overrides (`alignas`).
    pub alignment: ColdMap<DeclId, ExprId>,
    /// `[[deprecated("msg")]]` messages.
    pub deprecation: ColdMap<DeclId, StringLitId>,
    /// The pattern of instantiated decls without a `TemplLink` (§8.2).
    pub instantiated_from: ColdMap<DeclId, DeclId>,
    /// Injection points of `define_aggregate` members (§10).
    pub injection_point: ColdMap<DeclId, Loc>,
    /// Builtin function ids of implicit builtin `Function` decls (§5.3).
    pub builtin_id: ColdMap<DeclId, u32>,
    /// The typedef that names an unnamed class or enumeration for linkage
    /// purposes ([dcl.typedef]/9, clang's `TypedefNameForAnonDecl`): the
    /// tag's type prints with the typedef's name.
    pub typedef_for_anon: ColdMap<DeclId, DeclId>,
}

impl Default for ColdDeclData {
    fn default() -> Self {
        Self {
            lexical_ctx: ColdMap::new("lexical_ctx"),
            begin_loc: ColdMap::new("begin_loc"),
            delete_message: ColdMap::new("delete_message"),
            explicit_expr: ColdMap::new("explicit_expr"),
            ctor_inits: ColdMap::new("ctor_inits"),
            overrides: ColdMap::new("overrides"),
            friends: ColdMap::new("friends"),
            key_function: ColdMap::new("key_function"),
            dtor_delete: ColdMap::new("dtor_delete"),
            record_pack: ColdMap::new("record_pack"),
            requires_clause: ColdMap::new("requires_clause"),
            contracts: ColdMap::new("contracts"),
            asm_label: ColdMap::new("asm_label"),
            alignment: ColdMap::new("alignment"),
            deprecation: ColdMap::new("deprecation"),
            instantiated_from: ColdMap::new("instantiated_from"),
            injection_point: ColdMap::new("injection_point"),
            builtin_id: ColdMap::new("builtin_id"),
            typedef_for_anon: ColdMap::new("typedef_for_anon"),
        }
    }
}

impl ColdDeclData {
    fn rows(&self) -> [MemRow; 19] {
        [
            self.lexical_ctx.mem_row(),
            self.begin_loc.mem_row(),
            self.delete_message.mem_row(),
            self.explicit_expr.mem_row(),
            self.ctor_inits.mem_row(),
            self.overrides.mem_row(),
            self.friends.mem_row(),
            self.key_function.mem_row(),
            self.dtor_delete.mem_row(),
            self.record_pack.mem_row(),
            self.requires_clause.mem_row(),
            self.contracts.mem_row(),
            self.asm_label.mem_row(),
            self.alignment.mem_row(),
            self.deprecation.mem_row(),
            self.instantiated_from.mem_row(),
            self.injection_point.mem_row(),
            self.builtin_id.mem_row(),
            self.typedef_for_anon.mem_row(),
        ]
    }
}

// ---------------------------------------------------------------------------
// Arena
// ---------------------------------------------------------------------------

/// Declaration headers, payload arenas and cold maps (spec/ast.md §4).
///
/// Index 0 of every vector is a dummy. Reads take `&self`; the only
/// mutations are the write-once / monotone setters of §2.4. Nodes are
/// appended by the `AstBuilder`.
#[derive(Debug)]
pub struct DeclArena {
    nodes: Vec<DeclNode>,
    functions: Vec<FunctionData>,
    vars: Vec<VarData>,
    records: Vec<RecordData>,
    fields: Vec<FieldData>,
    enums: Vec<EnumData>,
    enumerators: Vec<EnumeratorData>,
    params: Vec<ParamData>,
    typedefs: Vec<TypedefData>,
    namespaces: Vec<NamespaceData>,
    scopes: Vec<ScopeData>,
    static_asserts: Vec<StaticAssertData>,
    base_specs: Vec<BaseSpec>,
    lets: Vec<LetData>,
    ctor_inits: Vec<CtorInit>,
    /// Cold per-declaration data.
    pub cold: ColdDeclData,
}

const DUMMY_GROUP: RedeclGroupId = RedeclGroupId::new(u32::MAX);
const DUMMY_REDECL: Redecl = Redecl { prev: None, group: DUMMY_GROUP };

impl Default for DeclArena {
    fn default() -> Self {
        let dummy = DeclNode {
            kind: DeclKind::Empty,
            flags: DeclFlags::empty(),
            misc: DeclMisc::new(),
            name: DeclName::EMPTY,
            ctx: None,
            next: None,
            loc: Loc::INVALID,
            ty: None,
            data: 0,
        };
        let qt = QualType::unqual(crate::ids::TypeId::new(1));
        Self {
            nodes: vec![dummy],
            functions: vec![FunctionData {
                redecl: DUMMY_REDECL,
                params: ListId::EMPTY,
                body: None,
                spec: FnSpec::new(),
                templ: TemplLink::NONE,
            }],
            vars: vec![VarData {
                redecl: DUMMY_REDECL,
                init: None,
                spec: VarSpec::new(),
                eval: VarEval::default(),
                templ: TemplLink::NONE,
            }],
            records: vec![RecordData {
                redecl: DUMMY_REDECL,
                members: Members::default(),
                bases: ListId::EMPTY,
                defn: DefinitionBits::empty(),
                templ: TemplLink::NONE,
                lambda: None,
                tag: TagKind::Struct,
            }],
            fields: vec![FieldData { bit_width: None, init: None, flags: 0 }],
            enums: vec![EnumData {
                redecl: DUMMY_REDECL,
                members: Members::default(),
                int_type: None,
                promotion_type: None,
                flags: 0,
                templ: TemplLink::NONE,
            }],
            enumerators: vec![EnumeratorData { value: None, init: None }],
            params: vec![ParamData { default_arg: None, original_ty: None, index: 0, flags: 0 }],
            typedefs: vec![TypedefData {
                redecl: DUMMY_REDECL,
                underlying: qt,
                templ: TemplLink::NONE,
            }],
            namespaces: vec![NamespaceData {
                redecl: DUMMY_REDECL,
                members: Members::default(),
                flags: 0,
            }],
            scopes: vec![ScopeData { members: Members::default(), aux: 0 }],
            static_asserts: vec![StaticAssertData { cond: ExprId::new(u32::MAX), message: None }],
            base_specs: vec![BaseSpec {
                ty: qt,
                loc: Loc::INVALID,
                is_virtual: false,
                access: Access::None,
                pack_expansion: false,
            }],
            lets: vec![LetData {
                extending: DeclId::new(DeclId::MAX_INDEX),
                mangling_index: 0,
                value: None,
            }],
            ctor_inits: vec![CtorInit {
                target: 0,
                kind: CtorInitKind::Delegating,
                written_index: CtorInit::IMPLICIT,
                init: ExprId::new(u32::MAX),
                loc: Loc::INVALID,
            }],
            cold: ColdDeclData::default(),
        }
    }
}

/// A kind-specific payload, passed to [`DeclArena::push`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Payload {
    /// No payload (or the inline `data` documented on the kind).
    None(u32),
    /// Function kinds.
    Function(FunctionData),
    /// Variable kinds.
    Var(VarData),
    /// Record kinds.
    Record(RecordData),
    /// `Field`.
    Field(FieldData),
    /// `Enum`.
    Enum(EnumData),
    /// `EnumConstant`.
    Enumerator(EnumeratorData),
    /// `ParmVar`.
    Param(ParamData),
    /// `Typedef` / `TypeAlias`.
    Typedef(TypedefData),
    /// `Namespace`.
    Namespace(NamespaceData),
    /// TU, `LinkageSpec`, `Export`, `RequiresExprBody`.
    Scope(ScopeData),
    /// `StaticAssert`.
    StaticAssert(StaticAssertData),
    /// `LifetimeExtendedTemporary`.
    Let(LetData),
}

impl Payload {
    const fn class(&self) -> PayloadClass {
        match self {
            Payload::None(_) => PayloadClass::None,
            Payload::Function(_) => PayloadClass::Function,
            Payload::Var(_) => PayloadClass::Var,
            Payload::Record(_) => PayloadClass::Record,
            Payload::Field(_) => PayloadClass::Field,
            Payload::Enum(_) => PayloadClass::Enum,
            Payload::Enumerator(_) => PayloadClass::Enumerator,
            Payload::Param(_) => PayloadClass::Param,
            Payload::Typedef(_) => PayloadClass::Typedef,
            Payload::Namespace(_) => PayloadClass::Namespace,
            Payload::Scope(_) => PayloadClass::Scope,
            Payload::StaticAssert(_) => PayloadClass::StaticAssert,
            Payload::Let(_) => PayloadClass::Let,
        }
    }
}

/// The header fields of a new declaration (everything but `next` and
/// `data`, which the arena fills).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NewDecl {
    /// The kind.
    pub kind: DeclKind,
    /// Initial flags.
    pub flags: DeclFlags,
    /// Misc bits.
    pub misc: DeclMisc,
    /// The name.
    pub name: DeclName,
    /// The semantic context.
    pub ctx: Option<DeclId>,
    /// The name's location.
    pub loc: Loc,
    /// The declared type.
    pub ty: Option<QualType>,
}

fn push_payload<T>(v: &mut Vec<T>, x: T) -> u32 {
    let i = v.len();
    assert!(i < u32::MAX as usize, "declaration payload space exhausted");
    v.push(x);
    i as u32
}

macro_rules! payload_getter {
    ($(#[$m:meta])* $name:ident, $mut_name:ident, $field:ident, $class:ident, $ty:ty) => {
        $(#[$m])*
        pub fn $name(&self, d: DeclId) -> Option<&$ty> {
            let n = self.get(d);
            if n.kind.payload() == PayloadClass::$class && n.data != 0 {
                Some(&self.$field[n.data as usize])
            } else {
                None
            }
        }
        #[allow(dead_code, reason = "not every payload has a late setter yet")]
        fn $mut_name(&mut self, d: DeclId) -> &mut $ty {
            let n = self.nodes[d.index() as usize];
            assert!(
                n.kind.payload() == PayloadClass::$class && n.data != 0,
                concat!("decl has no ", stringify!($class), " payload")
            );
            &mut self.$field[n.data as usize]
        }
    };
}

impl DeclArena {
    /// An arena holding only the dummies.
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of declarations (excluding the dummy).
    pub fn len(&self) -> usize {
        self.nodes.len() - 1
    }

    /// True if there are no declarations.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The header of `d`.
    pub fn get(&self, d: DeclId) -> &DeclNode {
        &self.nodes[d.index() as usize]
    }

    /// All declaration ids, in creation order.
    pub fn ids(&self) -> impl ExactSizeIterator<Item = DeclId> + '_ {
        (1..self.nodes.len() as u32).map(DeclId::new)
    }

    /// Appends a declaration with its payload. Does not link it into any
    /// member list (see [`DeclArena::append_member`]).
    ///
    /// # Panics
    /// If the payload does not match the kind, or on id exhaustion.
    pub fn push(&mut self, h: NewDecl, payload: Payload) -> DeclId {
        let class = payload.class();
        assert!(
            class == h.kind.payload() || matches!(payload, Payload::None(_)),
            "payload {class:?} does not match decl kind {:?}",
            h.kind
        );
        assert!(
            h.kind.payload() == PayloadClass::None || class != PayloadClass::None,
            "decl kind {:?} requires a payload",
            h.kind
        );
        let data = match payload {
            Payload::None(x) => x,
            Payload::Function(x) => push_payload(&mut self.functions, x),
            Payload::Var(x) => push_payload(&mut self.vars, x),
            Payload::Record(x) => push_payload(&mut self.records, x),
            Payload::Field(x) => push_payload(&mut self.fields, x),
            Payload::Enum(x) => push_payload(&mut self.enums, x),
            Payload::Enumerator(x) => push_payload(&mut self.enumerators, x),
            Payload::Param(x) => push_payload(&mut self.params, x),
            Payload::Typedef(x) => push_payload(&mut self.typedefs, x),
            Payload::Namespace(x) => push_payload(&mut self.namespaces, x),
            Payload::Scope(x) => push_payload(&mut self.scopes, x),
            Payload::StaticAssert(x) => push_payload(&mut self.static_asserts, x),
            Payload::Let(x) => push_payload(&mut self.lets, x),
        };
        let id = DeclId::new(self.nodes.len() as u32);
        self.nodes.push(DeclNode {
            kind: h.kind,
            flags: h.flags,
            misc: h.misc,
            name: h.name,
            ctx: h.ctx,
            next: None,
            loc: h.loc,
            ty: h.ty,
            data,
        });
        id
    }

    /// The id the next [`DeclArena::push`] returns.
    pub fn next_id(&self) -> DeclId {
        DeclId::new(self.nodes.len() as u32)
    }

    payload_getter!(
        /// The function payload of `d`, if `d` is a function kind.
        function, function_mut, functions, Function, FunctionData
    );
    payload_getter!(
        /// The variable payload of `d`, if `d` is a variable kind.
        var, var_mut, vars, Var, VarData
    );
    payload_getter!(
        /// The record payload of `d`, if `d` is a record kind.
        record, record_mut, records, Record, RecordData
    );
    payload_getter!(
        /// The field payload of `d`.
        field, field_mut, fields, Field, FieldData
    );
    payload_getter!(
        /// The enumeration payload of `d`.
        enumeration, enumeration_mut, enums, Enum, EnumData
    );
    payload_getter!(
        /// The enumerator payload of `d`.
        enumerator, enumerator_mut, enumerators, Enumerator, EnumeratorData
    );
    payload_getter!(
        /// The parameter payload of `d`.
        param, param_mut, params, Param, ParamData
    );
    payload_getter!(
        /// The typedef / alias payload of `d`.
        typedef, typedef_mut, typedefs, Typedef, TypedefData
    );
    payload_getter!(
        /// The namespace payload of `d`.
        namespace, namespace_mut, namespaces, Namespace, NamespaceData
    );
    payload_getter!(
        /// The scope payload of the TU, a `LinkageSpec`, `Export` or
        /// `RequiresExprBody`.
        scope, scope_mut, scopes, Scope, ScopeData
    );
    payload_getter!(
        /// The `static_assert` payload of `d`.
        static_assert, static_assert_mut, static_asserts, StaticAssert, StaticAssertData
    );
    payload_getter!(
        /// The `LifetimeExtendedTemporary` payload of `d`.
        lifetime_extended, lifetime_extended_mut, lets, Let, LetData
    );

    /// The redeclaration link of a redeclarable `d`.
    pub fn redecl(&self, d: DeclId) -> Option<Redecl> {
        let n = self.get(d);
        if n.data == 0 {
            return None;
        }
        let i = n.data as usize;
        match n.kind.payload() {
            PayloadClass::Function => Some(self.functions[i].redecl),
            PayloadClass::Var => Some(self.vars[i].redecl),
            PayloadClass::Record => Some(self.records[i].redecl),
            PayloadClass::Enum => Some(self.enums[i].redecl),
            PayloadClass::Typedef => Some(self.typedefs[i].redecl),
            PayloadClass::Namespace => Some(self.namespaces[i].redecl),
            _ => None,
        }
        .filter(|r| r.group != DUMMY_GROUP)
    }

    /// The builtin id of `d` if it is an implicit builtin function
    /// declaration ([`builtin`], §5.3).
    pub fn builtin_id(&self, d: DeclId) -> Option<u32> {
        self.cold.builtin_id.get(d).copied()
    }

    /// The member list of context `ctx` (`None` if `ctx` owns none).
    pub fn member_list(&self, ctx: DeclId) -> Option<Members> {
        let n = self.get(ctx);
        let i = n.data as usize;
        if i == 0 {
            return None;
        }
        match n.kind.payload() {
            PayloadClass::Record => Some(self.records[i].members),
            PayloadClass::Enum => Some(self.enums[i].members),
            PayloadClass::Namespace => Some(self.namespaces[i].members),
            PayloadClass::Scope => Some(self.scopes[i].members),
            _ => None,
        }
    }

    /// The lexical members of `ctx`, in declaration order.
    pub fn members(&self, ctx: DeclId) -> MemberIter<'_> {
        MemberIter { arena: self, cur: self.member_list(ctx).and_then(|m| m.first) }
    }

    /// The lexical context of `d` (`ctx` unless `HAS_LEXICAL_CTX`).
    pub fn lexical_ctx(&self, d: DeclId) -> Option<DeclId> {
        let n = self.get(d);
        if n.has(DeclFlags::HAS_LEXICAL_CTX) {
            self.cold.lexical_ctx.get(d).copied()
        } else {
            n.ctx
        }
    }

    /// The typedef naming unnamed tag `d` for linkage purposes
    /// ([dcl.typedef]/9), if any.
    pub fn typedef_for_anon(&self, d: DeclId) -> Option<DeclId> {
        self.cold.typedef_for_anon.get(d).copied()
    }

    /// Records `typedef_decl` as the name of unnamed tag `tag` for linkage
    /// purposes ([dcl.typedef]/9). Write-once.
    pub fn set_typedef_for_anon(
        &mut self,
        tag: DeclId,
        typedef_decl: DeclId,
    ) -> Result<(), AlreadySet> {
        self.cold.typedef_for_anon.insert(tag, typedef_decl)
    }

    /// Records a lexical context different from the semantic one. Must be
    /// called before `d` is appended to a member list.
    pub fn set_lexical_ctx(&mut self, d: DeclId, lexical: DeclId) -> Result<(), AlreadySet> {
        self.cold.lexical_ctx.insert(d, lexical)?;
        self.nodes[d.index() as usize].flags = self.get(d).flags.with(DeclFlags::HAS_LEXICAL_CTX);
        Ok(())
    }

    /// Appends `d` to `ctx`'s member list (O(1) through `next`).
    ///
    /// # Panics
    /// If `ctx` owns no member list, `d` is already linked, or `ctx` is a
    /// complete record or enumeration and `d` is neither `IMPLICIT` (lazily
    /// declared special members), `INJECTED` (`define_aggregate`, §10) nor
    /// a variable (an implicit specialization of a member variable
    /// template, listed when first referenced): a class's member list is
    /// frozen at `}` (§2.4).
    pub fn append_member(&mut self, ctx: DeclId, d: DeclId) {
        let dn = *self.get(d);
        assert!(dn.next.is_none(), "decl is already in a member list");
        let n = *self.get(ctx);
        let i = n.data as usize;
        let late_ok =
            dn.has(DeclFlags::IMPLICIT) || dn.has(DeclFlags::INJECTED) || dn.kind == DeclKind::Var;
        let members = match n.kind.payload() {
            PayloadClass::Record => {
                let r = &mut self.records[i];
                assert!(
                    !r.defn.contains(DefinitionBits::COMPLETE) || late_ok,
                    "member added to a complete class (frozen at `}}`, spec/ast.md §2.4)"
                );
                &mut r.members
            }
            PayloadClass::Enum => {
                let e = &mut self.enums[i];
                assert!(
                    e.flags & EnumData::COMPLETE == 0,
                    "enumerator added to a complete enumeration"
                );
                &mut e.members
            }
            PayloadClass::Namespace => &mut self.namespaces[i].members,
            PayloadClass::Scope if i != 0 => &mut self.scopes[i].members,
            _ => panic!("decl kind {:?} owns no member list", n.kind),
        };
        let last = members.last.replace(d);
        if last.is_none() {
            members.first = Some(d);
        }
        if let Some(last) = last {
            assert!(last != d, "decl appended twice");
            self.nodes[last.index() as usize].next = Some(d);
        }
    }

    /// Sets monotone flags ([`DeclFlags::MONOTONE`]: `USED`, `REFERENCED`,
    /// `INVALID`).
    ///
    /// # Panics
    /// If `f` contains other flags.
    pub fn set_flags(&mut self, d: DeclId, f: DeclFlags) {
        assert!(DeclFlags::MONOTONE.contains(f), "only USED/REFERENCED/INVALID may be set late");
        let n = &mut self.nodes[d.index() as usize];
        n.flags = n.flags.with(f);
    }

    /// Re-points the location of `d` (a class template specialization
    /// that becomes explicit is located at its explicit declaration, as
    /// clang).
    pub(crate) fn set_loc(&mut self, d: DeclId, loc: Loc) {
        self.nodes[d.index() as usize].loc = loc;
    }

    /// Sets `HAS_ATTRS` (by `AttrArena::attach`).
    pub(crate) fn set_has_attrs(&mut self, d: DeclId) {
        let n = &mut self.nodes[d.index() as usize];
        n.flags = n.flags.with(DeclFlags::HAS_ATTRS);
    }

    /// Moves parameter `p` into function `func`: a parameter is created
    /// while its declarator is parsed, so that it is in scope in the
    /// rest of the declarator ([basic.scope.param]), and adopted by the
    /// function once that exists (clang's `setDeclContext` in
    /// `ActOnFunctionDeclarator`).
    ///
    /// # Panics
    /// If `p` is not a `ParmVar`.
    pub fn set_param_owner(&mut self, p: DeclId, func: DeclId) {
        let n = &mut self.nodes[p.index() as usize];
        assert!(n.kind == DeclKind::ParmVar, "set_param_owner on a {:?}", n.kind);
        n.ctx = Some(func);
    }

    /// Re-points the declared type (deduced return types, resolved
    /// exception specifications; §2.4).
    pub fn set_ty(&mut self, d: DeclId, ty: QualType) {
        self.nodes[d.index() as usize].ty = Some(ty);
    }

    /// Stores the linkage/visibility caches of `misc` (cache semantics:
    /// idempotent; a different value for a computed cache is a bug).
    ///
    /// # Panics
    /// If a cache was already computed with a different value.
    pub fn set_linkage_cache(&mut self, d: DeclId, linkage: u8, visibility: u8) {
        assert!(linkage != 0 && linkage < 8 && visibility < 4, "linkage cache out of range");
        let n = &mut self.nodes[d.index() as usize];
        let old = n.misc.linkage_cache();
        assert!(
            old == 0 || (old == linkage && n.misc.visibility_cache() == visibility),
            "linkage cache changed"
        );
        n.misc =
            n.misc.with_field(9, 3, u16::from(linkage)).with_field(12, 2, u16::from(visibility));
    }

    /// Sets the access of member `d` ([class.access]), right after the
    /// builder created it: sema knows the current access-specifier, the
    /// typed constructors take only a [`crate::builder::DeclHead`] (WP-066b).
    pub fn set_access(&mut self, d: DeclId, a: Access) {
        let n = &mut self.nodes[d.index() as usize];
        n.misc = n.misc.with_access(a);
    }

    /// Sets a function's body (write-once, §2.4).
    pub fn set_body(&mut self, f: DeclId, body: StmtId) -> Result<(), AlreadySet> {
        set_once(&mut self.function_mut(f).body, body, "FunctionData.body", RawId::raw)
    }

    /// Sets late specifier bits ([`FnSpec::LATE`]: trivial, contracts,
    /// coroutine, escalated, deleted, implicit inline, explicitly
    /// defaulted, constexpr).
    ///
    /// # Panics
    /// If `bits` contains other bits.
    pub fn mark_function(&mut self, f: DeclId, bits: FnSpec) {
        assert!(bits.0 & !FnSpec::LATE.0 == 0, "FnSpec bit is not settable after creation");
        let d = self.function_mut(f);
        d.spec = FnSpec(d.spec.0 | bits.0);
    }

    /// Replaces the specifiers of function template specialization `f`
    /// when it becomes an explicit specialization: those are the explicit
    /// declaration's, not the template's ([temp.expl.spec]/13; WP-106).
    pub fn respecify_function(&mut self, f: DeclId, spec: FnSpec) {
        self.function_mut(f).spec = spec;
    }

    /// Sets the parameter list of function `f` (once a non-empty list is
    /// set it is frozen; parameters are created after their function
    /// because they name it as their semantic context).
    pub fn set_params(&mut self, f: DeclId, params: ListId) -> Result<(), AlreadySet> {
        let data = self.function_mut(f);
        if data.params != ListId::EMPTY {
            return Err(AlreadySet { slot: "FunctionData.params", existing: data.params.raw() });
        }
        data.params = params;
        Ok(())
    }

    /// Sets a template link that was [`TemplLink::NONE`] (M3: the pattern
    /// is created before its template decl).
    pub fn set_templ(&mut self, d: DeclId, link: TemplLink) -> Result<(), AlreadySet> {
        let n = *self.get(d);
        let i = n.data as usize;
        let slot = match n.kind.payload() {
            PayloadClass::Function => &mut self.functions[i].templ,
            PayloadClass::Var => &mut self.vars[i].templ,
            PayloadClass::Record => &mut self.records[i].templ,
            PayloadClass::Enum => &mut self.enums[i].templ,
            PayloadClass::Typedef => &mut self.typedefs[i].templ,
            _ => panic!("decl kind {:?} is not templatable", n.kind),
        };
        if !slot.is_none() {
            return Err(AlreadySet { slot: "TemplLink", existing: slot.0 });
        }
        *slot = link;
        Ok(())
    }

    /// Sets a variable's initializer (write-once, §2.4).
    pub fn set_init(&mut self, v: DeclId, init: ExprId) -> Result<(), AlreadySet> {
        set_once(&mut self.var_mut(v).init, init, "VarData.init", RawId::raw)
    }

    /// Sets late variable bits ([`VarSpec::LATE`]: NRVO).
    ///
    /// # Panics
    /// If `bits` contains other bits.
    pub fn mark_var(&mut self, v: DeclId, bits: VarSpec) {
        assert!(bits.0 & !VarSpec::LATE.0 == 0, "VarSpec bit is not settable after creation");
        let d = self.var_mut(v);
        d.spec = VarSpec(d.spec.0 | bits.0);
    }

    /// `NotEvaluated → Evaluating` (§9.2). Returns the current state
    /// unchanged as `Err` if it is not `NotEvaluated` (`Evaluating` there
    /// means a self-referential initializer).
    pub fn begin_eval(&mut self, v: DeclId) -> Result<(), VarEvalState> {
        let d = self.var_mut(v);
        match d.eval.state() {
            VarEvalState::NotEvaluated => {
                d.eval = VarEval(VarEval::EVALUATING);
                Ok(())
            }
            s => Err(s),
        }
    }

    /// `Evaluating → Value(id) / NotConstant` (`None` = not constant).
    ///
    /// # Panics
    /// If the state is not `Evaluating`, or `value` is one of the two
    /// reserved raw values.
    pub fn finish_eval(&mut self, v: DeclId, value: Option<ConstValueId>) {
        let d = self.var_mut(v);
        assert_eq!(d.eval.state(), VarEvalState::Evaluating, "finish_eval without begin_eval");
        d.eval = VarEval(match value {
            Some(c) => {
                assert!(
                    c.index() < VarEval::NOT_CONSTANT,
                    "ConstValueId collides with VarEval sentinels"
                );
                c.index()
            }
            None => VarEval::NOT_CONSTANT,
        });
    }

    /// `Evaluating → NotEvaluated`: an evaluation was abandoned without a
    /// verdict (e.g. it hit a resource limit and may be retried).
    ///
    /// # Panics
    /// If the state is not `Evaluating`.
    pub fn abort_eval(&mut self, v: DeclId) {
        let d = self.var_mut(v);
        assert_eq!(d.eval.state(), VarEvalState::Evaluating, "abort_eval without begin_eval");
        d.eval = VarEval(0);
    }

    /// `{` of a class definition: marks it being defined.
    ///
    /// # Panics
    /// If the record is already being defined or complete.
    pub fn begin_definition(&mut self, r: DeclId) {
        let d = self.record_mut(r);
        assert!(
            !d.defn.contains(DefinitionBits::BEING_DEFINED)
                && !d.defn.contains(DefinitionBits::COMPLETE),
            "class defined twice"
        );
        d.defn = d.defn.with(DefinitionBits::BEING_DEFINED);
    }

    /// Sets property bits of a class being defined.
    ///
    /// # Panics
    /// If the class is not being defined (properties freeze at `}`).
    pub fn set_definition_bits(&mut self, r: DeclId, bits: DefinitionBits) {
        let d = self.record_mut(r);
        assert!(
            d.defn.contains(DefinitionBits::BEING_DEFINED),
            "class properties are frozen at `}}`"
        );
        d.defn = d.defn.with(bits);
    }

    /// Sets the base-specifiers of a class being defined (once).
    pub fn set_bases(
        &mut self,
        r: DeclId,
        bases: &[BaseSpec],
        lists: &mut crate::lists::IdLists,
    ) -> Result<(), AlreadySet> {
        assert!(
            self.record_mut(r).defn.contains(DefinitionBits::BEING_DEFINED),
            "bases set outside the class definition"
        );
        if self.record_mut(r).bases != ListId::EMPTY {
            return Err(AlreadySet {
                slot: "RecordData.bases",
                existing: self.record_mut(r).bases.raw(),
            });
        }
        if bases.is_empty() {
            return Ok(());
        }
        let start = self.base_specs.len() as u32;
        self.base_specs.extend_from_slice(bases);
        let list = lists.push_raw(&(start..start + bases.len() as u32).collect::<Vec<_>>());
        self.record_mut(r).bases = list;
        Ok(())
    }

    /// The base-specifier at global index `i` (a value of a record's
    /// `bases` list or of a cast's base path, `ExprArena::cast_path`).
    pub fn base_spec(&self, i: u32) -> &BaseSpec {
        &self.base_specs[i as usize]
    }

    /// The base-specifiers of class `r`.
    pub fn bases<'a>(
        &'a self,
        r: DeclId,
        lists: &'a crate::lists::IdLists,
    ) -> impl Iterator<Item = &'a BaseSpec> + 'a {
        let list = self.record(r).map_or(ListId::EMPTY, |d| d.bases);
        lists.get(list).iter().map(|&i| &self.base_specs[i as usize])
    }

    /// `}` of a class definition: the class is complete and its member list
    /// frozen (§2.4).
    ///
    /// # Panics
    /// If the class is not being defined.
    pub fn complete_definition(&mut self, r: DeclId) {
        let d = self.record_mut(r);
        assert!(
            d.defn.contains(DefinitionBits::BEING_DEFINED),
            "complete_definition without begin_definition"
        );
        d.defn = d.defn.without(DefinitionBits::BEING_DEFINED).with(DefinitionBits::COMPLETE);
    }

    /// Sets a field's default member initializer (write-once).
    pub fn set_field_init(&mut self, f: DeclId, init: ExprId) -> Result<(), AlreadySet> {
        set_once(&mut self.field_mut(f).init, init, "FieldData.init", RawId::raw)
    }

    /// `{` of an enumeration definition.
    pub fn begin_enum_definition(&mut self, e: DeclId) {
        let d = self.enumeration_mut(e);
        assert!(
            d.flags & (EnumData::BEING_DEFINED | EnumData::COMPLETE) == 0,
            "enumeration defined twice"
        );
        d.flags |= EnumData::BEING_DEFINED;
    }

    /// Sets the underlying and promotion types (once: at declaration when
    /// fixed, else at `}`).
    pub fn set_enum_int_type(
        &mut self,
        e: DeclId,
        int_type: QualType,
        promotion: QualType,
    ) -> Result<(), AlreadySet> {
        let d = self.enumeration_mut(e);
        set_once(&mut d.int_type, int_type, "EnumData.int_type", QualType::to_raw)?;
        d.promotion_type = Some(promotion);
        Ok(())
    }

    /// `}` of an enumeration definition.
    pub fn complete_enum_definition(&mut self, e: DeclId) {
        let d = self.enumeration_mut(e);
        assert!(d.flags & EnumData::BEING_DEFINED != 0, "complete_enum_definition without begin");
        d.flags = (d.flags & !EnumData::BEING_DEFINED) | EnumData::COMPLETE;
    }

    /// Sets an enumerator's value (once; enumerators with a value-dependent
    /// initializer never get one).
    pub fn set_enumerator_value(&mut self, e: DeclId, v: ConstValueId) -> Result<(), AlreadySet> {
        set_once(&mut self.enumerator_mut(e).value, v, "EnumeratorData.value", RawId::raw)
    }

    /// Replaces an enumerator's initializer with `init`, which must wrap
    /// the previous one: at the closing brace of an enumeration without a
    /// fixed underlying type, sema converts each initializer to the
    /// enumeration's integer type ([dcl.enum]/5, clang's
    /// `ECD->setInitExpr` in `ActOnEnumBody`).
    pub fn replace_enumerator_init(&mut self, e: DeclId, init: ExprId) {
        self.enumerator_mut(e).init = Some(init);
    }

    /// Sets a parameter's default argument (write-once).
    pub fn set_default_arg(&mut self, p: DeclId, arg: ExprId) -> Result<(), AlreadySet> {
        set_once(&mut self.param_mut(p).default_arg, arg, "ParamData.default_arg", RawId::raw)
    }

    /// Marks a parameter's default argument as pending late parsing
    /// ([`ParamData::DEFAULT_ARG_PENDING`]; set before the class is
    /// complete, cleared implicitly by reading `default_arg` once set).
    pub fn mark_default_arg_pending(&mut self, p: DeclId) {
        self.param_mut(p).flags |= ParamData::DEFAULT_ARG_PENDING;
    }

    /// Sets the constant value of a lifetime-extended temporary
    /// (write-once, by the Oven through the bridge).
    pub fn set_lifetime_extended_value(
        &mut self,
        d: DeclId,
        v: ConstValueId,
    ) -> Result<(), AlreadySet> {
        set_once(&mut self.lifetime_extended_mut(d).value, v, "LetData.value", RawId::raw)
    }

    /// Sets the ctor-initializers of constructor `ctor` (write-once, with
    /// the body; spec/sema-class.md §8.2). An empty slice records nothing.
    pub fn set_ctor_inits(
        &mut self,
        ctor: DeclId,
        inits: &[CtorInit],
        lists: &mut crate::lists::IdLists,
    ) -> Result<(), AlreadySet> {
        assert!(
            self.get(ctor).kind() == DeclKind::CXXConstructor,
            "ctor-initializers on a non-constructor"
        );
        if let Some(&l) = self.cold.ctor_inits.get(ctor) {
            return Err(AlreadySet { slot: "ctor_inits", existing: l.raw() });
        }
        if inits.is_empty() {
            return Ok(());
        }
        let start = self.ctor_inits.len() as u32;
        self.ctor_inits.extend_from_slice(inits);
        let list = lists.push_raw(&(start..start + inits.len() as u32).collect::<Vec<_>>());
        self.cold.ctor_inits.insert(ctor, list)
    }

    /// The ctor-initializers of constructor `ctor`, in initialization
    /// order (empty if none were set).
    pub fn ctor_inits<'a>(
        &'a self,
        ctor: DeclId,
        lists: &'a crate::lists::IdLists,
    ) -> impl ExactSizeIterator<Item = &'a CtorInit> + 'a {
        let list = self.cold.ctor_inits.get(ctor).copied().unwrap_or(ListId::EMPTY);
        lists.get(list).iter().map(|&i| &self.ctor_inits[i as usize])
    }

    /// What the `Friend` declaration `d` befriends.
    ///
    /// # Panics
    /// If `d` is not a `Friend` declaration.
    pub fn friend_target(&self, d: DeclId) -> FriendTarget {
        let n = self.get(d);
        assert!(n.kind() == DeclKind::Friend, "expected Friend, found {:?}", n.kind());
        match n.ty() {
            Some(ty) => FriendTarget::Type { ty, owned_tag: DeclId::from_raw(n.data()) },
            None => FriendTarget::Decl(DeclId::new(n.data())),
        }
    }

    /// Bytes of the node and payload vectors (instantiation meter).
    pub(crate) fn live_bytes(&self) -> u64 {
        self.vec_rows().iter().map(|r| r.live_bytes).sum()
    }

    fn vec_rows(&self) -> [MemRow; 15] {
        [
            MemRow::of_vec("nodes", &self.nodes),
            MemRow::of_vec("functions", &self.functions),
            MemRow::of_vec("vars", &self.vars),
            MemRow::of_vec("records", &self.records),
            MemRow::of_vec("fields", &self.fields),
            MemRow::of_vec("enums", &self.enums),
            MemRow::of_vec("enumerators", &self.enumerators),
            MemRow::of_vec("params", &self.params),
            MemRow::of_vec("typedefs", &self.typedefs),
            MemRow::of_vec("namespaces", &self.namespaces),
            MemRow::of_vec("scopes", &self.scopes),
            MemRow::of_vec("static_asserts", &self.static_asserts),
            MemRow::of_vec("base_specs", &self.base_specs),
            MemRow::of_vec("lets", &self.lets),
            MemRow::of_vec("ctor_inits", &self.ctor_inits),
        ]
    }
}

/// Iterator over a context's lexical members ([`DeclArena::members`]).
#[derive(Clone, Debug)]
pub struct MemberIter<'a> {
    arena: &'a DeclArena,
    cur: Option<DeclId>,
}

impl Iterator for MemberIter<'_> {
    type Item = DeclId;
    fn next(&mut self) -> Option<DeclId> {
        let d = self.cur?;
        self.cur = self.arena.get(d).next;
        Some(d)
    }
}

impl HeapSize for DeclArena {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        for row in self.vec_rows() {
            r.push(row);
        }
        for row in self.cold.rows() {
            r.push(row.prefixed("cold"));
        }
        r
    }
}

/// Builtin function ids (the `builtin_id` cold data of an implicit builtin
/// `Function` decl, §5.3). The numbering is clang 19's `Builtin::ID` (its
/// target-independent part), so a `BuiltinAttr` dumps as clang's does.
/// [`ALL`] gives each one's spelling, signature and attributes in clang's
/// `Builtins.td` notation (spec/sema-templates.md §9.5).
pub mod builtin {
    /// `bool __builtin_is_constant_evaluated() noexcept` ([meta.const.eval]).
    pub const IS_CONSTANT_EVALUATED: u32 = 722;
    /// `void *__builtin_addressof(void &) noexcept` ([specialized.addressof];
    /// custom type-checking: the result is a pointer to the operand's type).
    pub const ADDRESSOF: u32 = 200;
    /// `__builtin_clz` (clang `Builtins.td` signature `iUi`).
    pub const CLZ: u32 = 341;
    /// `__builtin_clzl` (clang `Builtins.td` signature `iULi`).
    pub const CLZL: u32 = 342;
    /// `__builtin_clzll` (clang `Builtins.td` signature `iULLi`).
    pub const CLZLL: u32 = 343;
    /// `__builtin_ctz` (clang `Builtins.td` signature `iUi`).
    pub const CTZ: u32 = 443;
    /// `__builtin_ctzl` (clang `Builtins.td` signature `iULi`).
    pub const CTZL: u32 = 444;
    /// `__builtin_ctzll` (clang `Builtins.td` signature `iULLi`).
    pub const CTZLL: u32 = 445;
    /// `__builtin_popcount` (clang `Builtins.td` signature `iUi`).
    pub const POPCOUNT: u32 = 961;
    /// `__builtin_popcountl` (clang `Builtins.td` signature `iULi`).
    pub const POPCOUNTL: u32 = 962;
    /// `__builtin_popcountll` (clang `Builtins.td` signature `iULLi`).
    pub const POPCOUNTLL: u32 = 963;
    /// `__builtin_clzg` (clang `Builtins.td` signature `i.`).
    pub const CLZG: u32 = 344;
    /// `__builtin_ctzg` (clang `Builtins.td` signature `i.`).
    pub const CTZG: u32 = 446;
    /// `__builtin_popcountg` (clang `Builtins.td` signature `i.`).
    pub const POPCOUNTG: u32 = 964;
    /// `__builtin_bswap16` (clang `Builtins.td` signature `UsUs`).
    pub const BSWAP16: u32 = 179;
    /// `__builtin_bswap32` (clang `Builtins.td` signature `UiUi`).
    pub const BSWAP32: u32 = 180;
    /// `__builtin_bswap64` (clang `Builtins.td` signature `ULiULi`).
    pub const BSWAP64: u32 = 181;
    /// `__builtin_memcmp` (clang `Builtins.td` signature `ivC*vC*z`).
    pub const MEMCMP: u32 = 218;
    /// `__builtin_memcpy` (clang `Builtins.td` signature `v*v*vC*z`).
    pub const MEMCPY: u32 = 865;
    /// `__builtin_memmove` (clang `Builtins.td` signature `v*v*vC*z`).
    pub const MEMMOVE: u32 = 867;
    /// `__builtin_memchr` (clang `Builtins.td` signature `v*vC*iz`).
    pub const MEMCHR: u32 = 862;
    /// `__builtin_char_memchr` (clang `Builtins.td` signature `c*cC*iz`).
    pub const CHAR_MEMCHR: u32 = 208;
    /// `__builtin_strlen` (clang `Builtins.td` signature `zcC*`).
    pub const STRLEN: u32 = 1147;
    /// `__builtin_wcslen` (clang `Builtins.td` signature `zwC*`).
    pub const WCSLEN: u32 = 1390;
    /// `__builtin_wmemchr` (clang `Builtins.td` signature `w*wC*wz`).
    pub const WMEMCHR: u32 = 1377;
    /// `__builtin_wmemcmp` (clang `Builtins.td` signature `iwC*wC*z`).
    pub const WMEMCMP: u32 = 1379;
    /// `__builtin_isfinite` (clang `Builtins.td` signature `i.`).
    pub const ISFINITE: u32 = 725;
    /// `__builtin_isinf` (clang `Builtins.td` signature `i.`).
    pub const ISINF: u32 = 729;
    /// `__builtin_isnan` (clang `Builtins.td` signature `i.`).
    pub const ISNAN: u32 = 735;
    /// `__builtin_isnormal` (clang `Builtins.td` signature `i.`).
    pub const ISNORMAL: u32 = 736;
    /// `__builtin_signbit` (clang `Builtins.td` signature `i.`).
    pub const SIGNBIT: u32 = 1089;
    /// `__builtin_isgreater` (clang `Builtins.td` signature `i.`).
    pub const ISGREATER: u32 = 727;
    /// `__builtin_isgreaterequal` (clang `Builtins.td` signature `i.`).
    pub const ISGREATEREQUAL: u32 = 728;
    /// `__builtin_isless` (clang `Builtins.td` signature `i.`).
    pub const ISLESS: u32 = 731;
    /// `__builtin_islessequal` (clang `Builtins.td` signature `i.`).
    pub const ISLESSEQUAL: u32 = 732;
    /// `__builtin_islessgreater` (clang `Builtins.td` signature `i.`).
    pub const ISLESSGREATER: u32 = 733;
    /// `__builtin_isunordered` (clang `Builtins.td` signature `i.`).
    pub const ISUNORDERED: u32 = 742;
    /// `__builtin_huge_val` (clang `Builtins.td` signature `d`).
    pub const HUGE_VAL: u32 = 652;
    /// `__builtin_huge_valf` (clang `Builtins.td` signature `f`).
    pub const HUGE_VALF: u32 = 651;
    /// `__builtin_huge_vall` (clang `Builtins.td` signature `Ld`).
    pub const HUGE_VALL: u32 = 653;
    /// `__builtin_nan` (clang `Builtins.td` signature `dcC*`).
    pub const NAN: u32 = 892;
    /// `__builtin_nanf` (clang `Builtins.td` signature `fcC*`).
    pub const NANF: u32 = 890;
    /// `__builtin_nanl` (clang `Builtins.td` signature `LdcC*`).
    pub const NANL: u32 = 894;
    /// `__builtin_nans` (clang `Builtins.td` signature `dcC*`).
    pub const NANS: u32 = 899;
    /// `__builtin_nansf` (clang `Builtins.td` signature `fcC*`).
    pub const NANSF: u32 = 898;
    /// `__builtin_nansl` (clang `Builtins.td` signature `LdcC*`).
    pub const NANSL: u32 = 900;
    /// `__builtin_exp` (clang `Builtins.td` signature `dd`).
    pub const EXP: u32 = 509;
    /// `__builtin_expf` (clang `Builtins.td` signature `ff`).
    pub const EXPF: u32 = 507;
    /// `__builtin_expl` (clang `Builtins.td` signature `LdLd`).
    pub const EXPL: u32 = 511;
    /// `__builtin_exp2` (clang `Builtins.td` signature `dd`).
    pub const EXP2: u32 = 515;
    /// `__builtin_exp2f` (clang `Builtins.td` signature `ff`).
    pub const EXP2F: u32 = 513;
    /// `__builtin_exp2l` (clang `Builtins.td` signature `LdLd`).
    pub const EXP2L: u32 = 517;
    /// `__builtin_expm1` (clang `Builtins.td` signature `dd`).
    pub const EXPM1: u32 = 532;
    /// `__builtin_expm1f` (clang `Builtins.td` signature `ff`).
    pub const EXPM1F: u32 = 530;
    /// `__builtin_expm1l` (clang `Builtins.td` signature `LdLd`).
    pub const EXPM1L: u32 = 534;
    /// `__builtin_frexp` (clang `Builtins.td` signature `ddi*`).
    pub const FREXP: u32 = 615;
    /// `__builtin_frexpf` (clang `Builtins.td` signature `ffi*`).
    pub const FREXPF: u32 = 613;
    /// `__builtin_frexpl` (clang `Builtins.td` signature `LdLdi*`).
    pub const FREXPL: u32 = 617;
    /// `__builtin_ldexp` (clang `Builtins.td` signature `ddi`).
    pub const LDEXP: u32 = 759;
    /// `__builtin_ldexpf` (clang `Builtins.td` signature `ffi`).
    pub const LDEXPF: u32 = 757;
    /// `__builtin_ldexpl` (clang `Builtins.td` signature `LdLdi`).
    pub const LDEXPL: u32 = 761;
    /// `__builtin_pow` (clang `Builtins.td` signature `ddd`).
    pub const POW: u32 = 967;
    /// `__builtin_powf` (clang `Builtins.td` signature `fff`).
    pub const POWF: u32 = 965;
    /// `__builtin_powl` (clang `Builtins.td` signature `LdLdLd`).
    pub const POWL: u32 = 969;
    /// `__builtin_scalbn` (clang `Builtins.td` signature `ddi`).
    pub const SCALBN: u32 = 1074;
    /// `__builtin_scalbnf` (clang `Builtins.td` signature `ffi`).
    pub const SCALBNF: u32 = 1072;
    /// `__builtin_scalbnl` (clang `Builtins.td` signature `LdLdi`).
    pub const SCALBNL: u32 = 1076;
    /// `__builtin_scalbln` (clang `Builtins.td` signature `ddLi`).
    pub const SCALBLN: u32 = 1068;
    /// `__builtin_scalblnf` (clang `Builtins.td` signature `ffLi`).
    pub const SCALBLNF: u32 = 1066;
    /// `__builtin_scalblnl` (clang `Builtins.td` signature `LdLdLi`).
    pub const SCALBLNL: u32 = 1070;
    /// `__builtin_launder` (clang `Builtins.td` signature `v*v*`).
    pub const LAUNDER: u32 = 756;
    /// `__builtin_operator_new` (clang `Builtins.td` signature `v*z`).
    pub const OPERATOR_NEW: u32 = 224;
    /// `__builtin_operator_delete` (clang `Builtins.td` signature `vv*`).
    pub const OPERATOR_DELETE: u32 = 223;
    /// `__builtin_unreachable` (clang `Builtins.td` signature `v`).
    pub const UNREACHABLE: u32 = 1347;

    /// Attribute bits of an [`Info`] (clang's `Builtins.td` attributes).
    pub mod attr {
        /// `NoThrow`: an implicit `NoThrowAttr` and a `noexcept` type.
        pub const NOTHROW: u8 = 1;
        /// `Const`: an implicit `ConstAttr`.
        pub const CONST: u8 = 2;
        /// `Pure`: an implicit `PureAttr`.
        pub const PURE: u8 = 4;
        /// `NoReturn`: a `noreturn` function type.
        pub const NORETURN: u8 = 8;
        /// `CustomTypeChecking`: the arguments are not converted to the
        /// parameter types; sema checks the call itself.
        pub const CUSTOM: u8 = 16;
    }

    /// One builtin function.
    #[derive(Clone, Copy, Debug)]
    pub struct Info {
        /// clang's `Builtin::ID`.
        pub id: u32,
        /// The spelling.
        pub name: &'static str,
        /// The type in `Builtins.td` notation: the return type, then the
        /// parameter types; `.` makes it variadic. Base types: `v` void,
        /// `b` bool, `c` char, `w` `wchar_t`, `s` short, `i` int, `f`
        /// float, `d` double, `z` `size_t`; prefixes `U` unsigned, `L`
        /// long (`LL` long long, `Ld` long double); suffixes `*` pointer,
        /// `&` lvalue reference, `C` const.
        pub sig: &'static str,
        /// [`attr`] bits.
        pub attrs: u8,
    }

    const fn row(id: u32, name: &'static str, sig: &'static str, attrs: u8) -> Info {
        Info { id, name, sig, attrs }
    }

    use attr::{CONST as C, CUSTOM as T, NORETURN as R, NOTHROW as N, PURE as P};

    /// Every builtin function kiln declares.
    pub const ALL: &[Info] = &[
        row(IS_CONSTANT_EVALUATED, "__builtin_is_constant_evaluated", "b", N),
        row(ADDRESSOF, "__builtin_addressof", "v*v&", N | C | T),
        row(CLZ, "__builtin_clz", "iUi", N | C),
        row(CLZL, "__builtin_clzl", "iULi", N | C),
        row(CLZLL, "__builtin_clzll", "iULLi", N | C),
        row(CTZ, "__builtin_ctz", "iUi", N | C),
        row(CTZL, "__builtin_ctzl", "iULi", N | C),
        row(CTZLL, "__builtin_ctzll", "iULLi", N | C),
        row(POPCOUNT, "__builtin_popcount", "iUi", N | C),
        row(POPCOUNTL, "__builtin_popcountl", "iULi", N | C),
        row(POPCOUNTLL, "__builtin_popcountll", "iULLi", N | C),
        row(CLZG, "__builtin_clzg", "i.", N | C | T),
        row(CTZG, "__builtin_ctzg", "i.", N | C | T),
        row(POPCOUNTG, "__builtin_popcountg", "i.", N | C | T),
        row(BSWAP16, "__builtin_bswap16", "UsUs", N | C),
        row(BSWAP32, "__builtin_bswap32", "UiUi", N | C),
        row(BSWAP64, "__builtin_bswap64", "ULiULi", N | C),
        row(MEMCMP, "__builtin_memcmp", "ivC*vC*z", N),
        row(MEMCPY, "__builtin_memcpy", "v*v*vC*z", N),
        row(MEMMOVE, "__builtin_memmove", "v*v*vC*z", N),
        row(MEMCHR, "__builtin_memchr", "v*vC*iz", N),
        row(CHAR_MEMCHR, "__builtin_char_memchr", "c*cC*iz", N),
        row(STRLEN, "__builtin_strlen", "zcC*", N),
        row(WCSLEN, "__builtin_wcslen", "zwC*", N | P),
        row(WMEMCHR, "__builtin_wmemchr", "w*wC*wz", N),
        row(WMEMCMP, "__builtin_wmemcmp", "iwC*wC*z", N),
        row(ISFINITE, "__builtin_isfinite", "i.", N | C | T),
        row(ISINF, "__builtin_isinf", "i.", N | C | T),
        row(ISNAN, "__builtin_isnan", "i.", N | C | T),
        row(ISNORMAL, "__builtin_isnormal", "i.", N | C | T),
        row(SIGNBIT, "__builtin_signbit", "i.", N | C | T),
        row(ISGREATER, "__builtin_isgreater", "i.", N | C | T),
        row(ISGREATEREQUAL, "__builtin_isgreaterequal", "i.", N | C | T),
        row(ISLESS, "__builtin_isless", "i.", N | C | T),
        row(ISLESSEQUAL, "__builtin_islessequal", "i.", N | C | T),
        row(ISLESSGREATER, "__builtin_islessgreater", "i.", N | C | T),
        row(ISUNORDERED, "__builtin_isunordered", "i.", N | C | T),
        row(HUGE_VAL, "__builtin_huge_val", "d", N | C),
        row(HUGE_VALF, "__builtin_huge_valf", "f", N | C),
        row(HUGE_VALL, "__builtin_huge_vall", "Ld", N | C),
        row(NAN, "__builtin_nan", "dcC*", N | P),
        row(NANF, "__builtin_nanf", "fcC*", N | P),
        row(NANL, "__builtin_nanl", "LdcC*", N | P),
        row(NANS, "__builtin_nans", "dcC*", N | P),
        row(NANSF, "__builtin_nansf", "fcC*", N | P),
        row(NANSL, "__builtin_nansl", "LdcC*", N | P),
        row(EXP, "__builtin_exp", "dd", N),
        row(EXPF, "__builtin_expf", "ff", N),
        row(EXPL, "__builtin_expl", "LdLd", N),
        row(EXP2, "__builtin_exp2", "dd", N),
        row(EXP2F, "__builtin_exp2f", "ff", N),
        row(EXP2L, "__builtin_exp2l", "LdLd", N),
        row(EXPM1, "__builtin_expm1", "dd", N),
        row(EXPM1F, "__builtin_expm1f", "ff", N),
        row(EXPM1L, "__builtin_expm1l", "LdLd", N),
        row(FREXP, "__builtin_frexp", "ddi*", N),
        row(FREXPF, "__builtin_frexpf", "ffi*", N),
        row(FREXPL, "__builtin_frexpl", "LdLdi*", N),
        row(LDEXP, "__builtin_ldexp", "ddi", N),
        row(LDEXPF, "__builtin_ldexpf", "ffi", N),
        row(LDEXPL, "__builtin_ldexpl", "LdLdi", N),
        row(POW, "__builtin_pow", "ddd", N),
        row(POWF, "__builtin_powf", "fff", N),
        row(POWL, "__builtin_powl", "LdLdLd", N),
        row(SCALBN, "__builtin_scalbn", "ddi", N),
        row(SCALBNF, "__builtin_scalbnf", "ffi", N),
        row(SCALBNL, "__builtin_scalbnl", "LdLdi", N),
        row(SCALBLN, "__builtin_scalbln", "ddLi", N),
        row(SCALBLNF, "__builtin_scalblnf", "ffLi", N),
        row(SCALBLNL, "__builtin_scalblnl", "LdLdLi", N),
        row(LAUNDER, "__builtin_launder", "v*v*", N | T),
        row(OPERATOR_NEW, "__builtin_operator_new", "v*z", C | T),
        row(OPERATOR_DELETE, "__builtin_operator_delete", "vv*", N | T),
        row(UNREACHABLE, "__builtin_unreachable", "v", N | R),
    ];

    /// The builtin function spelled `name`, if kiln declares one.
    pub fn by_name(name: &str) -> Option<u32> {
        ALL.iter().find(|e| e.name == name).map(|e| e.id)
    }

    /// The table row of builtin `id`.
    pub fn info(id: u32) -> Option<&'static Info> {
        ALL.iter().find(|e| e.id == id)
    }

    /// Whether builtin `id` has custom type-checking.
    pub fn is_custom(id: u32) -> bool {
        info(id).is_some_and(|i| i.attrs & attr::CUSTOM != 0)
    }
}
