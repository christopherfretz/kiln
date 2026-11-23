//! Operand roles: what each untyped word of a node header means
//! (spec/ast.md §20, for tooling clients that read the AST).
//!
//! The headers keep their operands in plain `u32`/`u16`/`u8` words
//! ([`TypeNode`](crate::types::TypeNode) `aux`/`data` and its pair or
//! triple words, [`ExprNode`](crate::expr::ExprNode) and
//! [`StmtNode`](crate::stmt::StmtNode) `a`/`b`/`aux` and flag bits,
//! [`AttrNode`] `a`/`b`/`flags`). The schema of a snapshot sees only
//! `u32`; these tables say, per kind, whether a word is an `ExprId`, a
//! `DeclId`, a raw `QualType`, a `ListId` of what, a `Symbol`, a raw
//! `Loc`, flag bits, an enum discriminant, a count or an opaque value,
//! and on which flag a word's meaning depends (`CLEANUP`'s `a` is an
//! `ExprId` before [`AttrNode::CHECKED`] and a `DeclId` after). The cold
//! side maps are listed too ([`COLD_MAPS`]).
//!
//! The tables are data, not code that reads the AST: kiln's module image
//! derives its attribute format from [`attr_roles`]
//! (`kiln_sema::bmi_image::attrs`), so the two cannot drift, and the
//! tests check that every kind has a row and that each word's role
//! agrees with what the builder writes. Values are stable within a PCH
//! `FORMAT_VERSION`: a change to a row is a format change.

use crate::attr::{AttrKind, AttrNode};
use crate::expr::ExprKind;
use crate::stmt::StmtKind;
use crate::types::TypeKind;

/// What one word (or a bit field of one) holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// Always 0.
    Zero,
    /// An `ExprId` (raw; never 0 unless under [`Role::Opt`]).
    Expr,
    /// A `StmtId`.
    Stmt,
    /// A `DeclId`.
    Decl,
    /// An `AttrId`.
    Attr,
    /// A raw `QualType` (`QualType::to_raw`).
    Type,
    /// A `TypeId` (no qualifiers).
    TypeId,
    /// A `ProfileId`: a canonical expression profile
    /// (`TypeContext::profile_words`, `Ast::visit_profile`).
    Profile,
    /// An `NnsId`.
    Nns,
    /// An `ArgListId` (template arguments).
    ArgList,
    /// A `TemplateNameId`.
    TemplateName,
    /// A `Symbol` index.
    Symbol,
    /// A raw [`crate::names::DeclName`].
    DeclName,
    /// A `StringLitId`.
    StringLit,
    /// A `ConstValueId`.
    ConstValue,
    /// A `ReflectionId`.
    Reflection,
    /// A `LambdaId`.
    Lambda,
    /// An `AnnId` (an annotation, `Ast::annots`).
    Annotation,
    /// A raw `Loc`.
    Loc,
    /// A builtin function id ([`crate::decl::builtin`]).
    Builtin,
    /// The word or 0 (none).
    Opt(&'static Role),
    /// A `ListId` of words of the inner role ([`crate::lists::IdLists`]).
    List(&'static Role),
    /// An index into the named side arena (`StmtArena::ifs`, the type
    /// pairs): the record there is described by its Rust type.
    Side(&'static str),
    /// A typed Rust value (a cold map's value), named by its type.
    Record(&'static str),
    /// The discriminant of the named enum (`expr::CastKind`) or a small
    /// numbered code, at most `max`.
    Enum {
        /// The enum's path below `kiln_ast`, or what the code names.
        ty: &'static str,
        /// The largest valid value.
        max: u32,
    },
    /// 0 or 1.
    Bool,
    /// An integer (a count, an index, a value, packed fields the named
    /// item documents).
    Value(&'static str),
    /// Bit fields.
    Bits(&'static [Field]),
    /// `then` when `when` holds for the node, else `otherwise`.
    Cond {
        /// The condition.
        when: When,
        /// The role when it holds.
        then: &'static Role,
        /// The role when it does not.
        otherwise: &'static Role,
    },
}

/// A bit field of a word: `width` bits from bit `lo`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Field {
    /// Its name (the constant or accessor that reads it).
    pub name: &'static str,
    /// The lowest bit.
    pub lo: u8,
    /// The number of bits.
    pub width: u8,
    /// What the field holds.
    pub role: &'static Role,
}

impl Field {
    /// The field's bits in the word.
    pub const fn mask(&self) -> u32 {
        (((1u64 << self.width) - 1) << self.lo) as u32
    }
}

/// The condition of a [`Role::Cond`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum When {
    /// A bit of the node's flags is set: [`AttrNode`] `flags`, an
    /// expression's `raw_bits`, a type's `flags`, a statement's `flags`.
    Flag(u8),
    /// A bit of the node's `aux` is set.
    Aux(u16),
    /// The node's `aux` equals the value.
    AuxIs(u16),
    /// The node's `a` word equals the value (a marker such as an
    /// attribute's "not checked yet").
    AIs(u32),
    /// The node's `b` word equals the value.
    BIs(u32),
}

/// What a [`Role::Cond`] reads of a node: its flags, `aux` and its `a`
/// and `b` words (0 where the node has none).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Words {
    /// [`AttrNode`] `flags`, an expression's `raw_bits`, a statement's
    /// or a type's flags.
    pub flags: u8,
    /// The node's `aux`.
    pub aux: u16,
    /// The node's `a` word.
    pub a: u32,
    /// The node's `b` word.
    pub b: u32,
}

impl Words {
    /// A node with these flags, the other words 0.
    pub const fn flags(flags: u8) -> Words {
        Words { flags, aux: 0, a: 0, b: 0 }
    }

    /// The final (checked) form of an attribute, as kiln's module image
    /// carries it: [`AttrNode::CHECKED`] set and no marker word.
    pub const CHECKED_ATTR: Words = Words::flags(AttrNode::CHECKED);
}

impl Role {
    /// The role after resolving [`Role::Cond`]s for a node with these
    /// words (other roles are returned as they are).
    pub fn resolve(&self, n: Words) -> &Role {
        let mut r = self;
        while let Role::Cond { when, then, otherwise } = r {
            let holds = match *when {
                When::Flag(m) => n.flags & m != 0,
                When::Aux(m) => n.aux & m != 0,
                When::AuxIs(v) => n.aux == v,
                When::AIs(v) => n.a == v,
                When::BIs(v) => n.b == v,
            };
            r = if holds { then } else { otherwise };
        }
        r
    }
}

/// The words of one type kind (spec/types.md §5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TypeRoles {
    /// The kind.
    pub kind: TypeKind,
    /// `aux` (16 bits).
    pub aux: Role,
    /// `data`: the operand, or the index of its pair, triple, array or
    /// prototype record ([`Role::Side`]).
    pub data: Role,
    /// The pair or triple words `data` indexes, in order (empty for an
    /// inline kind and for `ConstantArray`/`Function`, whose records are
    /// typed: `ConstArray`, `FuncProto`).
    pub words: &'static [Role],
}

/// The words of one expression kind (spec/ast.md §5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ExprRoles {
    /// The kind.
    pub kind: ExprKind,
    /// `a`.
    pub a: Role,
    /// `b`.
    pub b: Role,
    /// `aux` (8 bits).
    pub aux: Role,
    /// The per-kind flags, bits 4-7 of `raw_bits` (bits 0-1 are the value
    /// category, 2-3 the object kind, for every kind).
    pub flags: &'static [Field],
}

/// The words of one statement kind (spec/ast.md §6). A statement's
/// `flags` byte is 0 for every kind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StmtRoles {
    /// The kind.
    pub kind: StmtKind,
    /// `a`.
    pub a: Role,
    /// `b`.
    pub b: Role,
    /// `aux` (16 bits).
    pub aux: Role,
}

/// The words of one attribute kind (spec/ast.md §7). The bits of
/// [`ATTR_FLAGS`] apply to every kind; `flags` lists the kind's own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AttrRoles {
    /// The kind.
    pub kind: AttrKind,
    /// `a`.
    pub a: Role,
    /// `b`.
    pub b: Role,
    /// The kind's own flag bits (bits 2, 4, 5).
    pub flags: &'static [Field],
}

/// One cold side map: `owner.map` maps a `key` to a `value`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ColdMapRoles {
    /// The struct holding the map (`ColdDeclData`).
    pub owner: &'static str,
    /// The field.
    pub map: &'static str,
    /// The key.
    pub key: Role,
    /// The value.
    pub value: Role,
}

/// The roles of type kind `k`.
pub fn type_roles(k: TypeKind) -> &'static TypeRoles {
    &TYPE_ROLES[k as usize]
}

/// The roles of expression kind `k`.
pub fn expr_roles(k: ExprKind) -> &'static ExprRoles {
    &EXPR_ROLES[k as usize]
}

