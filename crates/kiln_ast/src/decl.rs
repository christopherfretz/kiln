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
use crate::mem::{HeapSize, MemReport, MemRow, VecDims};
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
    /// One structured binding. The declaration it belongs to is a `Var`
    /// with an empty name (clang's `DecompositionDecl`): see
    /// [`DeclArena::decomp_bindings`] and [`DeclArena::binding_decomp`].
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
    /// A consteval-block-declaration ([dcl.pre], P3289R1): payload
    /// [`StaticAssertData`] whose `cond` is the call of the block's
    /// synthesized `consteval` lambda and whose `message` is `None`
    /// (spec/consteval-blocks.md §2).
    ConstevalBlock,
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
            K::StaticAssert | K::ConstevalBlock => PayloadClass::StaticAssert,
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
                | DeclKind::ConstevalBlock
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
            K::ConstevalBlock => "ConstevalBlock",
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
/// linkage_cache(3) visibility(2) first(1) definition(1)`.
///
/// `first` and `definition` are query caches the arena owns (WP-655):
/// set, the declaration is its entity's first declaration
/// ([`crate::Ast::first`] returns it) or its definition
/// ([`crate::Ast::definition`] returns it). Clear means "ask the
/// redeclaration group". A head's bits are ignored by
/// [`DeclArena::push`], which computes them.
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
    /// The declaration is known to be its entity's first declaration.
    pub const fn is_first(self) -> bool {
        self.field(14, 1) != 0
    }
    /// The declaration is known to be its entity's definition.
    pub const fn is_definition(self) -> bool {
        self.field(15, 1) != 0
    }
    /// Without the arena-owned `first`/`definition` caches.
    const fn without_redecl_caches(self) -> Self {
        DeclMisc(self.0 & !(0b11 << 14))
    }
    /// The caches of a declaration of `kind` without a redeclaration
    /// group: its own definition, and its own first unless a template
    /// (whose first follows its pattern's).
    const fn not_redeclarable(self, kind: DeclKind) -> Self {
        let first = !crate::templates::is_template_kind(kind);
        self.with_field(14, 1, first as u16).with_field(15, 1, 1)
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
/// final(1) explicit_object(1) virtual_written(1) storage_inherited(1)
/// not_selected(1)`.
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
    /// An immediate function ([expr.const]/20): `consteval`, or escalated
    /// (P2564). It is never emitted.
    pub const fn is_immediate(self) -> bool {
        matches!(self.constexpr_kind(), ConstexprKind::Consteval) || self.escalated()
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
    /// [dcl.constexpr]/7; WP-289: `not_selected`, known only at the
    /// class's `}`, [class.dtor]/4; WP-389: `return_deduced`, known only
    /// at the first `return`, [dcl.spec.auto.general]/3). The two fields are OR-ed in, so sema
    /// sets them only from [`Defaulted::No`] / [`ConstexprKind::None`].
    pub const LATE: FnSpec = FnSpec(
        (1 << 3)
            | (1 << 4)
            | (1 << 8)
            | (1 << 10)
            | (1 << 12)
            | (1 << 14)
            | (1 << 15)
            | (1 << 16)
            | (1 << 23)
            | (1 << 24),
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
    /// A prospective destructor that is not the destructor: overload
    /// resolution at the end of the class definition selected another
    /// ([class.dtor]/4); or another special member that is not eligible
    /// ([special]/6: its constraints are not satisfied, or another of
    /// its kind is at least as constrained). clang
    /// `isIneligibleOrNotSelected`. Set at class completion;
    /// [`crate::Ast::destructor`] skips it, and it does not count for the
    /// class's triviality.
    not_selected / with_not_selected = 23;
    /// The return type was deduced from a placeholder (`auto`,
    /// `decltype(auto)`; [dcl.spec.auto]): the declared type's return
    /// type is the deduced one, not the type as written. Set when the
    /// return type is deduced (WP-389; clang looks only at the type as
    /// written for the linkage, `getLVForNamespaceScopeDecl`).
    return_deduced / with_return_deduced = 24;
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
    /// Static TLS: `__thread` / `_Thread_local` (clang `TLS_Static`),
    /// whose initialization is constant ([basic.start.static]/2) and
    /// whose type is trivially destructible; accessed directly, with no
    /// `_ZTW` wrapper.
    Static = 1,
    /// `thread_local` (clang `TLS_Dynamic`): may be dynamically
    /// initialized and destroyed ([basic.stc.thread]).
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
    /// initialize an aggregate), and `inline` and any init style (an
    /// instantiated static data member becomes inline with its deferred
    /// initializer and takes its form, [temp.inst]/4),
    /// and `cxx_for_range` (a for-range-declaration is known as one after
    /// it is declared, clang `ActOnCXXForRangeDecl`).
    pub const LATE: VarSpec = VarSpec((1 << 12) | (1 << 10) | (1 << 9) | (1 << 6) | (1 << 5));
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
    /// Trivial default constructor: the class has a default constructor
    /// and an eligible one ([special]/6) is trivial, or the implicit one
    /// would be (clang `hasTrivialDefaultConstructor`). The `TRIVIAL_*`
    /// bits are clang's `HasTrivialSpecialMembers`: they do not say that
    /// every eligible member of the kind is trivial; that also needs the
    /// matching `NONTRIVIAL_*` bit clear (`Sema::all_trivial`).
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
    /// A closure type whose lambda has a *lambda-capture*
    /// ([expr.prim.lambda.closure]/13: no default constructor, a deleted
    /// copy assignment operator and no implicit move assignment; clang
    /// `!lambdaIsDefaultConstructibleAndAssignable`).
    pub const LAMBDA_CAPTURES: DefinitionBits = DefinitionBits(1 << 51);
    /// The class ends with a flexible array member, or with a member of
    /// a class that does (a GNU/C99 extension; clang
    /// `RecordDecl::hasFlexibleArrayMember`).
    pub const HAS_FLEXIBLE_ARRAY_MEMBER: DefinitionBits = DefinitionBits(1 << 52);
    /// `[[clang::trivial_abi]]` is in effect: the class has the attribute
    /// and clang's `checkIllFormedTrivialABIStruct` kept it. Its copy and
    /// move constructors and destructor are trivial for the purposes of
    /// calls (spec/abi.md §8.25; WP-460).
    pub const TRIVIAL_ABI: DefinitionBits = DefinitionBits(1 << 53);
    /// The copy constructor is trivial for the purposes of calls although
    /// not trivial (clang `hasTrivialCopyConstructorForCall` and not
    /// `hasTrivialCopyConstructor`): the class or a subobject has
    /// `TRIVIAL_ABI`. Trivial-for-call is this bit or `TRIVIAL_COPY_CTOR`.
    pub const COPY_CTOR_FOR_CALL: DefinitionBits = DefinitionBits(1 << 54);
    /// The move-constructor analogue of `COPY_CTOR_FOR_CALL`.
    pub const MOVE_CTOR_FOR_CALL: DefinitionBits = DefinitionBits(1 << 55);
    /// The destructor analogue of `COPY_CTOR_FOR_CALL`.
    pub const DTOR_FOR_CALL: DefinitionBits = DefinitionBits(1 << 56);
    /// The class has a non-trivial default constructor: an eligible
    /// declared one is non-trivial, or the implicit one still to be
    /// declared would be (clang `hasNonTrivialDefaultConstructor`). With
    /// `TRIVIAL_DEFAULT_CTOR` (clang `hasTrivialDefaultConstructor`: some
    /// eligible one is trivial) both can hold, as for
    /// `struct N { N(...) = delete; N() = default; };` ([special]/6;
    /// spec/sema-class.md §4.7.2, WP-1109).
    pub const NONTRIVIAL_DEFAULT_CTOR: DefinitionBits = DefinitionBits(1 << 57);
    /// clang `hasNonTrivialCopyConstructor`; see `NONTRIVIAL_DEFAULT_CTOR`.
    pub const NONTRIVIAL_COPY_CTOR: DefinitionBits = DefinitionBits(1 << 58);
    /// clang `hasNonTrivialMoveConstructor`; see `NONTRIVIAL_DEFAULT_CTOR`.
    pub const NONTRIVIAL_MOVE_CTOR: DefinitionBits = DefinitionBits(1 << 59);
    /// clang `hasNonTrivialCopyAssignment`; see `NONTRIVIAL_DEFAULT_CTOR`.
    pub const NONTRIVIAL_COPY_ASSIGN: DefinitionBits = DefinitionBits(1 << 60);
    /// clang `hasNonTrivialMoveAssignment`; see `NONTRIVIAL_DEFAULT_CTOR`.
    pub const NONTRIVIAL_MOVE_ASSIGN: DefinitionBits = DefinitionBits(1 << 61);

    /// No bits.
    pub const fn empty() -> Self {
        DefinitionBits(0)
    }
    /// The raw bits.
    pub const fn bits(self) -> u64 {
        self.0
    }
    /// The bits from [`DefinitionBits::bits`] (a module's declaration
    /// image carries them, spec/modules.md §11.11 "Classes").
    pub const fn from_bits(bits: u64) -> Self {
        DefinitionBits(bits)
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
    /// [`FieldData::MUTABLE`], [`FieldData::NO_UNIQUE_ADDRESS`], the
    /// in-class-initializer style in bits 2..=3 ([`InitStyle`]), and
    /// [`FieldData::HAS_DEFAULT_INIT`].
    pub flags: u32,
}

impl FieldData {
    /// `mutable`.
    pub const MUTABLE: u32 = 1;
    /// `[[no_unique_address]]`.
    pub const NO_UNIQUE_ADDRESS: u32 = 2;
    /// A default member initializer was written (known as the member is
    /// declared; the initializer itself is parsed after the class is
    /// complete).
    pub const HAS_DEFAULT_INIT: u32 = 16;

    /// `mutable`.
    pub const fn is_mutable(&self) -> bool {
        self.flags & Self::MUTABLE != 0
    }
    /// `[[no_unique_address]]`.
    pub const fn no_unique_address(&self) -> bool {
        self.flags & Self::NO_UNIQUE_ADDRESS != 0
    }
    /// The default member initializer's syntax: `= ...` ([`InitStyle::C`])
    /// or `{...}` ([`InitStyle::List`]); `C` when there is none.
    pub const fn init_style(&self) -> InitStyle {
        match (self.flags >> 2) & 3 {
            1 => InitStyle::Call,
            2 => InitStyle::List,
            3 => InitStyle::ParenList,
            _ => InitStyle::C,
        }
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
    /// [`ParamData::DEFAULT_ARG_PENDING`]; the high byte holds the
    /// function-scope depth ([`ParamData::scope_depth`]).
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
    /// The bits of `flags` holding the function-scope depth.
    pub const SCOPE_DEPTH_MASK: u16 = 0xff00;

    /// The `flags` bits recording function-scope depth `depth` (clamped
    /// to 255).
    pub fn scope_depth_flags(depth: usize) -> u16 {
        u16::try_from(depth.min(0xff)).unwrap_or(0xff) << 8
    }

    /// The function-scope depth: how many function prototype scopes
    /// enclose the parameter's own ([basic.scope.param]; clang's
    /// `getFunctionScopeDepth`). 0 for a parameter of a function declared
    /// outside any parameter-declaration-clause, 1 for one of a function
    /// type in such a function's parameter (`void f(int (*)(int a))`).
    /// The Itanium `fL<L>p` mangling of a parameter reference counts the
    /// prototype scopes between the reference and the parameter's.
    pub fn scope_depth(&self) -> u32 {
        u32::from(self.flags >> 8)
    }
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
    /// `LinkageSpec`: the [`LinkageLang`] in the low byte, plus
    /// [`ScopeData::LINKAGE_BRACELESS`]; else 0.
    pub aux: u32,
}

impl ScopeData {
    /// `aux` bit of a `LinkageSpec` written without braces
    /// (`extern "C" int x;`); its declaration is treated as if it had
    /// `extern` when deciding whether it is a definition ([dcl.link]/8,
    /// clang `isSingleLineLanguageLinkage`).
    pub const LINKAGE_BRACELESS: u32 = 1 << 8;

    /// The language of a `LinkageSpec`'s payload.
    pub fn linkage_lang(&self) -> Option<LinkageLang> {
        match self.aux & 0xff {
            1 => Some(LinkageLang::C),
            2 => Some(LinkageLang::Cxx),
            _ => None,
        }
    }
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
    /// Virtual destructor whose `dtor_delete` is a destroying `operator
    /// delete` → the `this` argument converted to its first parameter's
    /// type `C*` (clang `CXXDestructorDecl::getOperatorDeleteThisArg`,
    /// [expr.delete]/7.1; WP-457).
    pub dtor_delete_this: ColdMap<DeclId, ExprId>,
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
    /// A bodiless member function declaration → the start of the last
    /// token of its declarator, asm label and GNU attributes (clang's
    /// `FunctionDecl::getEndLoc` before a body is attached), where
    /// "must be declared with 'noexcept'" points; and a block-scope
    /// variable whose declarator ends after its name → the start of that
    /// last token (clang `VarDecl::getEndLoc` without an initializer).
    pub declarator_end: ColdMap<DeclId, Loc>,
    /// Builtin function ids of implicit builtin `Function` decls (§5.3).
    pub builtin_id: ColdMap<DeclId, u32>,
    /// WP-380: the `__builtin_` twin of a C library function GCC folds
    /// as an implicit builtin (`ceil` -> `__builtin_ceil`), recorded
    /// under `-stdlib=libstdc++` only (spec/sema.md §5.4).
    pub gnu_lib_twin: ColdMap<DeclId, u32>,
    /// WP-419: the `__builtin_` twin of a declaration clang recognizes
    /// as a C library builtin (`ceil` -> `__builtin_ceil`), recorded on
    /// every declaration of the chain (spec/sema.md §5.4).
    pub lib_builtin: ColdMap<DeclId, u32>,
    /// The typedef that names an unnamed class or enumeration for linkage
    /// purposes ([dcl.typedef]/9, clang's `TypedefNameForAnonDecl`): the
    /// tag's type prints with the typedef's name.
    pub typedef_for_anon: ColdMap<DeclId, DeclId>,
    /// Structured bindings ([dcl.struct.bind], WP-122): a `Var` that is a
    /// decomposition (clang's `DecompositionDecl`, the hidden variable
    /// `e`) → its `Binding`s (`ListId` of `DeclId`s, write-once).
    pub decomp_bindings: ColdMap<DeclId, ListId>,
    /// `Binding` → the decomposition variable it belongs to.
    pub binding_decomp: ColdMap<DeclId, DeclId>,
    /// `Binding` → its binding expression (an lvalue naming the element,
    /// member or holding variable; write-once).
    pub binding_expr: ColdMap<DeclId, ExprId>,
    /// `Binding` of a tuple-like type → its implicit holding variable
    /// ([dcl.struct.bind]/4; write-once).
    pub binding_var: ColdMap<DeclId, DeclId>,
    /// `ConstructorUsingShadow` → the shadow in the nominated base class
    /// it was made from, when the base inherited the constructor itself
    /// (clang `getNominatedBaseClassShadowDecl`; WP-114).
    pub ctor_shadow_nominated: ColdMap<DeclId, DeclId>,
    /// Inheriting constructor (the implicit `CXXConstructor` an inherited
    /// constructor materializes, [class.inhctor.init]) → the
    /// `ConstructorUsingShadow` it was made for (WP-114).
    pub inheriting_ctor: ColdMap<DeclId, DeclId>,
    /// Inheriting constructor → the base constructor it stands for (the
    /// shadow's target or a specialization of it; clang
    /// `getInheritedConstructor().getConstructor()`; WP-146).
    pub inherited_ctor: ColdMap<DeclId, DeclId>,
    /// Using-shadow in a class → the member of the class that hides it
    /// ([namespace.udecl]/14; recorded at the class's `}`).
    pub hidden_shadow: ColdMap<DeclId, DeclId>,
    /// Conversion function template specialization → the target type its
    /// name prints with (the pattern's, substituted before any return
    /// type deduction; clang's instantiated declaration name). The
    /// declaration keeps the pattern's name for the mangling (WP-170).
    pub conversion_spec_target: ColdMap<DeclId, QualType>,
    /// The implicit internal variable of a namespace-scope array compound
    /// literal → that `CompoundLiteralExpr` (clang's `.compoundliteral`
    /// global; `AstBuilder::file_scope_compound_literal`, WP-466).
    pub compound_literal: ColdMap<DeclId, ExprId>,
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
            dtor_delete_this: ColdMap::new("dtor_delete_this"),
            record_pack: ColdMap::new("record_pack"),
            requires_clause: ColdMap::new("requires_clause"),
            contracts: ColdMap::new("contracts"),
            asm_label: ColdMap::new("asm_label"),
            alignment: ColdMap::new("alignment"),
            deprecation: ColdMap::new("deprecation"),
            instantiated_from: ColdMap::new("instantiated_from"),
            injection_point: ColdMap::new("injection_point"),
            declarator_end: ColdMap::new("declarator_end"),
            builtin_id: ColdMap::new("builtin_id"),
            gnu_lib_twin: ColdMap::new("gnu_lib_twin"),
            lib_builtin: ColdMap::new("lib_builtin"),
            typedef_for_anon: ColdMap::new("typedef_for_anon"),
            decomp_bindings: ColdMap::new("decomp_bindings"),
            binding_decomp: ColdMap::new("binding_decomp"),
            binding_expr: ColdMap::new("binding_expr"),
            binding_var: ColdMap::new("binding_var"),
            ctor_shadow_nominated: ColdMap::new("ctor_shadow_nominated"),
            inheriting_ctor: ColdMap::new("inheriting_ctor"),
            inherited_ctor: ColdMap::new("inherited_ctor"),
            hidden_shadow: ColdMap::new("hidden_shadow"),
            conversion_spec_target: ColdMap::new("conversion_spec_target"),
            compound_literal: ColdMap::new("compound_literal"),
        }
    }
}

impl ColdDeclData {
    fn rows(&self) -> [MemRow; 33] {
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
            self.dtor_delete_this.mem_row(),
            self.record_pack.mem_row(),
            self.requires_clause.mem_row(),
            self.contracts.mem_row(),
            self.asm_label.mem_row(),
            self.alignment.mem_row(),
            self.deprecation.mem_row(),
            self.instantiated_from.mem_row(),
            self.injection_point.mem_row(),
            self.declarator_end.mem_row(),
            self.builtin_id.mem_row(),
            self.gnu_lib_twin.mem_row(),
            self.lib_builtin.mem_row(),
            self.typedef_for_anon.mem_row(),
            self.decomp_bindings.mem_row(),
            self.binding_decomp.mem_row(),
            self.binding_expr.mem_row(),
            self.binding_var.mem_row(),
            self.ctor_shadow_nominated.mem_row(),
            self.inheriting_ctor.mem_row(),
            self.inherited_ctor.mem_row(),
            self.hidden_shadow.mem_row(),
            self.conversion_spec_target.mem_row(),
            self.compound_literal.mem_row(),
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
    /// Per function payload: its minimum argument count
    /// ([`DeclArena::min_args`]; WP-663), or [`MIN_ARGS_UNKNOWN`].
    min_args: Vec<u8>,
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
    /// Class definitions in the order their `}` was reached
    /// ([`DeclArena::complete_definition`]; instantiations included).
    completed_records: Vec<DeclId>,
    /// The end of each top-level declaration of the translation unit, in
    /// order: the last member of the translation unit's member list and
    /// the class-completion epoch there ([`DeclArena::end_top_level_decl`]).
    top_level_ends: Vec<(Option<DeclId>, u32)>,
    /// Function definitions instantiated, in the order they completed,
    /// each with the number of top-level ends recorded by then
    /// ([`DeclArena::note_fn_def_inst`]).
    fn_def_insts: Vec<(DeclId, u32)>,
    /// Cold per-declaration data.
    pub cold: ColdDeclData,
    /// Every declaration [`DeclArena::set_ty`] retypes (WP-658; see
    /// [`crate::journal::Journal`]).
    pub ty_journal: crate::journal::Journal<DeclId>,
}

/// [`DeclArena::min_args`]'s "not recorded" byte: more than 254 counted
/// parameters, or a default argument added after the parameter list was
/// set and not yet refreshed ([`DeclArena::refresh_min_args`]).
const MIN_ARGS_UNKNOWN: u8 = u8::MAX;

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
            min_args: vec![0],
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
            completed_records: Vec::new(),
            top_level_ends: Vec::new(),
            fn_def_insts: Vec::new(),
            cold: ColdDeclData::default(),
            ty_journal: crate::journal::Journal::default(),
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
            // Invariant: Sema only mutates a payload of the decl's own class
            // (two Sema bugs that broke this were fixed in WP-141).
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
            Payload::Function(x) => {
                // No parameters yet (they name the function as their
                // context, so they are set after it).
                self.min_args.push(if x.params == ListId::EMPTY { 0 } else { MIN_ARGS_UNKNOWN });
                push_payload(&mut self.functions, x)
            }
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
        let mut misc = h.misc.without_redecl_caches();
        if data != 0 {
            let link = match h.kind.payload() {
                PayloadClass::Function => Some(self.functions[data as usize].redecl),
                PayloadClass::Var => Some(self.vars[data as usize].redecl),
                PayloadClass::Record => Some(self.records[data as usize].redecl),
                PayloadClass::Enum => Some(self.enums[data as usize].redecl),
                PayloadClass::Typedef => Some(self.typedefs[data as usize].redecl),
                PayloadClass::Namespace => Some(self.namespaces[data as usize].redecl),
                _ => None,
            }
            .filter(|r| r.group != DUMMY_GROUP);
            if let Some(r) = link {
                // A fresh group's first declaration (`new_group(id)`).
                misc = misc.with_field(14, 1, u16::from(r.prev.is_none()));
            } else {
                misc = misc.not_redeclarable(h.kind);
            }
        } else {
            misc = misc.not_redeclarable(h.kind);
        }
        self.nodes.push(DeclNode {
            kind: h.kind,
            flags: h.flags,
            misc,
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

    /// Marks `d` as its entity's first declaration (a template decl whose
    /// pattern is first, set by the builder, WP-655).
    pub(crate) fn mark_first(&mut self, d: DeclId) {
        let n = &mut self.nodes[d.index() as usize];
        n.misc = n.misc.with_field(14, 1, 1);
    }

    /// Marks `d` as its entity's definition (set by
    /// [`crate::Ast::set_definition`], WP-655).
    pub(crate) fn mark_definition(&mut self, d: DeclId) {
        let n = &mut self.nodes[d.index() as usize];
        n.misc = n.misc.with_field(15, 1, 1);
    }

    /// Clears `d`'s definition mark ([`crate::Ast::replace_definition`]).
    pub(crate) fn unmark_definition(&mut self, d: DeclId) {
        let n = &mut self.nodes[d.index() as usize];
        n.misc = n.misc.with_field(15, 1, 0);
    }

    /// The builtin id of `d` if it is an implicit builtin function
    /// declaration ([`builtin`], §5.3).
    pub fn builtin_id(&self, d: DeclId) -> Option<u32> {
        self.cold.builtin_id.get(d).copied()
    }

    /// The `__builtin_` twin the Oven folds a call of `d` as, if `d` is
    /// a C library function GCC treats as an implicit builtin (WP-380,
    /// spec/sema.md §5.4; recorded under `-stdlib=libstdc++` only).
    pub fn gnu_lib_twin(&self, d: DeclId) -> Option<u32> {
        self.cold.gnu_lib_twin.get(d).copied()
    }

    /// The `__builtin_` twin of `d` if `d` declares a C library function
    /// clang recognizes as a library builtin (WP-419, spec/sema.md §5.4):
    /// codegen lowers a call as clang's `EmitBuiltinExpr` lowers the
    /// library builtin (`ceil(x)` -> `llvm.ceil`).
    pub fn lib_builtin(&self, d: DeclId) -> Option<u32> {
        self.cold.lib_builtin.get(d).copied()
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

    /// Marks the linkage specification `ls` as written without braces.
    pub fn set_linkage_braceless(&mut self, ls: DeclId) {
        self.scope_mut(ls).aux |= ScopeData::LINKAGE_BRACELESS;
    }

    /// True if `d` is directly contained in a linkage specification
    /// written without braces (`extern "C" int x;`, [dcl.link]/8).
    pub fn is_single_line_linkage(&self, d: DeclId) -> bool {
        self.lexical_ctx(d).is_some_and(|c| {
            self.get(c).kind() == DeclKind::LinkageSpec
                && self.scope(c).is_some_and(|s| s.aux & ScopeData::LINKAGE_BRACELESS != 0)
        })
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

    /// The bindings of decomposition variable `d` ([dcl.struct.bind]),
    /// `None` when `d` is not a decomposition.
    pub fn decomp_bindings(&self, d: DeclId) -> Option<ListId> {
        self.cold.decomp_bindings.get(d).copied()
    }

    /// True if `d` is a decomposition variable (clang's
    /// `DecompositionDecl`).
    pub fn is_decomposition(&self, d: DeclId) -> bool {
        self.cold.decomp_bindings.contains(d)
    }

    /// The decomposition variable binding `b` belongs to.
    pub fn binding_decomp(&self, b: DeclId) -> Option<DeclId> {
        self.cold.binding_decomp.get(b).copied()
    }

    /// The binding expression of `Binding` `b`, once known.
    pub fn binding_expr(&self, b: DeclId) -> Option<ExprId> {
        self.cold.binding_expr.get(b).copied()
    }

    /// The holding variable of tuple-like `Binding` `b`.
    pub fn binding_var(&self, b: DeclId) -> Option<DeclId> {
        self.cold.binding_var.get(b).copied()
    }

    /// The base class's `ConstructorUsingShadow` that shadow `s` was made
    /// from (the nominated base inherited the constructor too).
    pub fn ctor_shadow_nominated(&self, s: DeclId) -> Option<DeclId> {
        self.cold.ctor_shadow_nominated.get(s).copied()
    }

    /// The `ConstructorUsingShadow` inheriting constructor `c` was made
    /// for, if `c` is one ([class.inhctor.init]).
    pub fn inheriting_ctor_shadow(&self, c: DeclId) -> Option<DeclId> {
        self.cold.inheriting_ctor.get(c).copied()
    }

    /// Records `c` as the inheriting constructor of shadow `s`.
    ///
    /// # Errors
    /// If `c` is already recorded.
    pub fn set_inheriting_ctor(&mut self, c: DeclId, s: DeclId) -> Result<(), AlreadySet> {
        self.cold.inheriting_ctor.insert(c, s)
    }

    /// The base constructor inheriting constructor `c` stands for, if
    /// `c` is one ([class.inhctor.init]).
    pub fn inherited_ctor(&self, c: DeclId) -> Option<DeclId> {
        self.cold.inherited_ctor.get(c).copied()
    }

    /// Records `b` as the base constructor inheriting constructor `c`
    /// stands for.
    ///
    /// # Errors
    /// If `c` is already recorded.
    pub fn set_inherited_ctor(&mut self, c: DeclId, b: DeclId) -> Result<(), AlreadySet> {
        self.cold.inherited_ctor.insert(c, b)
    }

    /// The namespace-scope array `CompoundLiteralExpr` whose object is
    /// implicit variable `d`, if `d` is one (WP-466).
    pub fn compound_literal_of(&self, d: DeclId) -> Option<ExprId> {
        self.cold.compound_literal.get(d).copied()
    }

    /// The class member hiding using-shadow `s`, if any.
    pub fn hidden_shadow(&self, s: DeclId) -> Option<DeclId> {
        self.cold.hidden_shadow.get(s).copied()
    }

    /// Records that member `by` hides using-shadow `s`.
    ///
    /// # Errors
    /// If `s` is already recorded hidden.
    pub fn set_hidden_shadow(&mut self, s: DeclId, by: DeclId) -> Result<(), AlreadySet> {
        self.cold.hidden_shadow.insert(s, by)
    }

    /// Records decomposition `v`'s bindings and links each back to `v`.
    /// Write-once.
    ///
    /// # Errors
    /// If `v` already has bindings.
    pub fn set_decomp_bindings(
        &mut self,
        v: DeclId,
        list: ListId,
        bindings: &[DeclId],
    ) -> Result<(), AlreadySet> {
        self.cold.decomp_bindings.insert(v, list)?;
        for &b in bindings {
            self.cold.binding_decomp.insert(b, v)?;
        }
        Ok(())
    }

    /// Replaces decomposition `v`'s binding list with `list` once its
    /// structured binding pack has been expanded ([dcl.struct.bind]/1,
    /// P1061): `elems` are the bindings the pack introduced, linked back
    /// to `v` here; the others keep their link.
    ///
    /// # Errors
    /// If `v` has no bindings yet, or an element is already linked.
    pub fn expand_decomp_bindings(
        &mut self,
        v: DeclId,
        list: ListId,
        elems: &[DeclId],
    ) -> Result<(), AlreadySet> {
        if !self.cold.decomp_bindings.refine(v, list) {
            return Err(AlreadySet { slot: "decomp_bindings", existing: 0 });
        }
        for &b in elems {
            self.cold.binding_decomp.insert(b, v)?;
        }
        Ok(())
    }

    /// Records binding `b`'s expression and, for a tuple-like type, its
    /// holding variable. Write-once.
    ///
    /// # Errors
    /// If already set.
    pub fn set_binding(
        &mut self,
        b: DeclId,
        expr: ExprId,
        holding: Option<DeclId>,
    ) -> Result<(), AlreadySet> {
        self.cold.binding_expr.insert(b, expr)?;
        if let Some(h) = holding {
            self.cold.binding_var.insert(b, h)?;
        }
        Ok(())
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

    /// Adds `DEPENDENT_CTX` to parameter `d`, declared before the first
    /// placeholder parameter made its function an abbreviated function
    /// template ([dcl.fct]/22): `void f(int c, auto x)` declares `c`
    /// while no template head is open yet, but `c` belongs to the
    /// template and is instantiated with it ([temp.inst]).
    ///
    /// # Panics
    /// If `d` is not a `ParmVar`.
    pub fn mark_dependent_param(&mut self, d: DeclId) {
        let n = &mut self.nodes[d.index() as usize];
        assert!(n.kind == DeclKind::ParmVar, "not a parameter: {:?}", n.kind);
        n.flags = n.flags.with(DeclFlags::DEPENDENT_CTX);
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

    /// Moves init-capture variable `v` into call operator `func`
    /// (`Builder::init_capture_var`), with `DEPENDENT_CTX` when `dependent`
    /// (the operator is a template pattern).
    ///
    /// # Panics
    /// If `v` is not a `Var`.
    pub fn set_init_capture_owner(&mut self, v: DeclId, func: DeclId, dependent: bool) {
        let n = &mut self.nodes[v.index() as usize];
        assert!(n.kind == DeclKind::Var, "set_init_capture_owner on a {:?}", n.kind);
        n.ctx = Some(func);
        if dependent {
            n.flags = n.flags.with(DeclFlags::DEPENDENT_CTX);
        }
    }

    /// Names `d` without making the name visible to lookup (a
    /// structured binding's holding variable, `Builder::holding_var`).
    pub fn set_name(&mut self, d: DeclId, name: DeclName) {
        self.nodes[d.index() as usize].name = name;
    }

    /// Re-points the declared type (deduced return types, resolved
    /// exception specifications; §2.4).
    pub fn set_ty(&mut self, d: DeclId, ty: QualType) {
        self.ty_journal.push(d);
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
    /// because they name it as their semantic context). `ids` are the
    /// list's parameters, from which the minimum argument count is
    /// recorded ([`DeclArena::min_args`]).
    pub fn set_params(
        &mut self,
        f: DeclId,
        params: ListId,
        ids: &[DeclId],
    ) -> Result<(), AlreadySet> {
        let data = self.function_mut(f);
        if data.params != ListId::EMPTY {
            return Err(AlreadySet { slot: "FunctionData.params", existing: data.params.raw() });
        }
        data.params = params;
        self.refresh_min_args(f, ids.iter().copied());
        Ok(())
    }

    /// The minimum argument count of function `f` ([dcl.fct.default]):
    /// its parameters up to the last without a default argument (or one
    /// pending late parsing), a parameter pack not counted wherever it
    /// is (clang `getMinRequiredArguments`). `None` when not recorded:
    /// not a function, more than 254 counted parameters, or a default
    /// argument added since the list was set and not yet refreshed
    /// ([`DeclArena::refresh_min_args`]); the caller counts then.
    #[inline]
    pub fn min_args(&self, f: DeclId) -> Option<usize> {
        let n = self.get(f);
        if n.kind.payload() != PayloadClass::Function {
            return None;
        }
        let m = self.min_args[n.data as usize];
        (m != MIN_ARGS_UNKNOWN).then_some(usize::from(m))
    }

    /// Records the minimum argument count of function `f` from its
    /// parameters `ids` ([`DeclArena::min_args`]).
    pub fn refresh_min_args(&mut self, f: DeclId, ids: impl Iterator<Item = DeclId>) {
        let n = *self.get(f);
        if n.kind.payload() != PayloadClass::Function {
            return;
        }
        let mut min = 0usize;
        let mut so_far = 0usize;
        for p in ids {
            let Some(x) = self.param(p) else { continue };
            if x.flags & ParamData::PACK != 0 {
                continue;
            }
            so_far += 1;
            if x.default_arg.is_none() && x.flags & ParamData::DEFAULT_ARG_PENDING == 0 {
                min = so_far;
            }
        }
        self.min_args[n.data as usize] =
            u8::try_from(min).ok().filter(|&m| m != MIN_ARGS_UNKNOWN).unwrap_or(MIN_ARGS_UNKNOWN);
    }

    /// Forgets the recorded minimum argument count of the function owning
    /// parameter `p` when `p` gains a default argument after the list was
    /// set (it is recounted until [`DeclArena::refresh_min_args`]).
    fn param_gains_default(&mut self, p: DeclId) {
        let Some(f) = self.get(p).ctx else { return };
        let n = *self.get(f);
        if n.kind.payload() == PayloadClass::Function
            && self.functions[n.data as usize].params != ListId::EMPTY
        {
            self.min_args[n.data as usize] = MIN_ARGS_UNKNOWN;
        }
    }

    /// Whether parameter `p` has a default argument (possibly pending).
    fn param_has_default(&self, p: DeclId) -> bool {
        self.param(p).is_some_and(|x| {
            x.default_arg.is_some() || x.flags & ParamData::DEFAULT_ARG_PENDING != 0
        })
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

    /// The class-key of class `r` becomes `tag`, the key of the class-head
    /// that defines it ([class.access]/3, [class.access.base]/2 follow the
    /// class-key of the definition): an implicit specialization takes its
    /// pattern's definition's key as that definition is instantiated (a
    /// template first declared `class` and defined `struct`, libstdc++ 16
    /// `__format::_Scanner`), and an explicit or partial specialization
    /// takes its own class-head's key, not the primary template's (WP-611).
    ///
    /// # Panics
    /// If `r` is complete, or `tag` changes a union to a class or back.
    pub fn set_definition_tag(&mut self, r: DeclId, tag: TagKind) {
        let d = self.record_mut(r);
        assert!(!d.defn.contains(DefinitionBits::COMPLETE), "class key of a complete class");
        assert_eq!(d.tag == TagKind::Union, tag == TagKind::Union, "union class-key mismatch");
        d.tag = tag;
    }

    /// Marks class `r` (being defined) as the closure type of lambda `l`
    /// ([expr.prim.lambda.closure]).
    pub fn set_closure(&mut self, r: DeclId, l: LambdaId) {
        let d = self.record_mut(r);
        d.lambda = Some(l);
        d.defn = d.defn.with(DefinitionBits::LAMBDA);
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
        self.completed_records.push(r);
    }

    /// The class definitions completed so far, in completion order (clang
    /// completes, and `-fdump-record-layouts` dumps, an implicit
    /// instantiation where it is first required, before the class whose
    /// member required it).
    pub fn completed_records(&self) -> &[DeclId] {
        &self.completed_records
    }

    /// The class-completion epoch: moves exactly when a class definition
    /// completes ([`DeclArena::complete_definition`], the only point a
    /// class becomes complete), so a cache of "still incomplete" facts is
    /// stale only when it differs from the epoch it was taken at.
    pub fn completion_epoch(&self) -> u32 {
        u32::try_from(self.completed_records.len()).unwrap_or(u32::MAX)
    }

    /// The parser finished a top-level declaration of translation unit
    /// `tu` (a namespace or linkage-specification block is one): clang
    /// hands it to code generation here (`HandleTopLevelDecl`), so the
    /// classes completed so far are the complete ones there.
    pub fn end_top_level_decl(&mut self, tu: DeclId) {
        let last = self.member_list(tu).and_then(|m| m.last);
        let e = self.completion_epoch();
        self.top_level_ends.push((last, e));
    }

    /// The ends of the top-level declarations, in order: the translation
    /// unit's last member and the class-completion epoch at each
    /// ([`DeclArena::end_top_level_decl`]). A member of the translation
    /// unit belongs to the first end at or after it in the member list;
    /// one after the last end was added at the end of the translation
    /// unit.
    pub fn top_level_ends(&self) -> &[(Option<DeclId>, u32)] {
        &self.top_level_ends
    }

    /// Sema finished instantiating the definition of function `f`: clang
    /// hands it to code generation there (`HandleTopLevelDecl`), before
    /// the top-level declaration being parsed, whose index is recorded
    /// (`top_level_ends().len()`; the count of all ends for one done at
    /// the end of the translation unit).
    pub fn note_fn_def_inst(&mut self, f: DeclId) {
        let k = u32::try_from(self.top_level_ends.len()).unwrap_or(u32::MAX);
        self.fn_def_insts.push((f, k));
    }

    /// The instantiated function definitions in completion order, each
    /// with the index of the top-level declaration it precedes
    /// ([`DeclArena::note_fn_def_inst`]).
    pub fn fn_def_insts(&self) -> &[(DeclId, u32)] {
        &self.fn_def_insts
    }

    /// Sets a field's default member initializer (write-once).
    pub fn set_field_init(&mut self, f: DeclId, init: ExprId) -> Result<(), AlreadySet> {
        set_once(&mut self.field_mut(f).init, init, "FieldData.init", RawId::raw)
    }

    /// Sets a bit-field's width expression (write-once; a module's
    /// declaration image builds it after the field, spec/modules.md
    /// §11.11 "Classes").
    pub fn set_field_bit_width(&mut self, f: DeclId, width: ExprId) -> Result<(), AlreadySet> {
        set_once(&mut self.field_mut(f).bit_width, width, "FieldData.bit_width", RawId::raw)
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

    /// Replaces the underlying type of a fixed enumeration: a GNU `mode`
    /// attribute on its definition (its promotion follows `int_type`).
    pub fn replace_fixed_enum_int_type(&mut self, e: DeclId, int_type: QualType) {
        self.enumeration_mut(e).int_type = Some(int_type);
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
        if !self.param_has_default(p) {
            self.param_gains_default(p);
        }
        set_once(&mut self.param_mut(p).default_arg, arg, "ParamData.default_arg", RawId::raw)
    }

    /// Takes back the default argument of a parameter of a class template
    /// member specialization whose instantiation at a use failed
    /// ([temp.inst]/12): as clang keeps the uninstantiated argument, the
    /// next use instantiates (and reports) it again.
    pub fn reset_failed_default_arg(&mut self, p: DeclId) {
        let d = self.param_mut(p);
        d.default_arg = None;
        d.flags |= ParamData::DEFAULT_ARG_PENDING;
    }

    /// Records a parameter's type before [dcl.fct]/5 adjustment (an array
    /// or function type), when it differs from its declared type.
    pub fn set_param_original_ty(&mut self, p: DeclId, ty: QualType) {
        self.param_mut(p).original_ty = Some(ty);
    }

    /// Marks a parameter's default argument as pending late parsing
    /// ([`ParamData::DEFAULT_ARG_PENDING`]; set before the class is
    /// complete, cleared implicitly by reading `default_arg` once set).
    pub fn mark_default_arg_pending(&mut self, p: DeclId) {
        if !self.param_has_default(p) {
            self.param_gains_default(p);
        }
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

    /// Bytes of the node and payload vectors (instantiation meter). The
    /// `min_args` side vector is a cache over the payloads, not a node,
    /// and stays out of the meter (it is a memory-report row).
    pub(crate) fn live_bytes(&self) -> u64 {
        let all: u64 = self.vec_rows().iter().map(|r| r.live_bytes()).sum();
        all - VecDims::of("min_args", &self.min_args).live_bytes()
    }

    fn vec_rows(&self) -> [VecDims; 19] {
        [
            VecDims::of("nodes", &self.nodes),
            VecDims::of("functions", &self.functions),
            VecDims::of("min_args", &self.min_args),
            VecDims::of("vars", &self.vars),
            VecDims::of("records", &self.records),
            VecDims::of("fields", &self.fields),
            VecDims::of("enums", &self.enums),
            VecDims::of("enumerators", &self.enumerators),
            VecDims::of("params", &self.params),
            VecDims::of("typedefs", &self.typedefs),
            VecDims::of("namespaces", &self.namespaces),
            VecDims::of("scopes", &self.scopes),
            VecDims::of("static_asserts", &self.static_asserts),
            VecDims::of("base_specs", &self.base_specs),
            VecDims::of("lets", &self.lets),
            VecDims::of("ctor_inits", &self.ctor_inits),
            VecDims::of("completed_records", &self.completed_records),
            VecDims::of("top_level_ends", &self.top_level_ends),
            VecDims::of("fn_def_insts", &self.fn_def_insts),
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
            r.push(row.row());
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
    /// `__builtin_complex(re, im)` (custom type-checking: `_Complex T`
    /// from two `T` operands).
    pub const COMPLEX: u32 = 210;
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
    /// `int __builtin_constant_p(...)` (clang `Builtins.td` signature `i.`;
    /// custom type-checking: one argument, decayed): whether the argument
    /// folds to a constant.
    pub const CONSTANT_P: u32 = 211;
    /// `int __builtin_classify_type(...)` (clang `Builtins.td` signature
    /// `i.`; custom type-checking: one argument, not converted or
    /// evaluated): the GCC type class of the argument's type
    /// ([`crate::Ast::gcc_type_class`]; WP-419).
    pub const CLASSIFY_TYPE: u32 = 209;
    /// `__builtin_bswap16` (clang `Builtins.td` signature `UsUs`).
    pub const BSWAP16: u32 = 179;
    /// `__builtin_bswap32` (clang `Builtins.td` signature `UiUi`).
    pub const BSWAP32: u32 = 180;
    /// `__builtin_bswap64` (clang `Builtins.td` signature `ULiULi`).
    pub const BSWAP64: u32 = 181;

    // WP-150: the remaining GCC integer and memory builtins
    // (spec/oven.md "Builtins", [`INT_MEM`]).
    /// `__builtin_ffs` (clang `Builtins.td` signature `ii`).
    pub const FFS: u32 = 539;
    /// `__builtin_ffsl` (clang `Builtins.td` signature `iLi`).
    pub const FFSL: u32 = 540;
    /// `__builtin_ffsll` (clang `Builtins.td` signature `iLLi`).
    pub const FFSLL: u32 = 541;
    /// `__builtin_parity` (clang `Builtins.td` signature `iUi`).
    pub const PARITY: u32 = 958;
    /// `__builtin_parityl` (clang `Builtins.td` signature `iULi`).
    pub const PARITYL: u32 = 959;
    /// `__builtin_parityll` (clang `Builtins.td` signature `iULLi`).
    pub const PARITYLL: u32 = 960;
    /// `__builtin_clrsb` (clang `Builtins.td` signature `ii`).
    pub const CLRSB: u32 = 337;
    /// `__builtin_clrsbl` (clang `Builtins.td` signature `iLi`).
    pub const CLRSBL: u32 = 338;
    /// `__builtin_clrsbll` (clang `Builtins.td` signature `iLLi`).
    pub const CLRSBLL: u32 = 339;
    /// `__builtin_bitreverse8` (clang `Builtins.td` signature `UcUc`).
    pub const BITREVERSE8: u32 = 186;
    /// `__builtin_bitreverse16` (clang `Builtins.td` signature `UsUs`).
    pub const BITREVERSE16: u32 = 187;
    /// `__builtin_bitreverse32` (clang `Builtins.td` signature `UiUi`).
    pub const BITREVERSE32: u32 = 188;
    /// `__builtin_bitreverse64` (clang `Builtins.td` signature `ULiULi`).
    pub const BITREVERSE64: u32 = 189;
    /// `__builtin_rotateleft8` (clang `Builtins.td` signature `UcUcUc`).
    pub const ROTATELEFT8: u32 = 1024;
    /// `__builtin_rotateleft16` (clang `Builtins.td` signature `UsUsUs`).
    pub const ROTATELEFT16: u32 = 1025;
    /// `__builtin_rotateleft32` (clang `Builtins.td` signature `UiUiUi`).
    pub const ROTATELEFT32: u32 = 1026;
    /// `__builtin_rotateleft64` (clang `Builtins.td` signature `ULiULiULi`).
    pub const ROTATELEFT64: u32 = 1027;
    /// `__builtin_rotateright8` (clang `Builtins.td` signature `UcUcUc`).
    pub const ROTATERIGHT8: u32 = 1028;
    /// `__builtin_rotateright16` (clang `Builtins.td` signature `UsUsUs`).
    pub const ROTATERIGHT16: u32 = 1029;
    /// `__builtin_rotateright32` (clang `Builtins.td` signature `UiUiUi`).
    pub const ROTATERIGHT32: u32 = 1030;
    /// `__builtin_rotateright64` (clang `Builtins.td` signature `ULiULiULi`).
    pub const ROTATERIGHT64: u32 = 1031;
    /// `__builtin_bcmp` (clang `Builtins.td` signature `ivC*vC*z`).
    pub const BCMP: u32 = 202;
    /// `__builtin_wmemcpy` (clang `Builtins.td` signature `w*w*wC*z`).
    pub const WMEMCPY: u32 = 1381;
    /// `__builtin_wmemmove` (clang `Builtins.td` signature `w*w*wC*z`).
    pub const WMEMMOVE: u32 = 1383;
    /// `__builtin_memset` (clang `Builtins.td` signature `v*v*iz`).
    pub const MEMSET: u32 = 869;
    /// `__builtin_mempcpy` (clang `Builtins.td` signature `v*v*vC*z`).
    pub const MEMPCPY: u32 = 220;

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
    /// `__builtin_strcmp` (clang `Builtins.td` signature `icC*cC*`).
    pub const STRCMP: u32 = 1140;
    /// `__builtin_strncmp` (clang `Builtins.td` signature `icC*cC*z`).
    pub const STRNCMP: u32 = 1151;
    /// `__builtin_strchr` (clang `Builtins.td` signature `c*cC*i`).
    pub const STRCHR: u32 = 1138;
    /// `__builtin_strrchr` (clang `Builtins.td` signature `c*cC*i`).
    pub const STRRCHR: u32 = 1180;
    /// `__builtin_strpbrk` (clang `Builtins.td` signature `c*cC*cC*`).
    pub const STRPBRK: u32 = 1178;
    /// `__builtin_strstr` (clang `Builtins.td` signature `c*cC*cC*`).
    pub const STRSTR: u32 = 1157;
    /// `__builtin_wcscmp` (clang `Builtins.td` signature `iwC*wC*`).
    pub const WCSCMP: u32 = 1388;
    /// `__builtin_wcsncmp` (clang `Builtins.td` signature `iwC*wC*z`).
    pub const WCSNCMP: u32 = 1392;
    /// `__builtin_wcschr` (clang `Builtins.td` signature `w*wC*w`).
    pub const WCSCHR: u32 = 1386;
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
    /// WP-419: `__builtin_signbitf` (`int(float)`; [`f128_twin`] maps it
    /// to [`SIGNBIT`]).
    pub const SIGNBITF: u32 = 1090;
    /// WP-419: `__builtin_signbitl` (`int(long double)`).
    pub const SIGNBITL: u32 = 1091;
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
    /// `void __builtin_va_start(__builtin_va_list &, ...) noexcept`
    /// ([cstdarg.syn], C 7.16.1.4; clang `Builtins.td` signature `vA.`,
    /// custom type-checking: sema checks the enclosing function and the
    /// last named parameter).
    pub const VA_START: u32 = 233;
    /// `void __builtin_va_end(__builtin_va_list &) noexcept` (signature `vA`).
    pub const VA_END: u32 = 1367;
    /// `void __builtin_va_copy(__builtin_va_list &, __builtin_va_list &)
    /// noexcept` (signature `vAA`).
    pub const VA_COPY: u32 = 1365;

    // WP-171: the coroutine builtins ([`CORO`]; clang `CoroLangBuiltin`s).
    /// `void __builtin_coro_resume(void *)` (signature `vv*`).
    pub const CORO_RESUME: u32 = 373;
    /// `void __builtin_coro_destroy(void *)` (signature `vv*`).
    pub const CORO_DESTROY: u32 = 365;
    /// `bool __builtin_coro_done(void *) noexcept` (signature `bv*`).
    pub const CORO_DONE: u32 = 366;
    /// `void *__builtin_coro_promise(void *, int, bool) noexcept`
    /// (signature `v*v*IiIb`: the alignment and direction are integer
    /// constant expressions).
    pub const CORO_PROMISE: u32 = 372;
    /// `void *__builtin_coro_noop() noexcept` (signature `v*`).
    pub const CORO_NOOP: u32 = 371;
    /// `size_t __builtin_coro_size() noexcept` (signature `z`).
    pub const CORO_SIZE: u32 = 374;
    /// `size_t __builtin_coro_align() noexcept` (signature `z`).
    pub const CORO_ALIGN: u32 = 362;
    /// `void *__builtin_coro_frame() noexcept` (signature `v*`).
    pub const CORO_FRAME: u32 = 368;
    /// `void *__builtin_coro_free(void *) noexcept` (signature `v*v*`).
    pub const CORO_FREE: u32 = 369;
    /// `void *__builtin_coro_id(int, void *, void *, void *) noexcept`
    /// (signature `v*Iiv*v*v*`).
    pub const CORO_ID: u32 = 370;
    /// `bool __builtin_coro_alloc() noexcept` (signature `b`).
    pub const CORO_ALLOC: u32 = 363;
    /// `void *__builtin_coro_begin(void *) noexcept` (signature `v*v*`).
    pub const CORO_BEGIN: u32 = 364;
    /// `bool __builtin_coro_end(void *, bool) noexcept` (signature
    /// `bv*Ib`).
    pub const CORO_END: u32 = 367;
    /// `char __builtin_coro_suspend(bool) noexcept` (signature `cIb`).
    pub const CORO_SUSPEND: u32 = 375;

    // WP-156: the vector reductions (clang `Builtins.td` signature `v.`,
    // custom type-checking: the call has the vector's element type).
    /// `__builtin_reduce_add`.
    pub const REDUCE_ADD: u32 = 991;
    /// `__builtin_reduce_and`.
    pub const REDUCE_AND: u32 = 992;
    /// `__builtin_reduce_max`.
    pub const REDUCE_MAX: u32 = 993;
    /// `__builtin_reduce_min`.
    pub const REDUCE_MIN: u32 = 994;
    /// `__builtin_reduce_mul`.
    pub const REDUCE_MUL: u32 = 995;
    /// `__builtin_reduce_or`.
    pub const REDUCE_OR: u32 = 996;
    /// `__builtin_reduce_xor`.
    pub const REDUCE_XOR: u32 = 997;

    /// `__builtin_fabs` (clang `Builtins.td` signature `dd`).
    pub const FABS: u32 = 550;
    /// `__builtin_fabsf` (clang `Builtins.td` signature `ff`).
    pub const FABSF: u32 = 548;
    /// `__builtin_fabsl` (clang `Builtins.td` signature `LdLd`).
    pub const FABSL: u32 = 552;
    /// `__builtin_copysign` (clang `Builtins.td` signature `ddd`).
    pub const COPYSIGN: u32 = 356;
    /// `__builtin_copysignf` (clang `Builtins.td` signature `fff`).
    pub const COPYSIGNF: u32 = 354;
    /// `__builtin_copysignl` (clang `Builtins.td` signature `LdLdLd`).
    pub const COPYSIGNL: u32 = 358;
    /// `__builtin_fmax` (clang `Builtins.td` signature `ddd`).
    pub const FMAX: u32 = 585;
    /// `__builtin_fmaxf` (clang `Builtins.td` signature `fff`).
    pub const FMAXF: u32 = 583;
    /// `__builtin_fmaxl` (clang `Builtins.td` signature `LdLdLd`).
    pub const FMAXL: u32 = 587;
    /// `__builtin_fmin` (clang `Builtins.td` signature `ddd`).
    pub const FMIN: u32 = 593;
    /// `__builtin_fminf` (clang `Builtins.td` signature `fff`).
    pub const FMINF: u32 = 591;
    /// `__builtin_fminl` (clang `Builtins.td` signature `LdLdLd`).
    pub const FMINL: u32 = 595;
    /// `__builtin_ceil` (clang `Builtins.td` signature `dd`).
    pub const CEIL: u32 = 312;
    /// `__builtin_ceilf` (clang `Builtins.td` signature `ff`).
    pub const CEILF: u32 = 310;
    /// `__builtin_ceill` (clang `Builtins.td` signature `LdLd`).
    pub const CEILL: u32 = 314;
    /// `__builtin_floor` (clang `Builtins.td` signature `dd`).
    pub const FLOOR: u32 = 569;
    /// `__builtin_floorf` (clang `Builtins.td` signature `ff`).
    pub const FLOORF: u32 = 567;
    /// `__builtin_floorl` (clang `Builtins.td` signature `LdLd`).
    pub const FLOORL: u32 = 571;
    /// `__builtin_trunc` (clang `Builtins.td` signature `dd`).
    pub const TRUNC: u32 = 1334;
    /// `__builtin_truncf` (clang `Builtins.td` signature `ff`).
    pub const TRUNCF: u32 = 1332;
    /// `__builtin_truncl` (clang `Builtins.td` signature `LdLd`).
    pub const TRUNCL: u32 = 1336;
    /// `__builtin_round` (clang `Builtins.td` signature `dd`).
    pub const ROUND: u32 = 1042;
    /// `__builtin_roundf` (clang `Builtins.td` signature `ff`).
    pub const ROUNDF: u32 = 1040;
    /// `__builtin_roundl` (clang `Builtins.td` signature `LdLd`).
    pub const ROUNDL: u32 = 1044;
    /// `__builtin_rint` (clang `Builtins.td` signature `dd`).
    pub const RINT: u32 = 1018;
    /// `__builtin_rintf` (clang `Builtins.td` signature `ff`).
    pub const RINTF: u32 = 1016;
    /// `__builtin_rintl` (clang `Builtins.td` signature `LdLd`).
    pub const RINTL: u32 = 1020;
    /// `__builtin_nearbyint` (clang `Builtins.td` signature `dd`).
    pub const NEARBYINT: u32 = 905;
    /// `__builtin_nearbyintf` (clang `Builtins.td` signature `ff`).
    pub const NEARBYINTF: u32 = 903;
    /// `__builtin_nearbyintl` (clang `Builtins.td` signature `LdLd`).
    pub const NEARBYINTL: u32 = 907;
    /// `__builtin_fma` (clang `Builtins.td` signature `dddd`).
    pub const FMA: u32 = 577;
    /// `__builtin_fmaf` (clang `Builtins.td` signature `ffff`).
    pub const FMAF: u32 = 575;
    /// `__builtin_fmal` (clang `Builtins.td` signature `LdLdLdLd`).
    pub const FMAL: u32 = 579;
    /// `__builtin_inf` (clang `Builtins.td` signature `d`).
    pub const INF: u32 = 673;
    /// `__builtin_inff` (clang `Builtins.td` signature `f`).
    pub const INFF: u32 = 672;
    /// `__builtin_infl` (clang `Builtins.td` signature `Ld`).
    pub const INFL: u32 = 674;
    /// `__builtin_abs` (clang `Builtins.td` signature `ii`).
    pub const ABS: u32 = 92;
    /// `__builtin_labs` (clang `Builtins.td` signature `LiLi`).
    pub const LABS: u32 = 94;
    /// `__builtin_llabs` (clang `Builtins.td` signature `LLiLLi`).
    pub const LLABS: u32 = 96;
    /// `__builtin_isinf_sign` (clang `Builtins.td` signature `i.`).
    pub const ISINF_SIGN: u32 = 730;
    /// `__builtin_fpclassify` (clang `Builtins.td` signature `iiiiii.`).
    pub const FPCLASSIFY: u32 = 542;
    /// `__builtin_issubnormal` (clang `Builtins.td` signature `i.`).
    pub const ISSUBNORMAL: u32 = 741;
    /// `__builtin_iszero` (clang `Builtins.td` signature `i.`).
    pub const ISZERO: u32 = 745;
    /// `__builtin_issignaling` (clang `Builtins.td` signature `i.`).
    pub const ISSIGNALING: u32 = 739;
    /// `__builtin_expect` (clang `Builtins.td` signature `LiLiLi`).
    pub const EXPECT: u32 = 528;
    /// `__builtin_expect_with_probability` (clang `Builtins.td` signature
    /// `LiLiLid`).
    pub const EXPECT_WITH_PROBABILITY: u32 = 529;
    /// `__builtin_isfpclass` (clang `Builtins.td` signature `i.`).
    pub const ISFPCLASS: u32 = 726;
    /// `__builtin_assume` (clang `Builtins.td` signature `vb`).
    pub const ASSUME: u32 = 148;
    /// `__builtin_trap` (clang `Builtins.td` signature `v`).
    pub const TRAP: u32 = 1331;
    /// `__builtin_abort` (clang `Builtins.td` signature `v`).
    pub const ABORT: u32 = 90;

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
        /// `ConstIgnoringExceptions` (`rint`, `fma`): with the default
        /// floating-point exception mode the implicit `ConstAttr` is added
        /// before the `NoThrowAttr` (clang `AddKnownFunctionAttributes`).
        /// Set together with [`CONST`].
        pub const CONST_EARLY: u8 = 32;
        /// `Consteval`: the declaration is an immediate function (clang
        /// `LazilyCreateBuiltin`, `isImmediate`).
        pub const CONSTEVAL: u8 = 64;
        /// `FunctionWithBuiltinPrefix` (clang `isLibFunction`): the
        /// `__builtin_`-prefixed spelling of a library function, which
        /// makes a call to it trivially recursive in a function labelled
        /// with the library name (clang `FunctionIsDirectlyRecursive`).
        /// Taken from clang 19 and 22 (`-O1 -disable-llvm-passes` on an
        /// `extern template` labelled with the library name), WP-462.
        pub const LIB_FUNCTION: u8 = 128;
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
        /// `&` lvalue reference, `C` const, `D` volatile, `R` restrict; `A` is a
        /// `__builtin_va_list` parameter (a reference to it, or the decayed
        /// pointer when it is an array), `a` a `__builtin_va_list` (decayed
        /// when it is an array), `P` the `FILE` type ([`LIB`] only). An `I`
        /// prefix marks an argument that must be an integer constant
        /// expression.
        pub sig: &'static str,
        /// [`attr`] bits.
        pub attrs: u8,
    }

    const fn row(id: u32, name: &'static str, sig: &'static str, attrs: u8) -> Info {
        Info { id, name, sig, attrs }
    }

    use attr::{
        CONST as C, CONST_EARLY as E, CONSTEVAL as I, CUSTOM as T, LIB_FUNCTION as F,
        NORETURN as R, NOTHROW as N, PURE as P,
    };

    /// Every builtin function kiln declares.
    pub static ALL: &[Info] = &[
        row(IS_CONSTANT_EVALUATED, "__builtin_is_constant_evaluated", "b", N),
        row(ADDRESSOF, "__builtin_addressof", "v*v&", N | C | T),
        row(COMPLEX, "__builtin_complex", "v.", N | C | T),
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
        row(CONSTANT_P, "__builtin_constant_p", "i.", N | C | T),
        row(CLASSIFY_TYPE, "__builtin_classify_type", "i.", N | C | T),
        row(BSWAP16, "__builtin_bswap16", "UsUs", N | C),
        row(BSWAP32, "__builtin_bswap32", "UiUi", N | C),
        row(BSWAP64, "__builtin_bswap64", "ULiULi", N | C),
        row(MEMCMP, "__builtin_memcmp", "ivC*vC*z", N | F),
        row(MEMCPY, "__builtin_memcpy", "v*v*vC*z", N | F),
        row(MEMMOVE, "__builtin_memmove", "v*v*vC*z", N | F),
        row(MEMCHR, "__builtin_memchr", "v*vC*iz", N | F),
        row(CHAR_MEMCHR, "__builtin_char_memchr", "c*cC*iz", N),
        row(STRLEN, "__builtin_strlen", "zcC*", N | F),
        row(WCSLEN, "__builtin_wcslen", "zwC*", N | P | F),
        row(WMEMCHR, "__builtin_wmemchr", "w*wC*wz", N | F),
        row(WMEMCMP, "__builtin_wmemcmp", "iwC*wC*z", N | F),
        row(STRCMP, "__builtin_strcmp", "icC*cC*", N | F),
        row(STRNCMP, "__builtin_strncmp", "icC*cC*z", N | F),
        row(STRCHR, "__builtin_strchr", "c*cC*i", N | F),
        row(STRRCHR, "__builtin_strrchr", "c*cC*i", N | F),
        row(STRPBRK, "__builtin_strpbrk", "c*cC*cC*", N | F),
        row(STRSTR, "__builtin_strstr", "c*cC*cC*", N | F),
        row(WCSCMP, "__builtin_wcscmp", "iwC*wC*", N | P | F),
        row(WCSNCMP, "__builtin_wcsncmp", "iwC*wC*z", N | P | F),
        row(WCSCHR, "__builtin_wcschr", "w*wC*w", N | P | F),
        row(ISFINITE, "__builtin_isfinite", "i.", N | C | T | F),
        row(ISINF, "__builtin_isinf", "i.", N | C | T | F),
        row(ISNAN, "__builtin_isnan", "i.", N | C | T | F),
        row(ISNORMAL, "__builtin_isnormal", "i.", N | C | T | F),
        row(SIGNBIT, "__builtin_signbit", "i.", N | C | T | F),
        row(SIGNBITF, "__builtin_signbitf", "if", N | C | F),
        row(SIGNBITL, "__builtin_signbitl", "iLd", N | C | F),
        row(ISGREATER, "__builtin_isgreater", "i.", N | C | T | F),
        row(ISGREATEREQUAL, "__builtin_isgreaterequal", "i.", N | C | T | F),
        row(ISLESS, "__builtin_isless", "i.", N | C | T | F),
        row(ISLESSEQUAL, "__builtin_islessequal", "i.", N | C | T | F),
        row(ISLESSGREATER, "__builtin_islessgreater", "i.", N | C | T | F),
        row(ISUNORDERED, "__builtin_isunordered", "i.", N | C | T | F),
        row(HUGE_VAL, "__builtin_huge_val", "d", N | C),
        row(HUGE_VALF, "__builtin_huge_valf", "f", N | C),
        row(HUGE_VALL, "__builtin_huge_vall", "Ld", N | C),
        row(NAN, "__builtin_nan", "dcC*", N | P | F),
        row(NANF, "__builtin_nanf", "fcC*", N | P | F),
        row(NANL, "__builtin_nanl", "LdcC*", N | P | F),
        row(NANS, "__builtin_nans", "dcC*", N | P | F),
        row(NANSF, "__builtin_nansf", "fcC*", N | P | F),
        row(NANSL, "__builtin_nansl", "LdcC*", N | P | F),
        row(FABS, "__builtin_fabs", "dd", N | C | F),
        row(FABSF, "__builtin_fabsf", "ff", N | C | F),
        row(FABSL, "__builtin_fabsl", "LdLd", N | C | F),
        row(COPYSIGN, "__builtin_copysign", "ddd", N | C | F),
        row(COPYSIGNF, "__builtin_copysignf", "fff", N | C | F),
        row(COPYSIGNL, "__builtin_copysignl", "LdLdLd", N | C | F),
        row(FMAX, "__builtin_fmax", "ddd", N | C | F),
        row(FMAXF, "__builtin_fmaxf", "fff", N | C | F),
        row(FMAXL, "__builtin_fmaxl", "LdLdLd", N | C | F),
        row(FMIN, "__builtin_fmin", "ddd", N | C | F),
        row(FMINF, "__builtin_fminf", "fff", N | C | F),
        row(FMINL, "__builtin_fminl", "LdLdLd", N | C | F),
        row(CEIL, "__builtin_ceil", "dd", N | C | F),
        row(CEILF, "__builtin_ceilf", "ff", N | C | F),
        row(CEILL, "__builtin_ceill", "LdLd", N | C | F),
        row(FLOOR, "__builtin_floor", "dd", N | C | F),
        row(FLOORF, "__builtin_floorf", "ff", N | C | F),
        row(FLOORL, "__builtin_floorl", "LdLd", N | C | F),
        row(TRUNC, "__builtin_trunc", "dd", N | C | F),
        row(TRUNCF, "__builtin_truncf", "ff", N | C | F),
        row(TRUNCL, "__builtin_truncl", "LdLd", N | C | F),
        row(ROUND, "__builtin_round", "dd", N | C | F),
        row(ROUNDF, "__builtin_roundf", "ff", N | C | F),
        row(ROUNDL, "__builtin_roundl", "LdLd", N | C | F),
        row(RINT, "__builtin_rint", "dd", N | C | E | F),
        row(RINTF, "__builtin_rintf", "ff", N | C | E | F),
        row(RINTL, "__builtin_rintl", "LdLd", N | C | E | F),
        row(NEARBYINT, "__builtin_nearbyint", "dd", N | C | F),
        row(NEARBYINTF, "__builtin_nearbyintf", "ff", N | C | F),
        row(NEARBYINTL, "__builtin_nearbyintl", "LdLd", N | C | F),
        row(FMA, "__builtin_fma", "dddd", N | C | E | F),
        row(FMAF, "__builtin_fmaf", "ffff", N | C | E | F),
        row(FMAL, "__builtin_fmal", "LdLdLdLd", N | C | E | F),
        row(INF, "__builtin_inf", "d", N | C),
        row(INFF, "__builtin_inff", "f", N | C),
        row(INFL, "__builtin_infl", "Ld", N | C),
        row(ABS, "__builtin_abs", "ii", N | C | F),
        row(LABS, "__builtin_labs", "LiLi", N | C | F),
        row(LLABS, "__builtin_llabs", "LLiLLi", N | C | F),
        row(ISINF_SIGN, "__builtin_isinf_sign", "i.", N | C | T | F),
        row(FPCLASSIFY, "__builtin_fpclassify", "iiiiii.", N | C | T | F),
        row(ISSUBNORMAL, "__builtin_issubnormal", "i.", N | C | T | F),
        row(ISZERO, "__builtin_iszero", "i.", N | C | T | F),
        row(ISSIGNALING, "__builtin_issignaling", "i.", N | C | T | F),
        row(EXPECT, "__builtin_expect", "LiLiLi", N | C),
        row(EXPECT_WITH_PROBABILITY, "__builtin_expect_with_probability", "LiLiLid", N | C),
        row(ISFPCLASS, "__builtin_isfpclass", "i.", N | C | T),
        row(ASSUME, "__builtin_assume", "vb", N),
        row(TRAP, "__builtin_trap", "v", N | R),
        row(ABORT, "__builtin_abort", "v", N | R | F),
        row(LAUNDER, "__builtin_launder", "v*v*", N | T),
        row(OPERATOR_NEW, "__builtin_operator_new", "v*z", C | T),
        row(OPERATOR_DELETE, "__builtin_operator_delete", "vv*", N | T),
        row(UNREACHABLE, "__builtin_unreachable", "v", N | R),
        row(VA_START, "__builtin_va_start", "vA.", N | T),
        row(VA_END, "__builtin_va_end", "vA", N | F),
        row(VA_COPY, "__builtin_va_copy", "vAA", N | F),
        // WP-156: the vector reductions.
        row(REDUCE_ADD, "__builtin_reduce_add", "v.", N | C | T),
        row(REDUCE_AND, "__builtin_reduce_and", "v.", N | C | T),
        row(REDUCE_MAX, "__builtin_reduce_max", "v.", N | C | T),
        row(REDUCE_MIN, "__builtin_reduce_min", "v.", N | C | T),
        row(REDUCE_MUL, "__builtin_reduce_mul", "v.", N | C | T),
        row(REDUCE_OR, "__builtin_reduce_or", "v.", N | C | T),
        row(REDUCE_XOR, "__builtin_reduce_xor", "v.", N | C | T),
    ];

    /// WP-171: the coroutine builtins the libc++ headers name (clang 19
    /// `Builtins.td` `CoroLangBuiltin`s). Sema type-checks calls; codegen
    /// and the Oven do not implement them.
    pub static CORO: &[Info] = &[
        row(CORO_RESUME, "__builtin_coro_resume", "vv*", 0),
        row(CORO_DESTROY, "__builtin_coro_destroy", "vv*", 0),
        row(CORO_DONE, "__builtin_coro_done", "bv*", N),
        row(CORO_PROMISE, "__builtin_coro_promise", "v*v*IiIb", N),
        row(CORO_NOOP, "__builtin_coro_noop", "v*", N),
        row(CORO_SIZE, "__builtin_coro_size", "z", N),
        row(CORO_ALIGN, "__builtin_coro_align", "z", N),
        row(CORO_FRAME, "__builtin_coro_frame", "v*", N),
        row(CORO_FREE, "__builtin_coro_free", "v*v*", N),
        row(CORO_ID, "__builtin_coro_id", "v*Iiv*v*v*", N),
        row(CORO_ALLOC, "__builtin_coro_alloc", "b", N),
        row(CORO_BEGIN, "__builtin_coro_begin", "v*v*", N),
        row(CORO_END, "__builtin_coro_end", "bv*Ib", N),
        row(CORO_SUSPEND, "__builtin_coro_suspend", "cIb", N),
    ];

    /// WP-171: the C library functions clang 19 recognizes as library
    /// builtins and gives implicit attributes (`Builtins.td` `LibBuiltin`
    /// rows with the `NoThrow` / `Const` / `Pure` attributes; ids are
    /// clang's `Builtin::ID`). A recognition table only: these names are
    /// not lazily declared and a call is an ordinary call. The first
    /// file-scope declaration with C language linkage whose type matches
    /// the signature ignoring the exception specification gets the
    /// attributes (clang `ActOnFunctionDeclarator` +
    /// `AddKnownFunctionAttributes`). `P` is the type the file-scope
    /// `FILE` typedef names. clang's library builtins without any of these
    /// attributes (`malloc`, `printf`, `strtol`, ...) are left out, except
    /// the ones clang constant-evaluates (`memcmp`, `bcmp`; WP-1055), which
    /// the never-constant check recognizes (spec/sema.md "Never
    /// constant") and whose calls codegen folds through their `__builtin_`
    /// twin (WP-940).
    pub static LIB: &[Info] = &[
        row(93, "abs", "ii", N | C),
        row(95, "labs", "LiLi", N | C),
        row(97, "llabs", "LLiLLi", N | C),
        row(866, "memcpy", "v*v*vC*z", N),
        row(868, "memmove", "v*v*vC*z", N),
        row(1143, "strcpy", "c*c*cC*", N),
        row(1154, "strncpy", "c*c*cC*z", N),
        row(1141, "strcmp", "icC*cC*", N),
        row(1152, "strncmp", "icC*cC*z", N),
        row(1137, "strcat", "c*c*cC*", N),
        row(1150, "strncat", "c*c*cC*z", N),
        row(863, "memchr", "v*vC*iz", N),
        row(1139, "strchr", "c*cC*i", N),
        row(1179, "strpbrk", "c*cC*cC*", N),
        row(1181, "strrchr", "c*cC*i", N),
        row(1156, "strspn", "zcC*cC*", N),
        row(1158, "strstr", "c*cC*cC*", N),
        row(870, "memset", "v*v*iz", N),
        row(1148, "strlen", "zcC*", N),
        row(544, "fprintf", "iP*cC*.", N),
        row(1114, "snprintf", "ic*zcC*.", N),
        row(1058, "sprintf", "ic*cC*.", N),
        row(1357, "vprintf", "icC*a", N),
        row(1372, "vfprintf", "iP*cC*a", N),
        row(1376, "vsnprintf", "ic*zcC*a", N),
        row(1374, "vsprintf", "ic*cC*a", N),
        row(1387, "wcschr", "w*wC*w", N | P),
        row(1389, "wcscmp", "iwC*wC*", N | P),
        row(1391, "wcslen", "zwC*", N | P),
        row(1393, "wcsncmp", "iwC*wC*z", N | P),
        row(1378, "wmemchr", "w*wC*wz", N),
        row(1380, "wmemcmp", "iwC*wC*z", N),
        row(1382, "wmemcpy", "w*w*wC*z", N),
        row(1384, "wmemmove", "w*w*wC*z", N),
        // WP-1055: `memcmp` has no implicit attribute but clang
        // constant-evaluates it (`__has_constexpr_builtin`), which the
        // never-constant check needs to know; so does the GNU `bcmp`.
        // The rest are the GNU `<string.h>` / `<strings.h>` builtins
        // with `NoThrow` (clang `GNULibBuiltin`, [`lib_gnu_only`]).
        row(864, "memcmp", "ivC*vC*z", 0),
        row(178, "bcmp", "ivC*vC*z", 0),
        row(185, "bcopy", "vvC*v*z", N),
        row(183, "bzero", "vv*z", N),
        row(671, "index", "c*cC*i", N),
        row(1015, "rindex", "c*cC*i", N),
        row(1129, "stpcpy", "c*c*cC*", N),
        row(1132, "stpncpy", "c*c*cC*z", N),
        row(1145, "strdup", "c*cC*", N),
        row(1175, "strndup", "c*cC*z", N),
        row(717, "isalnum", "ii", N | P),
        row(719, "isalpha", "ii", N | P),
        row(720, "isblank", "ii", N | P),
        row(721, "iscntrl", "ii", N | P),
        row(723, "isdigit", "ii", N | P),
        row(726, "isgraph", "ii", N | P),
        row(734, "islower", "ii", N | P),
        row(737, "isprint", "ii", N | P),
        row(738, "ispunct", "ii", N | P),
        row(740, "isspace", "ii", N | P),
        row(743, "isupper", "ii", N | P),
        row(744, "isxdigit", "ii", N | P),
        row(1328, "tolower", "ii", N | P),
        row(1330, "toupper", "ii", N | P),
        row(101, "acos", "dd", N),
        row(109, "acosh", "dd", N),
        row(136, "asin", "dd", N),
        row(144, "asinh", "dd", N),
        row(153, "atan", "dd", N),
        row(168, "atanh", "dd", N),
        row(294, "cbrt", "dd", N | C),
        row(313, "ceil", "dd", N | C),
        row(379, "cos", "dd", N),
        row(387, "cosh", "dd", N),
        row(490, "erf", "dd", N),
        row(497, "erfc", "dd", N),
        row(510, "exp", "dd", N),
        row(516, "exp2", "dd", N),
        row(533, "expm1", "dd", N),
        row(551, "fabs", "dd", N | C),
        row(570, "floor", "dd", N | C),
        row(768, "lgamma", "dd", N),
        row(789, "log", "dd", N),
        row(810, "log10", "dd", N),
        row(795, "log1p", "dd", N),
        row(802, "log2", "dd", N),
        row(820, "logb", "dd", N),
        row(906, "nearbyint", "dd", N | C),
        row(1019, "rint", "dd", N | C),
        row(1043, "round", "dd", N | C),
        row(1095, "sin", "dd", N),
        row(1103, "sinh", "dd", N),
        row(1118, "sqrt", "dd", N),
        row(1303, "tan", "dd", N),
        row(1311, "tanh", "dd", N),
        row(1321, "tgamma", "dd", N),
        row(1335, "trunc", "dd", N | C),
        row(159, "atan2", "ddd", N),
        row(357, "copysign", "ddd", N | C),
        row(560, "fdim", "ddd", N),
        row(586, "fmax", "ddd", N | C),
        row(594, "fmin", "ddd", N | C),
        row(602, "fmod", "ddd", N),
        row(659, "hypot", "ddd", N),
        row(913, "nextafter", "ddd", N),
        row(968, "pow", "ddd", N),
        row(1001, "remainder", "ddd", N),
        row(578, "fma", "dddd", N | C),
        row(616, "frexp", "ddi*", N),
        row(760, "ldexp", "ddi", N),
        row(881, "modf", "ddd*", N),
        row(893, "nan", "dcC*", N | P),
        row(667, "ilogb", "id", N),
        row(828, "lrint", "Lid", N),
        row(775, "llrint", "LLid", N),
        row(837, "lround", "Lid", N),
        row(781, "llround", "LLid", N),
        row(920, "nexttoward", "ddLd", N),
        row(1008, "remquo", "dddi*", N),
        row(1075, "scalbn", "ddi", N),
        row(1069, "scalbln", "ddLi", N),
        row(99, "acosf", "ff", N),
        row(107, "acoshf", "ff", N),
        row(134, "asinf", "ff", N),
        row(142, "asinhf", "ff", N),
        row(151, "atanf", "ff", N),
        row(166, "atanhf", "ff", N),
        row(292, "cbrtf", "ff", N | C),
        row(311, "ceilf", "ff", N | C),
        row(377, "cosf", "ff", N),
        row(385, "coshf", "ff", N),
        row(488, "erff", "ff", N),
        row(495, "erfcf", "ff", N),
        row(508, "expf", "ff", N),
        row(514, "exp2f", "ff", N),
        row(531, "expm1f", "ff", N),
        row(549, "fabsf", "ff", N | C),
        row(568, "floorf", "ff", N | C),
        row(766, "lgammaf", "ff", N),
        row(787, "logf", "ff", N),
        row(808, "log10f", "ff", N),
        row(793, "log1pf", "ff", N),
        row(800, "log2f", "ff", N),
        row(818, "logbf", "ff", N),
        row(904, "nearbyintf", "ff", N | C),
        row(1017, "rintf", "ff", N | C),
        row(1041, "roundf", "ff", N | C),
        row(1093, "sinf", "ff", N),
        row(1101, "sinhf", "ff", N),
        row(1116, "sqrtf", "ff", N),
        row(1301, "tanf", "ff", N),
        row(1309, "tanhf", "ff", N),
        row(1319, "tgammaf", "ff", N),
        row(1333, "truncf", "ff", N | C),
        row(157, "atan2f", "fff", N),
        row(355, "copysignf", "fff", N | C),
        row(558, "fdimf", "fff", N),
        row(584, "fmaxf", "fff", N | C),
        row(592, "fminf", "fff", N | C),
        row(600, "fmodf", "fff", N),
        row(657, "hypotf", "fff", N),
        row(911, "nextafterf", "fff", N),
        row(966, "powf", "fff", N),
        row(999, "remainderf", "fff", N),
        row(576, "fmaf", "ffff", N | C),
        row(614, "frexpf", "ffi*", N),
        row(758, "ldexpf", "ffi", N),
        row(879, "modff", "fff*", N),
        row(891, "nanf", "fcC*", N | P),
        row(665, "ilogbf", "if", N),
        row(826, "lrintf", "Lif", N),
        row(773, "llrintf", "LLif", N),
        row(835, "lroundf", "Lif", N),
        row(779, "llroundf", "LLif", N),
        row(918, "nexttowardf", "ffLd", N),
        row(1006, "remquof", "fffi*", N),
        row(1073, "scalbnf", "ffi", N),
        row(1067, "scalblnf", "ffLi", N),
        row(103, "acosl", "LdLd", N),
        row(111, "acoshl", "LdLd", N),
        row(138, "asinl", "LdLd", N),
        row(146, "asinhl", "LdLd", N),
        row(155, "atanl", "LdLd", N),
        row(170, "atanhl", "LdLd", N),
        row(296, "cbrtl", "LdLd", N | C),
        row(315, "ceill", "LdLd", N | C),
        row(381, "cosl", "LdLd", N),
        row(389, "coshl", "LdLd", N),
        row(492, "erfl", "LdLd", N),
        row(499, "erfcl", "LdLd", N),
        row(512, "expl", "LdLd", N),
        row(518, "exp2l", "LdLd", N),
        row(535, "expm1l", "LdLd", N),
        row(553, "fabsl", "LdLd", N | C),
        row(572, "floorl", "LdLd", N | C),
        row(770, "lgammal", "LdLd", N),
        row(791, "logl", "LdLd", N),
        row(812, "log10l", "LdLd", N),
        row(797, "log1pl", "LdLd", N),
        row(804, "log2l", "LdLd", N),
        row(822, "logbl", "LdLd", N),
        row(908, "nearbyintl", "LdLd", N | C),
        row(1021, "rintl", "LdLd", N | C),
        row(1045, "roundl", "LdLd", N | C),
        row(1097, "sinl", "LdLd", N),
        row(1105, "sinhl", "LdLd", N),
        row(1120, "sqrtl", "LdLd", N),
        row(1305, "tanl", "LdLd", N),
        row(1313, "tanhl", "LdLd", N),
        row(1323, "tgammal", "LdLd", N),
        row(1337, "truncl", "LdLd", N | C),
        row(161, "atan2l", "LdLdLd", N),
        row(359, "copysignl", "LdLdLd", N | C),
        row(562, "fdiml", "LdLdLd", N),
        row(588, "fmaxl", "LdLdLd", N | C),
        row(596, "fminl", "LdLdLd", N | C),
        row(604, "fmodl", "LdLdLd", N),
        row(661, "hypotl", "LdLdLd", N),
        row(915, "nextafterl", "LdLdLd", N),
        row(970, "powl", "LdLdLd", N),
        row(1003, "remainderl", "LdLdLd", N),
        row(580, "fmal", "LdLdLdLd", N | C),
        row(618, "frexpl", "LdLdi*", N),
        row(762, "ldexpl", "LdLdi", N),
        row(883, "modfl", "LdLdLd*", N),
        row(895, "nanl", "LdcC*", N | P),
        row(669, "ilogbl", "iLd", N),
        row(830, "lrintl", "LiLd", N),
        row(777, "llrintl", "LLiLd", N),
        row(839, "lroundl", "LiLd", N),
        row(783, "llroundl", "LLiLd", N),
        row(922, "nexttowardl", "LdLdLd", N),
        row(1010, "remquol", "LdLdLdi*", N),
        row(1077, "scalbnl", "LdLdi", N),
        row(1071, "scalblnl", "LdLdLi", N),
        // WP-186: recognized for its implicit `format(printf, 1, 2)` only
        // ([`lib_printf_format`]; clang's id is not used here).
        row(0, "printf", "icC*.", 0),
        // WP-419: C23's `roundeven`, clang 22's `fmaximum_num` /
        // `fminimum_num` ([`lib_min_compat`]) and the GNU `sincos`
        // ([`lib_gnu_only`]; clang 22). The ids are kiln-private.
        row(2048, "roundeven", "dd", N | C),
        row(2049, "roundevenf", "ff", N | C),
        row(2050, "roundevenl", "LdLd", N | C),
        row(2051, "fmaximum_num", "ddd", N | C),
        row(2052, "fmaximum_numf", "fff", N | C),
        row(2053, "fmaximum_numl", "LdLdLd", N | C),
        row(2054, "fminimum_num", "ddd", N | C),
        row(2055, "fminimum_numf", "fff", N | C),
        row(2056, "fminimum_numl", "LdLdLd", N | C),
        row(2057, "sincos", "vdd*d*", N),
        row(2058, "sincosf", "vff*f*", N),
        row(2059, "sincosl", "vLdLd*Ld*", N),
        // WP-902: `abort` (`NoThrow`, `NoReturn`: only a declaration whose
        // type is `noreturn`, `__attribute__((noreturn))`, matches), the
        // `<complex.h>` functions (`cproj` is `Const`; `creal` / `cimag` /
        // `conj` are below, WP-1073). The ids are
        // kiln-private. The GNU `string.h` / `strings.h` rows are above
        // (WP-1055).
        row(2060, "abort", "v", N | R),
        row(2061, "cabs", "dXd", N),
        row(2062, "cabsf", "fXf", N),
        row(2063, "cabsl", "LdXLd", N),
        row(2064, "cacos", "XdXd", N),
        row(2065, "cacosf", "XfXf", N),
        row(2066, "cacosl", "XLdXLd", N),
        row(2067, "cacosh", "XdXd", N),
        row(2068, "cacoshf", "XfXf", N),
        row(2069, "cacoshl", "XLdXLd", N),
        row(2070, "carg", "dXd", N),
        row(2071, "cargf", "fXf", N),
        row(2072, "cargl", "LdXLd", N),
        row(2073, "casin", "XdXd", N),
        row(2074, "casinf", "XfXf", N),
        row(2075, "casinl", "XLdXLd", N),
        row(2076, "casinh", "XdXd", N),
        row(2077, "casinhf", "XfXf", N),
        row(2078, "casinhl", "XLdXLd", N),
        row(2079, "catan", "XdXd", N),
        row(2080, "catanf", "XfXf", N),
        row(2081, "catanl", "XLdXLd", N),
        row(2082, "catanh", "XdXd", N),
        row(2083, "catanhf", "XfXf", N),
        row(2084, "catanhl", "XLdXLd", N),
        row(2085, "ccos", "XdXd", N),
        row(2086, "ccosf", "XfXf", N),
        row(2087, "ccosl", "XLdXLd", N),
        row(2088, "ccosh", "XdXd", N),
        row(2089, "ccoshf", "XfXf", N),
        row(2090, "ccoshl", "XLdXLd", N),
        row(2091, "cexp", "XdXd", N),
        row(2092, "cexpf", "XfXf", N),
        row(2093, "cexpl", "XLdXLd", N),
        row(2094, "clog", "XdXd", N),
        row(2095, "clogf", "XfXf", N),
        row(2096, "clogl", "XLdXLd", N),
        row(2097, "cpow", "XdXdXd", N),
        row(2098, "cpowf", "XfXfXf", N),
        row(2099, "cpowl", "XLdXLdXLd", N),
        row(2100, "cproj", "XdXd", N | C),
        row(2101, "cprojf", "XfXf", N | C),
        row(2102, "cprojl", "XLdXLd", N | C),
        row(2103, "csin", "XdXd", N),
        row(2104, "csinf", "XfXf", N),
        row(2105, "csinl", "XLdXLd", N),
        row(2106, "csinh", "XdXd", N),
        row(2107, "csinhf", "XfXf", N),
        row(2108, "csinhl", "XLdXLd", N),
        row(2109, "csqrt", "XdXd", N),
        row(2110, "csqrtf", "XfXf", N),
        row(2111, "csqrtl", "XLdXLd", N),
        row(2112, "ctan", "XdXd", N),
        row(2113, "ctanf", "XfXf", N),
        row(2114, "ctanl", "XLdXLd", N),
        row(2115, "ctanh", "XdXd", N),
        row(2116, "ctanhf", "XfXf", N),
        row(2117, "ctanhl", "XLdXLd", N),
        // WP-1073: clang's allocation functions (no implicit attribute
        // but `AddKnownFunctionAttributes`'s `alloc_size` / `alloc_align`,
        // [`lib_alloc`]; `memalign` GNU only), the `scanf` family (an
        // implicit `format(scanf, ...)`, [`lib_scanf_format`]), the GNU
        // `alloca` and `mempcpy`, `finite` (GNU) and glibc's `__finite`
        // (all dialects), and `creal` / `cimag` / `conj`, all lowered
        // inline by clang ([`lib_twin`]). The ids are kiln-private.
        row(2118, "malloc", "v*z", 0),
        row(2119, "calloc", "v*zz", 0),
        row(2120, "realloc", "v*v*z", 0),
        row(2121, "aligned_alloc", "v*zz", 0),
        row(2122, "memalign", "v*zz", 0),
        row(2123, "scanf", "icC*.", 0),
        row(2124, "fscanf", "iP*cC*.", 0),
        row(2125, "sscanf", "icC*cC*.", 0),
        row(2126, "vscanf", "icC*a", 0),
        row(2127, "vfscanf", "iP*cC*a", 0),
        row(2128, "vsscanf", "icC*cC*a", 0),
        row(2129, "alloca", "v*z", N),
        row(2130, "mempcpy", "v*v*vC*z", 0),
        row(2131, "finite", "id", N | C),
        row(2132, "finitef", "if", N | C),
        row(2133, "finitel", "iLd", N | C),
        row(2134, "__finite", "id", N | C),
        row(2135, "__finitef", "if", N | C),
        row(2136, "__finitel", "iLd", N | C),
        row(2137, "creal", "dXd", N | C),
        row(2138, "crealf", "fXf", N | C),
        row(2139, "creall", "LdXLd", N | C),
        row(2140, "cimag", "dXd", N | C),
        row(2141, "cimagf", "fXf", N | C),
        row(2142, "cimagl", "LdXLd", N | C),
        row(2143, "conj", "XdXd", N | C),
        row(2144, "conjf", "XfXf", N | C),
        row(2145, "conjl", "XLdXLd", N | C),
    ];

    /// The [`LIB`] row of the C library function spelled `name`.
    pub fn lib_by_name(name: &str) -> Option<&'static Info> {
        LIB.iter().find(|e| e.name == name)
    }

    /// WP-186: the implicit `format(printf, F, A)` indices of the [`LIB`]
    /// function `name` (clang `Builtins.td` `PrintfFormat` /
    /// `VPrintfFormat`, added by `AddKnownFunctionAttributes` before the
    /// other implicit attributes): one-based `(F, A)`, `A` = 0 for a
    /// `va_list`.
    pub fn lib_printf_format(name: &str) -> Option<(u16, u16)> {
        Some(match name {
            "printf" => (1, 2),
            "fprintf" | "sprintf" => (2, 3),
            "snprintf" => (3, 4),
            "vprintf" => (1, 0),
            "vfprintf" | "vsprintf" => (2, 0),
            "vsnprintf" => (3, 0),
            _ => return None,
        })
    }

    /// WP-1073: the implicit `format(scanf, F, A)` indices of the [`LIB`]
    /// function `name` (clang `Builtins.td` `ScanfFormat` /
    /// `VScanfFormat`), as [`lib_printf_format`].
    pub fn lib_scanf_format(name: &str) -> Option<(u16, u16)> {
        Some(match name {
            "scanf" => (1, 2),
            "fscanf" | "sscanf" => (2, 3),
            "vscanf" => (1, 0),
            "vfscanf" | "vsscanf" => (2, 0),
            _ => return None,
        })
    }

    /// WP-1073: the implicit `alloc_size` and `alloc_align` of the [`LIB`]
    /// allocation function `name` (clang `AddKnownFunctionAttributes`):
    /// the one-based `alloc_size` indices (second 0 when absent) and the
    /// one-based `alloc_align` index (0 when absent).
    pub fn lib_alloc(name: &str) -> Option<([u32; 2], u32)> {
        Some(match name {
            "malloc" => ([1, 0], 0),
            "calloc" => ([1, 2], 0),
            "realloc" => ([2, 0], 0),
            "aligned_alloc" | "memalign" => ([2, 0], 1),
            _ => return None,
        })
    }

    /// The `__builtin_` twin of the [`LIB`] function `name` at
    /// `-fclang-compat` level `compat` (WP-419: `ceil` ->
    /// `__builtin_ceil`); WP-1073: `finite` and glibc's `__finite` lower
    /// as `__builtin_isfinite` (clang `EmitBuiltinExpr`'s `BIfinite`
    /// family), with their `f` / `l` variants.
    pub fn lib_twin(name: &str, compat: u32) -> Option<u32> {
        match name {
            "finite" | "finitef" | "finitel" | "__finite" | "__finitef" | "__finitel" => {
                Some(ISFINITE)
            }
            _ => by_name_at(&format!("__builtin_{name}"), compat),
        }
    }

    /// WP-419: the lowest `-fclang-compat` level that recognizes the
    /// [`LIB`] function `id` (clang 19 lacks the `fmaximum_num` /
    /// `fminimum_num` and `sincos` library builtins).
    pub fn lib_min_compat(id: u32) -> u32 {
        if (2051..=2059).contains(&id) { 22 } else { 0 }
    }

    /// WP-419: whether the [`LIB`] function `id` is recognized in the GNU
    /// dialects only (clang `ALL_GNU_LANGUAGES`: `sincos`; WP-1055: the
    /// `GNULibBuiltin` string functions `bcmp`, `bcopy`, `bzero`, `index`,
    /// `rindex`, `stpcpy`, `stpncpy`, `strdup` and `strndup`; WP-1073:
    /// `memalign`, `alloca`, `mempcpy` and `finite`).
    pub fn lib_gnu_only(id: u32) -> bool {
        (2057..=2059).contains(&id)
            || (2129..=2133).contains(&id)
            || matches!(id, 178 | 183 | 185 | 671 | 1015 | 1129 | 1132 | 1145 | 1175 | 2122)
    }

    /// WP-940: whether a call of a C library function whose `__builtin_` twin
    /// is `b` is evaluated (after a note that keeps it from being a constant
    /// expression): clang `VisitBuiltinCallExpr`'s `BIstrlen` / `BIwcslen`,
    /// `BIstrcmp` family, `BIstrchr` family and `BImemcpy` family cases.
    pub fn lib_evaluated(b: u32) -> bool {
        matches!(
            b,
            STRLEN
                | WCSLEN
                | STRCMP
                | WCSCMP
                | STRNCMP
                | WCSNCMP
                | MEMCMP
                | BCMP
                | WMEMCMP
                | STRCHR
                | WCSCHR
                | MEMCHR
                | WMEMCHR
                | MEMCPY
                | MEMMOVE
                | WMEMCPY
                | WMEMMOVE
        )
    }

    /// WP-380: the `__builtin_` twin of the [`LIB`] function `name` when
    /// GCC 14 folds that function as an implicit builtin in constant
    /// evaluation and the Oven folds the twin: `ceil`, `floor`, `trunc`,
    /// `round`, `fabs`, `fmin`, `fmax` and `copysign`, each with its
    /// `f`/`l` variant (`ceil` -> `__builtin_ceil`). Left out: `rint` and
    /// `nearbyint` (GCC does not fold them: the result depends on the
    /// rounding mode). WP-393 adds `sqrt` (GCC folds it; the Oven
    /// evaluates `__builtin_sqrt` correctly rounded).
    pub fn gnu_lib_twin(name: &str) -> Option<u32> {
        const BASES: &[&str] =
            &["ceil", "floor", "trunc", "round", "fabs", "fmin", "fmax", "copysign", "sqrt"];
        let base = name.strip_suffix(['f', 'l']).filter(|b| BASES.contains(b)).unwrap_or(name);
        if !BASES.contains(&base) || lib_by_name(name).is_none() {
            return None;
        }
        by_name(&format!("__builtin_{name}"))
    }

    /// The math function a library or `__builtin_` name names, without
    /// its `__builtin_` prefix and its `f` / `l` / `f16` / `f128` type
    /// suffix (`__builtin_sqrtf` -> `sqrt`); `None` if not one of
    /// `bases`.
    fn math_base<'a>(name: &'a str, bases: &[&str]) -> Option<&'a str> {
        let n = name.strip_prefix("__builtin_").unwrap_or(name);
        if bases.contains(&n) {
            return Some(n);
        }
        ["f128", "f16", "f", "l"]
            .iter()
            .find_map(|s| n.strip_suffix(s))
            .filter(|b| bases.contains(b))
    }

    /// WP-806: clang 19's `ConstIgnoringErrnoAndExceptions` (`e`) math
    /// builtins: under `-fno-math-errno` the function (library or
    /// `__builtin_` form, any type suffix) gets an implicit `ConstAttr`
    /// (clang `Sema::AddKnownFunctionAttributes`).
    pub fn const_without_errno(name: &str) -> bool {
        const BASES: &[&str] = &[
            "acos",
            "acosh",
            "asin",
            "asinh",
            "atan",
            "atan2",
            "atanh",
            "cos",
            "cosh",
            "erf",
            "erfc",
            "exp",
            "exp2",
            "exp10",
            "expm1",
            "fdim",
            "fmod",
            "hypot",
            "ilogb",
            "ldexp",
            "llrint",
            "llround",
            "log",
            "log10",
            "log1p",
            "log2",
            "logb",
            "lrint",
            "lround",
            "nextafter",
            "nexttoward",
            "pow",
            "remainder",
            "scalbln",
            "scalbn",
            "sin",
            "sinh",
            "sqrt",
            "tan",
            "tanh",
            "tgamma",
        ];
        math_base(name, BASES).is_some()
    }

    /// How an errno-setting math builtin lowers when `errno` is off
    /// (clang `EmitBuiltinExpr`'s `GenerateIntrinsics` cases).
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum ErrnoIntrinsic {
        /// The elementwise `llvm.<base>` intrinsic with this many operands.
        Fp(&'static str, usize),
        /// `frem` (`fmod`).
        Frem,
        /// `llvm.ldexp.T.i32` (the `__builtin_` forms only).
        Ldexp,
        /// `llvm.<base>.R.T`, an integer result (`lround`, `lrint`, ...).
        ToInt(&'static str),
    }

    /// WP-806: the intrinsic the math builtin `name` (library or
    /// `__builtin_` form) lowers to without `errno` (clang 19
    /// `EmitBuiltinExpr`; clang 22 adds `atan2`, `clang22`). `ldexp` and
    /// `exp10` lower in their `__builtin_` form only.
    pub fn errno_intrinsic(name: &str, clang22: bool) -> Option<ErrnoIntrinsic> {
        const UNARY: &[&str] = &[
            "acos", "asin", "atan", "cos", "cosh", "exp", "exp2", "exp10", "log", "log10", "log2",
            "sin", "sinh", "sqrt", "tan", "tanh",
        ];
        const OTHER: &[&str] =
            &["atan2", "pow", "fmod", "ldexp", "lround", "llround", "lrint", "llrint"];
        let builtin = name.starts_with("__builtin_");
        if let Some(b) = math_base(name, UNARY) {
            let b = UNARY.iter().find(|&&u| u == b)?;
            return (builtin || *b != "exp10").then_some(ErrnoIntrinsic::Fp(b, 1));
        }
        let b = *OTHER.iter().find(|&&u| Some(u) == math_base(name, OTHER))?;
        Some(match b {
            "atan2" if clang22 => ErrnoIntrinsic::Fp("atan2", 2),
            "pow" => ErrnoIntrinsic::Fp("pow", 2),
            "fmod" => ErrnoIntrinsic::Frem,
            "ldexp" if builtin => ErrnoIntrinsic::Ldexp,
            "lround" | "llround" | "lrint" | "llrint" => ErrnoIntrinsic::ToInt(b),
            _ => return None,
        })
    }

    /// WP-150: the remaining GCC integer builtins (`ffs`, `parity`,
    /// `clrsb`, `bitreverse`, `rotate*`) and memory builtins. All but
    /// `memset`/`mempcpy` are constant-evaluated (clang 19 `Constexpr`).
    pub static INT_MEM: &[Info] = &[
        row(FFS, "__builtin_ffs", "ii", N | C | F),
        row(FFSL, "__builtin_ffsl", "iLi", N | C | F),
        row(FFSLL, "__builtin_ffsll", "iLLi", N | C | F),
        row(PARITY, "__builtin_parity", "iUi", N | C),
        row(PARITYL, "__builtin_parityl", "iULi", N | C),
        row(PARITYLL, "__builtin_parityll", "iULLi", N | C),
        row(CLRSB, "__builtin_clrsb", "ii", N | C),
        row(CLRSBL, "__builtin_clrsbl", "iLi", N | C),
        row(CLRSBLL, "__builtin_clrsbll", "iLLi", N | C),
        row(BITREVERSE8, "__builtin_bitreverse8", "UcUc", N | C),
        row(BITREVERSE16, "__builtin_bitreverse16", "UsUs", N | C),
        row(BITREVERSE32, "__builtin_bitreverse32", "UiUi", N | C),
        row(BITREVERSE64, "__builtin_bitreverse64", "ULiULi", N | C),
        row(ROTATELEFT8, "__builtin_rotateleft8", "UcUcUc", N | C),
        row(ROTATELEFT16, "__builtin_rotateleft16", "UsUsUs", N | C),
        row(ROTATELEFT32, "__builtin_rotateleft32", "UiUiUi", N | C),
        row(ROTATELEFT64, "__builtin_rotateleft64", "ULiULiULi", N | C),
        row(ROTATERIGHT8, "__builtin_rotateright8", "UcUcUc", N | C),
        row(ROTATERIGHT16, "__builtin_rotateright16", "UsUsUs", N | C),
        row(ROTATERIGHT32, "__builtin_rotateright32", "UiUiUi", N | C),
        row(ROTATERIGHT64, "__builtin_rotateright64", "ULiULiULi", N | C),
        row(BCMP, "__builtin_bcmp", "ivC*vC*z", N | F),
        row(WMEMCPY, "__builtin_wmemcpy", "w*w*wC*z", N | F),
        row(WMEMMOVE, "__builtin_wmemmove", "w*w*wC*z", N | F),
        row(MEMSET, "__builtin_memset", "v*v*iz", N | F),
        row(MEMPCPY, "__builtin_mempcpy", "v*v*vC*z", N | F),
    ];

    /// The C library math functions ([c.math], C23 7.12) that are only
    /// declared and called: never constant-evaluated by clang 19 (a call
    /// in a constant expression is "subexpression not valid"), lowered to
    /// a call of the library function the builtin names (`__builtin_sin`
    /// -> `sin`; with clang's default `-fmath-errno` on Linux none of them
    /// becomes an LLVM intrinsic). `frexp` is lowered to `llvm.frexp`.
    pub static LIBM: &[Info] = &[
        row(EXP, "__builtin_exp", "dd", N | F),
        row(EXPF, "__builtin_expf", "ff", N | F),
        row(EXPL, "__builtin_expl", "LdLd", N | F),
        row(EXP2, "__builtin_exp2", "dd", N | F),
        row(EXP2F, "__builtin_exp2f", "ff", N | F),
        row(EXP2L, "__builtin_exp2l", "LdLd", N | F),
        row(EXPM1, "__builtin_expm1", "dd", N | F),
        row(EXPM1F, "__builtin_expm1f", "ff", N | F),
        row(EXPM1L, "__builtin_expm1l", "LdLd", N | F),
        row(FREXP, "__builtin_frexp", "ddi*", N | F),
        row(FREXPF, "__builtin_frexpf", "ffi*", N | F),
        row(FREXPL, "__builtin_frexpl", "LdLdi*", N | F),
        row(LDEXP, "__builtin_ldexp", "ddi", N | F),
        row(LDEXPF, "__builtin_ldexpf", "ffi", N | F),
        row(LDEXPL, "__builtin_ldexpl", "LdLdi", N | F),
        row(POW, "__builtin_pow", "ddd", N | F),
        row(POWF, "__builtin_powf", "fff", N | F),
        row(POWL, "__builtin_powl", "LdLdLd", N | F),
        row(SCALBN, "__builtin_scalbn", "ddi", N | F),
        row(SCALBNF, "__builtin_scalbnf", "ffi", N | F),
        row(SCALBNL, "__builtin_scalbnl", "LdLdi", N | F),
        row(SCALBLN, "__builtin_scalbln", "ddLi", N | F),
        row(SCALBLNF, "__builtin_scalblnf", "ffLi", N | F),
        row(SCALBLNL, "__builtin_scalblnl", "LdLdLi", N | F),
        row(100, "__builtin_acos", "dd", N | F),
        row(98, "__builtin_acosf", "ff", N | F),
        row(108, "__builtin_acosh", "dd", N | F),
        row(106, "__builtin_acoshf", "ff", N | F),
        row(110, "__builtin_acoshl", "LdLd", N | F),
        row(102, "__builtin_acosl", "LdLd", N | F),
        row(135, "__builtin_asin", "dd", N | F),
        row(133, "__builtin_asinf", "ff", N | F),
        row(143, "__builtin_asinh", "dd", N | F),
        row(141, "__builtin_asinhf", "ff", N | F),
        row(145, "__builtin_asinhl", "LdLd", N | F),
        row(137, "__builtin_asinl", "LdLd", N | F),
        row(152, "__builtin_atan", "dd", N | F),
        row(158, "__builtin_atan2", "ddd", N | F),
        row(156, "__builtin_atan2f", "fff", N | F),
        row(160, "__builtin_atan2l", "LdLdLd", N | F),
        row(150, "__builtin_atanf", "ff", N | F),
        row(167, "__builtin_atanh", "dd", N | F),
        row(165, "__builtin_atanhf", "ff", N | F),
        row(169, "__builtin_atanhl", "LdLd", N | F),
        row(154, "__builtin_atanl", "LdLd", N | F),
        row(293, "__builtin_cbrt", "dd", N | C | F),
        row(291, "__builtin_cbrtf", "ff", N | C | F),
        row(295, "__builtin_cbrtl", "LdLd", N | C | F),
        row(378, "__builtin_cos", "dd", N | F),
        row(376, "__builtin_cosf", "ff", N | F),
        row(386, "__builtin_cosh", "dd", N | F),
        row(384, "__builtin_coshf", "ff", N | F),
        row(388, "__builtin_coshl", "LdLd", N | F),
        row(380, "__builtin_cosl", "LdLd", N | F),
        row(489, "__builtin_erf", "dd", N | F),
        row(496, "__builtin_erfc", "dd", N | F),
        row(494, "__builtin_erfcf", "ff", N | F),
        row(498, "__builtin_erfcl", "LdLd", N | F),
        row(487, "__builtin_erff", "ff", N | F),
        row(491, "__builtin_erfl", "LdLd", N | F),
        row(559, "__builtin_fdim", "ddd", N | F),
        row(557, "__builtin_fdimf", "fff", N | F),
        row(561, "__builtin_fdiml", "LdLdLd", N | F),
        row(601, "__builtin_fmod", "ddd", N | F),
        row(599, "__builtin_fmodf", "fff", N | F),
        row(603, "__builtin_fmodl", "LdLdLd", N | F),
        row(658, "__builtin_hypot", "ddd", N | F),
        row(656, "__builtin_hypotf", "fff", N | F),
        row(660, "__builtin_hypotl", "LdLdLd", N | F),
        row(666, "__builtin_ilogb", "id", N | F),
        row(664, "__builtin_ilogbf", "if", N | F),
        row(668, "__builtin_ilogbl", "iLd", N | F),
        row(767, "__builtin_lgamma", "dd", N | F),
        row(765, "__builtin_lgammaf", "ff", N | F),
        row(769, "__builtin_lgammal", "LdLd", N | F),
        row(774, "__builtin_llrint", "LLid", N | F),
        row(772, "__builtin_llrintf", "LLif", N | F),
        row(776, "__builtin_llrintl", "LLiLd", N | F),
        row(780, "__builtin_llround", "LLid", N | F),
        row(778, "__builtin_llroundf", "LLif", N | F),
        row(782, "__builtin_llroundl", "LLiLd", N | F),
        row(788, "__builtin_log", "dd", N | F),
        row(809, "__builtin_log10", "dd", N | F),
        row(807, "__builtin_log10f", "ff", N | F),
        row(811, "__builtin_log10l", "LdLd", N | F),
        row(794, "__builtin_log1p", "dd", N | F),
        row(792, "__builtin_log1pf", "ff", N | F),
        row(796, "__builtin_log1pl", "LdLd", N | F),
        row(801, "__builtin_log2", "dd", N | F),
        row(799, "__builtin_log2f", "ff", N | F),
        row(803, "__builtin_log2l", "LdLd", N | F),
        row(819, "__builtin_logb", "dd", N | F),
        row(817, "__builtin_logbf", "ff", N | F),
        row(821, "__builtin_logbl", "LdLd", N | F),
        row(786, "__builtin_logf", "ff", N | F),
        row(790, "__builtin_logl", "LdLd", N | F),
        row(827, "__builtin_lrint", "Lid", N | F),
        row(825, "__builtin_lrintf", "Lif", N | F),
        row(829, "__builtin_lrintl", "LiLd", N | F),
        row(836, "__builtin_lround", "Lid", N | F),
        row(834, "__builtin_lroundf", "Lif", N | F),
        row(838, "__builtin_lroundl", "LiLd", N | F),
        row(880, "__builtin_modf", "ddd*", N | F),
        row(878, "__builtin_modff", "fff*", N | F),
        row(882, "__builtin_modfl", "LdLdLd*", N | F),
        row(912, "__builtin_nextafter", "ddd", N | F),
        row(910, "__builtin_nextafterf", "fff", N | F),
        row(914, "__builtin_nextafterl", "LdLdLd", N | F),
        row(919, "__builtin_nexttoward", "ddLd", N | F),
        row(917, "__builtin_nexttowardf", "ffLd", N | F),
        row(921, "__builtin_nexttowardl", "LdLdLd", N | F),
        row(1000, "__builtin_remainder", "ddd", N | F),
        row(998, "__builtin_remainderf", "fff", N | F),
        row(1002, "__builtin_remainderl", "LdLdLd", N | F),
        row(1007, "__builtin_remquo", "dddi*", N | F),
        row(1005, "__builtin_remquof", "fffi*", N | F),
        row(1009, "__builtin_remquol", "LdLdLdi*", N | F),
        row(1094, "__builtin_sin", "dd", N | F),
        row(1092, "__builtin_sinf", "ff", N | F),
        row(1102, "__builtin_sinh", "dd", N | F),
        row(1100, "__builtin_sinhf", "ff", N | F),
        row(1104, "__builtin_sinhl", "LdLd", N | F),
        row(1096, "__builtin_sinl", "LdLd", N | F),
        row(1117, "__builtin_sqrt", "dd", N | F),
        row(1115, "__builtin_sqrtf", "ff", N | F),
        row(1119, "__builtin_sqrtl", "LdLd", N | F),
        // WP-393: clang's `__fp16` form (a call of the library `sqrtf16`).
        row(1121, "__builtin_sqrtf16", "hh", N | F),
        row(1302, "__builtin_tan", "dd", N | F),
        row(1300, "__builtin_tanf", "ff", N | F),
        row(1310, "__builtin_tanh", "dd", N | F),
        row(1308, "__builtin_tanhf", "ff", N | F),
        row(1312, "__builtin_tanhl", "LdLd", N | F),
        row(1304, "__builtin_tanl", "LdLd", N | F),
        row(1320, "__builtin_tgamma", "dd", N | F),
        row(1318, "__builtin_tgammaf", "ff", N | F),
        row(1322, "__builtin_tgammal", "LdLd", N | F),
        // C99 <complex.h> (C23 7.3): `_Complex` arguments and results go
        // through the ordinary complex call ABI (spec/abi.md §8.17).
        row(239, "__builtin_cabs", "dXd", N | F),
        row(237, "__builtin_cabsf", "fXf", N | F),
        row(241, "__builtin_cabsl", "LdXLd", N | F),
        row(245, "__builtin_cacos", "XdXd", N | F),
        row(243, "__builtin_cacosf", "XfXf", N | F),
        row(251, "__builtin_cacosh", "XdXd", N | F),
        row(249, "__builtin_cacoshf", "XfXf", N | F),
        row(253, "__builtin_cacoshl", "XLdXLd", N | F),
        row(247, "__builtin_cacosl", "XLdXLd", N | F),
        row(263, "__builtin_carg", "dXd", N | F),
        row(261, "__builtin_cargf", "fXf", N | F),
        row(265, "__builtin_cargl", "LdXLd", N | F),
        row(269, "__builtin_casin", "XdXd", N | F),
        row(267, "__builtin_casinf", "XfXf", N | F),
        row(275, "__builtin_casinh", "XdXd", N | F),
        row(273, "__builtin_casinhf", "XfXf", N | F),
        row(277, "__builtin_casinhl", "XLdXLd", N | F),
        row(271, "__builtin_casinl", "XLdXLd", N | F),
        row(281, "__builtin_catan", "XdXd", N | F),
        row(279, "__builtin_catanf", "XfXf", N | F),
        row(287, "__builtin_catanh", "XdXd", N | F),
        row(285, "__builtin_catanhf", "XfXf", N | F),
        row(289, "__builtin_catanhl", "XLdXLd", N | F),
        row(283, "__builtin_catanl", "XLdXLd", N | F),
        row(300, "__builtin_ccos", "XdXd", N | F),
        row(298, "__builtin_ccosf", "XfXf", N | F),
        row(306, "__builtin_ccosh", "XdXd", N | F),
        row(304, "__builtin_ccoshf", "XfXf", N | F),
        row(308, "__builtin_ccoshl", "XLdXLd", N | F),
        row(302, "__builtin_ccosl", "XLdXLd", N | F),
        row(320, "__builtin_cexp", "XdXd", N | F),
        row(318, "__builtin_cexpf", "XfXf", N | F),
        row(322, "__builtin_cexpl", "XLdXLd", N | F),
        row(333, "__builtin_clog", "XdXd", N | F),
        row(331, "__builtin_clogf", "XfXf", N | F),
        row(335, "__builtin_clogl", "XLdXLd", N | F),
        row(396, "__builtin_cpow", "XdXdXd", N | F),
        row(394, "__builtin_cpowf", "XfXfXf", N | F),
        row(398, "__builtin_cpowl", "XLdXLdXLd", N | F),
        row(402, "__builtin_cproj", "XdXd", N | C | F),
        row(400, "__builtin_cprojf", "XfXf", N | C | F),
        row(404, "__builtin_cprojl", "XLdXLd", N | C | F),
        row(414, "__builtin_csin", "XdXd", N | F),
        row(412, "__builtin_csinf", "XfXf", N | F),
        row(420, "__builtin_csinh", "XdXd", N | F),
        row(418, "__builtin_csinhf", "XfXf", N | F),
        row(422, "__builtin_csinhl", "XLdXLd", N | F),
        row(416, "__builtin_csinl", "XLdXLd", N | F),
        row(426, "__builtin_csqrt", "XdXd", N | F),
        row(424, "__builtin_csqrtf", "XfXf", N | F),
        row(428, "__builtin_csqrtl", "XLdXLd", N | F),
        row(432, "__builtin_ctan", "XdXd", N | F),
        row(430, "__builtin_ctanf", "XfXf", N | F),
        row(438, "__builtin_ctanh", "XdXd", N | F),
        row(436, "__builtin_ctanhf", "XfXf", N | F),
        row(440, "__builtin_ctanhl", "XLdXLd", N | F),
        row(434, "__builtin_ctanl", "XLdXLd", N | F),
    ];

    // ---- WP-154: atomic builtins (a separate block; see `atomic`) ----

    /// The atomic builtins ([atomics], C11 7.17, GCC `__atomic`): the
    /// operations with custom type-checking (`v.`, rebuilt by sema as an
    /// `AtomicExpr`), then the fences, flag operations and lock-freedom
    /// queries, which are ordinary calls.
    pub static ATOMIC: &[Info] = &[
        row(atomic::C11_INIT, "__c11_atomic_init", "v.", T),
        row(atomic::C11_LOAD, "__c11_atomic_load", "v.", T),
        row(atomic::C11_STORE, "__c11_atomic_store", "v.", T),
        row(atomic::C11_EXCHANGE, "__c11_atomic_exchange", "v.", T),
        row(atomic::C11_CMPXCHG_STRONG, "__c11_atomic_compare_exchange_strong", "v.", T),
        row(atomic::C11_CMPXCHG_WEAK, "__c11_atomic_compare_exchange_weak", "v.", T),
        row(atomic::C11_FETCH_ADD, "__c11_atomic_fetch_add", "v.", T),
        row(atomic::C11_FETCH_SUB, "__c11_atomic_fetch_sub", "v.", T),
        row(atomic::C11_FETCH_AND, "__c11_atomic_fetch_and", "v.", T),
        row(atomic::C11_FETCH_OR, "__c11_atomic_fetch_or", "v.", T),
        row(atomic::C11_FETCH_XOR, "__c11_atomic_fetch_xor", "v.", T),
        row(atomic::C11_FETCH_NAND, "__c11_atomic_fetch_nand", "v.", T),
        row(atomic::C11_FETCH_MAX, "__c11_atomic_fetch_max", "v.", T),
        row(atomic::C11_FETCH_MIN, "__c11_atomic_fetch_min", "v.", T),
        row(atomic::LOAD, "__atomic_load", "v.", T),
        row(atomic::LOAD_N, "__atomic_load_n", "v.", T),
        row(atomic::STORE, "__atomic_store", "v.", T),
        row(atomic::STORE_N, "__atomic_store_n", "v.", T),
        row(atomic::EXCHANGE, "__atomic_exchange", "v.", T),
        row(atomic::EXCHANGE_N, "__atomic_exchange_n", "v.", T),
        row(atomic::CMPXCHG, "__atomic_compare_exchange", "v.", T),
        row(atomic::CMPXCHG_N, "__atomic_compare_exchange_n", "v.", T),
        row(atomic::FETCH_ADD, "__atomic_fetch_add", "v.", T),
        row(atomic::FETCH_SUB, "__atomic_fetch_sub", "v.", T),
        row(atomic::FETCH_AND, "__atomic_fetch_and", "v.", T),
        row(atomic::FETCH_OR, "__atomic_fetch_or", "v.", T),
        row(atomic::FETCH_XOR, "__atomic_fetch_xor", "v.", T),
        row(atomic::FETCH_NAND, "__atomic_fetch_nand", "v.", T),
        row(atomic::FETCH_MAX, "__atomic_fetch_max", "v.", T),
        row(atomic::FETCH_MIN, "__atomic_fetch_min", "v.", T),
        row(atomic::ADD_FETCH, "__atomic_add_fetch", "v.", T),
        row(atomic::SUB_FETCH, "__atomic_sub_fetch", "v.", T),
        row(atomic::AND_FETCH, "__atomic_and_fetch", "v.", T),
        row(atomic::OR_FETCH, "__atomic_or_fetch", "v.", T),
        row(atomic::XOR_FETCH, "__atomic_xor_fetch", "v.", T),
        row(atomic::NAND_FETCH, "__atomic_nand_fetch", "v.", T),
        row(atomic::MAX_FETCH, "__atomic_max_fetch", "v.", T),
        row(atomic::MIN_FETCH, "__atomic_min_fetch", "v.", T),
        row(atomic::C11_THREAD_FENCE, "__c11_atomic_thread_fence", "vi", N),
        row(atomic::C11_SIGNAL_FENCE, "__c11_atomic_signal_fence", "vi", N),
        row(atomic::C11_IS_LOCK_FREE, "__c11_atomic_is_lock_free", "bz", N),
        row(atomic::TEST_AND_SET, "__atomic_test_and_set", "bvD*i", N),
        row(atomic::CLEAR, "__atomic_clear", "vvD*i", N),
        row(atomic::THREAD_FENCE, "__atomic_thread_fence", "vi", N),
        row(atomic::SIGNAL_FENCE, "__atomic_signal_fence", "vi", N),
        row(atomic::ALWAYS_LOCK_FREE, "__atomic_always_lock_free", "bzvCD*", N),
        row(atomic::IS_LOCK_FREE, "__atomic_is_lock_free", "bzvCD*", N),
    ];

    /// The atomic builtins' ids (clang 19 `Builtin::ID`s) and the
    /// classification of the `AtomicExpr` operations (clang's
    /// `AtomicExpr::AtomicOp` and `Sema::BuildAtomicExpr`). Every
    /// operation id is below 256: an `AtomicExpr` stores it in `aux`.
    #[allow(missing_docs, reason = "the ids are named after the builtins")]
    pub mod atomic {
        pub const ADD_FETCH: u32 = 1;
        pub const AND_FETCH: u32 = 2;
        pub const CMPXCHG: u32 = 3;
        pub const CMPXCHG_N: u32 = 4;
        pub const EXCHANGE: u32 = 5;
        pub const EXCHANGE_N: u32 = 6;
        pub const FETCH_ADD: u32 = 7;
        pub const FETCH_AND: u32 = 8;
        pub const FETCH_MAX: u32 = 9;
        pub const FETCH_MIN: u32 = 10;
        pub const FETCH_NAND: u32 = 11;
        pub const FETCH_OR: u32 = 12;
        pub const FETCH_SUB: u32 = 13;
        pub const FETCH_XOR: u32 = 14;
        pub const LOAD: u32 = 15;
        pub const LOAD_N: u32 = 16;
        pub const MAX_FETCH: u32 = 17;
        pub const MIN_FETCH: u32 = 18;
        pub const NAND_FETCH: u32 = 19;
        pub const OR_FETCH: u32 = 20;
        pub const STORE: u32 = 21;
        pub const STORE_N: u32 = 22;
        pub const SUB_FETCH: u32 = 23;
        pub const XOR_FETCH: u32 = 24;
        pub const C11_CMPXCHG_STRONG: u32 = 25;
        pub const C11_CMPXCHG_WEAK: u32 = 26;
        pub const C11_EXCHANGE: u32 = 27;
        pub const C11_FETCH_ADD: u32 = 28;
        pub const C11_FETCH_AND: u32 = 29;
        pub const C11_FETCH_MAX: u32 = 30;
        pub const C11_FETCH_MIN: u32 = 31;
        pub const C11_FETCH_NAND: u32 = 32;
        pub const C11_FETCH_OR: u32 = 33;
        pub const C11_FETCH_SUB: u32 = 34;
        pub const C11_FETCH_XOR: u32 = 35;
        pub const C11_INIT: u32 = 36;
        pub const C11_LOAD: u32 = 37;
        pub const C11_STORE: u32 = 38;
        pub const ALWAYS_LOCK_FREE: u32 = 172;
        pub const CLEAR: u32 = 173;
        pub const IS_LOCK_FREE: u32 = 174;
        pub const SIGNAL_FENCE: u32 = 175;
        pub const TEST_AND_SET: u32 = 176;
        pub const THREAD_FENCE: u32 = 177;
        pub const C11_IS_LOCK_FREE: u32 = 234;
        pub const C11_SIGNAL_FENCE: u32 = 235;
        pub const C11_THREAD_FENCE: u32 = 236;

        /// The argument shape of an operation (clang's `BuildAtomicExpr`
        /// `Form`).
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Form {
            Init,
            /// `(ptr, order)`.
            Load,
            /// `__atomic_load(ptr, ret, order)`.
            LoadCopy,
            /// A store: `(ptr, val, order)`.
            Copy,
            /// A read-modify-write: `(ptr, val, order)`.
            Arithmetic,
            /// `(ptr, val, order)`, returning the old value.
            Xchg,
            /// `__atomic_exchange(ptr, val, ret, order)`.
            GnuXchg,
            /// `(ptr, expected, desired, success, failure)`.
            C11CmpXchg,
            /// `(ptr, expected, desired, weak, success, failure)`.
            GnuCmpXchg,
        }

        impl Form {
            /// The number of arguments (clang's `NumArgs`).
            pub fn num_args(self) -> usize {
                match self {
                    Form::Init | Form::Load => 2,
                    Form::LoadCopy | Form::Copy | Form::Arithmetic | Form::Xchg => 3,
                    Form::GnuXchg => 4,
                    Form::C11CmpXchg => 5,
                    Form::GnuCmpXchg => 6,
                }
            }

            /// The number of value operands (clang's `NumVals`).
            pub fn num_vals(self) -> usize {
                match self {
                    Form::Load => 0,
                    Form::Init | Form::LoadCopy | Form::Copy | Form::Arithmetic | Form::Xchg => 1,
                    Form::GnuXchg | Form::C11CmpXchg => 2,
                    Form::GnuCmpXchg => 3,
                }
            }

            /// The number of operands of the `AtomicExpr` (clang's
            /// `getNumSubExprs`): `ptr`, the orders, the values, `weak`.
            pub fn num_sub_exprs(self) -> usize {
                self.num_args()
            }

            /// For each argument in source order, the index of the
            /// `AtomicExpr` operand it became (the inverse of clang's
            /// `SubExprs` permutation).
            pub fn arg_to_sub(self) -> &'static [usize] {
                match self {
                    Form::Init | Form::Load => &[0, 1],
                    Form::LoadCopy | Form::Copy | Form::Arithmetic | Form::Xchg => &[0, 2, 1],
                    Form::GnuXchg => &[0, 2, 3, 1],
                    Form::C11CmpXchg => &[0, 2, 4, 1, 3],
                    Form::GnuCmpXchg => &[0, 2, 4, 5, 1, 3],
                }
            }
        }

        /// The read-modify-write operation of an `Arithmetic` form.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Rmw {
            Add,
            Sub,
            And,
            Or,
            Xor,
            Nand,
            Max,
            Min,
        }

        /// Whether `id` is an `AtomicExpr` operation.
        pub fn is_op(id: u32) -> bool {
            (1..=38).contains(&id)
        }

        /// The form of operation `id`.
        pub fn form(id: u32) -> Option<Form> {
            Some(match id {
                C11_INIT => Form::Init,
                C11_LOAD | LOAD_N => Form::Load,
                LOAD => Form::LoadCopy,
                C11_STORE | STORE | STORE_N => Form::Copy,
                C11_EXCHANGE | EXCHANGE_N => Form::Xchg,
                EXCHANGE => Form::GnuXchg,
                C11_CMPXCHG_STRONG | C11_CMPXCHG_WEAK => Form::C11CmpXchg,
                CMPXCHG | CMPXCHG_N => Form::GnuCmpXchg,
                _ if rmw(id).is_some() => Form::Arithmetic,
                _ => return None,
            })
        }

        /// The read-modify-write of operation `id`, and whether it yields
        /// the new value (`__atomic_<op>_fetch`) rather than the old.
        pub fn rmw(id: u32) -> Option<(Rmw, bool)> {
            Some(match id {
                C11_FETCH_ADD | FETCH_ADD => (Rmw::Add, false),
                C11_FETCH_SUB | FETCH_SUB => (Rmw::Sub, false),
                C11_FETCH_AND | FETCH_AND => (Rmw::And, false),
                C11_FETCH_OR | FETCH_OR => (Rmw::Or, false),
                C11_FETCH_XOR | FETCH_XOR => (Rmw::Xor, false),
                C11_FETCH_NAND | FETCH_NAND => (Rmw::Nand, false),
                C11_FETCH_MAX | FETCH_MAX => (Rmw::Max, false),
                C11_FETCH_MIN | FETCH_MIN => (Rmw::Min, false),
                ADD_FETCH => (Rmw::Add, true),
                SUB_FETCH => (Rmw::Sub, true),
                AND_FETCH => (Rmw::And, true),
                OR_FETCH => (Rmw::Or, true),
                XOR_FETCH => (Rmw::Xor, true),
                NAND_FETCH => (Rmw::Nand, true),
                MAX_FETCH => (Rmw::Max, true),
                MIN_FETCH => (Rmw::Min, true),
                _ => return None,
            })
        }

        /// Whether operation `id` is a `__c11_atomic_*` one (its pointer
        /// operand points to an `_Atomic` type).
        pub fn is_c11(id: u32) -> bool {
            (25..=38).contains(&id)
        }

        /// Whether operation `id` is a GNU `_n` form or a C11 form (values
        /// passed by value; the other GNU forms pass them by address).
        pub fn by_value(id: u32) -> bool {
            !matches!(id, LOAD | STORE | EXCHANGE | CMPXCHG)
        }

        /// The clang spelling of operation `id` without its leading `__`
        /// (the `AtomicExpr` dump does not print it; diagnostics may).
        pub fn name(id: u32) -> &'static str {
            super::info(id).map_or("", |i| i.name)
        }

        /// Whether `id` is a `__c11_atomic_compare_exchange_weak` (weak
        /// by name; the GNU forms carry a `weak` operand).
        pub fn is_c11_weak(id: u32) -> bool {
            id == C11_CMPXCHG_WEAK
        }
    }

    // ---- end WP-154 ----

    // ---- the GCC legacy `__sync` builtins (a separate block; see `sync`) ----

    /// The GCC legacy `__sync` builtins (clang `Builtins.td`, ids clang
    /// 19's): each overloaded operation as written (`v.`, custom
    /// type-checking), then its `_1`, `_2`, `_4`, `_8` and `_16` forms
    /// (also custom type-checked: sema retargets a call to the form for
    /// the pointee's size, clang `BuiltinAtomicOverloaded`), the clang
    /// `int`/`unsigned` min/max extensions and `__sync_synchronize`.
    pub static SYNC: &[Info] = &[
        row(1193, "__sync_lock_test_and_set_1", "ccD*c.", T | N),
        row(1194, "__sync_lock_test_and_set_2", "ssD*s.", T | N),
        row(1195, "__sync_lock_test_and_set_4", "iiD*i.", T | N),
        row(1196, "__sync_lock_test_and_set_8", "LLiLLiD*LLi.", T | N),
        row(1197, "__sync_lock_test_and_set_16", "LLLiLLLiD*LLLi.", T | N),
        row(1198, "__sync_val_compare_and_swap_1", "ccD*cc.", T | N),
        row(1199, "__sync_val_compare_and_swap_2", "ssD*ss.", T | N),
        row(1200, "__sync_val_compare_and_swap_4", "iiD*ii.", T | N),
        row(1201, "__sync_val_compare_and_swap_8", "LLiLLiD*LLiLLi.", T | N),
        row(1202, "__sync_val_compare_and_swap_16", "LLLiLLLiD*LLLiLLLi.", T | N),
        row(1203, "__sync_add_and_fetch", "v.", T),
        row(1204, "__sync_add_and_fetch_1", "ccD*c.", T | N),
        row(1205, "__sync_add_and_fetch_2", "ssD*s.", T | N),
        row(1206, "__sync_add_and_fetch_4", "iiD*i.", T | N),
        row(1207, "__sync_add_and_fetch_8", "LLiLLiD*LLi.", T | N),
        row(1208, "__sync_add_and_fetch_16", "LLLiLLLiD*LLLi.", T | N),
        row(1209, "__sync_and_and_fetch", "v.", T),
        row(1210, "__sync_and_and_fetch_1", "ccD*c.", T | N),
        row(1211, "__sync_and_and_fetch_2", "ssD*s.", T | N),
        row(1212, "__sync_and_and_fetch_4", "iiD*i.", T | N),
        row(1213, "__sync_and_and_fetch_8", "LLiLLiD*LLi.", T | N),
        row(1214, "__sync_and_and_fetch_16", "LLLiLLLiD*LLLi.", T | N),
        row(1215, "__sync_bool_compare_and_swap", "v.", T),
        row(1216, "__sync_bool_compare_and_swap_1", "bcD*cc.", T | N),
        row(1217, "__sync_bool_compare_and_swap_2", "bsD*ss.", T | N),
        row(1218, "__sync_bool_compare_and_swap_4", "biD*ii.", T | N),
        row(1219, "__sync_bool_compare_and_swap_8", "bLLiD*LLiLLi.", T | N),
        row(1220, "__sync_bool_compare_and_swap_16", "bLLLiD*LLLiLLLi.", T | N),
        row(1221, "__sync_fetch_and_add", "v.", T),
        row(1222, "__sync_fetch_and_add_1", "ccD*c.", T | N),
        row(1223, "__sync_fetch_and_add_2", "ssD*s.", T | N),
        row(1224, "__sync_fetch_and_add_4", "iiD*i.", T | N),
        row(1225, "__sync_fetch_and_add_8", "LLiLLiD*LLi.", T | N),
        row(1226, "__sync_fetch_and_add_16", "LLLiLLLiD*LLLi.", T | N),
        row(1227, "__sync_fetch_and_and", "v.", T),
        row(1228, "__sync_fetch_and_and_1", "ccD*c.", T | N),
        row(1229, "__sync_fetch_and_and_2", "ssD*s.", T | N),
        row(1230, "__sync_fetch_and_and_4", "iiD*i.", T | N),
        row(1231, "__sync_fetch_and_and_8", "LLiLLiD*LLi.", T | N),
        row(1232, "__sync_fetch_and_and_16", "LLLiLLLiD*LLLi.", T | N),
        row(1233, "__sync_fetch_and_max", "iiD*i", N),
        row(1234, "__sync_fetch_and_min", "iiD*i", N),
        row(1235, "__sync_fetch_and_nand", "v.", T),
        row(1236, "__sync_fetch_and_nand_1", "ccD*c.", T | N),
        row(1237, "__sync_fetch_and_nand_2", "ssD*s.", T | N),
        row(1238, "__sync_fetch_and_nand_4", "iiD*i.", T | N),
        row(1239, "__sync_fetch_and_nand_8", "LLiLLiD*LLi.", T | N),
        row(1240, "__sync_fetch_and_nand_16", "LLLiLLLiD*LLLi.", T | N),
        row(1241, "__sync_fetch_and_or", "v.", T),
        row(1242, "__sync_fetch_and_or_1", "ccD*c.", T | N),
        row(1243, "__sync_fetch_and_or_2", "ssD*s.", T | N),
        row(1244, "__sync_fetch_and_or_4", "iiD*i.", T | N),
        row(1245, "__sync_fetch_and_or_8", "LLiLLiD*LLi.", T | N),
        row(1246, "__sync_fetch_and_or_16", "LLLiLLLiD*LLLi.", T | N),
        row(1247, "__sync_fetch_and_sub", "v.", T),
        row(1248, "__sync_fetch_and_sub_1", "ccD*c.", T | N),
        row(1249, "__sync_fetch_and_sub_2", "ssD*s.", T | N),
        row(1250, "__sync_fetch_and_sub_4", "iiD*i.", T | N),
        row(1251, "__sync_fetch_and_sub_8", "LLiLLiD*LLi.", T | N),
        row(1252, "__sync_fetch_and_sub_16", "LLLiLLLiD*LLLi.", T | N),
        row(1253, "__sync_fetch_and_umax", "UiUiD*Ui", N),
        row(1254, "__sync_fetch_and_umin", "UiUiD*Ui", N),
        row(1255, "__sync_fetch_and_xor", "v.", T),
        row(1256, "__sync_fetch_and_xor_1", "ccD*c.", T | N),
        row(1257, "__sync_fetch_and_xor_2", "ssD*s.", T | N),
        row(1258, "__sync_fetch_and_xor_4", "iiD*i.", T | N),
        row(1259, "__sync_fetch_and_xor_8", "LLiLLiD*LLi.", T | N),
        row(1260, "__sync_fetch_and_xor_16", "LLLiLLLiD*LLLi.", T | N),
        row(1261, "__sync_lock_release", "v.", T),
        row(1262, "__sync_lock_release_1", "vcD*.", T | N),
        row(1263, "__sync_lock_release_2", "vsD*.", T | N),
        row(1264, "__sync_lock_release_4", "viD*.", T | N),
        row(1265, "__sync_lock_release_8", "vLLiD*.", T | N),
        row(1266, "__sync_lock_release_16", "vLLLiD*.", T | N),
        row(1267, "__sync_lock_test_and_set", "v.", T),
        row(1268, "__sync_nand_and_fetch", "v.", T),
        row(1269, "__sync_nand_and_fetch_1", "ccD*c.", T | N),
        row(1270, "__sync_nand_and_fetch_2", "ssD*s.", T | N),
        row(1271, "__sync_nand_and_fetch_4", "iiD*i.", T | N),
        row(1272, "__sync_nand_and_fetch_8", "LLiLLiD*LLi.", T | N),
        row(1273, "__sync_nand_and_fetch_16", "LLLiLLLiD*LLLi.", T | N),
        row(1274, "__sync_or_and_fetch", "v.", T),
        row(1275, "__sync_or_and_fetch_1", "ccD*c.", T | N),
        row(1276, "__sync_or_and_fetch_2", "ssD*s.", T | N),
        row(1277, "__sync_or_and_fetch_4", "iiD*i.", T | N),
        row(1278, "__sync_or_and_fetch_8", "LLiLLiD*LLi.", T | N),
        row(1279, "__sync_or_and_fetch_16", "LLLiLLLiD*LLLi.", T | N),
        row(1280, "__sync_sub_and_fetch", "v.", T),
        row(1281, "__sync_sub_and_fetch_1", "ccD*c.", T | N),
        row(1282, "__sync_sub_and_fetch_2", "ssD*s.", T | N),
        row(1283, "__sync_sub_and_fetch_4", "iiD*i.", T | N),
        row(1284, "__sync_sub_and_fetch_8", "LLiLLiD*LLi.", T | N),
        row(1285, "__sync_sub_and_fetch_16", "LLLiLLLiD*LLLi.", T | N),
        row(1286, "__sync_swap", "v.", T),
        row(1287, "__sync_swap_1", "ccD*c.", T | N),
        row(1288, "__sync_swap_2", "ssD*s.", T | N),
        row(1289, "__sync_swap_4", "iiD*i.", T | N),
        row(1290, "__sync_swap_8", "LLiLLiD*LLi.", T | N),
        row(1291, "__sync_swap_16", "LLLiLLLiD*LLLi.", T | N),
        row(1292, "__sync_synchronize", "v", N),
        row(1293, "__sync_val_compare_and_swap", "v.", T),
        row(1294, "__sync_xor_and_fetch", "v.", T),
        row(1295, "__sync_xor_and_fetch_1", "ccD*c.", T | N),
        row(1296, "__sync_xor_and_fetch_2", "ssD*s.", T | N),
        row(1297, "__sync_xor_and_fetch_4", "iiD*i.", T | N),
        row(1298, "__sync_xor_and_fetch_8", "LLiLLiD*LLi.", T | N),
        row(1299, "__sync_xor_and_fetch_16", "LLLiLLLiD*LLLi.", T | N),
    ];

    /// The overloaded `__sync` operations ([`SYNC`]): their families, the
    /// `_N` form for a size, and the operands each takes (clang
    /// `Sema::BuiltinAtomicOverloaded`, `CodeGenFunction::EmitBuiltinExpr`).
    pub mod sync {
        use super::atomic::Rmw;
        use super::{by_name, info};

        /// `void __sync_synchronize() noexcept`: a `seq_cst` fence.
        pub const SYNCHRONIZE: u32 = 1292;
        /// `int __sync_fetch_and_max(volatile int *, int) noexcept`.
        pub const FETCH_AND_MAX: u32 = 1233;
        /// `int __sync_fetch_and_min(volatile int *, int) noexcept`.
        pub const FETCH_AND_MIN: u32 = 1234;
        /// `unsigned __sync_fetch_and_umax(volatile unsigned *, unsigned)`.
        pub const FETCH_AND_UMAX: u32 = 1253;
        /// `unsigned __sync_fetch_and_umin(volatile unsigned *, unsigned)`.
        pub const FETCH_AND_UMIN: u32 = 1254;

        /// An overloaded `__sync` operation.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Op {
            /// `__sync_fetch_and_<op>`: the old value.
            Fetch(Rmw),
            /// `__sync_<op>_and_fetch`: the new value.
            Post(Rmw),
            /// `__sync_val_compare_and_swap`: the old value.
            ValCas,
            /// `__sync_bool_compare_and_swap`: whether it swapped.
            BoolCas,
            /// `__sync_lock_test_and_set`: an exchange.
            LockTestAndSet,
            /// `__sync_lock_release`: a release store of 0.
            LockRelease,
            /// `__sync_swap`: an exchange.
            Swap,
        }

        /// The families, by name after `__sync_`, in clang's
        /// `BuiltinIndices` order.
        const FAMILIES: &[(&str, Op)] = &[
            ("fetch_and_add", Op::Fetch(Rmw::Add)),
            ("fetch_and_sub", Op::Fetch(Rmw::Sub)),
            ("fetch_and_or", Op::Fetch(Rmw::Or)),
            ("fetch_and_and", Op::Fetch(Rmw::And)),
            ("fetch_and_xor", Op::Fetch(Rmw::Xor)),
            ("fetch_and_nand", Op::Fetch(Rmw::Nand)),
            ("add_and_fetch", Op::Post(Rmw::Add)),
            ("sub_and_fetch", Op::Post(Rmw::Sub)),
            ("and_and_fetch", Op::Post(Rmw::And)),
            ("or_and_fetch", Op::Post(Rmw::Or)),
            ("xor_and_fetch", Op::Post(Rmw::Xor)),
            ("nand_and_fetch", Op::Post(Rmw::Nand)),
            ("val_compare_and_swap", Op::ValCas),
            ("bool_compare_and_swap", Op::BoolCas),
            ("lock_test_and_set", Op::LockTestAndSet),
            ("lock_release", Op::LockRelease),
            ("swap", Op::Swap),
        ];

        /// The operation builtin `id` performs and, for an `_N` form, N
        /// (`None` for the overloaded spelling).
        pub fn op(id: u32) -> Option<(Op, Option<u32>)> {
            let name = info(id)?.name.strip_prefix("__sync_")?;
            FAMILIES.iter().find_map(|&(f, op)| {
                let rest = name.strip_prefix(f)?;
                if rest.is_empty() {
                    return Some((op, None));
                }
                let n = rest.strip_prefix('_')?.parse().ok()?;
                Some((op, Some(n)))
            })
        }

        /// The `_N` form of the operation of `id` for an `n`-byte value
        /// (`None` unless `n` is 1, 2, 4, 8 or 16).
        pub fn sized(id: u32, n: u64) -> Option<u32> {
            if !matches!(n, 1 | 2 | 4 | 8 | 16) {
                return None;
            }
            let name = info(id)?.name;
            let base = match op(id)? {
                (_, None) => name,
                (_, Some(_)) => &name[..name.rfind('_')?],
            };
            by_name(&format!("{base}_{n}"))
        }

        /// The value operands after the pointer (clang's `NumFixed`).
        pub fn num_fixed(op: Op) -> usize {
            match op {
                Op::ValCas | Op::BoolCas => 2,
                Op::LockRelease => 0,
                _ => 1,
            }
        }
    }

    // ---- end `__sync` ----

    // ---- WP-270: checked arithmetic, alloca, printf-family builtins ----

    /// `bool __builtin_add_overflow(...) noexcept` (custom type-checking:
    /// two integer operands and a pointer to a non-const integer result;
    /// clang `BuiltinOverflow`).
    pub const ADD_OVERFLOW: u32 = 113;
    /// `__builtin_sub_overflow` (as [`ADD_OVERFLOW`]).
    pub const SUB_OVERFLOW: u32 = 1187;
    /// `__builtin_mul_overflow` (as [`ADD_OVERFLOW`]).
    pub const MUL_OVERFLOW: u32 = 887;
    /// `__builtin_sadd_overflow` (`biCiCi*`).
    pub const SADD_OVERFLOW: u32 = 1062;
    /// `__builtin_saddl_overflow` (`bLiCLiCLi*`).
    pub const SADDL_OVERFLOW: u32 = 1063;
    /// `__builtin_saddll_overflow` (`bLLiCLLiCLLi*`).
    pub const SADDLL_OVERFLOW: u32 = 1064;
    /// `__builtin_uadd_overflow` (`bUiCUiCUi*`).
    pub const UADD_OVERFLOW: u32 = 1340;
    /// `__builtin_uaddl_overflow` (`bULiCULiCULi*`).
    pub const UADDL_OVERFLOW: u32 = 1341;
    /// `__builtin_uaddll_overflow` (`bULLiCULLiCULLi*`).
    pub const UADDLL_OVERFLOW: u32 = 1342;
    /// `__builtin_ssub_overflow` (`biCiCi*`).
    pub const SSUB_OVERFLOW: u32 = 1123;
    /// `__builtin_ssubl_overflow` (`bLiCLiCLi*`).
    pub const SSUBL_OVERFLOW: u32 = 1124;
    /// `__builtin_ssubll_overflow` (`bLLiCLLiCLLi*`).
    pub const SSUBLL_OVERFLOW: u32 = 1125;
    /// `__builtin_usub_overflow` (`bUiCUiCUi*`).
    pub const USUB_OVERFLOW: u32 = 1349;
    /// `__builtin_usubl_overflow` (`bULiCULiCULi*`).
    pub const USUBL_OVERFLOW: u32 = 1350;
    /// `__builtin_usubll_overflow` (`bULLiCULLiCULLi*`).
    pub const USUBLL_OVERFLOW: u32 = 1351;
    /// `__builtin_smul_overflow` (`biCiCi*`).
    pub const SMUL_OVERFLOW: u32 = 1110;
    /// `__builtin_smull_overflow` (`bLiCLiCLi*`).
    pub const SMULL_OVERFLOW: u32 = 1111;
    /// `__builtin_smulll_overflow` (`bLLiCLLiCLLi*`).
    pub const SMULLL_OVERFLOW: u32 = 1112;
    /// `__builtin_umul_overflow` (`bUiCUiCUi*`).
    pub const UMUL_OVERFLOW: u32 = 1343;
    /// `__builtin_umull_overflow` (`bULiCULiCULi*`).
    pub const UMULL_OVERFLOW: u32 = 1344;
    /// `__builtin_umulll_overflow` (`bULLiCULLiCULLi*`).
    pub const UMULLL_OVERFLOW: u32 = 1345;
    // WP-557: the multiprecision carry builtins (clang "Multiprecision
    // Arithmetic Builtins"; kiln-private ids): `T f(T x, T y, T carryin,
    // T *carryout)` for `T` unsigned char/short/int/long/long long.
    /// `__builtin_addcb` (`UcUcUcUcUc*`).
    pub const ADDCB: u32 = 2160;
    /// `__builtin_addcs` (`UsUsUsUsUs*`).
    pub const ADDCS: u32 = 2161;
    /// `__builtin_addc` (`UiUiUiUiUi*`).
    pub const ADDC: u32 = 2162;
    /// `__builtin_addcl` (`ULiULiULiULiULi*`).
    pub const ADDCL: u32 = 2163;
    /// `__builtin_addcll` (`ULLiULLiULLiULLiULLi*`).
    pub const ADDCLL: u32 = 2164;
    /// `__builtin_subcb` (as [`ADDCB`]).
    pub const SUBCB: u32 = 2165;
    /// `__builtin_subcs` (as [`ADDCS`]).
    pub const SUBCS: u32 = 2166;
    /// `__builtin_subc` (as [`ADDC`]).
    pub const SUBC: u32 = 2167;
    /// `__builtin_subcl` (as [`ADDCL`]).
    pub const SUBCL: u32 = 2168;
    /// `__builtin_subcll` (as [`ADDCLL`]).
    pub const SUBCLL: u32 = 2169;
    /// `void *__builtin_alloca(size_t) noexcept` (`v*z`).
    pub const ALLOCA: u32 = 123;
    /// `void *__builtin_alloca_with_align(size_t, size_t) noexcept`
    /// (`v*zIz`; the alignment is in bits, a constant power of 2 in
    /// [8, 2^31 - 1]).
    pub const ALLOCA_WITH_ALIGN: u32 = 127;
    /// `__builtin_vsnprintf` (`ic*RzcC*Ra`; lowered to `vsnprintf`).
    pub const VSNPRINTF: u32 = 1375;
    /// `__builtin_snprintf` (`ic*RzcC*R.`; lowered to `snprintf`).
    pub const SNPRINTF: u32 = 1113;
    /// `__builtin_sprintf` (`ic*RcC*R.`; lowered to `sprintf`).
    pub const SPRINTF: u32 = 1057;
    /// WP-841: the rest of clang 19's `printf` / `scanf` family
    /// (`Builtins.td`; the `FILE` forms `__builtin_fprintf`,
    /// `__builtin_vfprintf`, `__builtin_fscanf` and `__builtin_vfscanf`
    /// are WP-1104's [`LIBC_FWD`] rows).
    /// Each is lowered to the library function. The ids are kiln-private.
    /// `__builtin_printf` (`icC*R.`; `format(printf, 1, 2)`).
    pub const PRINTF: u32 = 2380;
    /// `__builtin_vprintf` (`icC*Ra`; `format(printf, 1, 0)`).
    pub const VPRINTF: u32 = 2381;
    /// `__builtin_vsprintf` (`ic*RcC*Ra`; `format(printf, 2, 0)`).
    pub const VSPRINTF: u32 = 2382;
    /// `__builtin_scanf` (`icC*R.`; not `NoThrow`).
    pub const SCANF: u32 = 2383;
    /// `__builtin_sscanf` (`icC*RcC*R.`; not `NoThrow`).
    pub const SSCANF: u32 = 2384;
    /// `__builtin_vscanf` (`icC*Ra`; not `NoThrow`).
    pub const VSCANF: u32 = 2385;
    /// `__builtin_vsscanf` (`icC*RcC*Ra`; not `NoThrow`).
    pub const VSSCANF: u32 = 2386;
    /// `unsigned long long __builtin_readcyclecounter()` (clang
    /// `NoThrow`, every target; WP-1057): the processor's cycle counter,
    /// `llvm.readcyclecounter`; never a constant. Kiln-private id.
    pub const READCYCLECOUNTER: u32 = 2387;

    /// WP-270: the checked-arithmetic builtins (clang 19 `Builtins.td`
    /// `__builtin_*_overflow`; all constant-evaluated), WP-557's carry
    /// builtins (constant-evaluated), `alloca` and the
    /// `printf`-family library builtins libstdc++ names (lowered to the
    /// libc function; never constant-evaluated). `R` is `restrict`.
    pub static CHECKED: &[Info] = &[
        row(ADD_OVERFLOW, "__builtin_add_overflow", "b.", N | T),
        row(SUB_OVERFLOW, "__builtin_sub_overflow", "b.", N | T),
        row(MUL_OVERFLOW, "__builtin_mul_overflow", "b.", N | T),
        row(SADD_OVERFLOW, "__builtin_sadd_overflow", "biCiCi*", N),
        row(SADDL_OVERFLOW, "__builtin_saddl_overflow", "bLiCLiCLi*", N),
        row(SADDLL_OVERFLOW, "__builtin_saddll_overflow", "bLLiCLLiCLLi*", N),
        row(UADD_OVERFLOW, "__builtin_uadd_overflow", "bUiCUiCUi*", N),
        row(UADDL_OVERFLOW, "__builtin_uaddl_overflow", "bULiCULiCULi*", N),
        row(UADDLL_OVERFLOW, "__builtin_uaddll_overflow", "bULLiCULLiCULLi*", N),
        row(SSUB_OVERFLOW, "__builtin_ssub_overflow", "biCiCi*", N),
        row(SSUBL_OVERFLOW, "__builtin_ssubl_overflow", "bLiCLiCLi*", N),
        row(SSUBLL_OVERFLOW, "__builtin_ssubll_overflow", "bLLiCLLiCLLi*", N),
        row(USUB_OVERFLOW, "__builtin_usub_overflow", "bUiCUiCUi*", N),
        row(USUBL_OVERFLOW, "__builtin_usubl_overflow", "bULiCULiCULi*", N),
        row(USUBLL_OVERFLOW, "__builtin_usubll_overflow", "bULLiCULLiCULLi*", N),
        row(SMUL_OVERFLOW, "__builtin_smul_overflow", "biCiCi*", N),
        row(SMULL_OVERFLOW, "__builtin_smull_overflow", "bLiCLiCLi*", N),
        row(SMULLL_OVERFLOW, "__builtin_smulll_overflow", "bLLiCLLiCLLi*", N),
        row(UMUL_OVERFLOW, "__builtin_umul_overflow", "bUiCUiCUi*", N),
        row(UMULL_OVERFLOW, "__builtin_umull_overflow", "bULiCULiCULi*", N),
        row(UMULLL_OVERFLOW, "__builtin_umulll_overflow", "bULLiCULLiCULLi*", N),
        row(ADDCB, "__builtin_addcb", "UcUcUcUcUc*", N),
        row(ADDCS, "__builtin_addcs", "UsUsUsUsUs*", N),
        row(ADDC, "__builtin_addc", "UiUiUiUiUi*", N),
        row(ADDCL, "__builtin_addcl", "ULiULiULiULiULi*", N),
        row(ADDCLL, "__builtin_addcll", "ULLiULLiULLiULLiULLi*", N),
        row(SUBCB, "__builtin_subcb", "UcUcUcUcUc*", N),
        row(SUBCS, "__builtin_subcs", "UsUsUsUsUs*", N),
        row(SUBC, "__builtin_subc", "UiUiUiUiUi*", N),
        row(SUBCL, "__builtin_subcl", "ULiULiULiULiULi*", N),
        row(SUBCLL, "__builtin_subcll", "ULLiULLiULLiULLiULLi*", N),
        row(ALLOCA, "__builtin_alloca", "v*z", N | F),
        row(ALLOCA_WITH_ALIGN, "__builtin_alloca_with_align", "v*zIz", N | F),
        row(VSNPRINTF, "__builtin_vsnprintf", "ic*RzcC*Ra", N | F),
        row(SNPRINTF, "__builtin_snprintf", "ic*RzcC*R.", N | F),
        row(SPRINTF, "__builtin_sprintf", "ic*RcC*R.", N | F),
        row(PRINTF, "__builtin_printf", "icC*R.", N | F),
        row(VPRINTF, "__builtin_vprintf", "icC*Ra", N | F),
        row(VSPRINTF, "__builtin_vsprintf", "ic*RcC*Ra", N | F),
        row(SCANF, "__builtin_scanf", "icC*R.", F),
        row(SSCANF, "__builtin_sscanf", "icC*RcC*R.", F),
        row(VSCANF, "__builtin_vscanf", "icC*Ra", F),
        row(VSSCANF, "__builtin_vsscanf", "icC*RcC*Ra", F),
    ];

    /// The operation and signedness of a checked-arithmetic builtin:
    /// `(op, typed)` with `op` 0 add, 1 sub, 2 mul; `typed` is false for
    /// the generic forms.
    pub fn checked_op(id: u32) -> Option<(u8, bool)> {
        Some(match id {
            ADD_OVERFLOW => (0, false),
            SUB_OVERFLOW => (1, false),
            MUL_OVERFLOW => (2, false),
            SADD_OVERFLOW | SADDL_OVERFLOW | SADDLL_OVERFLOW | UADD_OVERFLOW | UADDL_OVERFLOW
            | UADDLL_OVERFLOW => (0, true),
            SSUB_OVERFLOW | SSUBL_OVERFLOW | SSUBLL_OVERFLOW | USUB_OVERFLOW | USUBL_OVERFLOW
            | USUBLL_OVERFLOW => (1, true),
            SMUL_OVERFLOW | SMULL_OVERFLOW | SMULLL_OVERFLOW | UMUL_OVERFLOW | UMULL_OVERFLOW
            | UMULLL_OVERFLOW => (2, true),
            _ => return None,
        })
    }

    /// Whether builtin `id` is a carry builtin (WP-557): `Some(false)` for
    /// `__builtin_addc*` (`x + y + carryin`), `Some(true)` for
    /// `__builtin_subc*` (`x - y - carryin`).
    pub fn carry_op(id: u32) -> Option<bool> {
        match id {
            ADDCB | ADDCS | ADDC | ADDCL | ADDCLL => Some(false),
            SUBCB | SUBCS | SUBC | SUBCL | SUBCLL => Some(true),
            _ => None,
        }
    }

    /// The `printf` format-attribute indices of a builtin (clang 19
    /// `Builtins.td` `PrintfFormat<N>` / `VPrintfFormat<N>`, added by
    /// `Sema::AddKnownFunctionAttributes` as an implicit
    /// `format(printf, F, A)`): one-based `(F, A)`, `A` = 0 when the data
    /// arguments are a `va_list` (WP-282).
    pub fn printf_format(id: u32) -> Option<(u16, u16)> {
        Some(match id {
            // WP-803: the `__*_chk` rows are clang `PrintfFormat` /
            // `VPrintfFormat` indices + 1.
            PRINTF => (1, 2),
            VPRINTF => (1, 0),
            SPRINTF | PRINTF_CHK | FPRINTF => (2, 3),
            SNPRINTF | FPRINTF_CHK => (3, 4),
            VSNPRINTF | VFPRINTF_CHK => (3, 0),
            SPRINTF_CHK => (4, 5),
            SNPRINTF_CHK => (5, 6),
            VSPRINTF_CHK => (4, 0),
            VSNPRINTF_CHK => (5, 0),
            VSPRINTF | VPRINTF_CHK | VFPRINTF => (2, 0),
            _ => return None,
        })
    }

    // ---- end WP-270 ----

    // ---- WP-282 ----

    /// `void *__builtin_assume_aligned(const void *, size_t, ...)`
    /// (custom-checked: 2 or 3 arguments, the alignment a constant power
    /// of 2; clang `Sema::BuiltinAssumeAligned`).
    pub const ASSUME_ALIGNED: u32 = 201;
    /// `size_t __builtin_object_size(const void *, int)` (the type a
    /// constant in [0, 3]).
    pub const OBJECT_SIZE: u32 = 957;
    /// `size_t __builtin_dynamic_object_size(const void *, int)`.
    pub const DYNAMIC_OBJECT_SIZE: u32 = 451;
    /// `double __builtin_powi(double, int)`.
    pub const POWI: u32 = 974;
    /// `float __builtin_powif(float, int)`.
    pub const POWIF: u32 = 973;
    /// `long double __builtin_powil(long double, int)`.
    pub const POWIL: u32 = 975;
    /// `__builtin_shufflevector(v1, v2, i...)` (custom-checked; a
    /// `ShuffleVectorExpr`, clang `Sema::BuiltinShuffleVector`).
    pub const SHUFFLEVECTOR: u32 = 1085;
    /// `[[noreturn]] void __builtin_verbose_trap(const char *, const char *)`
    /// (both arguments string literals).
    pub const VERBOSE_TRAP: u32 = 1370;
    /// `__fp16 __builtin_copysignf16(__fp16, __fp16)`.
    pub const COPYSIGNF16: u32 = 360;
    /// `__builtin_ceilf128`.
    pub const CEILF128: u32 = 317;
    /// `__builtin_copysignf128`.
    pub const COPYSIGNF128: u32 = 361;
    /// `__builtin_fabsf128`.
    pub const FABSF128: u32 = 555;
    /// `__builtin_floorf128`.
    pub const FLOORF128: u32 = 574;
    /// `__builtin_fmaxf128`.
    pub const FMAXF128: u32 = 590;
    /// `__builtin_fminf128`.
    pub const FMINF128: u32 = 598;
    /// WP-419: `__builtin_fmaximum_numf128` (clang 22).
    pub const FMAXIMUM_NUMF128: u32 = 2060;
    /// WP-419: `__builtin_fminimum_numf128` (clang 22).
    pub const FMINIMUM_NUMF128: u32 = 2061;
    /// `__builtin_frexpf128`.
    pub const FREXPF128: u32 = 620;
    /// `__builtin_huge_valf128`.
    pub const HUGE_VALF128: u32 = 654;
    /// `__builtin_inff128`.
    pub const INFF128: u32 = 675;
    /// `__builtin_nanf128`.
    pub const NANF128: u32 = 897;
    /// `__builtin_nansf128`.
    pub const NANSF128: u32 = 902;
    /// `__builtin_nearbyintf128`.
    pub const NEARBYINTF128: u32 = 909;
    /// `__builtin_rintf128`.
    pub const RINTF128: u32 = 1023;
    /// `__builtin_roundevenf128`.
    pub const ROUNDEVENF128: u32 = 1055;
    /// `__builtin_roundf128`.
    pub const ROUNDF128: u32 = 1053;
    /// `__builtin_truncf128`.
    pub const TRUNCF128: u32 = 1339;

    // ---- WP-361: the P1467 `_FloatN` builtins libstdc++'s `<limits>`
    // names (GCC's; clang 19 has only the `f16` ones). kiln-private ids
    // above clang's range; each evaluates and lowers as its `float` /
    // `double` / `long double` twin ([`f128_twin`]), driven by the result
    // type.
    /// `__builtin_huge_valf16`.
    pub const HUGE_VALF16: u32 = 2000;
    /// `__builtin_inff16`.
    pub const INFF16: u32 = 2001;
    /// `__builtin_nanf16`.
    pub const NANF16: u32 = 2002;
    /// `__builtin_nansf16`.
    pub const NANSF16: u32 = 2003;
    /// `__builtin_huge_valf32`.
    pub const HUGE_VALF32: u32 = 2004;
    /// `__builtin_inff32`.
    pub const INFF32: u32 = 2005;
    /// `__builtin_nanf32`.
    pub const NANF32: u32 = 2006;
    /// `__builtin_nansf32`.
    pub const NANSF32: u32 = 2007;
    /// `__builtin_huge_valf64`.
    pub const HUGE_VALF64: u32 = 2008;
    /// `__builtin_inff64`.
    pub const INFF64: u32 = 2009;
    /// `__builtin_nanf64`.
    pub const NANF64: u32 = 2010;
    /// `__builtin_nansf64`.
    pub const NANSF64: u32 = 2011;
    /// `__builtin_nansf16b` (GCC; `__bf16`).
    pub const NANSF16B: u32 = 2012;
    /// `__builtin_nextafterf16` (GCC; named in an `if consteval` branch
    /// of libstdc++'s `<cmath>`; evaluated by the Oven's P0533 path and
    /// lowered inline, as there is no libm entry point).
    pub const NEXTAFTERF16: u32 = 2013;
    /// `__builtin_nextafterf16b` (GCC; as [`NEXTAFTERF16`]).
    pub const NEXTAFTERF16B: u32 = 2014;

    // ---- WP-416: clang builtins kiln lacked (kiln-private ids). The
    // alignment builtins and the element-wise math builtins are
    // `CustomTypeChecking` (sema `builtin_align` / `builtin_elementwise`).
    /// `__builtin_align_up`.
    pub const ALIGN_UP: u32 = 2100;
    /// `__builtin_align_down`.
    pub const ALIGN_DOWN: u32 = 2101;
    /// `__builtin_is_aligned`.
    pub const IS_ALIGNED: u32 = 2102;
    /// `__builtin_debugtrap`.
    pub const DEBUGTRAP: u32 = 2103;
    /// `__builtin_frame_address`.
    pub const FRAME_ADDRESS: u32 = 2104;
    /// `__builtin_elementwise_abs`.
    pub const ELEMENTWISE_ABS: u32 = 2105;
    /// `__builtin_elementwise_max`.
    pub const ELEMENTWISE_MAX: u32 = 2106;
    /// `__builtin_elementwise_min`.
    pub const ELEMENTWISE_MIN: u32 = 2107;
    // ---- WP-432 (kiln-private ids).
    /// `__builtin_return_address`.
    pub const RETURN_ADDRESS: u32 = 2108;
    /// `__builtin_elementwise_add_sat` (`CustomTypeChecking`, sema
    /// `builtin_elementwise_sat`).
    pub const ELEMENTWISE_ADD_SAT: u32 = 2109;
    /// `__builtin_elementwise_sub_sat`.
    pub const ELEMENTWISE_SUB_SAT: u32 = 2110;
    // ---- WP-473 (kiln extension).
    /// `__kiln_plainly_constant_evaluated()`: whether the running
    /// evaluation is plainly constant-evaluated (the corresponding
    /// expression of a consteval block; spec/consteval-blocks.md §3);
    /// false at runtime.
    pub const KILN_PLAINLY_CONSTANT_EVALUATED: u32 = 2150;
    // ---- WP-484 (GCC builtins libstdc++'s `<meta>` tests for).
    /// `bool __builtin_is_string_literal(const CharT *p)` (GCC 16;
    /// `std::is_string_literal`, [meta.string.literal]): whether `p`
    /// points into a string literal object. Consteval.
    pub const IS_STRING_LITERAL: u32 = 2151;
    // ---- WP-630 (the generic builtins the x86 intrinsic headers use).
    /// `__builtin_nontemporal_store(v, p)` (clang `Builtins.td`
    /// `NontemporalStore`, `void(...)`, `CustomTypeChecking`): `p` a
    /// pointer to an integer, floating, pointer or vector type, `v`
    /// converted to its pointee; a nontemporal store. Kiln-private id.
    pub const NONTEMPORAL_STORE: u32 = 2153;
    /// `__builtin_nontemporal_load(p)`: the pointee of `p` (unqualified)
    /// by a nontemporal load. Kiln-private id.
    pub const NONTEMPORAL_LOAD: u32 = 2154;
    /// `__builtin_nondeterministic_value(x)` (`NoThrow`,
    /// `CustomTypeChecking`): an arbitrary value of `x`'s type (an
    /// integer, floating or vector type; `freeze poison`), `x` not
    /// evaluated. Kiln-private id.
    pub const NONDETERMINISTIC_VALUE: u32 = 2155;
    // ---- WP-757 (P1494R5, [intro.abstract]/5).
    /// `void __builtin_observable_checkpoint()` (GCC 16, `NoThrow`, `Leaf`;
    /// libstdc++'s `std::observable_checkpoint` in `<utility>`): an
    /// observable checkpoint. Undefined behavior after it does not reach
    /// back past it; lowered to an opaque memory barrier
    /// (spec/codegen.md). Not a constant expression. Kiln-private id
    /// (clang 22 has no such builtin).
    pub const OBSERVABLE_CHECKPOINT: u32 = 2170;
    // ---- WP-610.
    /// `void __builtin_prefetch(const void *addr, ...)` (clang 19
    /// `Builtins.td` `Prefetch`, signature `vvC*.`, `NoThrow, Const`): the
    /// optional `rw` and `locality` are integer constants in [0, 1] and
    /// [0, 3] (clang `Sema::BuiltinPrefetch`).
    pub const PREFETCH: u32 = 976;
    // ---- WP-726 (kiln-private ids).
    /// `void __builtin_memcpy_inline(void *, const void *, size_t)` (clang
    /// `Builtins.td` `MemcpyInline`, `vv*vC*Iz`, `NoThrow`): the size an
    /// integer constant expression; `llvm.memcpy.inline`. Not a constant
    /// expression.
    pub const MEMCPY_INLINE: u32 = 2176;
    /// `void __builtin_assume_separate_storage(const volatile void *,
    /// const volatile void *)` (clang `BuiltinAssumeSeparateStorage`,
    /// `NoThrow, Const`): `llvm.assume(i1 true)` with a
    /// `"separate_storage"` operand bundle.
    pub const ASSUME_SEPARATE_STORAGE: u32 = 2177;
    /// `long __builtin_unpredictable(long)` (clang `BuiltinUnpredictable`,
    /// `LiLi`, `NoThrow, Const`): the value of its operand; a branch or
    /// `switch` on it carries `!unpredictable` when optimizing (WP-779).
    /// Not a constant expression (as in clang).
    pub const UNPREDICTABLE: u32 = 2178;
    // ---- WP-773 (P3726R2, [obj.lifetime]).
    /// `void __builtin_start_lifetime(T *p)` (GCC trunk, `NoThrow`,
    /// `CustomTypeChecking`; libstdc++'s `std::start_lifetime` builds on
    /// it): `T` a complete implicit-lifetime aggregate. In a constant
    /// evaluation it begins the lifetime of `*p` without initializing it,
    /// a union member becoming the active member (spec/oven.md "C++26
    /// union lifetime"); at runtime it evaluates `p` and does nothing.
    /// Kiln-private id (clang 22 has no such builtin).
    pub const START_LIFETIME: u32 = 2179;
    /// `void *__builtin_thread_pointer()` (clang `NoThrow`; WP-782): the
    /// thread pointer, `llvm.thread.pointer`; never a constant.
    pub const THREAD_POINTER: u32 = 2180;
    // ---- WP-774.
    /// `void __builtin_clear_padding(T *p)` (GCC, `NoThrow`,
    /// `CustomTypeChecking`; libstdc++'s and libc++'s `std::atomic` and
    /// `std::atomic_ref` use it): `T` complete, trivially copyable and
    /// not `const`. Zeroes the padding bits of `*p` (`kiln_abi`
    /// `padding_mask`); a validated no-op in a constant evaluation
    /// (spec/codegen.md "Clearing padding"). Kiln-private id (clang 22
    /// has no such builtin).
    pub const CLEAR_PADDING: u32 = 2274;

    // WP-803: the `_FORTIFY_SOURCE` checking builtins (clang 19
    // `Builtins.td` `__builtin___*_chk`; kiln-private ids). Each is a call
    // of the library function its name less `__builtin_` names, except a
    // `memcpy` / `memmove` / `memset` one whose size and object size are
    // constants with size <= object size, which lowers as the unchecked
    // builtin (clang `EmitBuiltinExpr`).
    /// `__builtin___memcpy_chk`.
    pub const MEMCPY_CHK: u32 = 2181;
    /// `__builtin___memmove_chk`.
    pub const MEMMOVE_CHK: u32 = 2182;
    /// `__builtin___mempcpy_chk`.
    pub const MEMPCPY_CHK: u32 = 2183;
    /// `__builtin___memset_chk`.
    pub const MEMSET_CHK: u32 = 2184;
    /// `__builtin___memccpy_chk`.
    pub const MEMCCPY_CHK: u32 = 2185;
    /// `__builtin___stpcpy_chk`.
    pub const STPCPY_CHK: u32 = 2186;
    /// `__builtin___strcat_chk`.
    pub const STRCAT_CHK: u32 = 2187;
    /// `__builtin___strcpy_chk`.
    pub const STRCPY_CHK: u32 = 2188;
    /// `__builtin___stpncpy_chk`.
    pub const STPNCPY_CHK: u32 = 2189;
    /// `__builtin___strncat_chk`.
    pub const STRNCAT_CHK: u32 = 2190;
    /// `__builtin___strncpy_chk`.
    pub const STRNCPY_CHK: u32 = 2191;
    /// `__builtin___strlcat_chk`.
    pub const STRLCAT_CHK: u32 = 2192;
    /// `__builtin___strlcpy_chk`.
    pub const STRLCPY_CHK: u32 = 2193;
    /// `__builtin___snprintf_chk`.
    pub const SNPRINTF_CHK: u32 = 2194;
    /// `__builtin___sprintf_chk`.
    pub const SPRINTF_CHK: u32 = 2195;
    /// `__builtin___vsnprintf_chk`.
    pub const VSNPRINTF_CHK: u32 = 2196;
    /// `__builtin___vsprintf_chk`.
    pub const VSPRINTF_CHK: u32 = 2197;
    /// `__builtin___fprintf_chk`.
    pub const FPRINTF_CHK: u32 = 2198;
    /// `__builtin___printf_chk`.
    pub const PRINTF_CHK: u32 = 2199;
    /// `__builtin___vfprintf_chk`.
    pub const VFPRINTF_CHK: u32 = 2200;
    /// `__builtin___vprintf_chk`.
    pub const VPRINTF_CHK: u32 = 2201;

    /// Whether `id` is one of the WP-803 `__builtin___*_chk` builtins.
    pub fn is_chk(id: u32) -> bool {
        (MEMCPY_CHK..=VPRINTF_CHK).contains(&id)
    }

    /// WP-282: library builtins libc++ and libstdc++ name that WP-270 left
    /// out.
    pub static EXTRA: &[Info] = &[
        row(ASSUME_ALIGNED, "__builtin_assume_aligned", "v*vC*z.", N | C | T),
        row(OBJECT_SIZE, "__builtin_object_size", "zvC*Ii", N),
        row(DYNAMIC_OBJECT_SIZE, "__builtin_dynamic_object_size", "zvC*Ii", N),
        row(POWI, "__builtin_powi", "ddi", N | C | F),
        row(POWIF, "__builtin_powif", "ffi", N | C | F),
        row(POWIL, "__builtin_powil", "LdLdi", N | C | F),
        row(SHUFFLEVECTOR, "__builtin_shufflevector", "v.", N | C | T),
        row(VERBOSE_TRAP, "__builtin_verbose_trap", "vcC*cC*", N | R),
        row(COPYSIGNF16, "__builtin_copysignf16", "hhh", N | C | F),
        row(IS_WITHIN_LIFETIME, "__builtin_is_within_lifetime", "bv*", N | T | I),
        // WP-416.
        row(ALIGN_UP, "__builtin_align_up", "v*.", N | C | T),
        row(ALIGN_DOWN, "__builtin_align_down", "v*.", N | C | T),
        row(IS_ALIGNED, "__builtin_is_aligned", "b.", N | C | T),
        row(DEBUGTRAP, "__builtin_debugtrap", "v", N),
        row(FRAME_ADDRESS, "__builtin_frame_address", "v*IUi", N),
        row(ELEMENTWISE_ABS, "__builtin_elementwise_abs", "v.", N | C | T),
        row(ELEMENTWISE_MAX, "__builtin_elementwise_max", "v.", N | C | T),
        row(ELEMENTWISE_MIN, "__builtin_elementwise_min", "v.", N | C | T),
        // WP-432.
        row(RETURN_ADDRESS, "__builtin_return_address", "v*IUi", N),
        row(ELEMENTWISE_ADD_SAT, "__builtin_elementwise_add_sat", "v.", N | C | T),
        row(ELEMENTWISE_SUB_SAT, "__builtin_elementwise_sub_sat", "v.", N | C | T),
        // WP-473.
        row(KILN_PLAINLY_CONSTANT_EVALUATED, "__kiln_plainly_constant_evaluated", "b", N),
        // WP-484.
        row(IS_STRING_LITERAL, "__builtin_is_string_literal", "b.", N | T | I),
        // WP-610.
        row(PREFETCH, "__builtin_prefetch", "vvC*.", N | C),
        // WP-630.
        row(NONTEMPORAL_STORE, "__builtin_nontemporal_store", "v.", T),
        row(NONTEMPORAL_LOAD, "__builtin_nontemporal_load", "v.", T),
        row(NONDETERMINISTIC_VALUE, "__builtin_nondeterministic_value", "v.", N | T),
        // WP-757.
        row(OBSERVABLE_CHECKPOINT, "__builtin_observable_checkpoint", "v", N),
        // WP-726.
        row(MEMCPY_INLINE, "__builtin_memcpy_inline", "vv*vC*Iz", N),
        row(ASSUME_SEPARATE_STORAGE, "__builtin_assume_separate_storage", "vvCD*vCD*", N | C),
        row(UNPREDICTABLE, "__builtin_unpredictable", "LiLi", N | C),
        // WP-773.
        row(START_LIFETIME, "__builtin_start_lifetime", "v.", N | T),
        // WP-782.
        row(THREAD_POINTER, "__builtin_thread_pointer", "v*", N),
        // WP-1057.
        row(READCYCLECOUNTER, "__builtin_readcyclecounter", "ULLi", N),
        // WP-774.
        row(CLEAR_PADDING, "__builtin_clear_padding", "v.", N | T),
        // WP-803.
        row(MEMCPY_CHK, "__builtin___memcpy_chk", "v*v*vC*zz", N | F),
        row(MEMMOVE_CHK, "__builtin___memmove_chk", "v*v*vC*zz", N | F),
        row(MEMPCPY_CHK, "__builtin___mempcpy_chk", "v*v*vC*zz", N | F),
        row(MEMSET_CHK, "__builtin___memset_chk", "v*v*izz", N | F),
        row(MEMCCPY_CHK, "__builtin___memccpy_chk", "v*v*vC*izz", N),
        row(STPCPY_CHK, "__builtin___stpcpy_chk", "c*c*cC*z", N | F),
        row(STRCAT_CHK, "__builtin___strcat_chk", "c*c*cC*z", N | F),
        row(STRCPY_CHK, "__builtin___strcpy_chk", "c*c*cC*z", N | F),
        row(STPNCPY_CHK, "__builtin___stpncpy_chk", "c*c*cC*zz", N | F),
        row(STRNCAT_CHK, "__builtin___strncat_chk", "c*c*cC*zz", N | F),
        row(STRNCPY_CHK, "__builtin___strncpy_chk", "c*c*cC*zz", N | F),
        row(STRLCAT_CHK, "__builtin___strlcat_chk", "zc*cC*zz", N | F),
        row(STRLCPY_CHK, "__builtin___strlcpy_chk", "zc*cC*zz", N | F),
        row(SNPRINTF_CHK, "__builtin___snprintf_chk", "ic*RzizcC*R.", F),
        row(SPRINTF_CHK, "__builtin___sprintf_chk", "ic*RizcC*R.", F),
        row(VSNPRINTF_CHK, "__builtin___vsnprintf_chk", "ic*RzizcC*Ra", F),
        row(VSPRINTF_CHK, "__builtin___vsprintf_chk", "ic*RizcC*Ra", F),
        row(FPRINTF_CHK, "__builtin___fprintf_chk", "iP*RicC*R.", F),
        row(PRINTF_CHK, "__builtin___printf_chk", "iicC*R.", F),
        row(VFPRINTF_CHK, "__builtin___vfprintf_chk", "iP*RicC*Ra", F),
        row(VPRINTF_CHK, "__builtin___vprintf_chk", "iicC*Ra", F),
        row(BZERO, "__builtin_bzero", "vv*z", N | F),
        row(BCOPY, "__builtin_bcopy", "vvC*v*z", N | F),
        row(CREAL, "__builtin_creal", "dXd", N | C | F),
        row(CREALF, "__builtin_crealf", "fXf", N | C | F),
        row(CREALL, "__builtin_creall", "LdXLd", N | C | F),
        row(CIMAG, "__builtin_cimag", "dXd", N | C | F),
        row(CIMAGF, "__builtin_cimagf", "fXf", N | C | F),
        row(CIMAGL, "__builtin_cimagl", "LdXLd", N | C | F),
        row(CONJ, "__builtin_conj", "XdXd", N | C | F),
        row(CONJF, "__builtin_conjf", "XfXf", N | C | F),
        row(CONJL, "__builtin_conjl", "XLdXLd", N | C | F),
    ];

    // WP-1073: library builtins clang lowers inline (`EmitBuiltinExpr`)
    // that kiln lacked; kiln-private ids. A recognized `bzero` / `bcopy`
    // / `creal` / `cimag` / `conj` ([`LIB`]) lowers as its twin.
    /// `__builtin_bzero` (`llvm.memset` of zero).
    pub const BZERO: u32 = 2400;
    /// `__builtin_bcopy` (`llvm.memmove`, source first).
    pub const BCOPY: u32 = 2401;
    /// `__builtin_creal` (the real part).
    pub const CREAL: u32 = 2402;
    /// `__builtin_crealf`.
    pub const CREALF: u32 = 2403;
    /// `__builtin_creall`.
    pub const CREALL: u32 = 2404;
    /// `__builtin_cimag` (the imaginary part).
    pub const CIMAG: u32 = 2405;
    /// `__builtin_cimagf`.
    pub const CIMAGF: u32 = 2406;
    /// `__builtin_cimagl`.
    pub const CIMAGL: u32 = 2407;
    /// `__builtin_conj` (the imaginary part negated).
    pub const CONJ: u32 = 2408;
    /// `__builtin_conjf`.
    pub const CONJF: u32 = 2409;
    /// `__builtin_conjl`.
    pub const CONJL: u32 = 2410;

    // WP-1104: the `__builtin_`-prefixed C library functions clang 19 and
    // 22 declare that kiln lacked: `Builtins.td` `Builtin` rows with
    // `FunctionWithBuiltinPrefix` and `LibBuiltin` rows with
    // `AddBuiltinPrefixedAlias`, diffed against kiln's tables. Each is a
    // call of the library function it names (clang `emitLibraryCall`),
    // except the two `alloca` forms, which lower as `__builtin_alloca` /
    // `__builtin_alloca_with_align` (kiln has no `-ftrivial-auto-var-init`,
    // so "uninitialized" changes nothing). None is constant-evaluated.
    // The ids are kiln-private.
    /// `void *__builtin_malloc(size_t) noexcept` (`v*z`).
    pub const MALLOC: u32 = 2420;
    /// `void *__builtin_calloc(size_t, size_t) noexcept` (`v*zz`).
    pub const CALLOC: u32 = 2421;
    /// `void *__builtin_realloc(void *, size_t) noexcept` (`v*v*z`).
    pub const REALLOC: u32 = 2422;
    /// `void __builtin_free(void *) noexcept` (`vv*`).
    pub const FREE: u32 = 2423;
    /// `__builtin_fprintf` (`iP*RcC*R.`; `format(printf, 2, 3)`).
    pub const FPRINTF: u32 = 2424;
    /// `__builtin_vfprintf` (`iP*RcC*Ra`; `format(printf, 2, 0)`).
    pub const VFPRINTF: u32 = 2425;
    /// `__builtin_fscanf` (`iP*RcC*R.`; `format(scanf, 2, 3)`; not
    /// `NoThrow`).
    pub const FSCANF: u32 = 2426;
    /// `__builtin_vfscanf` (`iP*RcC*Ra`; `format(scanf, 2, 0)`; not
    /// `NoThrow`).
    pub const VFSCANF: u32 = 2427;
    /// `__builtin_index` (`c*cC*i`).
    pub const INDEX: u32 = 2428;
    /// `__builtin_rindex` (`c*cC*i`).
    pub const RINDEX: u32 = 2429;
    /// `__builtin_stpcpy` (`c*c*cC*`).
    pub const STPCPY: u32 = 2430;
    /// `__builtin_stpncpy` (`c*c*cC*z`).
    pub const STPNCPY: u32 = 2431;
    /// `__builtin_strcasecmp` (`icC*cC*`; not `NoThrow`).
    pub const STRCASECMP: u32 = 2432;
    /// `__builtin_strncasecmp` (`icC*cC*z`; not `NoThrow`).
    pub const STRNCASECMP: u32 = 2433;
    /// `__builtin_strcat` (`c*c*cC*`).
    pub const STRCAT: u32 = 2434;
    /// `__builtin_strcpy` (`c*c*cC*`).
    pub const STRCPY: u32 = 2435;
    /// `__builtin_strncat` (`c*c*cC*z`).
    pub const STRNCAT: u32 = 2436;
    /// `__builtin_strncpy` (`c*c*cC*z`).
    pub const STRNCPY: u32 = 2437;
    /// `__builtin_strcspn` (`zcC*cC*`).
    pub const STRCSPN: u32 = 2438;
    /// `__builtin_strspn` (`zcC*cC*`).
    pub const STRSPN: u32 = 2439;
    /// `__builtin_strdup` (`c*cC*`).
    pub const STRDUP: u32 = 2440;
    /// `__builtin_strndup` (`c*cC*z`).
    pub const STRNDUP: u32 = 2441;
    /// `__builtin_alloca_uninitialized` (`v*z`; as [`ALLOCA`]).
    pub const ALLOCA_UNINITIALIZED: u32 = 2442;
    /// `__builtin_alloca_with_align_uninitialized` (`v*zIz`; as
    /// [`ALLOCA_WITH_ALIGN`]).
    pub const ALLOCA_WITH_ALIGN_UNINITIALIZED: u32 = 2443;

    /// WP-1104: the `__builtin_` library forwards ([`MALLOC`] ...).
    pub static LIBC_FWD: &[Info] = &[
        row(MALLOC, "__builtin_malloc", "v*z", N | F),
        row(CALLOC, "__builtin_calloc", "v*zz", N | F),
        row(REALLOC, "__builtin_realloc", "v*v*z", N | F),
        row(FREE, "__builtin_free", "vv*", N | F),
        row(FPRINTF, "__builtin_fprintf", "iP*RcC*R.", N | F),
        row(VFPRINTF, "__builtin_vfprintf", "iP*RcC*Ra", N | F),
        row(FSCANF, "__builtin_fscanf", "iP*RcC*R.", F),
        row(VFSCANF, "__builtin_vfscanf", "iP*RcC*Ra", F),
        row(INDEX, "__builtin_index", "c*cC*i", N | F),
        row(RINDEX, "__builtin_rindex", "c*cC*i", N | F),
        row(STPCPY, "__builtin_stpcpy", "c*c*cC*", N | F),
        row(STPNCPY, "__builtin_stpncpy", "c*c*cC*z", N | F),
        row(STRCASECMP, "__builtin_strcasecmp", "icC*cC*", F),
        row(STRNCASECMP, "__builtin_strncasecmp", "icC*cC*z", F),
        row(STRCAT, "__builtin_strcat", "c*c*cC*", N | F),
        row(STRCPY, "__builtin_strcpy", "c*c*cC*", N | F),
        row(STRNCAT, "__builtin_strncat", "c*c*cC*z", N | F),
        row(STRNCPY, "__builtin_strncpy", "c*c*cC*z", N | F),
        row(STRCSPN, "__builtin_strcspn", "zcC*cC*", N | F),
        row(STRSPN, "__builtin_strspn", "zcC*cC*", N | F),
        row(STRDUP, "__builtin_strdup", "c*cC*", N | F),
        row(STRNDUP, "__builtin_strndup", "c*cC*z", N | F),
        row(ALLOCA_UNINITIALIZED, "__builtin_alloca_uninitialized", "v*z", N | F),
        row(
            ALLOCA_WITH_ALIGN_UNINITIALIZED,
            "__builtin_alloca_with_align_uninitialized",
            "v*zIz",
            N | F,
        ),
    ];

    /// Whether `id` is a WP-1104 library forward ([`LIBC_FWD`]).
    pub fn is_libc_fwd(id: u32) -> bool {
        (MALLOC..=ALLOCA_WITH_ALIGN_UNINITIALIZED).contains(&id)
    }

    /// The `scanf` format-attribute indices of a builtin (clang
    /// `Builtins.td` `ScanfFormat<N>` / `VScanfFormat<N>`), as
    /// [`printf_format`].
    pub fn scanf_format(id: u32) -> Option<(u16, u16)> {
        Some(match id {
            SCANF => (1, 2),
            VSCANF => (1, 0),
            SSCANF | FSCANF => (2, 3),
            VSSCANF | VFSCANF => (2, 0),
            _ => return None,
        })
    }

    /// WP-282: the `__float128` (`LLd`) math builtins (clang 19
    /// `Builtins.td` `F128` variants), one row each. The ones with a
    /// `long double` twin kiln already lowers or evaluates go down the
    /// twin's path ([`f128_twin`]; both paths are driven by the operand
    /// types, so the `__float128` operands give `fp128` intrinsics and
    /// quad-precision Oven arithmetic); `roundevenf128` is
    /// `llvm.roundeven`; the rest are library calls of the function the
    /// builtin names, like [`LIBM`] ([`is_libm`]).
    pub static F128: &[Info] = &[
        row(105, "__builtin_acosf128", "LLdLLd", N | F),
        row(112, "__builtin_acoshf128", "LLdLLd", N | F),
        row(140, "__builtin_asinf128", "LLdLLd", N | F),
        row(147, "__builtin_asinhf128", "LLdLLd", N | F),
        row(162, "__builtin_atan2f128", "LLdLLdLLd", N | F),
        row(164, "__builtin_atanf128", "LLdLLd", N | F),
        row(171, "__builtin_atanhf128", "LLdLLd", N | F),
        row(297, "__builtin_cbrtf128", "LLdLLd", N | C | F),
        row(CEILF128, "__builtin_ceilf128", "LLdLLd", N | C | F),
        row(COPYSIGNF128, "__builtin_copysignf128", "LLdLLdLLd", N | C | F),
        row(383, "__builtin_cosf128", "LLdLLd", N | F),
        row(391, "__builtin_coshf128", "LLdLLd", N | F),
        row(500, "__builtin_erfcf128", "LLdLLd", N | F),
        row(493, "__builtin_erff128", "LLdLLd", N | F),
        row(525, "__builtin_exp10f128", "LLdLLd", N | F),
        row(520, "__builtin_exp2f128", "LLdLLd", N | F),
        row(527, "__builtin_expf128", "LLdLLd", N | F),
        row(536, "__builtin_expm1f128", "LLdLLd", N | F),
        row(FABSF128, "__builtin_fabsf128", "LLdLLd", N | C | F),
        row(563, "__builtin_fdimf128", "LLdLLdLLd", N | F),
        row(FLOORF128, "__builtin_floorf128", "LLdLLd", N | C | F),
        row(582, "__builtin_fmaf128", "LLdLLdLLdLLd", N | F),
        row(FMAXF128, "__builtin_fmaxf128", "LLdLLdLLd", N | C | F),
        row(FMINF128, "__builtin_fminf128", "LLdLLdLLd", N | C | F),
        // WP-419: clang 22's `fmaximum_num` / `fminimum_num` / `sincos`
        // `f128` forms (kiln-private ids, [`min_compat`]).
        row(FMAXIMUM_NUMF128, "__builtin_fmaximum_numf128", "LLdLLdLLd", N | C | F),
        row(FMINIMUM_NUMF128, "__builtin_fminimum_numf128", "LLdLLdLLd", N | C | F),
        row(2062, "__builtin_sincosf128", "vLLdLLd*LLd*", N | F),
        row(606, "__builtin_fmodf128", "LLdLLdLLd", N | F),
        row(FREXPF128, "__builtin_frexpf128", "LLdLLdi*", N | F),
        row(HUGE_VALF128, "__builtin_huge_valf128", "LLd", N | C),
        row(662, "__builtin_hypotf128", "LLdLLdLLd", N | F),
        row(663, "__builtin_ilogbf128", "iLLd", N | F),
        row(INFF128, "__builtin_inff128", "LLd", N | C),
        row(764, "__builtin_ldexpf128", "LLdLLdi", N | F),
        row(771, "__builtin_lgammaf128", "LLdLLd", N | F),
        row(754, "__builtin_llrintf128", "LLiLLd", N | F),
        row(755, "__builtin_llroundf128", "LLiLLd", N | F),
        row(814, "__builtin_log10f128", "LLdLLd", N | F),
        row(798, "__builtin_log1pf128", "LLdLLd", N | F),
        row(806, "__builtin_log2f128", "LLdLLd", N | F),
        row(823, "__builtin_logbf128", "LLdLLd", N | F),
        row(816, "__builtin_logf128", "LLdLLd", N | F),
        row(831, "__builtin_lrintf128", "LiLLd", N | F),
        row(840, "__builtin_lroundf128", "LiLLd", N | F),
        row(884, "__builtin_modff128", "LLdLLdLLd*", N | F),
        row(NANF128, "__builtin_nanf128", "LLdcC*", N | P | F),
        row(NANSF128, "__builtin_nansf128", "LLdcC*", N | P | F),
        row(NEARBYINTF128, "__builtin_nearbyintf128", "LLdLLd", N | C | F),
        row(916, "__builtin_nextafterf128", "LLdLLdLLd", N | F),
        row(923, "__builtin_nexttowardf128", "LLdLLdLLd", N | F),
        row(972, "__builtin_powf128", "LLdLLdLLd", N | F),
        row(1004, "__builtin_remainderf128", "LLdLLdLLd", N | F),
        row(1011, "__builtin_remquof128", "LLdLLdLLdi*", N | F),
        row(RINTF128, "__builtin_rintf128", "LLdLLd", N | C | F),
        row(ROUNDEVENF128, "__builtin_roundevenf128", "LLdLLd", N | C | F),
        row(ROUNDF128, "__builtin_roundf128", "LLdLLd", N | C | F),
        row(1080, "__builtin_scalblnf128", "LLdLLdLi", N | F),
        row(1081, "__builtin_scalbnf128", "LLdLLdi", N | F),
        row(1099, "__builtin_sinf128", "LLdLLd", N | F),
        row(1107, "__builtin_sinhf128", "LLdLLd", N | F),
        row(1122, "__builtin_sqrtf128", "LLdLLd", N | F),
        row(1307, "__builtin_tanf128", "LLdLLd", N | F),
        row(1315, "__builtin_tanhf128", "LLdLLd", N | F),
        row(1324, "__builtin_tgammaf128", "LLdLLd", N | F),
        row(TRUNCF128, "__builtin_truncf128", "LLdLLd", N | C | F),
        // WP-365: GCC's `<complex.h>` `f128` forms (clang 19 has none),
        // which libstdc++'s `<complex>` names where `_Float128` is not
        // `long double` (x86-64); kiln-private ids, library calls of the
        // glibc function the builtin names.
        row(2015, "__builtin_cabsf128", "LLdXLLd", N),
        row(2016, "__builtin_cacosf128", "XLLdXLLd", N),
        row(2017, "__builtin_cacoshf128", "XLLdXLLd", N),
        row(2018, "__builtin_cargf128", "LLdXLLd", N),
        row(2019, "__builtin_casinf128", "XLLdXLLd", N),
        row(2020, "__builtin_casinhf128", "XLLdXLLd", N),
        row(2021, "__builtin_catanf128", "XLLdXLLd", N),
        row(2022, "__builtin_catanhf128", "XLLdXLLd", N),
        row(2023, "__builtin_ccosf128", "XLLdXLLd", N),
        row(2024, "__builtin_ccoshf128", "XLLdXLLd", N),
        row(2025, "__builtin_cexpf128", "XLLdXLLd", N),
        row(2026, "__builtin_clogf128", "XLLdXLLd", N),
        row(2027, "__builtin_cpowf128", "XLLdXLLdXLLd", N),
        row(2028, "__builtin_cprojf128", "XLLdXLLd", N),
        row(2029, "__builtin_csinf128", "XLLdXLLd", N),
        row(2030, "__builtin_csinhf128", "XLLdXLLd", N),
        row(2031, "__builtin_csqrtf128", "XLLdXLLd", N),
        row(2032, "__builtin_ctanf128", "XLLdXLLd", N),
        row(2033, "__builtin_ctanhf128", "XLLdXLLd", N),
    ];

    /// WP-361: the P1467 `_Float16` (`x`), `_Float32` (kiln `k`),
    /// `_Float64` (kiln `K`) and `__bf16` (`y`) forms of the builtins
    /// libstdc++ names.
    pub static FLOATN: &[Info] = &[
        row(HUGE_VALF16, "__builtin_huge_valf16", "x", N | C),
        row(INFF16, "__builtin_inff16", "x", N | C),
        row(NANF16, "__builtin_nanf16", "xcC*", N | P | F),
        row(NANSF16, "__builtin_nansf16", "xcC*", N | P | F),
        row(HUGE_VALF32, "__builtin_huge_valf32", "k", N | C),
        row(INFF32, "__builtin_inff32", "k", N | C),
        row(NANF32, "__builtin_nanf32", "kcC*", N | P),
        row(NANSF32, "__builtin_nansf32", "kcC*", N | P),
        row(HUGE_VALF64, "__builtin_huge_valf64", "K", N | C),
        row(INFF64, "__builtin_inff64", "K", N | C),
        row(NANF64, "__builtin_nanf64", "KcC*", N | P),
        row(NANSF64, "__builtin_nansf64", "KcC*", N | P),
        row(NANSF16B, "__builtin_nansf16b", "ycC*", N | P),
        row(NEXTAFTERF16, "__builtin_nextafterf16", "xxx", N),
        row(NEXTAFTERF16B, "__builtin_nextafterf16b", "yyy", N),
    ];

    /// The `long double` builtin a [`F128`] builtin is lowered and
    /// evaluated as, if any; also the
    /// `float` / `double` twin of a [`FLOATN`] builtin, and (WP-403) the
    /// [`f16_twin`] of an `__fp16` builtin.
    pub fn f128_twin(id: u32) -> Option<u32> {
        Some(match id {
            HUGE_VALF16 | HUGE_VALF32 => HUGE_VALF,
            INFF16 | INFF32 => INFF,
            NANF16 | NANF32 => NANF,
            NANSF16 | NANSF32 | NANSF16B => NANSF,
            HUGE_VALF64 => HUGE_VAL,
            INFF64 => INF,
            NANF64 => NAN,
            NANSF64 => NANS,
            CEILF128 => CEILL,
            COPYSIGNF128 => COPYSIGNL,
            FABSF128 => FABSL,
            FLOORF128 => FLOORL,
            FMAXF128 => FMAXL,
            FMINF128 => FMINL,
            FREXPF128 => FREXPL,
            HUGE_VALF128 => HUGE_VALL,
            INFF128 => INFL,
            NANF128 => NANL,
            NANSF128 => NANSL,
            NEARBYINTF128 => NEARBYINTL,
            RINTF128 => RINTL,
            ROUNDF128 => ROUNDL,
            TRUNCF128 => TRUNCL,
            FMAXIMUM_NUMF128 => FMAXIMUM_NUML,
            FMINIMUM_NUMF128 => FMINIMUM_NUML,
            SIGNBITF | SIGNBITL => SIGNBIT,
            _ => return f16_twin(id),
        })
    }

    /// Whether builtin `id` is a [`F128`] builtin.
    pub fn is_f128(id: u32) -> bool {
        F128.iter().any(|e| e.id == id)
    }

    // ---- end WP-282 ----

    // ---- WP-403: the `__fp16` (`h`) libm builtins ----

    /// `__builtin_fabsf16`.
    pub const FABSF16: u32 = 554;
    /// `__builtin_fmaxf16`.
    pub const FMAXF16: u32 = 589;
    /// `__builtin_fminf16`.
    pub const FMINF16: u32 = 597;
    /// `__builtin_roundevenf16` (`llvm.roundeven`).
    pub const ROUNDEVENF16: u32 = 1054;
    /// `__builtin_canonicalizef16` (`llvm.canonicalize`; not a constant).
    pub const CANONICALIZEF16: u32 = 2034;
    /// `__builtin_fmaximum_numf16` (clang 22; `llvm.maximumnum`).
    pub const FMAXIMUM_NUMF16: u32 = 2037;
    /// `__builtin_fminimum_numf16` (clang 22; `llvm.minimumnum`).
    pub const FMINIMUM_NUMF16: u32 = 2038;

    /// The `__builtin_<fn>f16` libm builtins clang declares on `__fp16`
    /// (`Builtins.td` `F16F128` variants; the ids are clang 19's, one
    /// below the `f128` twin, and kiln-private from 2034 for the ones
    /// clang 19 lacks) that lower to an LLVM intrinsic: the ones with a
    /// `float` twin go down its path ([`f16_twin`]; the `__fp16` operand
    /// type picks the `half` overload and the Oven format), the others
    /// are lowered by id (`kiln_codegen` `fp_intrinsic`). `sqrtf16` is in [`LIBM`] and
    /// `copysignf16` in [`EXTRA`].
    pub static F16_INTR: &[Info] = &[
        row(CANONICALIZEF16, "__builtin_canonicalizef16", "hh", N | C),
        row(316, "__builtin_ceilf16", "hh", N | C | F),
        row(FABSF16, "__builtin_fabsf16", "hh", N | C | F),
        row(573, "__builtin_floorf16", "hh", N | C | F),
        row(581, "__builtin_fmaf16", "hhhh", N | F),
        row(FMAXF16, "__builtin_fmaxf16", "hhh", N | C | F),
        row(FMAXIMUM_NUMF16, "__builtin_fmaximum_numf16", "hhh", N | C | F),
        row(FMINF16, "__builtin_fminf16", "hhh", N | C | F),
        row(FMINIMUM_NUMF16, "__builtin_fminimum_numf16", "hhh", N | C | F),
        row(619, "__builtin_frexpf16", "hhi*", N | F),
        row(1022, "__builtin_rintf16", "hh", N | C | F),
        row(1052, "__builtin_roundf16", "hh", N | C | F),
        row(ROUNDEVENF16, "__builtin_roundevenf16", "hh", N | C | F),
        row(1338, "__builtin_truncf16", "hh", N | C | F),
    ];

    /// The `__fp16` libm builtins that are library calls of the function
    /// the builtin names (clang 19 and 22 at `-fmath-errno`), like
    /// [`LIBM`] ([`is_libm`]).
    pub static F16_LIBM: &[Info] = &[
        row(104, "__builtin_acosf16", "hh", N | F),
        row(139, "__builtin_asinf16", "hh", N | F),
        row(2035, "__builtin_atan2f16", "hhh", N | F),
        row(163, "__builtin_atanf16", "hh", N | F),
        row(382, "__builtin_cosf16", "hh", N | F),
        row(390, "__builtin_coshf16", "hh", N | F),
        row(524, "__builtin_exp10f16", "hh", N | F),
        row(519, "__builtin_exp2f16", "hh", N | F),
        row(526, "__builtin_expf16", "hh", N | F),
        row(605, "__builtin_fmodf16", "hhh", N | F),
        row(763, "__builtin_ldexpf16", "hhi", N | F),
        row(813, "__builtin_log10f16", "hh", N | F),
        row(805, "__builtin_log2f16", "hh", N | F),
        row(815, "__builtin_logf16", "hh", N | F),
        row(971, "__builtin_powf16", "hhh", N | F),
        row(1098, "__builtin_sinf16", "hh", N | F),
        row(2036, "__builtin_sincosf16", "vhh*h*", N | F),
        row(1106, "__builtin_sinhf16", "hh", N | F),
        row(1306, "__builtin_tanf16", "hh", N | F),
        row(1314, "__builtin_tanhf16", "hh", N | F),
    ];

    /// The `float` builtin a [`F16_INTR`] builtin is lowered and
    /// evaluated as, if any.
    pub fn f16_twin(id: u32) -> Option<u32> {
        Some(match id {
            316 => CEILF,
            FABSF16 => FABSF,
            573 => FLOORF,
            581 => FMAF,
            FMAXF16 => FMAXF,
            FMINF16 => FMINF,
            619 => FREXPF,
            1022 => RINTF,
            1052 => ROUNDF,
            1338 => TRUNCF,
            _ => return None,
        })
    }

    /// The lowest `-fclang-compat` level that declares builtin `id`
    /// (clang 19 lacks `atan2f16`, `sincosf16`, `sincos{,f,l}`, the
    /// `fmaximum_num` / `fminimum_num` family, and WP-419's `sincospi{,f,l}`
    /// and `f128` forms; `__has_builtin` answers from the
    /// generated clang tables, which agree).
    pub fn min_compat(id: u32) -> u32 {
        match id {
            // 2039..=2047 (WP-410): `sincos`, `fmaximum_num`, `fminimum_num`;
            // 2060..=2065 (WP-419): the `f128` forms and `sincospi`;
            // IS_WITHIN_LIFETIME (WP-418): clang 20.
            2035
            | 2036
            | FMAXIMUM_NUMF16
            | FMINIMUM_NUMF16
            | 2039..=2047
            | 2060..=2065
            | IS_WITHIN_LIFETIME => 22,
            _ => 0,
        }
    }

    /// [`by_name`] at `-fclang-compat=compat`: `None` for a builtin that
    /// level does not declare ([`min_compat`]).
    pub fn by_name_at(name: &str, compat: u32) -> Option<u32> {
        by_name(name).filter(|&id| min_compat(id) <= compat)
    }

    // ---- end WP-403 ----

    // ---- WP-410: the remaining `__has_builtin` libm builtins ----

    /// `__builtin_roundeven` (`llvm.roundeven`).
    pub const ROUNDEVEN: u32 = 1048;
    /// `__builtin_roundevenf`.
    pub const ROUNDEVENF: u32 = 1046;
    /// `__builtin_roundevenl`.
    pub const ROUNDEVENL: u32 = 1050;
    /// `__builtin_canonicalize` (`llvm.canonicalize`; not a constant).
    pub const CANONICALIZE: u32 = 258;
    /// `__builtin_canonicalizef`.
    pub const CANONICALIZEF: u32 = 257;
    /// `__builtin_canonicalizel`.
    pub const CANONICALIZEL: u32 = 259;
    /// `__builtin_fmaximum_num` (clang 22; `llvm.maximumnum`).
    pub const FMAXIMUM_NUM: u32 = 2042;
    /// `__builtin_fmaximum_numf` (clang 22).
    pub const FMAXIMUM_NUMF: u32 = 2043;
    /// `__builtin_fmaximum_numl` (clang 22).
    pub const FMAXIMUM_NUML: u32 = 2044;
    /// `__builtin_fminimum_num` (clang 22; `llvm.minimumnum`).
    pub const FMINIMUM_NUM: u32 = 2045;
    /// `__builtin_fminimum_numf` (clang 22).
    pub const FMINIMUM_NUMF: u32 = 2046;
    /// `__builtin_fminimum_numl` (clang 22).
    pub const FMINIMUM_NUML: u32 = 2047;

    /// The `double`/`float`/`long double` builtins clang answers
    /// `__has_builtin` for that lower to an LLVM intrinsic (by id,
    /// `kiln_codegen` `fp_intrinsic`): `roundeven` (evaluated as a P0533
    /// function, [`p0533_base`]), `canonicalize` (never a constant) and
    /// clang 22's `fmaximum_num` / `fminimum_num` (constant-evaluated by
    /// clang 22, IEEE 754-2019 `maximumNumber` / `minimumNumber`). The ids
    /// are clang 19's, kiln-private from 2042 for the ones it lacks.
    pub static C_MATH_INTR: &[Info] = &[
        row(CANONICALIZE, "__builtin_canonicalize", "dd", N | C),
        row(CANONICALIZEF, "__builtin_canonicalizef", "ff", N | C),
        row(CANONICALIZEL, "__builtin_canonicalizel", "LdLd", N | C),
        row(FMAXIMUM_NUM, "__builtin_fmaximum_num", "ddd", N | C | F),
        row(FMAXIMUM_NUMF, "__builtin_fmaximum_numf", "fff", N | C | F),
        row(FMAXIMUM_NUML, "__builtin_fmaximum_numl", "LdLdLd", N | C | F),
        row(FMINIMUM_NUM, "__builtin_fminimum_num", "ddd", N | C | F),
        row(FMINIMUM_NUMF, "__builtin_fminimum_numf", "fff", N | C | F),
        row(FMINIMUM_NUML, "__builtin_fminimum_numl", "LdLdLd", N | C | F),
        row(ROUNDEVEN, "__builtin_roundeven", "dd", N | C | F),
        row(ROUNDEVENF, "__builtin_roundevenf", "ff", N | C | F),
        row(ROUNDEVENL, "__builtin_roundevenl", "LdLd", N | C | F),
    ];

    /// The `double`/`float`/`long double` builtins clang answers
    /// `__has_builtin` for that are library calls, like [`LIBM`]
    /// ([`is_libm`]): `exp10` and clang 22's `sincos` (GNU extensions;
    /// clang 19 lacks `sincos`) and (WP-419) `sincospi`.
    pub static C_MATH_LIB: &[Info] = &[
        row(522, "__builtin_exp10", "dd", N | F),
        row(521, "__builtin_exp10f", "ff", N | F),
        row(523, "__builtin_exp10l", "LdLd", N | F),
        row(2039, "__builtin_sincos", "vdd*d*", N | F),
        row(2040, "__builtin_sincosf", "vff*f*", N | F),
        row(2041, "__builtin_sincosl", "vLdLd*Ld*", N | F),
        // WP-419: clang 22's `sincospi` (kiln-private ids).
        row(2063, "__builtin_sincospi", "vdd*d*", N | F),
        row(2064, "__builtin_sincospif", "vff*f*", N | F),
        row(2065, "__builtin_sincospil", "vLdLd*Ld*", N | F),
    ];

    // ---- end WP-410 ----

    /// `__builtin_is_within_lifetime(p)` ([meta.const.eval]/4, P2641R4;
    /// clang 20, `Consteval` and `CustomTypeChecking`): whether `p` points
    /// to an object within its lifetime. Kiln-private id.
    pub const IS_WITHIN_LIFETIME: u32 = 2090;

    /// The builtin function spelled `name`, if kiln declares one.
    pub fn by_name(name: &str) -> Option<u32> {
        if name.starts_with(neon::PREFIX) {
            return neon::by_name(name);
        }
        if name.starts_with(x86::PREFIX) || name.starts_with(x86::HEADER_PREFIX) {
            return x86::by_name(name);
        }
        // WP-961: clang's plain x86 target builtin `__rdtsc` (`ULLi`, the
        // `__builtin_ia32_rdtsc` row's signature and codegen); ia32intrin.h's
        // `_rdtsc()` expands to it. Target-gated like every x86 row.
        if name == "__rdtsc" {
            return x86::by_name("__builtin_ia32_rdtsc");
        }
        if name.starts_with(arm::PREFIX) {
            return arm::by_name(name);
        }
        ALL.iter()
            .chain(INT_MEM)
            .chain(LIBM)
            .chain(ATOMIC)
            .chain(SYNC)
            .chain(CORO)
            .chain(CHECKED)
            .chain(EXTRA)
            .chain(F128)
            .chain(FLOATN)
            .chain(F16_INTR)
            .chain(F16_LIBM)
            .chain(C_MATH_INTR)
            .chain(C_MATH_LIB)
            .chain(LIBC_FWD)
            .find(|e| e.name == name)
            .map(|e| e.id)
    }
    /// The table row of builtin `id`. By value: a NEON or x86 row is decoded
    /// on access ([`neon::row`], WP-653; [`x86::row`], WP-652).
    pub fn info(id: u32) -> Option<Info> {
        if let Some(r) = neon::row(id) {
            return Some(r.info);
        }
        if let Some(r) = x86::row(id) {
            return Some(r.info);
        }
        if let Some(r) = arm::row(id) {
            return Some(r.info);
        }
        ALL.iter()
            .chain(INT_MEM)
            .chain(LIBM)
            .chain(ATOMIC)
            .chain(SYNC)
            .chain(CORO)
            .chain(CHECKED)
            .chain(EXTRA)
            .chain(F128)
            .chain(FLOATN)
            .chain(F16_INTR)
            .chain(F16_LIBM)
            .chain(C_MATH_INTR)
            .chain(C_MATH_LIB)
            .chain(LIBC_FWD)
            .find(|e| e.id == id)
            .copied()
    }

    /// The parameters of builtin `id` that must be integer constant
    /// expressions (an `I` prefix in its signature), as a bit mask by
    /// parameter index (clang `ASTContext::GetBuiltinType`'s
    /// `IntegerConstantArgs`).
    pub fn ice_params(id: u32) -> u32 {
        let Some(e) = info(id) else { return 0 };
        let (mut mask, mut idx, mut ice) = (0u32, 0i32, false);
        for &c in e.sig.as_bytes() {
            match c {
                b'I' => ice = true,
                b'U'
                | b'L'
                | b'S'
                | b'W'
                | b'Z'
                | b'V'
                | b'0'..=b'9'
                | b'C'
                | b'D'
                | b'R'
                | b'*'
                | b'&' => {}
                b'.' => break,
                _ => {
                    if ice && idx > 0 {
                        mask |= 1 << (idx - 1);
                    }
                    ice = false;
                    idx += 1;
                }
            }
        }
        mask
    }

    /// Whether builtin `id` is a [`CORO`] builtin.
    pub fn is_coro(id: u32) -> bool {
        CORO.iter().any(|e| e.id == id)
    }

    /// WP-365: the `<cmath>` function of the P0533 builtin `id` (the
    /// rounding, remainder, `fdim`/`fma`, exponent and `nextafter`
    /// families, and WP-393's `sqrt`): its name without the `__builtin_` prefix and the `f`,
    /// `l`, `f128`, `f16` or `f16b` suffix (`"ceil"` for
    /// `__builtin_ceilf128`); the Oven evaluates these and clang 19 does
    /// not.
    pub fn p0533_base(id: u32) -> Option<&'static str> {
        const BASES: &[&str] = &[
            "ceil",
            "floor",
            "trunc",
            "round",
            "roundeven",
            "rint",
            "nearbyint",
            "lround",
            "llround",
            "lrint",
            "llrint",
            "fmod",
            "remainder",
            "remquo",
            "fdim",
            "fma",
            "frexp",
            "ldexp",
            "scalbn",
            "scalbln",
            "ilogb",
            "logb",
            "modf",
            "nextafter",
            "nexttoward",
            // WP-393: P1383's `sqrt`, correctly rounded.
            "sqrt",
        ];
        let name = info(id)?.name.strip_prefix("__builtin_")?;
        BASES.iter().copied().find(|base| {
            name.strip_prefix(base)
                .is_some_and(|s| matches!(s, "" | "f" | "l" | "f128" | "f16" | "f16b"))
        })
    }

    /// Whether builtin `id` is a [`LIBM`] function.
    /// A [`F128`] builtin without a `long double` twin (other than
    /// `roundevenf128`, an intrinsic) counts as one.
    pub fn is_libm(id: u32) -> bool {
        LIBM.iter().any(|e| e.id == id)
            || F16_LIBM.iter().any(|e| e.id == id)
            || C_MATH_LIB.iter().any(|e| e.id == id)
            || (is_f128(id) && f128_twin(id).is_none() && id != ROUNDEVENF128)
    }

    /// Whether builtin `id` has custom type-checking.
    pub fn is_custom(id: u32) -> bool {
        info(id).is_some_and(|i| i.attrs & attr::CUSTOM != 0)
    }

    /// WP-558: the `AArch64` NEON / FP16 target builtins
    /// (`__builtin_neon_*`), clang's `TARGET_BUILTIN` rows of
    /// `arm_neon.inc` / `arm_fp16.inc` for every name kiln's resource
    /// `arm_neon.h` and `arm_fp16.h` use (spec/landed/sema/WP-558.md "Target
    /// builtins"). The generated table (`scripts/gen-neon-builtins.py`) is
    /// sorted by name; a row's id is [`neon::BASE`] plus its index, so
    /// lookup by name is a binary search and by id an index. It is plain
    /// data (WP-653): its strings are offsets into one blob, so a
    /// [`neon::Row`] is decoded on access and returned by value. Signatures
    /// add the NEON prototype codes to [`Info::sig`]'s: `V<N>` a GNU
    /// vector of `N` elements, `S` signed, `W` `int64_t` (`long`), `h`
    /// `__fp16`, `y` `__bf16`, `LLL` `__int128`.
    pub mod neon {
        use super::{Info, attr};

        /// The common prefix of the names.
        pub const PREFIX: &str = "__builtin_neon_";
        /// The id of the table's first row (kiln-private ids).
        pub const BASE: u32 = 0x1_0000;

        /// One builtin and the target features it needs, decoded from its
        /// table row ([`row`]).
        #[derive(Clone, Copy, Debug)]
        pub struct Row {
            /// The builtin (always `NOTHROW`).
            pub info: Info,
            /// clang's required-features string: comma-separated names,
            /// all of which must be enabled for the calling function.
            pub features: &'static str,
        }

        /// A row of the generated table: `(offset, length)` of the name,
        /// the prototype and the features string in
        /// `neon_builtins::STRS`. No pointers, so the table needs no
        /// load-time relocation and the binary holds one copy (WP-653).
        #[derive(Clone, Copy, Debug)]
        pub(crate) struct Packed {
            name: u32,
            sig: u32,
            features: u32,
            name_len: u8,
            sig_len: u8,
            features_len: u8,
        }

        pub(crate) const fn r(name: (u32, u8), sig: (u32, u8), features: (u32, u8)) -> Packed {
            Packed {
                name: name.0,
                sig: sig.0,
                features: features.0,
                name_len: name.1,
                sig_len: sig.1,
                features_len: features.1,
            }
        }

        /// The `len` bytes of the string blob at `at` (ASCII, so any range
        /// is on character boundaries).
        fn text(at: u32, len: u8) -> &'static str {
            let at = at as usize;
            &crate::neon_builtins::STRS[at..at + usize::from(len)]
        }

        /// The number of rows.
        pub fn len() -> usize {
            crate::neon_builtins::NEON.len()
        }

        /// Every row, sorted by name.
        pub fn rows() -> impl Iterator<Item = Row> {
            (0..len()).filter_map(|i| row(BASE + i as u32))
        }

        /// The NEON builtin spelled `name`.
        pub fn by_name(name: &str) -> Option<u32> {
            let t = &crate::neon_builtins::NEON;
            let i = t.binary_search_by(|p| text(p.name, p.name_len).cmp(name)).ok()?;
            Some(BASE + i as u32)
        }

        /// The row of builtin `id`, if it is a NEON builtin.
        pub fn row(id: u32) -> Option<Row> {
            let p = crate::neon_builtins::NEON.get(usize::try_from(id.checked_sub(BASE)?).ok()?)?;
            Some(Row {
                info: Info {
                    id,
                    name: text(p.name, p.name_len),
                    sig: text(p.sig, p.sig_len),
                    attrs: attr::NOTHROW,
                },
                features: text(p.features, p.features_len),
            })
        }

        /// The required-features string of the NEON builtin `name`.
        pub fn required_features(name: &str) -> Option<&'static str> {
            row(by_name(name)?).map(|r| r.features)
        }

        /// Whether `id` is a NEON builtin.
        pub fn is_neon(id: u32) -> bool {
            id.checked_sub(BASE).is_some_and(|i| (i as usize) < len())
        }

        /// Whether the required-features string `req` holds when exactly
        /// the features `on` accepts are enabled (clang
        /// `Builtin::evaluateRequiredTargetFeatures`: `,` is all of, `|`
        /// any of).
        pub fn features_hold(req: &str, on: impl Fn(&str) -> bool) -> bool {
            req.split(',').all(|all| all.split('|').any(|f| on(f.trim())))
        }

        /// The first feature of `req` that `on` does not accept (for a
        /// diagnostic), if any.
        pub fn first_missing(req: &str, on: impl Fn(&str) -> bool) -> Option<&str> {
            req.split(',').find(|all| !all.split('|').any(|f| on(f.trim())))
        }
    }

    /// WP-724: the `AArch64` ACLE target builtins: every `__builtin_arm_*`
    /// name clang 19's `BuiltinsAArch64.def` declares outside the NEON, SVE
    /// and SME tables, with its prototype, attributes and required-features
    /// string, sorted by name; a row's id is [`arm::BASE`] plus its index
    /// (kiln-private ids). `__builtin_arm_ldrex` / `ldaex` / `strex` /
    /// `stlex` are custom type-checked (clang
    /// `CheckARMBuiltinExclusiveCall`).
    pub mod arm {
        use super::{Info, attr};

        /// The common prefix of the names.
        pub const PREFIX: &str = "__builtin_arm_";
        /// The id of the table's first row.
        pub const BASE: u32 = 0x3_0000;

        const NONE: u8 = 0;
        const N: u8 = attr::NOTHROW;
        const NC: u8 = attr::NOTHROW | attr::CONST;
        const NR: u8 = attr::NOTHROW | attr::NORETURN;
        const T: u8 = attr::CUSTOM;

        /// `(name, prototype, attributes, required features)`, sorted by
        /// name.
        static ROWS: &[(&str, &str, u8, &str)] = &[
            ("__builtin_arm_chkfeat", "WUiWUi", NONE, ""),
            ("__builtin_arm_clrex", "v", NONE, ""),
            ("__builtin_arm_cls", "UiZUi", NC, ""),
            ("__builtin_arm_cls64", "UiWUi", NC, ""),
            ("__builtin_arm_clz", "UiZUi", NC, ""),
            ("__builtin_arm_clz64", "UiWUi", NC, ""),
            ("__builtin_arm_crc32b", "UiUiUc", NC, "crc"),
            ("__builtin_arm_crc32cb", "UiUiUc", NC, "crc"),
            ("__builtin_arm_crc32cd", "UiUiWUi", NC, "crc"),
            ("__builtin_arm_crc32ch", "UiUiUs", NC, "crc"),
            ("__builtin_arm_crc32cw", "UiUiUi", NC, "crc"),
            ("__builtin_arm_crc32d", "UiUiWUi", NC, "crc"),
            ("__builtin_arm_crc32h", "UiUiUs", NC, "crc"),
            ("__builtin_arm_crc32w", "UiUiUi", NC, "crc"),
            ("__builtin_arm_dmb", "vUi", NC, ""),
            ("__builtin_arm_dsb", "vUi", NC, ""),
            ("__builtin_arm_gcspopm", "WUiWUi", N, "gcs"),
            ("__builtin_arm_gcsss", "vC*vC*", N, "gcs"),
            ("__builtin_arm_isb", "vUi", NC, ""),
            ("__builtin_arm_jcvt", "Zid", NC, "v8.3a"),
            ("__builtin_arm_ld64b", "vvC*WUi*", N, "ls64"),
            ("__builtin_arm_ldaex", "v.", T, ""),
            ("__builtin_arm_ldrex", "v.", T, ""),
            ("__builtin_arm_nop", "v", NONE, ""),
            ("__builtin_arm_prefetch", "vvC*UiUiUiUi", NC, ""),
            ("__builtin_arm_rbit", "UiUi", NC, ""),
            ("__builtin_arm_rbit64", "WUiWUi", NC, ""),
            ("__builtin_arm_rint32x", "dd", NONE, "v8.5a"),
            ("__builtin_arm_rint32xf", "ff", NONE, "v8.5a"),
            ("__builtin_arm_rint32z", "dd", NONE, "v8.5a"),
            ("__builtin_arm_rint32zf", "ff", NONE, "v8.5a"),
            ("__builtin_arm_rint64x", "dd", NONE, "v8.5a"),
            ("__builtin_arm_rint64xf", "ff", NONE, "v8.5a"),
            ("__builtin_arm_rint64z", "dd", NONE, "v8.5a"),
            ("__builtin_arm_rint64zf", "ff", NONE, "v8.5a"),
            ("__builtin_arm_rndr", "iWUi*", N, "rand"),
            ("__builtin_arm_rndrrs", "iWUi*", N, "rand"),
            ("__builtin_arm_rsr", "UicC*", NC, ""),
            ("__builtin_arm_rsr128", "LLLUicC*", NC, "d128"),
            ("__builtin_arm_rsr64", "WUicC*", NC, ""),
            ("__builtin_arm_rsrp", "v*cC*", NC, ""),
            ("__builtin_arm_sev", "v", NONE, ""),
            ("__builtin_arm_sevl", "v", NONE, ""),
            ("__builtin_arm_st64b", "vv*WUiC*", N, "ls64"),
            ("__builtin_arm_st64bv", "WUiv*WUiC*", N, "ls64"),
            ("__builtin_arm_st64bv0", "WUiv*WUiC*", N, "ls64"),
            ("__builtin_arm_stlex", "i.", T, ""),
            ("__builtin_arm_strex", "i.", T, ""),
            ("__builtin_arm_trap", "vUIs", NR, ""),
            ("__builtin_arm_wfe", "v", NONE, ""),
            ("__builtin_arm_wfi", "v", NONE, ""),
            ("__builtin_arm_wsr", "vcC*Ui", NC, ""),
            ("__builtin_arm_wsr128", "vcC*LLLUi", NC, "d128"),
            ("__builtin_arm_wsr64", "vcC*WUi", NC, ""),
            ("__builtin_arm_wsrp", "vcC*vC*", NC, ""),
            ("__builtin_arm_yield", "v", NONE, ""),
        ];

        /// One builtin and the target features it needs.
        #[derive(Clone, Copy, Debug)]
        pub struct Row {
            /// The builtin.
            pub info: Info,
            /// clang's required-features string (empty: none).
            pub features: &'static str,
        }

        /// The number of rows.
        pub fn len() -> usize {
            ROWS.len()
        }

        /// The ACLE builtin spelled `name`.
        pub fn by_name(name: &str) -> Option<u32> {
            let i = ROWS.binary_search_by(|r| r.0.cmp(name)).ok()?;
            Some(BASE + i as u32)
        }

        /// The row of builtin `id`, if it is an ACLE builtin.
        pub fn row(id: u32) -> Option<Row> {
            let &(name, sig, attrs, features) =
                ROWS.get(usize::try_from(id.checked_sub(BASE)?).ok()?)?;
            Some(Row { info: Info { id, name, sig, attrs }, features })
        }

        /// Whether `id` is an ACLE builtin.
        pub fn is_arm(id: u32) -> bool {
            id.checked_sub(BASE).is_some_and(|i| (i as usize) < len())
        }

        /// Whether `id` is `__builtin_arm_ldrex` or `__builtin_arm_ldaex`
        /// (`Some(true)`), `__builtin_arm_strex` or `__builtin_arm_stlex`
        /// (`Some(false)`).
        pub fn exclusive_load(id: u32) -> Option<bool> {
            match row(id)?.info.name.strip_prefix(PREFIX)? {
                "ldrex" | "ldaex" => Some(true),
                "strex" | "stlex" => Some(false),
                _ => None,
            }
        }
    }

    /// WP-630: the x86 target builtins: every `__builtin_ia32_*` name kiln's
    /// resource headers (clang 19's) reference, `__builtin_ia32_pause`, and
    /// clang's x86 header builtins (`_mm_pause`, `_mm_mfence`, ...;
    /// spec/landed/sema/WP-630.md). The generated table
    /// (`scripts/gen-x86-builtins.py`) is sorted by name; a row's id is
    /// [`x86::BASE`] plus its index. It is plain data (WP-652): its strings
    /// are offsets into one blob, so a [`x86::Row`] is decoded on access and
    /// returned by value. Prototypes use [`Info::sig`]'s
    /// notation with the NEON codes ([`neon`]): `V<N>` vectors, `x`
    /// `_Float16`, `y` `__bf16`, `I` an integer constant expression.
    pub mod x86 {
        use super::{Info, attr};

        /// The common prefix of the `__builtin_ia32_*` names.
        pub const PREFIX: &str = "__builtin_ia32_";
        /// The common prefix of the header builtins' names.
        pub const HEADER_PREFIX: &str = "_mm_";
        /// The id of the table's first row (kiln-private ids).
        pub const BASE: u32 = 0x2_0000;

        /// No attributes.
        pub(crate) const NONE: u8 = 0;
        /// `NoThrow`.
        pub(crate) const N: u8 = attr::NOTHROW;
        /// `NoThrow`, `Const`.
        pub(crate) const NC: u8 = attr::NOTHROW | attr::CONST;

        /// A check clang applies to a builtin's immediate operands
        /// (`SemaX86::CheckBuiltinFunctionCall`, after the operands are
        /// converted and found to be integer constants). The `u8` is the
        /// operand's index.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Imm {
            /// The value is in `[lo, hi]`, else `-Wargument-outside-range`
            /// (an error by default; clang `BuiltinConstantArgRange` with
            /// `RangeIsError = false`).
            Range(u8, i32, i32),
            /// The value is in `[lo, hi]`, else an error (the AMX tile
            /// numbers).
            RangeError(u8, i32, i32),
            /// A rounding-control operand: 4, 8, 9, 10 or 11, else
            /// "invalid rounding argument" (clang
            /// `CheckBuiltinRoundingOrSAE` with `HasRC`).
            Rounding(u8),
            /// A suppress-all-exceptions operand: 4, 8 or 12.
            Sae(u8),
            /// A gather / scatter scale: 1, 2, 4 or 8.
            Scale(u8),
            /// The (immediate) tile operands name different tiles (clang
            /// `CheckBuiltinTileDuplicate`).
            DistinctTiles,
            /// "this builtin is only available on 32-bit targets" on every
            /// call (`readeflags_u32` / `writeeflags_u32`).
            Only32Bit,
        }

        /// One builtin, decoded from its table row ([`row`]).
        #[derive(Clone, Copy, Debug)]
        pub struct Row {
            /// The builtin.
            pub info: Info,
            /// clang's required-features string (as for
            /// [`super::neon::Row::features`]; empty: none).
            pub features: &'static str,
            /// The immediate operand checks.
            pub imms: &'static [Imm],
            /// A header builtin (clang `TARGET_HEADER_BUILTIN`): its header
            /// declares it as an ordinary function, which is the builtin
            /// when it has the builtin's type in a namespace-scope context
            /// with C language linkage; declaring it implicitly warns
            /// ([`header_of`]).
            pub header: bool,
        }

        /// A row of the generated table: `(offset, length)` of the name,
        /// the prototype and the features string in
        /// `x86_builtins::STRS`, and of the immediate checks in
        /// `x86_builtins::IMMS`. No pointers, so the table needs no
        /// load-time relocation and the binary holds one copy (WP-652).
        #[derive(Clone, Copy, Debug)]
        pub(crate) struct Packed {
            name: u32,
            sig: u32,
            features: u32,
            name_len: u8,
            sig_len: u8,
            features_len: u8,
            attrs: u8,
            imms: u16,
            imms_len: u8,
            header: bool,
        }

        pub(crate) const fn r(
            name: (u32, u8),
            sig: (u32, u8),
            attrs: u8,
            features: (u32, u8),
            imms: (u16, u8),
        ) -> Packed {
            Packed {
                name: name.0,
                sig: sig.0,
                features: features.0,
                name_len: name.1,
                sig_len: sig.1,
                features_len: features.1,
                attrs,
                imms: imms.0,
                imms_len: imms.1,
                header: false,
            }
        }

        pub(crate) const fn h(
            name: (u32, u8),
            sig: (u32, u8),
            attrs: u8,
            features: (u32, u8),
            imms: (u16, u8),
        ) -> Packed {
            Packed { header: true, ..r(name, sig, attrs, features, imms) }
        }

        /// The `len` bytes of the string blob at `at` (ASCII, so any range
        /// is on character boundaries).
        fn text(at: u32, len: u8) -> &'static str {
            let at = at as usize;
            &crate::x86_builtins::STRS[at..at + usize::from(len)]
        }

        /// The number of rows.
        pub fn len() -> usize {
            crate::x86_builtins::X86.len()
        }

        /// Every row, sorted by name.
        pub fn rows() -> impl Iterator<Item = Row> {
            (0..len()).filter_map(|i| row(BASE + i as u32))
        }

        /// The x86 builtin spelled `name`.
        pub fn by_name(name: &str) -> Option<u32> {
            let t = &crate::x86_builtins::X86;
            let i = t.binary_search_by(|p| text(p.name, p.name_len).cmp(name)).ok()?;
            Some(BASE + i as u32)
        }

        /// The row of builtin `id`, if it is an x86 builtin.
        pub fn row(id: u32) -> Option<Row> {
            let p = crate::x86_builtins::X86.get(usize::try_from(id.checked_sub(BASE)?).ok()?)?;
            let imms = usize::from(p.imms);
            Some(Row {
                info: Info {
                    id,
                    name: text(p.name, p.name_len),
                    sig: text(p.sig, p.sig_len),
                    attrs: p.attrs,
                },
                features: text(p.features, p.features_len),
                imms: &crate::x86_builtins::IMMS[imms..imms + usize::from(p.imms_len)],
                header: p.header,
            })
        }

        /// Whether `id` is an x86 builtin.
        pub fn is_x86(id: u32) -> bool {
            id.checked_sub(BASE).is_some_and(|i| (i as usize) < len())
        }

        /// The header that declares header builtin `id` (clang's
        /// `XMMINTRIN_H` / `EMMINTRIN_H`).
        pub fn header_of(id: u32) -> Option<&'static str> {
            let r = row(id).filter(|r| r.header)?;
            Some(match r.info.name {
                "_mm_getcsr" | "_mm_setcsr" | "_mm_sfence" => "xmmintrin.h",
                _ => "emmintrin.h",
            })
        }

        /// The required-features string of the `__builtin_ia32_*` builtin
        /// `name` (for `__has_builtin`; header builtins are not answered).
        pub fn required_features(name: &str) -> Option<&'static str> {
            row(by_name(name)?).filter(|r| !r.header).map(|r| r.features)
        }
    }
}