/// The roles of statement kind `k`.
pub fn stmt_roles(k: StmtKind) -> &'static StmtRoles {
    &STMT_ROLES[k as usize]
}

/// The roles of attribute kind `k` (`None` for a number no kind has).
pub fn attr_roles(k: AttrKind) -> Option<&'static AttrRoles> {
    ATTR_ROLES.binary_search_by_key(&k.0, |r| r.kind.0).ok().map(|i| &ATTR_ROLES[i])
}

// Shorthands for the rows.
const Z: Role = Role::Zero;
const E: Role = Role::Expr;
const S: Role = Role::Stmt;
const D: Role = Role::Decl;
const T: Role = Role::Type;
const L: Role = Role::Loc;
const B: Role = Role::Bool;
const OE: Role = Role::Opt(&Role::Expr);
const OD: Role = Role::Opt(&Role::Decl);
const OT: Role = Role::Opt(&Role::Type);
const EL: Role = Role::List(&Role::Expr);
const DL: Role = Role::List(&Role::Decl);
const SL: Role = Role::List(&Role::Stmt);
/// An operand that is an `ExprId`, or the profile of the canonical form
/// (`aux` bit 8, `AUX_PROFILE`).
const PROFILED: Role = Role::Cond { when: When::Aux(0x100), then: &Role::Profile, otherwise: &E };
/// A cast's base path (`Ast::cast_path`): base-specifier indices.
const PATH: Role = Role::Opt(&Role::List(&Role::Value("base-specifier index")));

const fn v(what: &'static str) -> Role {
    Role::Value(what)
}
const fn en(ty: &'static str, max: u32) -> Role {
    Role::Enum { ty, max }
}
const fn bit(name: &'static str, lo: u8) -> Field {
    Field { name, lo, width: 1, role: &Role::Bool }
}
const fn ty(kind: TypeKind, aux: Role, data: Role, words: &'static [Role]) -> TypeRoles {
    TypeRoles { kind, aux, data, words }
}
const fn ex(kind: ExprKind, a: Role, b: Role, aux: Role, flags: &'static [Field]) -> ExprRoles {
    ExprRoles { kind, a, b, aux, flags }
}
const fn st(kind: StmtKind, a: Role, b: Role, aux: Role) -> StmtRoles {
    StmtRoles { kind, a, b, aux }
}
const fn at(kind: AttrKind, a: Role, b: Role) -> AttrRoles {
    AttrRoles { kind, a, b, flags: &[] }
}
const fn atf(kind: AttrKind, a: Role, b: Role, flags: &'static [Field]) -> AttrRoles {
    AttrRoles { kind, a, b, flags }
}
const fn cold(owner: &'static str, map: &'static str, key: Role, value: Role) -> ColdMapRoles {
    ColdMapRoles { owner, map, key, value }
}

const PAIR: Role = Role::Side("pairs");
const TRIPLE: Role = Role::Side("triples");
const PROFILE_BIT: Field = bit("AUX_PROFILE", 8);
const TAG_KEYWORD: Role = en("types::TagKeyword", 6);

/// Every type kind, by discriminant.
pub static TYPE_ROLES: [TypeRoles; TypeKind::COUNT] = {
    use TypeKind as K;
    [
        ty(K::Invalid, Z, Z, &[]),
        ty(K::Builtin, Z, en("types::BuiltinType", 95), &[]),
        ty(K::BitInt, B, v("width in bits"), &[]),
        ty(K::DependentBitInt, B, Role::Profile, &[]),
        ty(K::Pointer, Z, T, &[]),
        ty(K::LValueRef, Z, T, &[]),
        ty(K::RValueRef, Z, T, &[]),
        ty(K::MemberPointer, Z, PAIR, &[T, Role::TypeId]),
        ty(K::ConstantArray, Z, Role::Side("arrays: types::ConstArray"), &[]),
        ty(K::IncompleteArray, Z, T, &[]),
        ty(K::VariableArray, Z, PAIR, &[T, E]),
        ty(K::DependentSizedArray, Role::Bits(&[PROFILE_BIT]), PAIR, &[T, PROFILED]),
        ty(K::Function, Z, Role::Side("protos: types::FuncProto"), &[]),
        // `aux` 1: the dependent type of the declaration (its own id).
        ty(K::Record, B, D, &[]),
        ty(K::Enum, B, D, &[]),
        ty(K::Complex, Z, T, &[]),
        ty(K::Atomic, Z, T, &[]),
        ty(K::Vector, en("types::VectorKind", 5), PAIR, &[T, v("element count")]),
        ty(
            K::DependentVector,
            Role::Bits(&[
                Field { name: "kind", lo: 0, width: 8, role: &en("types::VectorKind", 5) },
                PROFILE_BIT,
            ]),
            PAIR,
            &[T, PROFILED],
        ),
        ty(K::ExtQual, Z, PAIR, &[T, v("types::AddrSpace word")]),
        ty(K::Typedef, Z, PAIR, &[D, T]),
        ty(K::Using, Z, PAIR, &[D, T]),
        ty(K::Elaborated, TAG_KEYWORD, PAIR, &[Role::Opt(&Role::Nns), T]),
        ty(K::QualifiedArray, v("types::Quals bits"), Role::TypeId, &[]),
        ty(K::TemplateTypeParm, Z, D, &[]),
        ty(
            K::CanonTemplateTypeParm,
            Role::Bits(&[
                Field { name: "depth", lo: 0, width: 15, role: &v("template depth") },
                bit("pack", 15),
            ]),
            v("template parameter index"),
            &[],
        ),
        ty(K::SubstTemplateTypeParm, v("pack index + 1, 0: none"), TRIPLE, &[T, D, v("index")]),
        ty(K::SubstTemplateTypeParmPack, Z, TRIPLE, &[D, v("index"), Role::ArgList]),
        ty(
            K::TemplateSpecialization,
            Role::Bits(&[bit("alias", 0)]),
            TRIPLE,
            &[Role::TemplateName, Role::ArgList, OT],
        ),
        ty(K::InjectedClassName, Z, PAIR, &[D, T]),
        ty(K::DependentName, TAG_KEYWORD, PAIR, &[Role::Nns, Role::Symbol]),
        ty(
            K::DependentTemplateSpecialization,
            TAG_KEYWORD,
            TRIPLE,
            &[Role::Nns, Role::Symbol, Role::ArgList],
        ),
        ty(K::PackExpansion, Z, PAIR, &[T, v("expansion count + 1, 0: unknown")]),
        ty(K::PackIndexing, Role::Bits(&[PROFILE_BIT]), TRIPLE, &[T, PROFILED, OT]),
        ty(K::Decltype, Z, PAIR, &[E, T]),
        ty(K::DependentDecltype, Z, Role::Profile, &[]),
        ty(
            K::TypeOf,
            Role::Bits(&[bit("TYPEOF_UNQUAL", 0), bit("TYPEOF_TYPE", 1), PROFILE_BIT]),
            PAIR,
            &[
                Role::Cond {
                    when: When::Aux(0x100),
                    then: &Role::Profile,
                    otherwise: &Role::Cond { when: When::Aux(2), then: &T, otherwise: &E },
                },
                OT,
            ],
        ),
        ty(K::UnaryTransform, en("types::UnaryTransformKind", 15), PAIR, &[T, OT]),
        ty(
            K::Auto,
            Role::Bits(&[
                Field { name: "keyword", lo: 0, width: 2, role: &en("types::AutoKeyword", 2) },
                bit("pack", 2),
            ]),
            TRIPLE,
            &[OT, OD, Role::Opt(&Role::ArgList)],
        ),
        ty(K::DeducedTemplateSpecialization, Z, PAIR, &[Role::TemplateName, OT]),
        ty(
            K::Attributed,
            Role::Bits(&[
                Field { name: "kind", lo: 0, width: 8, role: &en("types::TypeAttrKind", 8) },
                Field {
                    name: "spelling",
                    lo: 8,
                    width: 8,
                    role: &v("a CallingConv node's written spelling + 1"),
                },
            ]),
            PAIR,
            &[T, T],
        ),
        ty(K::Splice, Role::Bits(&[PROFILE_BIT]), PAIR, &[PROFILED, OT]),
        // `aux` is always 1 (dependent).
        ty(K::UnresolvedUsing, B, D, &[]),
        ty(K::Paren, Z, T, &[]),
        ty(K::DependentAddrSpace, Role::Bits(&[PROFILE_BIT]), PAIR, &[T, PROFILED]),
        ty(K::Decayed, Z, PAIR, &[T, T]),
    ]
};

const ARG_IS_TYPE: When = When::Flag(1 << 4);
const CAST: Role = en("expr::CastKind", 46);
const CAST_FLAGS: &[Field] = &[];
const BINOP: Role = en("expr::BinaryOp", 32);
const NON_ODR: Role = en("expr::NonOdrUse", 3);
const SATISFACTION: Role = en("satisfaction: 0 not computed, 1 false, 2 true", 2);
const CONSTRUCT: Role = Role::Bits(&[
    Field { name: "kind", lo: 0, width: 2, role: &en("expr::ConstructionKind", 3) },
    bit("list", 2),
    bit("std_init_list", 3),
    bit("zeroing", 4),
    bit("immediate_escalating", 5),
]);
const CONSTRUCT_FLAGS: &[Field] = &[bit("HAD_MULTIPLE_CANDIDATES", 6)];
const COAWAIT_AUX: Role = Role::Bits(&[
    bit("implicit", 0),
    Field { name: "suspend", lo: 1, width: 2, role: &en("expr::SuspendKind", 2) },
]);
const OP_OPERAND: Role = Role::Cond { when: ARG_IS_TYPE, then: &T, otherwise: &E };

/// Every expression kind, by discriminant.
pub static EXPR_ROLES: [ExprRoles; ExprKind::IntegerPackExpr as usize + 1] = {
    use ExprKind as X;
    [
        ex(
            X::IntegerLiteral,
            Role::Cond {
                when: When::Flag(1 << 4),
                then: &Role::ConstValue,
                otherwise: &v("value bits 0..32"),
            },
            Role::Cond { when: When::Flag(1 << 4), then: &Z, otherwise: &v("value bits 32..64") },
            en("expr::Radix", 3),
            &[bit("WIDE", 4)],
        ),
        ex(X::FloatingLiteral, Role::ConstValue, Z, Z, &[]),
        ex(X::CharacterLiteral, v("code unit value"), Z, en("expr::CharKind", 4), &[]),
        ex(X::StringLiteral, Role::StringLit, Z, en("expr::StringKind", 5), &[]),
        ex(X::CXXBoolLiteralExpr, B, Z, Z, &[]),
        ex(X::CXXNullPtrLiteralExpr, Z, Z, Z, &[]),
        ex(X::UserDefinedLiteral, Z, Z, Z, &[]),
        ex(X::ImaginaryLiteral, E, Z, Z, &[]),
        ex(X::EmbedExpr, Z, Z, Z, &[]),
        ex(X::GNUNullExpr, Z, Z, Z, &[]),
        ex(
            X::DeclRefExpr,
            D,
            Role::Opt(&Role::Side("declref_extras: expr::DeclRefExtra")),
            NON_ODR,
            &[
                bit("NAMES_CLOSURE_COPY", 4),
                bit("REFERS_TO_ENCLOSING", 5),
                bit("HAD_MULTIPLE_CANDIDATES", 6),
                bit("IMMEDIATE_ESCALATING", 7),
            ],
        ),
        ex(
            X::MemberExpr,
            E,
            D,
            NON_ODR,
            &[bit("ARROW", 4), bit("SPLICED", 5), bit("HAD_MULTIPLE_CANDIDATES", 6)],
        ),
        ex(X::CXXThisExpr, Z, Z, B, &[]),
        ex(X::PredefinedExpr, OE, OD, en("expr::PredefinedKind", 2), &[]),
        ex(X::SourceLocExpr, D, Z, en("expr::SourceLocKind", 5), &[]),
        ex(X::UnresolvedLookupExpr, Role::DeclName, DL, Role::Bits(&[bit("adl", 0)]), &[]),
        ex(X::UnresolvedMemberExpr, Z, Z, Z, &[]),
        ex(X::DependentScopeDeclRefExpr, Role::Nns, Role::DeclName, Z, &[]),
        ex(
            X::CXXDependentScopeMemberExpr,
            OE,
            Role::DeclName,
            Role::Bits(&[bit("arrow", 0), bit("template", 1)]),
            &[],
        ),
        ex(X::SubstNonTypeTemplateParmExpr, D, E, Z, &[bit("REFERENCE_PARAMETER", 4)]),
        ex(X::SubstNonTypeTemplateParmPackExpr, D, Role::ArgList, Z, &[]),
        ex(X::FunctionParmPackExpr, D, DL, Z, &[]),
        ex(X::UnaryOperator, E, Z, en("expr::UnaryOp", 13), &[bit("CANNOT_OVERFLOW", 4)]),
        ex(X::BinaryOperator, E, E, BINOP, &[]),
        ex(
            X::CompoundAssignOperator,
            E,
            Role::Side("compound_assigns: expr::CompoundAssignExtra"),
            BINOP,
            &[],
        ),
        ex(X::ConditionalOperator, E, Role::Side("cond_arms: [then, else]"), Z, &[]),
        ex(X::BinaryConditionalOperator, EL, Z, Z, &[]),
        ex(X::ArraySubscriptExpr, E, E, Z, &[]),
        ex(X::ParenExpr, E, L, Z, &[]),
        ex(X::CallExpr, E, EL, Z, &[bit("ADL", 5), bit("ASSUME_IGNORED", 6)]),
        ex(X::CXXMemberCallExpr, E, EL, Z, &[]),
        ex(X::CXXOperatorCallExpr, E, EL, en("names::OverloadedOperator", 44), &[bit("ADL", 5)]),
        ex(X::CXXRewrittenBinaryOperator, E, EL, BINOP, &[]),
        ex(X::ParenListExpr, EL, Z, Z, &[]),
        ex(X::ImplicitCastExpr, E, PATH, CAST, &[bit("PART_OF_EXPLICIT_CAST", 4)]),
        ex(X::CStyleCastExpr, E, PATH, CAST, CAST_FLAGS),
        ex(X::CXXFunctionalCastExpr, E, PATH, CAST, CAST_FLAGS),
        ex(X::CXXStaticCastExpr, E, PATH, CAST, CAST_FLAGS),
        ex(X::CXXConstCastExpr, E, PATH, CAST, CAST_FLAGS),
        ex(X::CXXReinterpretCastExpr, E, PATH, CAST, CAST_FLAGS),
        ex(X::CXXDynamicCastExpr, E, PATH, CAST, CAST_FLAGS),
        ex(X::BuiltinBitCastExpr, E, PATH, CAST, CAST_FLAGS),
        ex(X::InitListExpr, EL, Role::Side("init_lists: expr::InitListExtra"), Z, &[]),
        ex(X::ImplicitValueInitExpr, Z, Z, Z, &[]),
        ex(X::DesignatedInitExpr, E, Z, Z, &[]),
        ex(X::CXXConstructExpr, D, EL, CONSTRUCT, CONSTRUCT_FLAGS),
        ex(X::CXXTemporaryObjectExpr, D, EL, CONSTRUCT, CONSTRUCT_FLAGS),
        ex(
            X::CXXInheritedCtorInitExpr,
            D,
            Z,
            Role::Bits(&[
                Field { name: "kind", lo: 0, width: 2, role: &en("expr::ConstructionKind", 3) },
                bit("inherited_from_vbase", 2),
            ]),
            &[],
        ),
        ex(X::CXXDefaultArgExpr, D, D, Z, &[]),
        ex(X::CXXDefaultInitExpr, D, D, en("rewritten: always 1", 1), &[]),
        ex(X::CXXScalarValueInitExpr, Z, Z, Z, &[]),
        ex(X::CXXParenListInitExpr, EL, OE, Z, &[]),
        ex(X::ArrayInitLoopExpr, E, E, Z, &[]),
        ex(X::ArrayInitIndexExpr, Z, Z, Z, &[]),
        ex(X::CXXStdInitializerListExpr, E, Z, Z, &[]),
        ex(
            X::CompoundLiteralExpr,
            E,
            Role::Cond { when: When::AuxIs(1), then: &D, otherwise: &Z },
            B,
            &[],
        ),
        ex(X::NoInitExpr, Z, Z, Z, &[]),
        ex(X::DesignatedInitUpdateExpr, E, E, Z, &[]),
        ex(
            X::MaterializeTemporaryExpr,
            E,
            OD,
            Role::Bits(&[
                Field { name: "storage", lo: 0, width: 2, role: &en("expr::StorageDuration", 3) },
                bit("bound_to_lvalue_ref", 2),
            ]),
            &[],
        ),
        ex(X::CXXBindTemporaryExpr, E, D, Z, &[]),
        ex(X::ExprWithCleanups, E, Z, B, &[]),
        ex(X::OpaqueValueExpr, OE, Z, Z, &[]),
        ex(
            X::CXXNewExpr,
            Role::Side("news: expr::NewExtra"),
            Z,
            en("expr::NewInitStyle", 2),
            &[bit("GLOBAL", 4), bit("ARRAY_FORM", 5), bit("PASS_ALIGN", 6)],
        ),
        ex(
            X::CXXDeleteExpr,
            E,
            OD,
            Role::Bits(&[bit("sized", 0), bit("aligned", 1)]),
            &[bit("GLOBAL", 4), bit("ARRAY_FORM", 5), bit("ARRAY_AS_WRITTEN", 6)],
        ),
        ex(
            X::CXXPseudoDestructorExpr,
            E,
            T,
            Role::Bits(&[bit("arrow", 0), bit("qualified", 1)]),
            &[],
        ),
        ex(X::CXXThrowExpr, OE, Z, Z, &[]),
        ex(X::CXXNoexceptExpr, E, B, Z, &[]),
        ex(X::CXXTypeidExpr, OP_OPERAND, Z, B, &[bit("ARG_IS_TYPE", 4)]),
        ex(
            X::UnaryExprOrTypeTraitExpr,
            OP_OPERAND,
            Z,
            en("expr::TraitKind", 4),
            &[bit("ARG_IS_TYPE", 4)],
        ),
        ex(
            X::TypeTraitExpr,
            Role::List(&T),
            v("the value: 0 or 1, or a count; 0 while dependent"),
            en("expr::TypeTrait", 92),
            &[],
        ),
        ex(X::ArrayTypeTraitExpr, T, Z, en("expr::TypeTrait", 92), &[]),
        ex(X::ExpressionTraitExpr, E, B, en("expr::TypeTrait", 92), &[]),
        ex(X::OffsetOfExpr, EL, v("byte offset"), B, &[]),
        ex(X::SizeOfPackExpr, D, Z, Z, &[]),
        ex(X::RequiresExpr, D, Role::Side("templates::RequiresData"), SATISFACTION, &[]),
        ex(X::ConceptSpecializationExpr, D, Role::ArgList, SATISFACTION, &[]),
        ex(X::LambdaExpr, Role::Lambda, Z, Z, &[]),
        ex(X::PackExpansionExpr, E, Z, Z, &[]),
        ex(X::CXXFoldExpr, OE, OE, BINOP, &[]),
        ex(X::PackIndexingExpr, E, E, Z, &[]),
        ex(X::CXXUnresolvedConstructExpr, EL, Z, B, &[]),
        ex(X::CoawaitExpr, EL, Z, COAWAIT_AUX, &[]),
        ex(X::DependentCoawaitExpr, E, Z, Z, &[]),
        ex(X::CoyieldExpr, EL, Z, COAWAIT_AUX, &[]),
        ex(
            X::ConstantExpr,
            E,
            Role::Opt(&Role::ConstValue),
            Role::Bits(&[bit("CONSTANT_ELIDED", 0), bit("CONSTANT_VALUE_HIDDEN", 1)]),
            &[],
        ),
        ex(X::StmtExpr, S, Z, Z, &[]),
        ex(X::AddrLabelExpr, D, Z, Z, &[]),
        ex(
            X::ChooseExpr,
            E,
            Role::Side("cond_arms: [lhs, rhs]"),
            en("0 dependent, 1 lhs chosen, 2 rhs chosen", 2),
            &[],
        ),
        ex(X::VAArgExpr, E, T, Z, &[]),
        ex(X::AtomicExpr, EL, Z, en("decl::builtin::atomic id", 38), &[]),
        ex(
            X::ShuffleVectorExpr,
            EL,
            Role::Cond {
                when: When::Aux(1),
                then: &Role::List(&v("lane index, u32::MAX: poison")),
                otherwise: &Z,
            },
            B,
            &[],
        ),
        ex(X::ConvertVectorExpr, E, Z, Z, &[]),
        ex(X::ExtVectorElementExpr, E, Role::Symbol, Z, &[bit("ARROW", 4)]),
        ex(
            X::CXXReflectExpr,
            Role::Cond {
                when: When::AuxIs(12),
                then: &E,
                otherwise: &v("the reflectee's payload word (reflect::Reflectee)"),
            },
            Role::Opt(&Role::Reflection),
            en("reflect::Reflectee tag; 12: a dependent name", 12),
            &[],
        ),
        ex(X::CXXSpliceExpr, E, OE, Role::Bits(&[bit("arrow", 0), bit("template", 1)]), &[]),
        ex(X::RecoveryExpr, EL, Z, Z, &[]),
        ex(
            X::CXXExpansionSelectExpr,
            v("synthetic template depth"),
            Z,
            en("stmt::ExpansionForm", 3),
            &[],
        ),
        ex(X::PseudoObjectExpr, EL, v("index of the result among the semantic forms"), Z, &[]),
        ex(
            X::GenericSelectionExpr,
            Role::List(&v("control, its loc, then (type or 0, loc, expr) per association")),
            OE,
            Z,
            &[bit("ARG_IS_TYPE", 4)],
        ),
        ex(X::IntegerPackExpr, E, Z, Z, &[]),
    ]
};

/// Every statement kind, by discriminant.
pub static STMT_ROLES: [StmtRoles; StmtKind::ContractAssertStmt as usize + 1] = {
    use StmtKind as K;
    [
        st(K::CompoundStmt, SL, L, Role::Opt(&Role::Side("fp_overrides: fp::FpOverride"))),
        st(K::DeclStmt, DL, Z, Z),
        st(K::ExprStmt, E, Z, Z),
        st(K::NullStmt, Z, Z, Z),
        st(K::IfStmt, Role::Side("ifs: stmt::IfData"), Z, en("stmt::IfKind", 3)),
        st(K::SwitchStmt, Role::Side("switches: stmt::SwitchData"), Z, Z),
        st(K::CaseStmt, Role::Side("cases: stmt::CaseData"), Z, Z),
        st(K::DefaultStmt, S, Z, Z),
        st(K::BreakStmt, Z, Z, Z),
        st(K::ContinueStmt, Z, Z, Z),
        st(K::WhileStmt, Role::Side("loops: stmt::LoopData"), Z, Z),
        st(K::DoStmt, Role::Side("loops: stmt::LoopData"), L, Z),
        st(K::ForStmt, Role::Side("loops: stmt::LoopData"), Z, Z),
        st(K::ReturnStmt, OE, OD, Z),
        st(K::LabelStmt, D, S, Z),
        st(K::GotoStmt, D, Z, Z),
        st(K::IndirectGotoStmt, E, L, Z),
        st(K::AttributedStmt, Role::List(&Role::Attr), S, Z),
        st(K::CXXForRangeStmt, Role::Side("range_fors: stmt::RangeForData"), Z, Z),
        st(
            K::CXXExpansionStmt,
            Role::Side("expansions: stmt::ExpansionData"),
            v("synthetic template depth"),
            Role::Bits(&[
                Field { name: "form", lo: 0, width: 2, role: &en("stmt::ExpansionForm", 3) },
                bit("EXPANDED", 2),
            ]),
        ),
        st(K::CXXTryStmt, S, SL, Z),
        st(K::CXXCatchStmt, OD, S, Z),
        st(K::CoroutineBodyStmt, Role::Side("coros: stmt::CoroutineBodyData"), Z, Z),
        st(K::CoreturnStmt, OE, OE, B),
        st(
            K::GCCAsmStmt,
            Role::Side("asms: stmt::AsmData"),
            Z,
            Role::Bits(&[bit("VOLATILE", 0), bit("GOTO", 1), bit("INLINE", 2), bit("BASIC", 3)]),
        ),
        st(
            K::ContractAssertStmt,
            E,
            OD,
            Role::Bits(&[
                Field { name: "kind", lo: 0, width: 2, role: &en("stmt::ContractKind", 2) },
                Field { name: "semantic", lo: 2, width: 2, role: &v("evaluation-semantic slot") },
            ]),
        ),
    ]
};

/// The flag bits every attribute kind has.
pub const ATTR_FLAGS: &[Field] =
    &[bit("IMPLICIT", 0), bit("inherited", 1), bit("GNU_SCOPED", 3), bit("CHECKED", 6)];

const CHECKED: When = When::Flag(AttrNode::CHECKED);
/// Before the checks: the raw `ExprId` of the first argument as written
/// (0: none), `b` the argument count.
const ARG0: Role = Role::Opt(&Role::Expr);
const ARGC: Role = Role::Value("argument count");
const VISIBILITY: Role = en("visibility: 0 default, 1 hidden, 2 protected", 2);
/// `visibility` / `type_visibility` before the check: the `a` marker.
const VIS_UNCHECKED: When = When::AIs(u32::MAX);
const VIS_A: Role =
    Role::Cond { when: VIS_UNCHECKED, then: &v("UNCHECKED"), otherwise: &VISIBILITY };
const VIS_ARG: Role = Role::Cond { when: When::Flag(4), then: &L, otherwise: &E };
const VIS_B: Role = Role::Cond {
    when: VIS_UNCHECKED,
    then: &VIS_ARG,
    otherwise: &en("1: hidden written as internal", 1),
};
const TYPE_VIS_B: Role = Role::Cond { when: VIS_UNCHECKED, then: &VIS_ARG, otherwise: &Z };
const MSG: Role = Role::Opt(&Role::Expr);
/// Every attribute kind, by number.
pub static ATTR_ROLES: &[AttrRoles] = {
    use AttrKind as K;
    &[
        // `alignas(E)` / `aligned(E)`: the expression; `alignas(type-id)`: `b`.
        at(K::ALIGNED, OE, OT),
        // `a` the assumption, `b` the raw `Loc` constant evaluation reports.
        atf(K::ASSUME, E, L, &[bit("ASSUME_IGNORED", 2)]),
        at(K::CARRIES_DEPENDENCY, Z, Z),
        // The message string literal.
        at(K::DEPRECATED, MSG, Z),
        atf(K::FALLTHROUGH, Z, Z, &[bit("FALLTHROUGH_SCOPED", 2)]),
        at(K::INDETERMINATE, Z, Z),
        at(K::LIKELY, Z, Z),
        at(K::UNLIKELY, Z, Z),
        at(K::MAYBE_UNUSED, Z, Z),
        atf(K::NODISCARD, MSG, Z, &[bit("CLANG_SCOPED", 2)]),
        at(K::NORETURN, Z, Z),
        at(K::NO_UNIQUE_ADDRESS, Z, Z),
        at(K::ANNOTATION, E, Z),
        at(K::ALWAYS_INLINE, Z, Z),
        at(K::NOINLINE, Z, Z),
        at(K::PACKED, Z, Z),
        at(K::USED, Z, Z),
        at(K::UNUSED, Z, Z),
        at(K::WEAK, Z, Z),
        // Before the check `a` is `UNCHECKED` (`u32::MAX`) and `b` the
        // argument: its `ExprId`, or the `Loc` of an identifier argument
        // (`IDENT_ARG`); the check pushes a new node with the value.
        atf(K::VISIBILITY, VIS_A, VIS_B, &[bit("IDENT_ARG", 2)]),
        at(K::SECTION, Role::Opt(&Role::StringLit), Z),
        at(K::FORMAT, v("archetype (0 printf)"), v("F | A << 16, one-based argument indices")),
        at(K::CONST, Z, Z),
        at(K::PURE, Z, Z),
        at(K::COLD, Z, Z),
        at(K::NOTHROW, Z, Z),
        // The condition and the message string literal.
        at(K::ENABLE_IF, E, E),
        atf(
            K::DIAGNOSE_IF,
            E,
            E,
            &[bit("error", 2), bit("names a parameter", 4), bit("warning group", 5)],
        ),
        at(K::TRIVIAL_ABI, Z, Z),
        at(K::PREFERRED_NAME, Z, T),
        at(K::EXCLUDE_FROM_EXPLICIT_INSTANTIATION, Z, Z),
        // The sorted, `,`-joined tags.
        at(K::ABI_TAG, Role::Symbol, Z),
        at(K::INTERNAL_LINKAGE, Z, Z),
        atf(
            K::LIFETIMEBOUND,
            en("1: the implicit object parameter", 1),
            Z,
            &[bit("LIFETIMEBOUND_ARGS", 2)],
        ),
        at(
            K::NO_SANITIZE,
            v("sanitize::no_sanitize_words set"),
            v("sanitize::no_sanitize_words spelling"),
        ),
        atf(
            K::MODE,
            Role::Cond {
                when: When::Flag(AttrNode::MODE_DEFERRED),
                then: &v("the mode word, applied at instantiation"),
                otherwise: &v("width | kind << 8 | complex << 11 | vector << 12"),
            },
            Role::Symbol,
            &[bit("MODE_DEFERRED", 2)],
        ),
        at(K::VECTOR_SIZE, E, Z),
        at(K::ASM_LABEL, Role::StringLit, Z),
        at(K::BUILTIN, Role::Builtin, Z),
        at(K::EXT_VECTOR_TYPE, E, Z),
        at(K::MALLOC, Z, Z),
        // `IntegerLiteral`s: the one-based parameter indices.
        at(K::ALLOC_SIZE, E, OE),
        at(K::ALLOC_ALIGN, E, Z),
        at(K::USING_IF_EXISTS, Z, Z),
        at(K::HOT, Z, Z),
        at(K::FLAG_ENUM, Z, Z),
        at(K::ENUM_EXTENSIBILITY, en("1 open, 0 closed", 1), Z),
        at(K::CORO_ONLY_DESTROY_WHEN_COMPLETE, Z, Z),
        at(K::OPTNONE, Z, Z),
        at(K::MINSIZE, Z, Z),
        at(K::NAKED, Z, Z),
        at(K::MAY_ALIAS, Z, Z),
        // `IntegerLiteral`s: the one-based parameter indices; empty: all.
        at(K::NONNULL, EL, Z),
        at(K::RETURNS_NONNULL, Z, Z),
        at(K::INIT_PRIORITY, v("priority"), Z),
        at(K::LOOP_HINT, OE, v("attr::LoopHint::pack: option | state << 8 | spelling << 16")),
        at(K::UNINITIALIZED, Z, Z),
        atf(K::TYPE_VISIBILITY, VIS_A, TYPE_VIS_B, &[bit("IDENT_ARG", 2)]),
        at(K::TYPE_NULLABLE, Z, Z),
        at(K::NEON_VECTOR_TYPE, E, Z),
        at(K::NEON_POLYVECTOR_TYPE, E, Z),
        at(K::TARGET, Role::Opt(&Role::StringLit), Z),
        at(K::GNU_INLINE, Z, Z),
        at(K::NODEBUG, Z, Z),
        at(K::MIN_VECTOR_WIDTH, v("width"), Z),
        at(K::WEAKREF, Role::Opt(&Role::StringLit), Z),
        at(K::ALIAS, Role::StringLit, L),
        at(K::ANALYZER_NORETURN, Z, Z),
        at(K::DISABLE_TAIL_CALLS, Z, Z),
        at(K::IFUNC, Role::StringLit, L),
        at(K::ARTIFICIAL, Z, Z),
        at(K::PASS_OBJECT_SIZE, en("object-size type", 3), en("1: the dynamic spelling", 1)),
        at(K::OVERLOADABLE, Z, Z),
        atf(
            K::CONSTRUCTOR,
            Role::Cond {
                when: CHECKED,
                then: &Role::Cond {
                    when: When::Flag(AttrNode::PRIORITY_DEPENDENT),
                    then: &E,
                    otherwise: &v("priority"),
                },
                otherwise: &ARG0,
            },
            Role::Cond { when: CHECKED, then: &Z, otherwise: &ARGC },
            &[bit("PRIORITY_DEPENDENT", 2)],
        ),
        atf(
            K::DESTRUCTOR,
            Role::Cond {
                when: CHECKED,
                then: &Role::Cond {
                    when: When::Flag(AttrNode::PRIORITY_DEPENDENT),
                    then: &E,
                    otherwise: &v("priority"),
                },
                otherwise: &ARG0,
            },
            Role::Cond { when: CHECKED, then: &Z, otherwise: &ARGC },
            &[bit("PRIORITY_DEPENDENT", 2)],
        ),
        at(
            K::CLEANUP,
            Role::Cond { when: CHECKED, then: &D, otherwise: &ARG0 },
            Role::Cond { when: CHECKED, then: &L, otherwise: &ARGC },
        ),
        at(K::COMMON, Z, Z),
        at(
            K::TLS_MODEL,
            Role::Cond { when: CHECKED, then: &en("TLS model, 1-4", 4), otherwise: &ARG0 },
            Role::Cond { when: CHECKED, then: &Z, otherwise: &ARGC },
        ),
        at(K::RETURNS_TWICE, Z, Z),
        at(K::MUSTTAIL, Z, Z),
        at(K::TRANSPARENT_UNION, Z, Z),
        at(K::SELECTANY, Z, Z),
        at(K::MS_STRUCT, Z, Z),
        at(K::RETAIN, Z, Z),
        at(K::NO_DESTROY, Z, Z),
        at(K::ALWAYS_DESTROY, Z, Z),
        atf(
            K::CALLING_CONV,
            // `a` 0xff: the convention is not supported on the target.
            Role::Cond {
                when: When::AIs(0xff),
                then: &v("UNSUPPORTED"),
                otherwise: &en("types::CallConv", 15),
            },
            v("index in attr::CC_ATTR_SPELLINGS"),
            &[bit("IN_CHUNK", 4)],
        ),
        atf(K::NO_CALLER_SAVED_REGS, Z, Z, &[bit("IN_CHUNK", 4)]),
        atf(K::REGPARM, v("N"), Z, &[bit("IN_CHUNK", 4)]),
        // The sorted, `,`-joined names (`*`: all).
        at(K::NO_BUILTIN, Role::Symbol, Z),
        at(K::GCC_STRUCT, Z, Z),
        at(K::ARM_SVE_VECTOR_BITS, E, Z),
        at(K::BUILTIN_ALIAS, Role::Symbol, en("1: the __clang_arm_builtin_alias spelling", 1)),
        at(K::TARGET_VERSION, Role::StringLit, L),
        at(K::TARGET_CLONES, Role::Symbol, en("1: kept", 1)),
        at(K::CPU_DISPATCH, Role::Symbol, L),
        at(K::CPU_SPECIFIC, Role::Symbol, L),
        at(K::NO_STACK_PROTECTOR, Z, Z),
        at(K::PATCHABLE_FUNCTION_ENTRY, v("N"), v("M")),
        at(
            K::ZERO_CALL_USED_REGS,
            en(
                "index in attr::ZERO_CALL_USED_REGS_KINDS",
                crate::attr::ZERO_CALL_USED_REGS_KINDS.len() as u32 - 1,
            ),
            Z,
        ),
        at(K::SPECULATIVE_LOAD_HARDENING, Z, Z),
        at(K::NO_SPECULATIVE_LOAD_HARDENING, Z, Z),
        at(K::NO_SPLIT_STACK, Z, Z),
        at(K::NO_INSTRUMENT_FUNCTION, Z, Z),
        at(K::NO_PROFILE_INSTRUMENT_FUNCTION, Z, Z),
        at(K::XRAY_INSTRUMENT, en("1 always, 0 never", 1), Z),
        at(K::XRAY_LOG_ARGS, v("N"), Z),
        at(K::DISABLE_SANITIZER_INSTRUMENTATION, Z, Z),
        at(K::NOCF_CHECK, Z, Z),
        at(K::FUNCTION_RETURN, en("1 thunk-extern, 0 keep", 1), Z),
        at(K::CFI_CANONICAL_JUMP_TABLE, Z, Z),
        at(K::FORCE_ALIGN_ARG_POINTER, Z, Z),
        at(K::FLATTEN, Z, Z),
        at(K::LEAF, Z, Z),
        at(K::NOMERGE, Z, Z),
        at(K::NOT_TAIL_CALLED, Z, Z),
        at(K::NODUPLICATE, Z, Z),
        at(K::CONVERGENT, Z, Z),
        at(K::NOUWTABLE, Z, Z),
        at(K::ASSUME_ALIGNED, v("alignment"), v("offset")),
        atf(
            K::ALIGN_VALUE,
            Role::Cond {
                when: When::Flag(AttrNode::ALIGN_VALUE_DEPENDENT),
                then: &E,
                otherwise: &v("alignment"),
            },
            Z,
            &[bit("ALIGN_VALUE_DEPENDENT", 2)],
        ),
        at(
            K::CALLBACK,
            Role::Cond {
                when: CHECKED,
                then: &Role::List(&v("parameter index, 0: this, u32::MAX: unknown")),
                otherwise: &Role::List(&v("argument as written: kind, value, loc")),
            },
            Z,
        ),
        at(K::NOESCAPE, Z, Z),
        at(K::MAYBE_UNDEF, Z, Z),
        at(K::CODE_ALIGN, v("N"), Z),
        at(K::LOADER_UNINITIALIZED, Z, Z),
        at(K::LTO_VISIBILITY_PUBLIC, Z, Z),
        at(K::OMP_ASSUME, Role::Opt(&Role::StringLit), Z),
        at(K::STANDALONE_DEBUG, Z, Z),
        at(K::CORO_AWAIT_ELIDABLE, Z, Z),
        at(K::CORO_AWAIT_ELIDABLE_ARGUMENT, Z, Z),
        atf(
            K::GSL_OWNER,
            Z,
            Role::Cond { when: When::Flag(AttrNode::GSL_HAS_TYPE), then: &T, otherwise: &Z },
            &[bit("GSL_HAS_TYPE", 2)],
        ),
        atf(
            K::GSL_POINTER,
            Z,
            Role::Cond { when: When::Flag(AttrNode::GSL_HAS_TYPE), then: &T, otherwise: &Z },
            &[bit("GSL_HAS_TYPE", 2)],
        ),
        at(K::UNAVAILABLE, MSG, Z),
        at(K::REQUIRE_CONSTANT_INIT, Z, Z),
        // The string literal and the extra arguments.
        at(K::ANNOTATE, OE, Role::Opt(&EL)),
        atf(
            K::SME_TYPE,
            v("types::sme bits"),
            Role::Bits(&[
                Field { name: "keyword", lo: 0, width: 4, role: &v("index in attr::SME_KEYWORDS") },
                Field { name: "za", lo: 4, width: 14, role: &v("offset of \"za\" from loc") },
                Field { name: "zt0", lo: 18, width: 14, role: &v("offset of \"zt0\" from loc") },
            ]),
            &[bit("SME_IN_TYPE", 2)],
        ),
        at(K::ARM_LOCALLY_STREAMING, Z, v("as SME_TYPE's b")),
        at(K::ARM_NEW, Role::Bits(&[bit("za", 0), bit("zt0", 1)]), v("as SME_TYPE's b")),
        at(K::NONSTRING, Z, Z),
        at(K::NOCOMMON, Z, Z),
        at(
            K::PRAGMA_CLANG_SECTION,
            Role::Opt(&Role::StringLit),
            en(
                "index in attr::CLANG_SECTION_KINDS",
                crate::attr::CLANG_SECTION_KINDS.len() as u32 - 1,
            ),
        ),
        // Before the check `a` is the argument's raw `ExprId` (0: none)
        // and `b` the argument count; the checked node has `b` 0.
        at(
            K::CODE_MODEL,
            Role::Cond { when: When::BIs(0), then: &en("1 small, 2 large", 2), otherwise: &E },
            Role::Cond { when: When::BIs(0), then: &Z, otherwise: &ARGC },
        ),
        // `a` 1: the signature check failed (the subject check drops it).
        at(
            K::X86_INTERRUPT,
            Role::Cond { when: When::AIs(1), then: &v("FAILED"), otherwise: &Z },
            Z,
        ),
        at(K::WEAK_IMPORT, Z, Z),
        at(K::SWIFT_PARAM_ABI, en("types::ParamAbi", 4), Z),
        atf(
            K::ADDRESS_SPACE,
            Role::Cond {
                when: When::Flag(AttrNode::SPACE_DEPENDENT),
                then: &E,
                otherwise: &v("types::AddrSpace word"),
            },
            Z,
            &[bit("SPACE_DEPENDENT", 2)],
        ),
    ]
};

/// Every cold side map (`ColdMap` field) of the AST.
pub static COLD_MAPS: &[ColdMapRoles] = &[
    cold("OperLookups", "by_expr", E, Role::Side("oper_lookup::OperLookups sets")),
    cold("AnnotArena", "of_decl", D, Role::List(&Role::Annotation)),
    cold(
        "AnnotArena",
        "of_base",
        Role::Record("(DeclId, u32): the class's first declaration, the base's index"),
        Role::List(&Role::Annotation),
    ),
    cold("AttrArena", "decl_attrs", D, Role::List(&Role::Attr)),
    cold("TemplateTables", "defaults", D, Role::Record("TemplateArg")),
    cold("TemplateTables", "inherited_defaults", D, D),
    cold("TemplateTables", "trailing_requires", D, E),
    cold("TemplateTables", "pattern_mem_inits", D, Role::Record("PatternMemInits")),
    cold("TemplateTables", "expr_targs", E, Role::ArgList),
    cold("TemplateTables", "expr_nns", E, Role::Nns),
    cold("TemplateTables", "expr_begin", E, L),
    cold("TemplateTables", "size_of_pack_origin", E, D),
    cold("TemplateTables", "size_of_pack_partial", E, Role::ArgList),
    cold("TemplateTables", "pack_index_origin", E, E),
    cold("TemplateTables", "member_op_loc", E, L),
    cold("TemplateTables", "pseudo_dtor_name", E, Role::Symbol),
    cold("TemplateTables", "pseudo_dtor_first", E, Role::Record("(QualType, Loc)")),
    cold("TemplateTables", "template_kw", E, L),
    cold("TemplateTables", "ttp_owner", D, D),
    cold("TemplateTables", "member_like_friend", D, D),
    cold("ColdExprData", "end_loc", E, L),
    cold("ColdExprData", "member_extra", E, Role::Record("DeclRefExtra")),
    cold("ColdExprData", "member_overload", E, Role::Record("OverloadedMember")),
    cold("ColdExprData", "designators", E, Role::Record("Box<[Designator]>")),
    cold("ColdExprData", "syntactic_list", E, E),
    cold("ColdExprData", "array_trait", E, Role::Record("ArrayTraitExtra")),
    cold("ColdExprData", "trait_rhs_loc", E, L),
    cold("ColdExprData", "source_loc", E, Role::Record("SourceLocValues")),
    cold("ColdExprData", "string_maker", E, Role::Record("StringMaker")),
    cold("ColdExprData", "default_imm", E, Role::ConstValue),
    cold("ColdExprData", "union_field", E, D),
    cold("ColdExprData", "cast_written", E, T),
    cold(
        "ColdExprData",
        "offsetof_pattern",
        E,
        Role::Record("(QualType, Box<[OffsetofComponent]>)"),
    ),
    cold("ColdExprData", "offsetof_layout", E, Role::Record("Box<[OffsetofStep]>")),
    cold("ColdExprData", "offsetof_designator", E, Role::Record("(QualType, Box<[OffsetofNode]>)")),
    cold("ColdExprData", "splice_args", E, Role::ArgList),
    cold("ColdExprData", "default_arg_rewritten", E, E),
    cold("ColdExprData", "fp_round", E, Role::Record("fp::FpRound")),
    cold("Ast", "contract_sites", S, Role::Record("stmt::ContractSite")),
    cold("ColdDeclData", "lexical_ctx", D, D),
    cold("ColdDeclData", "begin_loc", D, L),
    cold("ColdDeclData", "delete_message", D, E),
    cold("ColdDeclData", "explicit_expr", D, E),
    cold("ColdDeclData", "ctor_inits", D, Role::List(&Role::Side("DeclArena::ctor_inits"))),
    cold("ColdDeclData", "overrides", D, DL),
    cold("ColdDeclData", "friends", D, DL),
    cold("ColdDeclData", "key_function", D, D),
    cold("ColdDeclData", "dtor_delete", D, D),
    cold("ColdDeclData", "dtor_delete_this", D, E),
    cold("ColdDeclData", "record_pack", D, v("#pragma pack value")),
    cold("ColdDeclData", "contracts", D, SL),
    cold("ColdDeclData", "asm_label", D, Role::StringLit),
    cold("ColdDeclData", "alignment", D, E),
    cold("ColdDeclData", "deprecation", D, Role::StringLit),
    cold("ColdDeclData", "injection_point", D, L),
    cold("ColdDeclData", "declarator_end", D, L),
    cold("ColdDeclData", "builtin_id", D, Role::Builtin),
    cold("ColdDeclData", "gnu_lib_twin", D, Role::Builtin),
    cold("ColdDeclData", "lib_builtin", D, Role::Builtin),
    cold("ColdDeclData", "typedef_for_anon", D, D),
    cold("ColdDeclData", "decomp_bindings", D, DL),
    cold("ColdDeclData", "binding_decomp", D, D),
    cold("ColdDeclData", "binding_expr", D, E),
    cold("ColdDeclData", "binding_var", D, D),
    cold("ColdDeclData", "ctor_shadow_nominated", D, D),
    cold("ColdDeclData", "inheriting_ctor", D, D),
    cold("ColdDeclData", "inherited_ctor", D, D),
    cold("ColdDeclData", "hidden_shadow", D, D),
    cold("ColdDeclData", "using_shadows", D, DL),
    cold("ColdDeclData", "shadow_using", D, D),
    cold("ColdDeclData", "conversion_spec_target", D, T),
    cold("ColdDeclData", "compound_literal", D, E),
    cold("ColdDeclData", "init_fp", D, Role::Record("fp::FpOverride")),
    cold("ColdDeclData", "closure_owner", D, D),
    cold("ColdDeclData", "written_ns", D, D),
    cold("ColdDeclData", "friend_form", D, Role::Value("friend_forms row")),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr as x;
    use crate::types as t;

    /// The variants of `pub enum name` in `src`, in order.
    fn variants(src: &str, name: &str) -> Vec<String> {
        let start = src.find(&format!("pub enum {name} {{")).expect("the enum");
        let body = &src[start..];
        let body = &body[..body.find("\n}").expect("its end")];
        body.lines()
            .skip(1)
            .filter_map(|l| {
                let l = l.trim();
                let w = l.split([',', ' ', '=']).next()?;
                (!l.starts_with("//") && !l.starts_with('#') && w.starts_with(char::is_uppercase))
                    .then(|| w.to_owned())
            })
            .collect()
    }

    #[test]
    fn every_type_kind_has_its_row() {
        let names = variants(include_str!("types/kinds.rs"), "TypeKind");
        assert_eq!(names.len(), TYPE_ROLES.len());
        for (i, r) in TYPE_ROLES.iter().enumerate() {
            assert_eq!(r.kind as usize, i, "TYPE_ROLES[{i}] is {:?}", r.kind);
            assert_eq!(format!("{:?}", r.kind), names[i]);
            let n = match crate::types::node::shape(r.kind) {
                crate::types::node::Shape::Pair => 2,
                crate::types::node::Shape::Triple => 3,
                _ => 0,
            };
            assert_eq!(r.words.len(), n, "{:?}: the payload words", r.kind);
            assert_eq!(matches!(r.data, PAIR | TRIPLE), n != 0, "{:?}: data", r.kind);
        }
    }

    #[test]
    fn every_expr_and_stmt_kind_has_its_row() {
        let names = variants(include_str!("expr.rs"), "ExprKind");
        assert_eq!(names.len(), EXPR_ROLES.len());
        for (i, r) in EXPR_ROLES.iter().enumerate() {
            assert_eq!(r.kind as usize, i, "EXPR_ROLES[{i}] is {:?}", r.kind);
            assert_eq!(format!("{:?}", r.kind), names[i]);
            for f in r.flags {
                assert!(
                    f.lo >= 4 && f.lo + f.width <= 8,
                    "{:?}: flag {} outside 4..8",
                    r.kind,
                    f.name
                );
            }
        }
        let names = variants(include_str!("stmt.rs"), "StmtKind");
        assert_eq!(names.len(), STMT_ROLES.len());
        for (i, r) in STMT_ROLES.iter().enumerate() {
            assert_eq!(r.kind as usize, i, "STMT_ROLES[{i}] is {:?}", r.kind);
            assert_eq!(format!("{:?}", r.kind), names[i]);
        }
    }

    #[test]
    fn every_attr_kind_has_one_row() {
        let src = include_str!("attr.rs");
        let mut kinds: Vec<u16> = src
            .lines()
            .filter_map(|l| {
                let rest = l.trim().strip_prefix("pub const ")?;
                let (_, v) = rest.split_once(": AttrKind = AttrKind(")?;
                v.split(')').next()?.parse().ok()
            })
            .collect();
        kinds.sort_unstable();
        assert!(kinds.len() > 140, "the AttrKind constants were not found");
        let rows: Vec<u16> = ATTR_ROLES.iter().map(|r| r.kind.0).collect();
        assert!(rows.windows(2).all(|w| w[0] < w[1]), "ATTR_ROLES is sorted by number, once each");
        assert_eq!(rows, kinds, "ATTR_ROLES and the AttrKind constants");
        let common: u8 = ATTR_FLAGS.iter().map(|f| f.mask() as u8).fold(0, |a, b| a | b);
        for r in ATTR_ROLES {
            for f in r.flags {
                assert_eq!(
                    f.mask() as u8 & common,
                    0,
                    "{}: flag {} is a common bit",
                    r.kind.0,
                    f.name
                );
            }
        }
    }

    #[test]
    fn every_cold_map_has_its_row() {
        let files: [(&str, &str); 7] = [
            ("OperLookups", include_str!("oper_lookup.rs")),
            ("AnnotArena", include_str!("annot.rs")),
            ("AttrArena", include_str!("attr.rs")),
            ("TemplateTables", include_str!("templates.rs")),
            ("ColdExprData", include_str!("expr.rs")),
            ("Ast", include_str!("ast.rs")),
            ("ColdDeclData", include_str!("decl.rs")),
        ];
        let mut found = Vec::new();
        for (owner, src) in files {
            for l in src.lines() {
                let l = l.trim().trim_start_matches("pub ");
                if let Some((name, ty)) = l.split_once(": ")
                    && (ty.starts_with("ColdMap<") || ty.starts_with("crate::slot::ColdMap<"))
                {
                    found.push((owner, name));
                }
            }
        }
        let rows: Vec<(&str, &str)> = COLD_MAPS.iter().map(|c| (c.owner, c.map)).collect();
        assert_eq!(rows, found, "COLD_MAPS and the ColdMap fields");
    }

    fn walk(r: &'static Role, out: &mut Vec<&'static Role>) {
        out.push(r);
        match r {
            Role::Opt(i) | Role::List(i) => walk(i, out),
            Role::Bits(fs) => fs.iter().for_each(|f| walk(f.role, out)),
            Role::Cond { then, otherwise, .. } => {
                walk(then, out);
                walk(otherwise, out);
            }
            _ => {}
        }
    }

    fn all_roles() -> Vec<&'static Role> {
        let mut out = Vec::new();
        for r in &TYPE_ROLES {
            walk(&r.aux, &mut out);
            walk(&r.data, &mut out);
            for w in r.words {
                walk(w, &mut out);
            }
        }
        for r in &EXPR_ROLES {
            for w in [&r.a, &r.b, &r.aux] {
                walk(w, &mut out);
            }
            for f in r.flags {
                walk(f.role, &mut out);
            }
        }
        for r in &STMT_ROLES {
            for w in [&r.a, &r.b, &r.aux] {
                walk(w, &mut out);
            }
        }
        for r in ATTR_ROLES {
            for w in [&r.a, &r.b] {
                walk(w, &mut out);
            }
            for f in r.flags {
                walk(f.role, &mut out);
            }
        }
        for c in COLD_MAPS {
            walk(&c.key, &mut out);
            walk(&c.value, &mut out);
        }
        out
    }

    /// The largest value `valid` accepts.
    fn top(valid: impl Fn(u8) -> bool) -> u32 {
        (0..=255u8).filter(|&v| valid(v)).max().map_or(0, u32::from)
    }

    #[test]
    fn enum_maxima_are_the_enums() {
        let decoders: Vec<(&str, u32)> = vec![
            ("expr::CastKind", top(|v| x::CastKind::from_u8(v).is_some())),
            ("expr::UnaryOp", top(|v| x::UnaryOp::from_u8(v).is_some())),
            ("expr::BinaryOp", top(|v| x::BinaryOp::from_u8(v).is_some())),
            ("expr::CharKind", top(|v| x::CharKind::from_u8(v).is_some())),
            ("expr::StringKind", top(|v| x::StringKind::from_u8(v).is_some())),
            ("expr::TraitKind", top(|v| x::TraitKind::from_u8(v).is_some())),
            ("expr::NonOdrUse", top(|v| x::NonOdrUse::from_u8(v).is_some())),
            ("expr::Radix", top(|v| x::Radix::from_u8(v).is_some())),
            ("expr::ConstructionKind", top(|v| x::ConstructionKind::from_u8(v).is_some())),
            ("expr::StorageDuration", top(|v| x::StorageDuration::from_u8(v).is_some())),
            ("expr::NewInitStyle", top(|v| x::NewInitStyle::from_u8(v).is_some())),
            ("expr::TypeTrait", x::TypeTrait::all().iter().map(|&t| t as u32).max().unwrap()),
            ("expr::PredefinedKind", x::PredefinedKind::ALL.len() as u32 - 1),
            ("expr::SourceLocKind", x::SourceLocKind::ALL.len() as u32 - 1),
            ("expr::SuspendKind", x::SuspendKind::Handle as u32),
            (
                "names::OverloadedOperator",
                top(|v| crate::names::OverloadedOperator::from_u8(v).is_some()),
            ),
            ("types::BuiltinType", t::BuiltinType::COUNT as u32),
            ("types::UnaryTransformKind", top(|v| t::UnaryTransformKind::from_u8(v).is_some())),
            ("types::VectorKind", top(|v| t::VectorKind::from_u8(v) as u8 == v)),
            ("types::TagKeyword", top(|v| t::TagKeyword::from_u8(v) as u8 == v)),
            ("types::AutoKeyword", top(|v| t::AutoKeyword::from_u8(v) as u8 == v)),
            ("types::CallConv", top(|v| t::CallConv::from_u8(v) as u8 == v)),
            ("types::TypeAttrKind", top(|v| t::TypeAttrKind::from_u16(v.into()) as u8 == v)),
            ("types::ParamAbi", top(|v| t::ParamAbi::from_u8(v) as u8 == v)),
            ("stmt::IfKind", crate::stmt::IfKind::NotConsteval as u32),
            ("stmt::ContractKind", crate::stmt::ContractKind::Post as u32),
            ("stmt::ExpansionForm", crate::stmt::ExpansionForm::MASK.into()),
            ("decl::builtin::atomic id", 38),
        ];
        let mut bad = Vec::new();
        for r in all_roles() {
            if let Role::Enum { ty, max } = *r {
                match decoders.iter().find(|(n, _)| *n == ty) {
                    Some(&(_, m)) if m != max => {
                        bad.push(format!("{ty}: {max}, the enum's is {m}"));
                    }
                    None if ty.contains("::") && !ty.contains(' ') => {
                        bad.push(format!("{ty}: no decoder in the test"));
                    }
                    _ => {}
                }
            }
        }
        bad.sort();
        bad.dedup();
        assert!(bad.is_empty(), "{bad:#?}");
    }

    #[test]
    fn bit_fields_do_not_overlap() {
        for r in all_roles() {
            if let Role::Bits(fs) = r {
                let mut seen = 0u32;
                for f in *fs {
                    assert_eq!(seen & f.mask(), 0, "{} overlaps", f.name);
                    seen |= f.mask();
                }
            }
        }
    }

    #[test]
    fn cond_resolves_on_the_checked_flag() {
        let cleanup = attr_roles(AttrKind::CLEANUP).unwrap();
        assert_eq!(cleanup.a.resolve(Words::CHECKED_ATTR), &Role::Decl);
        assert_eq!(cleanup.a.resolve(Words::default()), &Role::Opt(&Role::Expr));
        assert_eq!(cleanup.b.resolve(Words::CHECKED_ATTR), &Role::Loc);
        assert!(attr_roles(AttrKind(14)).is_none());
    }

    /// The stable names of spec/ast.md §20: their values are pinned here
    /// (a change is a format change), and each table names every value.
    #[test]
    fn stable_names_keep_their_values() {
        use crate::decl::builtin::{self, atomic};
        use crate::fp::fp_bit;
        use crate::types::{AddrSpace, CallConv, FuncInfo, sme};
        let layout = [
            FuncInfo::VARIADIC,
            FuncInfo::REF_SHIFT,
            FuncInfo::QUALS_SHIFT,
            FuncInfo::CC_SHIFT,
            FuncInfo::NORETURN,
            FuncInfo::TRAILING,
            FuncInfo::NO_CSR,
            FuncInfo::REGPARM_SHIFT,
            FuncInfo::SME_SHIFT,
        ];
        assert_eq!(layout, [1, 1, 3, 6, 1 << 10, 1 << 11, 1 << 12, 13, 16]);
        let f = FuncInfo::new().with_sme(sme::STREAMING | sme::INOUT << sme::ZA_SHIFT);
        assert_eq!(f.bits(), (1 | 3 << 3) << FuncInfo::SME_SHIFT);
        let f =
            FuncInfo::new().with_variadic(true).with_noreturn(true).with_cc(CallConv::from_u8(2));
        assert_eq!(f.bits(), FuncInfo::VARIADIC | FuncInfo::NORETURN | 2 << FuncInfo::CC_SHIFT);
        assert_eq!(sme::FLAGS.map(|(_, v)| v), [1, 2, 4]);
        assert_eq!(sme::STATES.map(|(_, v)| v), [1, 2, 3, 4]);
        let bits = fp_bit::ALL.map(|(_, v)| v);
        assert_eq!(bits, std::array::from_fn(|i| 1 << i));
        for (name, s) in AddrSpace::NAMED.into_iter().skip(1) {
            assert_eq!(AddrSpace::opencl(name), Some(s), "{name}");
        }
        assert_eq!(AddrSpace::NAMED.map(|(_, s)| s.0), [0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(AddrSpace::TARGET_BASE, 0x20);
        assert!(atomic::ALL.windows(2).all(|w| w[0] < w[1]));
        let src = include_str!("decl.rs");
        let module = &src[src.find("pub mod atomic {").unwrap()..];
        let module = &module[..module.find("pub enum Form").unwrap()];
        let consts = module.matches(": u32 = ").count();
        assert_eq!(consts, atomic::ALL.len(), "atomic::ALL lists every id constant");
        assert_eq!(atomic::ALL[..38], std::array::from_fn::<u32, 38, _>(|i| i as u32 + 1));
        for id in atomic::ALL {
            let n = atomic::name(id);
            assert!(n.starts_with("__atomic_") || n.starts_with("__c11_atomic_"), "{id}: {n}");
            assert!(builtin::info(id).is_some());
            assert_eq!(atomic::is_op(id), id <= 38);
        }
        assert_eq!(
            (atomic::FETCH_ADD, atomic::C11_FETCH_ADD, atomic::C11_THREAD_FENCE),
            (7, 28, 236)
        );
    }

    /// The descriptors of the borrowed views (spec/ast.md §20.2) are the
    /// golden `view_descriptors.txt`: a change is a `FORMAT_VERSION` bump
    /// (and a new golden).
    #[test]
    fn view_descriptors_are_pinned() {
        use crate::consts::{ByteImage, ConstValue, Scalar};
        use crate::names::DeclNameView;
        use crate::types::{AddrSpace, ArgView, ParamAbi, TypeView};
        use kiln_snap::Describe;
        use std::fmt::Write;
        let got = [
            ("TypeView", TypeView::descriptor()),
            ("ArgView", ArgView::descriptor()),
            ("ParamAbi", ParamAbi::descriptor()),
            ("AddrSpace", AddrSpace::descriptor()),
            ("ConstValue", ConstValue::descriptor()),
            ("Scalar", Scalar::descriptor()),
            ("ByteImage", ByteImage::descriptor()),
            ("DeclNameView", DeclNameView::descriptor()),
        ];
        let mut text = String::new();
        for (name, d) in &got {
            writeln!(text, "{name} {}", d.text()).unwrap();
        }
        assert_eq!(text, include_str!("view_descriptors.txt"), "a view's layout changed");
    }
}
