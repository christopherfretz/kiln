//! Stage S2 of the declaration image (spec/modules.md §11.11 "Part 2a",
//! "Bodies", WP-1064): the expression, statement, constant and string
//! records that carry the bodies of inline, `constexpr` and `consteval`
//! functions, variable initializers and default arguments.
//!
//! A record is the AST node's header (kind, value-category bits, `aux`,
//! type, location) plus its two payload slots, each written as the fields
//! a per-kind schema names ([`expr_shape`], [`stmt_shape`]): an id becomes
//! an index into the image's own arena (one index space per arena, as for
//! declarations and types), a type a [`QRef`], a location a main-file
//! offset. The writer, the validator and the importer all walk the same
//! schema, so one table defines the format. Records are emitted in post
//! order, so every expression's operands precede it and every statement's
//! sub-statements precede it; the only edge back from an expression to a
//! statement (a GNU statement expression) is checked by
//! [`validate_bodies`] to point below the expression, which makes every
//! image acyclic.
//!
//! Any node, side record or cold annotation outside the schema makes the
//! writer emit no image (the fallback path).

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::consts::{FloatKind, IntBits, NullKind, Scalar};
use kiln_ast::decl::{DeclMisc, NewDecl, Payload as AstPayload, VarEvalState};
use kiln_ast::expr::{
    BinaryOp, CastKind, CharKind, CompoundAssignExtra, ConstructionKind, DeclRefExtra, ExprDeps,
    ExprKind, ExprNode, InitListExtra, NewExtra, NewInitStyle, NonOdrUse, Radix, StorageDuration,
    StringKind, TraitKind, TypeTrait, UnaryOp,
};
use kiln_ast::names::OverloadedOperator;
use kiln_ast::stmt::{CaseData, IfData, LoopData, RangeForData, StmtKind, StmtNode, SwitchData};
use kiln_ast::strings::StringEncoding;
use kiln_ast::types::Nns;
use kiln_ast::{AttrId, ConstValueId, ExprId, ListId, NnsId, RawId, StmtId, StringLitId};
use kiln_source::fx::FxHashSet;

use super::constraints::CONCEPT_KIND;
use super::{
    Access, ConstexprKind, DEP, DeclFlags, DeclId, DeclKind, DeclName, DeclRec, FnBody, FnSpec,
    FxHashMap, Image, LEX_TU, Loc, NONE, Payload, QRef, QualType, Rd, Res, Sema, W, Wr, Writer,
    bad, var_spec,
};

/// A parameter reference in a declaration field: `PARAM | i` names the
/// image's `i`-th parameter, counting the parameters of the function
/// records in order.
pub(super) const PARAM: u32 = 0x8000_0000;
/// A variable's evaluation state "not a constant" (`eval` field).
pub(super) const NOT_CONST: u32 = u32::MAX - 1;

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum ConstRec {
    Int {
        ty: QRef,
        signed: bool,
        width: u32,
        words: Vec<u64>,
    },
    Bool {
        ty: QRef,
        value: bool,
    },
    Float {
        ty: QRef,
        kind: u8,
        bits: u128,
    },
    Null {
        ty: QRef,
        kind: u8,
    },
    /// WP-1343: a non-null pointer or reference ([`super::aggconsts`]).
    Ptr {
        ty: QRef,
        base: super::aggconsts::PtrRec,
        offset: u64,
        designated: Option<QRef>,
        past: bool,
    },
    /// WP-1343: a class, array or union value: its bytes, its
    /// pointer-like subobjects (offset, earlier constant), its padding
    /// (offset, length) and its active union members (offset, field
    /// record).
    Agg {
        ty: QRef,
        bytes: Vec<u8>,
        relocs: Vec<(u32, u32)>,
        padding: Vec<(u32, u32)>,
        unions: Vec<(u32, u32)>,
    },
}

impl ConstRec {
    fn ty(&self) -> QRef {
        match self {
            ConstRec::Int { ty, .. }
            | ConstRec::Bool { ty, .. }
            | ConstRec::Float { ty, .. }
            | ConstRec::Null { ty, .. }
            | ConstRec::Ptr { ty, .. }
            | ConstRec::Agg { ty, .. } => *ty,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct ExprRec {
    /// Index into [`EXPR_KINDS`].
    pub(super) kind: u8,
    /// `ExprNode` bits: value category, object kind, per-kind flags.
    pub(super) bits: u8,
    pub(super) aux: u8,
    /// Which cold annotations follow the slots ([`COLD_END`] ...).
    pub(super) cold: u16,
    /// `ExprDeps` bits: type, value and instantiation dependence (S4: a
    /// pattern's body; 0 elsewhere).
    pub(super) deps: u8,
    pub(super) ty: QRef,
    pub(super) loc: u32,
    pub(super) words: Vec<u32>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct StmtRec {
    /// Index into [`STMT_KINDS`].
    pub(super) kind: u8,
    pub(super) aux: u16,
    pub(super) loc: u32,
    pub(super) words: Vec<u32>,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct Bodies {
    pub(super) consts: Vec<ConstRec>,
    /// (encoding, code-unit bytes).
    pub(super) strings: Vec<(u8, Vec<u8>)>,
    pub(super) exprs: Vec<ExprRec>,
    pub(super) stmts: Vec<StmtRec>,
}

/// Cold annotations carried: `end_loc`, `cast_written`, `trait_rhs_loc`,
/// `expr_begin`, `member_op_loc`, `default_arg_rewritten`, a fold's
/// `...` (sema's `fold_ellipsis`, WP-1279), a name's qualifier and
/// template arguments (WP-1279), in this order after the slots.
const COLD_END: u16 = 1;
const COLD_WRITTEN: u16 = 2;
const COLD_RHS: u16 = 4;
const COLD_BEGIN: u16 = 8;
const COLD_OP: u16 = 16;
/// The lifetime-extended copy of a default argument (WP-1149) or default
/// member initializer (WP-1177, spec/sema.md §7.6a): an expression field,
/// so the copy is written before its use.
const COLD_REWRITE: u16 = 32;
const COLD_ELLIPSIS: u16 = 64;
/// A name's written qualifier and explicit template arguments (the
/// templates' `expr_nns` / `expr_targs`, WP-1279): an [`F::Nns`] and an
/// [`F::TA`].
const COLD_LOOKUP: u16 = 128;
/// The `template` keyword of a dependent member or qualified name
/// (`t.template get<3>()`, the templates' `template_kw`, WP-1284;
/// [temp.names]/3): its location. The first bit of the wider field.
const COLD_TEMPLATE_KW: u16 = 256;
/// A default argument or member initializer use whose initializer holds
/// a source-location builtin (WP-1310): the location and context its
/// values are for, and whether they include the `__impl` object.
const COLD_SRCLOC: u16 = 512;
/// The naming class of an unresolved name that found class members with
/// no using-shadow among them (sema's `ule_found`, WP-1314): an
/// [`F::OD`] of [`Dc::Ctx`] (a class), only on `UnresolvedLookupExpr`.
const COLD_NAMING: u16 = 1024;
/// What an unresolved name found when that is not its declarations
/// (WP-1342: a using-declaration's shadow in place of the declaration it
/// names): an [`F::DL`] of [`Dc::Lookup`] as long as the name's, only on
/// `UnresolvedLookupExpr`.
const COLD_FOUND: u16 = 2048;
/// A member access's name as written (the `member_extra` of a
/// `MemberExpr`, WP-1343: `p->Base::f`, `x.template get<0>`, the using
/// shadow found): an [`F::Nns`], an [`F::OD`] of [`Dc::Found`] and an
/// [`F::TA`], the fields of [`Side::DeclRef`].
const COLD_MEMBER: u16 = 4096;
/// An array type trait's dimension operand and value (`array_trait`,
/// WP-1343: `__array_rank(T)`, `__array_extent(T, I)`): an [`F::OE`]
/// and the value's low and high words, only on `ArrayTypeTraitExpr`.
const COLD_ARRAY: u16 = 8192;
/// The parameter pack of the `sizeof...` an integer literal was
/// substituted from (the templates' `size_of_pack_origin`, WP-1343; it
/// mangles as `sZ`): an [`F::D`] of [`Dc::Pack`], only on
/// `IntegerLiteral`.
const COLD_PACK: u16 = 16384;
/// Every annotation bit.
const COLD_ALL: u16 = 32767;

// ---------------------------------------------------------------------------
// The schema
// ---------------------------------------------------------------------------

/// What a declaration field may name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Dc {
    /// A function, variable, enumerator or parameter.
    Value,
    /// A variable.
    Var,
    /// A local variable (a declaration statement's).
    Local,
    Label,
    Let,
    Param,
    /// A declaration context: a function (an implicit special member
    /// too), class, namespace, export block or the translation unit.
    Ctx,
    Namespace,
    /// A class member: a field, member function, static data member or
    /// enumerator.
    Member,
    /// A field.
    Field,
    /// A constructor (an implicit one too).
    Ctor,
    /// A destructor (an implicit one too).
    Dtor,
    /// A declaration reference's found declaration: a [`Dc::Value`] or
    /// the function template a specialization was named through.
    Found,
    /// A template parameter pack or a function parameter pack (WP-1279).
    Pack,
    /// What an unresolved name found at the definition (WP-1279): a
    /// [`Dc::Found`] or a function or variable template.
    Lookup,
    /// A concept (WP-1284).
    Concept,
}

/// A field: how one or more words are read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum F {
    /// Any 32-bit value.
    Raw,
    Bool,
    /// An expression; an optional one ([`NONE`] for none).
    E,
    OE,
    S,
    OS,
    D(Dc),
    OD(Dc),
    Cv,
    OCv,
    Str,
    Loc,
    /// A [`QRef`] (two words).
    Qt,
    /// Counted lists of expressions, statements, declarations, types.
    EL,
    SL,
    DL(Dc),
    QL,
    /// A cast's base path: 0 none, else 1 + the number of steps, then
    /// each step's (class record, base position) pair.
    Path,
    /// A nested-name-specifier: a count, then its components outermost
    /// first (0 `::`; 1 namespace + record; 2 type + [`QRef`] + bool).
    Nns,
    /// A statement's attributes: a count, then two words each, the
    /// argument-free kind, syntax and flags ([`attr_word`]) and the
    /// location.
    AL,
    /// A member name of a dependent member access (S4) or an unresolved
    /// name (WP-1279): an identifier, as an index into the image's
    /// undeclared names (`pnames`), or an operator-function-id, as
    /// `N_OP` plus the operator.
    Name,
    /// Explicit template arguments (WP-1279): 0 for none, else 1 plus
    /// the count, then three words per argument: 0 and a [`QRef`] for a
    /// type, 1, the expression and whether it has a profile for an
    /// expression.
    TA,
    /// A `_Generic` selection's operands (WP-1132): the association
    /// count `n`, the controlling operand, then `n` associations of four
    /// words each, the type as a [`QRef`] ([`NONE`] and 0 for `default`),
    /// the type's (or `default`'s) location and the expression. `Gen`
    /// has an expression control (expression, location: two words),
    /// `GenT` a type-name control ([`QRef`], location: three words).
    Gen,
    GenT,
    /// A requires-expression's body (WP-1284): a record of the image's
    /// requires-expression bodies (requires.rs).
    RB,
    /// A requires-expression's requirements (WP-1284): its body (as in
    /// [`F::RB`]), the count, then each requirement, its kind and
    /// operands ([`super::requires::req_words`]).
    Reqs,
}

/// The words of a `_Generic` control in an [`F::Gen`] / [`F::GenT`] token.
const fn gen_ctrl_words(f: F) -> usize {
    if matches!(f, F::GenT) { 3 } else { 2 }
}

/// The associations of an [`F::Gen`] / [`F::GenT`] token: four words each.
fn gen_assocs(f: F, w: &[u32]) -> std::slice::ChunksExact<'_, u32> {
    w[1 + gen_ctrl_words(f)..].chunks_exact(4)
}

/// The statement attributes the image carries: argument-free ones.
const STMT_ATTRS: [AttrKind; 3] = [AttrKind::FALLTHROUGH, AttrKind::LIKELY, AttrKind::UNLIKELY];

/// An attribute's kind, syntax and flags in one word (`kind | syntax <<
/// 16 | flags << 24`), if the image carries it.
fn attr_word(a: &AttrNode) -> Option<u32> {
    let flags = AttrNode::FALLTHROUGH_SCOPED | AttrNode::GNU_SCOPED;
    let syntax = match a.syntax {
        AttrSyntax::Cxx11 => 0u32,
        AttrSyntax::Gnu => 1,
        _ => return None,
    };
    (STMT_ATTRS.contains(&a.kind) && a.flags & !flags == 0 && a.a == 0 && a.b == 0)
        .then(|| u32::from(a.kind.0) | syntax << 16 | u32::from(a.flags) << 24)
}

/// The inverse of [`attr_word`] (with `loc`).
fn word_attr(w: u32, loc: Loc) -> Option<AttrNode> {
    let kind = AttrKind((w & 0xffff) as u16);
    let syntax = match (w >> 16) & 0xff {
        0 => AttrSyntax::Cxx11,
        1 => AttrSyntax::Gnu,
        _ => return None,
    };
    let a = AttrNode { kind, syntax, flags: (w >> 24) as u8, loc, a: 0, b: 0 };
    (attr_word(&a) == Some(w)).then_some(a)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Side {
    Arms,
    CompoundAssign,
    InitList,
    DeclRef,
    If,
    Switch,
    Case,
    Loop,
    RangeFor,
    /// A new-expression's allocated type, array bound, initializer,
    /// placement arguments (present flag and list), allocation and
    /// deallocation functions and type location (WP-1320).
    New,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    /// The slot is 0.
    Zero,
    F(F),
    Side(Side),
}

fn side_fields(s: Side) -> &'static [F] {
    use F::{Bool, D, E, EL, Loc, Nns, OD, OE, OS, Qt, S, SL};
    match s {
        Side::Arms => &[E, E],
        Side::CompoundAssign => &[E, Qt, Qt],
        // WP-1343: the union member a braced list initializes.
        Side::InitList => &[OE, Loc, OD(Dc::Field)],
        // WP-1320: and the explicit template arguments (`declval<T>()`).
        Side::DeclRef => &[Nns, OD(Dc::Found), F::TA],
        Side::If => &[OS, OD(Dc::Var), OE, OS, OS],
        Side::Switch => &[OS, OD(Dc::Var), E, S, SL],
        Side::Case => &[E, OE, S],
        Side::Loop => &[OS, OD(Dc::Var), OE, OE, OS],
        Side::RangeFor => &[OS, D(Dc::Var), OD(Dc::Var), OD(Dc::Var), OE, OE, D(Dc::Var), S, Loc],
        Side::New => &[Qt, OE, OE, Bool, EL, OD(Dc::Value), OD(Dc::Value), Loc],
    }
}

/// The expression kinds the image carries; the record's kind byte indexes
/// this table (append only).
pub(super) const EXPR_KINDS: [ExprKind; 69] = {
    use ExprKind as X;
    [
        X::IntegerLiteral,
        X::FloatingLiteral,
        X::CharacterLiteral,
        X::StringLiteral,
        X::CXXBoolLiteralExpr,
        X::CXXNullPtrLiteralExpr,
        X::GNUNullExpr,
        X::DeclRefExpr,
        X::PredefinedExpr,
        X::UnaryOperator,
        X::BinaryOperator,
        X::CompoundAssignOperator,
        X::ConditionalOperator,
        X::ChooseExpr,
        X::ArraySubscriptExpr,
        X::ParenExpr,
        X::CallExpr,
        X::CXXOperatorCallExpr,
        X::ImplicitCastExpr,
        X::CStyleCastExpr,
        X::CXXFunctionalCastExpr,
        X::CXXStaticCastExpr,
        X::CXXConstCastExpr,
        X::CXXReinterpretCastExpr,
        X::InitListExpr,
        X::ImplicitValueInitExpr,
        X::CXXScalarValueInitExpr,
        X::UnaryExprOrTypeTraitExpr,
        X::TypeTraitExpr,
        X::CXXNoexceptExpr,
        X::ConstantExpr,
        X::MaterializeTemporaryExpr,
        X::ExprWithCleanups,
        X::OpaqueValueExpr,
        X::StmtExpr,
        X::CXXThrowExpr,
        X::CXXDefaultArgExpr,
        X::AddrLabelExpr,
        X::ArrayInitLoopExpr,
        X::ArrayInitIndexExpr,
        X::MemberExpr,
        X::CXXThisExpr,
        X::CXXMemberCallExpr,
        X::CXXConstructExpr,
        X::CXXTemporaryObjectExpr,
        X::CXXDefaultInitExpr,
        X::CXXBindTemporaryExpr,
        // S4 (WP-1139): `T(args)` / `T{args}` and `x.m` / `x->m` with a
        // type-dependent object, in a pattern.
        X::CXXUnresolvedConstructExpr,
        X::CXXDependentScopeMemberExpr,
        // WP-1132: `_Generic`, resolved or (in a pattern) result-dependent.
        X::GenericSelectionExpr,
        // WP-1279: `sizeof...(P)`, `e...` and fold-expressions, in a
        // pattern.
        X::SizeOfPackExpr,
        X::PackExpansionExpr,
        X::CXXFoldExpr,
        // WP-1279: a name found at the definition and looked up again
        // (with argument-dependent lookup) at the instantiation.
        X::UnresolvedLookupExpr,
        // WP-1284: a dependent concept-id, unchecked.
        X::ConceptSpecializationExpr,
        // WP-1284: a dependent requires-expression (requires.rs).
        X::RequiresExpr,
        // WP-1284: `T::name` / `T::template name<args>`, looked up at the
        // instantiation ([temp.dep.expr]/3).
        X::DependentScopeDeclRefExpr,
        // WP-1300: a substituted non-type parameter, as an alias
        // specialization's underlying type holds it ([temp.alias]/2).
        X::SubstNonTypeTemplateParmExpr,
        // WP-1310: a source-location builtin call, its values computed
        // again by the importer (libc++ `<source_location>`).
        X::SourceLocExpr,
        // WP-1320: `T x(args)` with a dependent type or argument, kept as
        // written until the instantiation ([dcl.init.general]/16).
        X::ParenListExpr,
        X::CXXPseudoDestructorExpr,
        X::CXXNewExpr,
        // WP-1343: a comparison rewritten to `==` / `<=>` (its semantic
        // form and the operands as written), `delete`, `typeid`,
        // `dynamic_cast`.
        X::CXXRewrittenBinaryOperator,
        X::CXXDeleteExpr,
        X::CXXTypeidExpr,
        X::CXXDynamicCastExpr,
        // `__builtin_bit_cast`, a cast; a GNU `__atomic_*` / `__c11_*`
        // operation (its operands, the operation in `aux`).
        X::BuiltinBitCastExpr,
        X::AtomicExpr,
        // `__array_rank` / `__array_extent` (the dimension and value are
        // [`COLD_ARRAY`]).
        X::ArrayTypeTraitExpr,
    ]
};

/// The statement kinds the image carries (append only).
pub(super) const STMT_KINDS: [StmtKind; 20] = {
    use StmtKind as K;
    [
        K::CompoundStmt,
        K::DeclStmt,
        K::ExprStmt,
        K::NullStmt,
        K::IfStmt,
        K::SwitchStmt,
        K::CaseStmt,
        K::DefaultStmt,
        K::BreakStmt,
        K::ContinueStmt,
        K::WhileStmt,
        K::DoStmt,
        K::ForStmt,
        K::CXXForRangeStmt,
        K::ReturnStmt,
        K::LabelStmt,
        K::GotoStmt,
        K::AttributedStmt,
        // WP-1343: a try block and its handlers ([except.pre]).
        K::CXXTryStmt,
        K::CXXCatchStmt,
    ]
};

/// Per-kind flag bit 4 (`WIDE`, `ARG_IS_TYPE`) changes a slot's meaning.
const FLAG4: u8 = 1 << 4;

fn expr_shape(k: ExprKind, bits: u8) -> [Slot; 2] {
    use ExprKind as X;
    use Slot::{Side as Sd, Zero as Z};
    let f = Slot::F;
    let flag4 = bits & FLAG4 != 0;
    match k {
        X::IntegerLiteral if flag4 => [f(F::Cv), Z],
        X::IntegerLiteral => [f(F::Raw), f(F::Raw)],
        X::FloatingLiteral => [f(F::Cv), Z],
        X::CharacterLiteral => [f(F::Raw), Z],
        X::StringLiteral => [f(F::Str), Z],
        X::CXXBoolLiteralExpr => [f(F::Bool), Z],
        X::DeclRefExpr => [f(F::D(Dc::Value)), Sd(Side::DeclRef)],
        X::PredefinedExpr | X::OpaqueValueExpr | X::CXXThrowExpr => [f(F::OE), Z],
        X::BinaryOperator | X::ArraySubscriptExpr | X::ArrayInitLoopExpr => [f(F::E), f(F::E)],
        X::CompoundAssignOperator => [f(F::E), Sd(Side::CompoundAssign)],
        X::ConditionalOperator | X::ChooseExpr => [f(F::E), Sd(Side::Arms)],
        X::ParenExpr => [f(F::E), f(F::Loc)],
        X::CallExpr
        | X::CXXOperatorCallExpr
        | X::CXXMemberCallExpr
        | X::CXXRewrittenBinaryOperator => [f(F::E), f(F::EL)],
        X::CXXDeleteExpr => [f(F::E), f(F::OD(Dc::Value))],
        X::CXXUnresolvedConstructExpr | X::ParenListExpr | X::AtomicExpr => [f(F::EL), Z],
        X::CXXDependentScopeMemberExpr => [f(F::E), f(F::Name)],
        X::DependentScopeDeclRefExpr => [f(F::Nns), f(F::Name)],
        // `b` is the selected association's expression, NONE while the
        // selection is result-dependent (checked to be an association's).
        X::GenericSelectionExpr if flag4 => [f(F::GenT), f(F::OE)],
        X::GenericSelectionExpr => [f(F::Gen), f(F::OE)],
        X::ImplicitCastExpr
        | X::CStyleCastExpr
        | X::CXXFunctionalCastExpr
        | X::CXXStaticCastExpr
        | X::CXXConstCastExpr
        | X::CXXReinterpretCastExpr
        | X::CXXDynamicCastExpr
        | X::BuiltinBitCastExpr => [f(F::E), f(F::Path)],
        X::ArrayTypeTraitExpr => [f(F::Qt), Z],
        X::InitListExpr => [f(F::EL), Sd(Side::InitList)],
        X::UnaryExprOrTypeTraitExpr | X::CXXTypeidExpr if flag4 => [f(F::Qt), Z],
        X::UnaryOperator
        | X::ExprWithCleanups
        | X::UnaryExprOrTypeTraitExpr
        | X::CXXTypeidExpr
        | X::PackExpansionExpr => [f(F::E), Z],
        X::TypeTraitExpr => [f(F::QL), f(F::Bool)],
        X::CXXNoexceptExpr => [f(F::E), f(F::Bool)],
        // The object and the destroyed type; `->` and qualified in aux
        // (WP-1320).
        X::CXXPseudoDestructorExpr => [f(F::E), f(F::Qt)],
        X::CXXNewExpr => [Sd(Side::New), Z],
        X::ConstantExpr => [f(F::E), f(F::OCv)],
        X::MaterializeTemporaryExpr => [f(F::E), f(F::OD(Dc::Let))],
        X::StmtExpr => [f(F::S), Z],
        X::CXXDefaultArgExpr => [f(F::D(Dc::Param)), f(F::D(Dc::Ctx))],
        X::AddrLabelExpr => [f(F::D(Dc::Label)), Z],
        X::MemberExpr => [f(F::E), f(F::D(Dc::Member))],
        X::CXXConstructExpr | X::CXXTemporaryObjectExpr => [f(F::D(Dc::Ctor)), f(F::EL)],
        X::CXXDefaultInitExpr => [f(F::D(Dc::Field)), f(F::D(Dc::Ctx))],
        X::CXXBindTemporaryExpr => [f(F::E), f(F::D(Dc::Dtor))],
        X::SizeOfPackExpr => [f(F::D(Dc::Pack)), Z],
        X::CXXFoldExpr => [f(F::OE), f(F::OE)],
        X::UnresolvedLookupExpr => [f(F::Name), f(F::DL(Dc::Lookup))],
        X::ConceptSpecializationExpr => [f(F::D(Dc::Concept)), f(F::TA)],
        X::RequiresExpr => [f(F::RB), f(F::Reqs)],
        X::SubstNonTypeTemplateParmExpr => [f(F::D(Dc::Value)), f(F::E)],
        X::SourceLocExpr => [f(F::D(Dc::Ctx)), Z],
        _ => [Z, Z],
    }
}

fn expr_aux_ok(k: ExprKind, aux: u8) -> bool {
    use ExprKind as X;
    match k {
        X::IntegerLiteral => Radix::from_u8(aux).is_some(),
        X::CharacterLiteral => CharKind::from_u8(aux).is_some(),
        X::StringLiteral => StringKind::from_u8(aux).is_some(),
        X::DeclRefExpr | X::MemberExpr => NonOdrUse::from_u8(aux).is_some(),
        X::CXXConstructExpr | X::CXXTemporaryObjectExpr => {
            ConstructionKind::from_u8(aux & 3).is_some() && aux >> 6 == 0
        }
        X::CXXDefaultInitExpr => aux == 1,
        X::PredefinedExpr => usize::from(aux) < kiln_ast::expr::PredefinedKind::ALL.len(),
        X::SourceLocExpr => usize::from(aux) < kiln_ast::expr::SourceLocKind::ALL.len(),
        X::UnaryOperator => UnaryOp::from_u8(aux).is_some(),
        X::BinaryOperator
        | X::CompoundAssignOperator
        | X::CXXFoldExpr
        | X::CXXRewrittenBinaryOperator => BinaryOp::from_u8(aux).is_some(),
        X::ChooseExpr => aux <= 2,
        X::CXXOperatorCallExpr => OverloadedOperator::from_u8(aux).is_some(),
        X::ImplicitCastExpr
        | X::CStyleCastExpr
        | X::CXXFunctionalCastExpr
        | X::CXXStaticCastExpr
        | X::CXXConstCastExpr
        | X::CXXReinterpretCastExpr
        | X::CXXDynamicCastExpr
        | X::BuiltinBitCastExpr => CastKind::from_u8(aux).is_some(),
        X::AtomicExpr => kiln_ast::decl::builtin::atomic::form(u32::from(aux)).is_some(),
        X::ArrayTypeTraitExpr => TypeTrait::all()
            .iter()
            .any(|&t| t as u8 == aux && matches!(t, TypeTrait::ArrayRank | TypeTrait::ArrayExtent)),
        X::UnaryExprOrTypeTraitExpr => TraitKind::from_u8(aux).is_some(),
        X::TypeTraitExpr => usize::from(aux) < TypeTrait::all().len(),
        // The constant's result kind; `->` and the `template` keyword
        // ([temp.names]/3, WP-1284).
        // A delete-expression's: sized and aligned deallocation.
        X::ConstantExpr
        | X::CXXDependentScopeMemberExpr
        | X::CXXPseudoDestructorExpr
        | X::CXXDeleteExpr => aux <= 3,
        X::CXXNewExpr => NewInitStyle::from_u8(aux).is_some(),
        X::MaterializeTemporaryExpr => StorageDuration::from_u8(aux & 3).is_some() && aux >> 3 == 0,
        X::CXXThisExpr
        | X::ExprWithCleanups
        | X::CXXUnresolvedConstructExpr
        | X::UnresolvedLookupExpr
        // A typeid's: a potentially-evaluated operand.
        | X::CXXTypeidExpr => aux <= 1,
        _ => aux == 0,
    }
}

fn stmt_shape(k: StmtKind) -> [Slot; 2] {
    use Slot::{Side as Sd, Zero as Z};
    use StmtKind as K;
    let f = Slot::F;
    match k {
        K::CompoundStmt => [f(F::SL), f(F::Loc)],
        K::DeclStmt => [f(F::DL(Dc::Local)), Z],
        K::ExprStmt => [f(F::E), Z],
        K::IfStmt => [Sd(Side::If), Z],
        K::SwitchStmt => [Sd(Side::Switch), Z],
        K::CaseStmt => [Sd(Side::Case), Z],
        K::DefaultStmt => [f(F::S), Z],
        K::WhileStmt | K::ForStmt => [Sd(Side::Loop), Z],
        K::DoStmt => [Sd(Side::Loop), f(F::Loc)],
        K::CXXForRangeStmt => [Sd(Side::RangeFor), Z],
        K::ReturnStmt => [f(F::OE), f(F::OD(Dc::Var))],
        K::LabelStmt => [f(F::D(Dc::Label)), f(F::S)],
        K::GotoStmt => [f(F::D(Dc::Label)), Z],
        K::AttributedStmt => [f(F::AL), f(F::S)],
        K::CXXTryStmt => [f(F::S), f(F::SL)],
        // The exception declaration's variable (none for `catch (...)`).
        K::CXXCatchStmt => [f(F::OD(Dc::Var)), f(F::S)],
        _ => [Z, Z],
    }
}

fn stmt_aux_ok(k: StmtKind, aux: u16) -> bool {
    match k {
        StmtKind::IfStmt => aux <= 3,
        _ => aux == 0,
    }
}

/// The fields of a record, in order: both slots, then the cold
/// annotations named by `cold`.
fn layout(shape: [Slot; 2], cold: u16) -> Vec<F> {
    let mut v = Vec::new();
    for s in shape {
        match s {
            Slot::Zero => {}
            Slot::F(f) => v.push(f),
            Slot::Side(sd) => v.extend_from_slice(side_fields(sd)),
        }
    }
    for (bit, f) in [
        (COLD_END, F::Loc),
        (COLD_WRITTEN, F::Qt),
        (COLD_RHS, F::Loc),
        (COLD_BEGIN, F::Loc),
        (COLD_OP, F::Loc),
        (COLD_REWRITE, F::E),
        (COLD_ELLIPSIS, F::Loc),
        (COLD_LOOKUP, F::Nns),
        (COLD_LOOKUP, F::TA),
        (COLD_TEMPLATE_KW, F::Loc),
        (COLD_SRCLOC, F::Loc),
        (COLD_SRCLOC, F::D(Dc::Ctx)),
        (COLD_SRCLOC, F::Bool),
        (COLD_NAMING, F::OD(Dc::Ctx)),
        (COLD_FOUND, F::DL(Dc::Lookup)),
        (COLD_MEMBER, F::Nns),
        (COLD_MEMBER, F::OD(Dc::Found)),
        (COLD_MEMBER, F::TA),
        (COLD_ARRAY, F::OE),
        (COLD_ARRAY, F::Raw),
        (COLD_ARRAY, F::Raw),
        (COLD_PACK, F::D(Dc::Pack)),
    ] {
        if cold & bit != 0 {
            v.push(f);
        }
    }
    v
}

/// Splits `words` into one token per field of `fields`; `None` if the
/// words do not match.
fn tokens<'w>(fields: &[F], words: &'w [u32]) -> Option<Vec<(F, &'w [u32])>> {
    let mut out = Vec::with_capacity(fields.len());
    let mut p = 0usize;
    for &f in fields {
        let n = match f {
            F::Qt => 2,
            F::EL | F::SL | F::DL(_) => 1 + *words.get(p)? as usize,
            F::QL | F::AL => 1 + 2 * (*words.get(p)? as usize),
            F::Gen | F::GenT => {
                (*words.get(p)? as usize).checked_mul(4)?.checked_add(1 + gen_ctrl_words(f))?
            }
            F::Path => match *words.get(p)? as usize {
                0 => 1,
                n => 1 + 2 * (n - 1),
            },
            F::TA => match *words.get(p)? as usize {
                0 => 1,
                n => 1 + 3 * (n - 1),
            },
            F::Reqs => {
                let n = *words.get(p + 1)? as usize;
                let mut q = p + 2;
                for _ in 0..n {
                    q = q.checked_add(super::requires::req_words(*words.get(q)?)?)?;
                }
                q - p
            }
            F::Nns => {
                let n = *words.get(p)? as usize;
                let mut q = p + 1;
                for _ in 0..n {
                    q += match *words.get(q)? {
                        0 => 1,
                        1 | 3 | 4 => 2,
                        2 => 4,
                        _ => return None,
                    };
                }
                q - p
            }
            _ => 1,
        };
        let end = p.checked_add(n)?;
        out.push((f, words.get(p..end)?));
        p = end;
    }
    (p == words.len()).then_some(out)
}

// ---------------------------------------------------------------------------
// Encoding and decoding
// ---------------------------------------------------------------------------

fn encode_const(w: &mut Wr, c: &ConstRec) {
    match c {
        ConstRec::Int { ty, signed, width, words } => {
            w.u8(0);
            w.q(*ty);
            w.u8(u8::from(*signed));
            w.u32(*width);
            for x in words {
                w.0.extend_from_slice(&x.to_le_bytes());
            }
        }
        ConstRec::Bool { ty, value } => {
            w.u8(1);
            w.q(*ty);
            w.u8(u8::from(*value));
        }
        ConstRec::Float { ty, kind, bits } => {
            w.u8(2);
            w.q(*ty);
            w.u8(*kind);
            w.0.extend_from_slice(&bits.to_le_bytes());
        }
        ConstRec::Null { ty, kind } => {
            w.u8(3);
            w.q(*ty);
            w.u8(*kind);
        }
        ConstRec::Ptr { .. } | ConstRec::Agg { .. } => super::aggconsts::encode(w, c),
    }
}

fn words(w: &mut Wr, v: &[u32]) {
    w.len(v.len());
    for &x in v {
        w.u32(x);
    }
}

/// Writes an arena as an offset table and a blob (records addressable by
/// index, for the lazy loader of WP-1069).
fn arena<T>(w: &mut Wr, recs: &[T], one: impl Fn(&mut Wr, &T)) {
    let mut blob = Wr(Vec::new());
    w.len(recs.len());
    for r in recs {
        w.u32(u32::try_from(blob.0.len()).unwrap_or(u32::MAX));
        one(&mut blob, r);
    }
    w.len(blob.0.len());
    w.0.extend_from_slice(&blob.0);
}

pub(super) fn encode_bodies(w: &mut Wr, b: &Bodies) {
    w.len(b.consts.len());
    for c in &b.consts {
        encode_const(w, c);
    }
    w.len(b.strings.len());
    for (enc, bytes) in &b.strings {
        w.u8(*enc);
        w.len(bytes.len());
        w.0.extend_from_slice(bytes);
    }
    arena(w, &b.exprs, |w, e| {
        for x in [e.kind, e.bits, e.aux] {
            w.u8(x);
        }
        w.u16(e.cold);
        w.u8(e.deps);
        w.q(e.ty);
        w.u32(e.loc);
        words(w, &e.words);
    });
    arena(w, &b.stmts, |w, s| {
        w.u8(s.kind);
        w.u16(s.aux);
        w.u32(s.loc);
        words(w, &s.words);
    });
}

fn decode_arena<T>(r: &mut Rd<'_>, min: usize, one: impl Fn(&mut Rd<'_>) -> Res<T>) -> Res<Vec<T>> {
    let n = r.count(4 + min)?;
    let mut offsets = Vec::with_capacity(n);
    for _ in 0..n {
        offsets.push(r.u32()?);
    }
    let len = r.count(1)?;
    let mut br = Rd(r.take(len)?);
    let mut v = Vec::with_capacity(n);
    for o in offsets {
        if o as usize != len - br.0.len() {
            return bad("bad body record offset");
        }
        v.push(one(&mut br)?);
    }
    if !br.0.is_empty() {
        return bad("trailing body record bytes");
    }
    Ok(v)
}

fn decode_words(r: &mut Rd<'_>) -> Res<Vec<u32>> {
    let n = r.count(4)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(r.u32()?);
    }
    Ok(v)
}

pub(super) fn decode_bodies(r: &mut Rd<'_>) -> Res<Bodies> {
    let mut b = Bodies::default();
    let nc = r.count(7)?;
    b.consts.reserve(nc);
    for _ in 0..nc {
        let tag = r.u8()?;
        let ty = r.q()?;
        b.consts.push(match tag {
            0 => {
                let signed = r.u8()? != 0;
                let width = r.u32()?;
                if width == 0 || width > IntBits::MAX_WIDTH {
                    return bad("bad integer width");
                }
                let n = IntBits::words_for(width);
                if n.saturating_mul(8) > r.0.len() {
                    return bad("truncated");
                }
                let mut words = Vec::with_capacity(n);
                for _ in 0..n {
                    words.push(r.u64()?);
                }
                if IntBits::from_words(width, &words).is_none() {
                    return bad("bad integer constant");
                }
                ConstRec::Int { ty, signed, width, words }
            }
            1 => ConstRec::Bool { ty, value: r.u8()? != 0 },
            2 => {
                let kind = r.u8()?;
                let mut a = [0u8; 16];
                a.copy_from_slice(r.take(16)?);
                if FloatKind::from_u8(kind).is_none() {
                    return bad("bad floating-point kind");
                }
                ConstRec::Float { ty, kind, bits: u128::from_le_bytes(a) }
            }
            3 => {
                let kind = r.u8()?;
                if null_kind(kind).is_none() {
                    return bad("bad null kind");
                }
                ConstRec::Null { ty, kind }
            }
            super::aggconsts::TAG_PTR | super::aggconsts::TAG_AGG => {
                super::aggconsts::decode(r, tag, ty)?
            }
            _ => return bad("unknown constant kind"),
        });
    }
    let ns = r.count(5)?;
    b.strings.reserve(ns);
    for _ in 0..ns {
        let enc = r.u8()?;
        let n = r.count(1)?;
        let bytes = r.take(n)?.to_vec();
        match string_encoding(enc) {
            Some(e) if bytes.len().is_multiple_of(e.unit_bytes() as usize) => {}
            _ => return bad("bad string literal"),
        }
        b.strings.push((enc, bytes));
    }
    b.exprs = decode_arena(r, 6 + 5 + 4 + 4, |r| {
        let (kind, bits, aux, cold, deps) = (r.u8()?, r.u8()?, r.u8()?, r.u16()?, r.u8()?);
        let (ty, loc) = (r.q()?, r.u32()?);
        Ok(ExprRec { kind, bits, aux, cold, deps, ty, loc, words: decode_words(r)? })
    })?;
    b.stmts = decode_arena(r, 1 + 2 + 4 + 4, |r| {
        Ok(StmtRec { kind: r.u8()?, aux: r.u16()?, loc: r.u32()?, words: decode_words(r)? })
    })?;
    Ok(b)
}

fn null_kind(k: u8) -> Option<NullKind> {
    Some(match k {
        0 => NullKind::Pointer,
        1 => NullKind::MemberData,
        2 => NullKind::MemberFunction,
        _ => return None,
    })
}

fn string_encoding(e: u8) -> Option<StringEncoding> {
    Some(match e {
        0 => StringEncoding::Ordinary,
        1 => StringEncoding::Utf8,
        2 => StringEncoding::Utf16,
        3 => StringEncoding::Utf32,
        4 => StringEncoding::Wide16,
        5 => StringEncoding::Wide32,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Whether record `i` is a local declaration (its context a function).
pub(super) fn is_local(img: &Image, d: &DeclRec) -> bool {
    d.ctx != NONE
        && img
            .decls
            .get(d.ctx as usize)
            .is_some_and(|c| matches!(c.payload, Payload::Function { .. }))
}

/// Whether local record `d` (index `i`) of the using family is sound
/// (WP-1343): a using-declaration's qualifier names earlier records; a
/// shadow's target is an earlier namespace-scope declaration of its name
/// (or a shadow of one); a directive nominates an earlier namespace's
/// first declaration.
pub(super) fn local_using_ok(img: &Image, i: usize, d: &DeclRec) -> bool {
    let before = |x: u32| img.decls.get(x as usize).filter(|_| (x as usize) < i);
    // A local's name is a `pnames` entry, a namespace-scope one's a
    // `names` entry.
    let same_name = |d: &DeclRec, t: &DeclRec| {
        img.pnames
            .get(d.name as usize)
            .is_some_and(|n| img.names.get(t.name as usize).is_some_and(|m| m == n))
    };
    match &d.payload {
        Payload::Using { steps } => d.name != NONE && super::steps_ok(img, steps, i),
        Payload::Shadow { target } => before(*target).is_some_and(|t| {
            t.flags & DEP == 0
                && !is_local(img, t)
                && same_name(d, t)
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
        }),
        Payload::UsingDir { ns } => {
            d.name == NONE
                && before(*ns)
                    .is_some_and(|n| matches!(n.payload, Payload::Namespace) && n.prev == NONE)
        }
        Payload::NsAlias { target } => {
            d.name != NONE
                && before(*target).is_some_and(|t| {
                    !is_local(img, t)
                        && matches!(t.payload, Payload::Namespace | Payload::NsAlias { .. })
                })
        }
        // The enumeration's name, as at namespace scope.
        Payload::UsingEnum { e } => before(*e).is_some_and(|t| {
            !is_local(img, t)
                && same_name(d, t)
                && matches!(t.payload, Payload::Enum { def: Some(_), .. })
        }),
        _ => false,
    }
}

/// The number of parameters in the image.
pub(super) fn param_count(img: &Image) -> usize {
    img.decls
        .iter()
        .map(|d| match &d.payload {
            Payload::Function { params, .. } => params.len(),
            _ => 0,
        })
        .sum()
}

/// [`dc_ok`] for a parameter word: a function parameter, or (WP-1284)
/// a requires-expression parameter.
fn param_dc_ok(img: &Image, nparams: usize, w: u32, dc: Dc) -> bool {
    if w & super::requires::REQP != 0 {
        return matches!(dc, Dc::Value | Dc::Pack)
            && ((w & !(PARAM | super::requires::REQP)) as usize) < img.reqs.nparams;
    }
    matches!(dc, Dc::Value | Dc::Param | Dc::Pack) && ((w & !PARAM) as usize) < nparams
}

/// The declaration a chain of shadow records from `w` ends in; `None`
/// when the chain does not point strictly backwards.
fn shadow_end(img: &Image, w: u32) -> Option<u32> {
    let mut at = w;
    while let Some(DeclRec { payload: Payload::Shadow { target }, .. }) = img.decls.get(at as usize)
    {
        if *target >= at {
            return None;
        }
        at = *target;
    }
    Some(at)
}

/// Whether declaration word `w` names something of class `dc`.
#[allow(clippy::too_many_lines)] // one arm per declaration class
pub(super) fn dc_ok(img: &Image, nparams: usize, w: u32, dc: Dc) -> bool {
    if w == LEX_TU {
        return dc == Dc::Ctx;
    }
    if w == NONE {
        return false;
    }
    if w & PARAM != 0 {
        return param_dc_ok(img, nparams, w, dc);
    }
    let Some(d) = img.decls.get(w as usize) else { return false };
    if dc == Dc::Concept {
        return matches!(d.payload, Payload::Template { kind: CONCEPT_KIND, .. });
    }
    if dc == Dc::Lookup {
        return matches!(d.payload, Payload::Template { kind: 0 | 3, .. })
            || dc_ok(img, nparams, w, Dc::Found);
    }
    if dc == Dc::Found && matches!(d.payload, Payload::Template { kind: 0, .. }) {
        return true;
    }
    // S6: a declaration found through a using-declaration's shadow (of
    // a shadow ...): the declaration it ends in.
    if dc == Dc::Found && matches!(d.payload, Payload::Shadow { .. }) {
        return shadow_end(img, w).is_some_and(|at| dc_ok(img, nparams, at, Dc::Found));
    }
    // A specialization or member reference (S4b) names what its pattern
    // declares: a function template's pattern function, a class
    // template's pattern member.
    let d = match d.payload {
        Payload::Spec { template, .. } => match img.decls.get(template as usize) {
            Some(DeclRec { payload: Payload::Template { pattern, .. }, .. }) => {
                let Some(p) = img.decls.get(*pattern as usize) else { return false };
                p
            }
            _ => return false,
        },
        Payload::Member { pattern, .. } => {
            let Some(p) = img.decls.get(pattern as usize) else { return false };
            p
        }
        _ => d,
    };
    let in_class = || {
        img.decls.get(d.ctx as usize).is_some_and(|c| matches!(c.payload, Payload::Record { .. }))
    };
    match dc {
        Dc::Value | Dc::Found | Dc::Lookup => {
            matches!(
                d.payload,
                Payload::Function { .. }
                    | Payload::Var { .. }
                    | Payload::Enumerator { .. }
                    | Payload::Implicit { .. }
                    | Payload::Nttp { .. }
                    // A local structured binding (WP-1343).
                    | Payload::Binding { .. }
            )
        }
        Dc::Member => {
            matches!(
                d.payload,
                Payload::Function { .. }
                    | Payload::Var { .. }
                    | Payload::Enumerator { .. }
                    | Payload::Implicit { .. }
                    | Payload::Field { .. }
            ) && in_class()
        }
        Dc::Field => matches!(d.payload, Payload::Field { .. }),
        Dc::Ctor => {
            matches!(
                d.payload,
                Payload::Function { kind: 2, .. } | Payload::Implicit { kind: 0..=2 }
            )
        }
        Dc::Dtor => {
            matches!(d.payload, Payload::Function { kind: 3, .. } | Payload::Implicit { kind: 5 })
        }
        Dc::Var => matches!(d.payload, Payload::Var { .. }),
        // A block-scope function declaration's statement (WP-1293).
        Dc::Local => {
            matches!(
                d.payload,
                Payload::Var { .. }
                    | Payload::Function { kind: 0, .. }
                    | Payload::Assert { .. }
                    | Payload::Typedef { .. }
                    // WP-1343: a block-scope using-declaration or
                    // using-directive.
                    | Payload::Using { .. }
                    | Payload::Shadow { .. }
                    | Payload::UsingDir { .. }
                    | Payload::UsingEnum { .. }
                    | Payload::NsAlias { .. }
                    | Payload::Binding { .. }
            ) && is_local(img, d)
        }
        Dc::Label => matches!(d.payload, Payload::Label),
        Dc::Let => matches!(d.payload, Payload::Let { .. }),
        Dc::Param | Dc::Concept => false,
        Dc::Pack => {
            matches!(d.payload, Payload::Ttp { pack: true, .. } | Payload::Nttp { pack: true, .. })
                || matches!(&d.payload, Payload::Tttp(t) if t.pack)
        }
        // A default argument or member initializer's use context: any
        // declaration context the image carries.
        Dc::Ctx => matches!(
            d.payload,
            Payload::Function { .. }
                | Payload::Implicit { .. }
                | Payload::Record { .. }
                | Payload::Namespace
                | Payload::Export
        ),
        Dc::Namespace => matches!(d.payload, Payload::Namespace),
    }
}

/// The body sections' structural checks (after the declarations').
#[allow(clippy::too_many_lines)] // one check per section, in order
pub(super) fn validate_bodies(img: &Image, qref_ok: &dyn Fn(QRef) -> bool) -> Res<()> {
    let b = &img.bodies;
    validate_section(img, b, param_count(img), qref_ok, false)?;
    let ne = b.exprs.len();
    // The declaration fields that point into the bodies.
    for d in &img.decls {
        match &d.payload {
            Payload::Function { body, params, inits, .. } => {
                if let FnBody::Stmt(s) = body
                    && b.stmts.get(*s as usize).is_none_or(|r| r.kind != 0)
                {
                    return bad("bad function body");
                }
                if params.iter().any(|p| p.default != NONE && p.default as usize >= ne) {
                    return bad("bad default argument");
                }
                if inits.iter().any(|x| x.init as usize >= ne) {
                    return bad("bad ctor-initializer");
                }
            }
            Payload::Field { width, init, .. } => {
                if [*width, *init].iter().any(|&x| x != NONE && x as usize >= ne) {
                    return bad("bad field initializer");
                }
            }
            Payload::Enumerator { init, .. } => {
                if *init != NONE && *init as usize >= ne {
                    return bad("bad enumerator initializer");
                }
            }
            Payload::Assert { cond, msg } => {
                if *cond as usize >= ne || (*msg != NONE && *msg as usize >= ne) {
                    return bad("bad static assertion");
                }
            }
            Payload::Nttp { default, profile, sect, .. } => {
                // A section default is checked with its record (templ.rs).
                if (!*sect && *default != NONE && *default as usize >= ne)
                    || (*default == NONE && *profile)
                {
                    return bad("bad default template argument");
                }
            }
            Payload::Binding { expr, .. } => {
                if *expr != NONE && *expr as usize >= ne {
                    return bad("bad binding expression");
                }
            }
            Payload::Var { init, eval, spec, .. } => {
                // A section initializer is checked with its record
                // (specinit.rs).
                if spec & super::specinit::INIT_SECT == 0 && *init != NONE && *init as usize >= ne {
                    return bad("bad variable initializer");
                }
                if *eval != NONE && *eval != NOT_CONST && *eval as usize >= b.consts.len() {
                    return bad("bad variable value");
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// One body section's structural checks: the image's bodies, or
/// (`texpr`, WP-1300) the section of one expression a type or template
/// argument holds, which has no statements and names no parameter,
/// local declaration or requires-expression ([`super::exprs`]).
#[allow(clippy::too_many_lines)] // one check per field kind, in order
pub(super) fn validate_section(
    img: &Image,
    b: &Bodies,
    nparams: usize,
    qref_ok: &dyn Fn(QRef) -> bool,
    texpr: bool,
) -> Res<()> {
    let (ne, ns) = (b.exprs.len(), b.stmts.len());
    if texpr && (ne == 0 || ns != 0) {
        return bad("bad expression section");
    }
    // Since WP-1343 a section may name a function or requires-expression
    // parameter (its record bound is its function's, or every record's).
    let dok = |w: u32, dc: Dc| {
        (!texpr || !matches!(dc, Dc::Local | Dc::Label | Dc::Let)) && dc_ok(img, nparams, w, dc)
    };
    for (k, c) in b.consts.iter().enumerate() {
        if !qref_ok(c.ty())
            || !super::aggconsts::validate(img, nparams, &b.consts, k, texpr, qref_ok)
        {
            return bad("bad constant type");
        }
    }
    // Checks one token; `self_e` / `self_s` bound the operands of an
    // expression / statement record (post order).
    let tok = |f: F, w: &[u32], e_lim: usize, s_lim: usize| -> bool {
        let opt = |x: u32, lim: usize| x == NONE || (x as usize) < lim;
        match f {
            F::Raw | F::Loc => true,
            F::Bool => w[0] <= 1,
            F::Path => w[1..].chunks(2).all(|p| {
                img.decls.get(p[0] as usize).is_some_and(|r| match &r.payload {
                    Payload::Record { def: Some(df), .. } => (p[1] as usize) < df.bases.len(),
                    _ => false,
                })
            }),
            F::E => (w[0] as usize) < e_lim,
            F::OE => opt(w[0], e_lim),
            F::S => (w[0] as usize) < s_lim,
            F::OS => opt(w[0], s_lim),
            F::D(dc) => dok(w[0], dc),
            F::OD(dc) => w[0] == NONE || dok(w[0], dc),
            F::Cv => (w[0] as usize) < b.consts.len(),
            F::OCv => opt(w[0], b.consts.len()),
            F::Str => (w[0] as usize) < b.strings.len(),
            F::Qt => w[1] <= 7 && qref_ok(QRef { ty: w[0], quals: w[1] as u8 }),
            F::EL => w[1..].iter().all(|&x| (x as usize) < e_lim),
            F::SL => w[1..].iter().all(|&x| (x as usize) < s_lim),
            F::DL(dc) => w[1..].iter().all(|&x| dok(x, dc)),
            F::QL => {
                w[1..].chunks(2).all(|q| q[1] <= 7 && qref_ok(QRef { ty: q[0], quals: q[1] as u8 }))
            }
            F::AL => w[1..].chunks(2).all(|a| word_attr(a[0], Loc::INVALID).is_some()),
            F::Name => {
                (w[0] as usize) < img.pnames.len()
                    || w[0].checked_sub(super::N_OP).is_some_and(|o| {
                        u8::try_from(o).ok().and_then(OverloadedOperator::from_u8).is_some()
                    })
            }
            F::TA => w[1..].chunks(3).all(|a| match a[0] {
                0 => a[2] <= 7 && qref_ok(QRef { ty: a[1], quals: a[2] as u8 }),
                1 => (a[1] as usize) < e_lim && a[2] <= 1,
                // A return-type requirement's invented parameter.
                2 => !texpr && super::requires::invented_ok(img, a[1], a[2]),
                // A template (WP-1342).
                3 => !texpr && super::tttp::tname_rec(img, a[1]),
                4 => {
                    !texpr
                        && qref_ok(QRef { ty: a[1], quals: 0 })
                        && super::tttp::dep_tname_ty(img, a[1])
                }
                _ => false,
            }),
            F::RB => !texpr && (w[0] as usize) < img.reqs.recs.len(),
            F::Reqs => !texpr && super::requires::reqs_ok(img, w, e_lim, qref_ok),
            F::Gen | F::GenT => {
                let q =
                    |ty: u32, quals: u32| quals <= 7 && qref_ok(QRef { ty, quals: quals as u8 });
                let ctrl = match f {
                    F::GenT => q(w[1], w[2]),
                    _ => (w[1] as usize) < e_lim,
                };
                let mut defaults = 0usize;
                let assocs = gen_assocs(f, w).all(|a| {
                    let ty = if a[0] == NONE {
                        defaults += 1;
                        a[1] == 0
                    } else {
                        q(a[0], a[1])
                    };
                    ty && (a[3] as usize) < e_lim
                });
                ctrl && assocs && defaults <= 1
            }
            F::Nns => {
                let mut p = 1;
                while p < w.len() {
                    let ok = match w[p] {
                        0 => true,
                        1 => dok(w[p + 1], Dc::Namespace),
                        // S6: a namespace alias.
                        3 => img
                            .decls
                            .get(w[p + 1] as usize)
                            .is_some_and(|d| matches!(d.payload, Payload::NsAlias { .. })),
                        // WP-1284: a dependent identifier.
                        4 => (w[p + 1] as usize) < img.pnames.len(),
                        _ => {
                            w[p + 3] <= 1
                                && w[p + 2] <= 7
                                && qref_ok(QRef { ty: w[p + 1], quals: w[p + 2] as u8 })
                        }
                    };
                    if !ok {
                        return false;
                    }
                    p += [1, 2, 4, 2, 2][w[p] as usize];
                }
                true
            }
        }
    };
    // Statements first: `hi[s]` is one past the highest expression a
    // statement reaches through its sub-statements (all below it).
    let mut hi = vec![0usize; ns];
    for (i, s) in b.stmts.iter().enumerate() {
        let Some(&k) = STMT_KINDS.get(usize::from(s.kind)) else {
            return bad("unknown statement kind");
        };
        if !stmt_aux_ok(k, s.aux) {
            return bad("bad statement bits");
        }
        let fields = layout(stmt_shape(k), 0);
        let Some(toks) = tokens(&fields, &s.words) else {
            return bad("bad statement record");
        };
        let mut h = 0usize;
        for (f, w) in toks {
            if !tok(f, w, ne, i) {
                return bad("bad statement operand");
            }
            match f {
                F::E | F::OE if w[0] != NONE => h = h.max(w[0] as usize + 1),
                F::S | F::OS if w[0] != NONE => h = h.max(hi[w[0] as usize]),
                F::SL => h = w[1..].iter().fold(h, |h, &x| h.max(hi[x as usize])),
                _ => {}
            }
        }
        hi[i] = h;
    }
    for (i, e) in b.exprs.iter().enumerate() {
        let Some(&k) = EXPR_KINDS.get(usize::from(e.kind)) else {
            return bad("unknown expression kind");
        };
        if e.bits & 3 == 3
            || (e.bits >> 2) & 3 == 3
            || !expr_aux_ok(k, e.aux)
            || e.cold & !COLD_ALL != 0
            || (e.cold & (COLD_REWRITE | COLD_SRCLOC) != 0
                && !matches!(k, ExprKind::CXXDefaultArgExpr | ExprKind::CXXDefaultInitExpr))
            || (e.cold & (COLD_NAMING | COLD_FOUND) != 0 && k != ExprKind::UnresolvedLookupExpr)
            || (e.cold & COLD_FOUND != 0 && e.cold & COLD_NAMING == 0)
            || (e.cold & COLD_MEMBER != 0 && k != ExprKind::MemberExpr)
            || (e.cold & COLD_ARRAY != 0 && k != ExprKind::ArrayTypeTraitExpr)
            || (e.cold & COLD_PACK != 0 && k != ExprKind::IntegerLiteral)
            || e.deps & !ExprDeps::TYPE_VALUE_INST.with(ExprDeps::UNEXPANDED_PACK).bits() != 0
        {
            return bad("bad expression bits");
        }
        if !qref_ok(e.ty) {
            return bad("bad expression type");
        }
        let fields = layout(expr_shape(k, e.bits), e.cold);
        let Some(toks) = tokens(&fields, &e.words) else {
            return bad("bad expression record");
        };
        for &(f, w) in &toks {
            if !tok(f, w, i, ns) {
                return bad("bad expression operand");
            }
            if f == F::S && hi[w[0] as usize] > i {
                return bad("a statement expression reaching itself");
            }
        }
        if k == ExprKind::MaterializeTemporaryExpr
            && matches!(e.aux & 3, 0) != (toks[1].1[0] == NONE)
        {
            return bad("bad temporary");
        }
        if k == ExprKind::RequiresExpr && toks[0].1[0] != toks[1].1[0] {
            return bad("bad requires-expression");
        }
        // A dependent-scope name always has its qualifier.
        if k == ExprKind::DependentScopeDeclRefExpr && toks[0].1[0] == 0 {
            return bad("bad dependent-scope name");
        }
        // A `_Generic`'s selected expression is one of its associations'.
        if k == ExprKind::GenericSelectionExpr {
            let (f, w) = toks[0];
            let sel = toks[1].1[0];
            if sel != NONE && !gen_assocs(f, w).any(|a| a[3] == sel) {
                return bad("bad generic selection");
            }
        }
    }
    Ok(())
}

/// Every main-file offset the bodies hold (for the importer's bound
/// check).
pub(super) fn body_locs(img: &Image, out: &mut Vec<u32>) {
    section_locs(&img.bodies, out);
    super::requires::requires_locs(img, out);
}

/// One past the highest declaration record a validated expression
/// section names (0 for none): its parameters, the translation unit and
/// [`NONE`] are no records (WP-1300, [`super::exprs`]).
pub(super) fn section_decl_bound(b: &Bodies) -> usize {
    let rec = |w: u32| if w == NONE || w == LEX_TU || w & PARAM != 0 { 0 } else { w as usize + 1 };
    let mut hi = 0usize;
    section_decl_words(b, &mut |w| hi = hi.max(rec(w)));
    hi
}

/// The highest function parameter and requires-expression parameter a
/// section names (WP-1343), if any.
pub(super) fn section_max_params(b: &Bodies) -> (Option<u32>, Option<u32>) {
    let (mut hi, mut req) = (None, None);
    section_decl_words(b, &mut |w| {
        if w == NONE || w == LEX_TU || w & PARAM == 0 {
        } else if w & super::requires::REQP == 0 {
            hi = hi.max(Some(w & !PARAM));
        } else {
            req = req.max(Some(w & !(PARAM | super::requires::REQP)));
        }
    });
    (hi, req)
}

/// The function record of each parameter of the image, in parameter
/// order (WP-1343: a section naming a parameter is built once its
/// function's parameters are, at that function's record).
pub(super) fn param_owners(img: &Image) -> Vec<u32> {
    let mut v = Vec::new();
    for (i, d) in img.decls.iter().enumerate() {
        if let Payload::Function { params, .. } = &d.payload {
            v.extend(std::iter::repeat_n(i as u32, params.len()));
        }
    }
    v
}

/// Every declaration word a validated section's expressions hold.
fn section_decl_words(b: &Bodies, out: &mut dyn FnMut(u32)) {
    for e in &b.exprs {
        let Some(&k) = EXPR_KINDS.get(usize::from(e.kind)) else { continue };
        for (f, w) in tokens(&layout(expr_shape(k, e.bits), e.cold), &e.words).unwrap_or_default() {
            match f {
                F::D(_) | F::OD(_) => out(w[0]),
                F::DL(_) => w[1..].iter().for_each(|&x| out(x)),
                F::Path => w[1..].chunks(2).for_each(|p| out(p[0])),
                F::Nns => {
                    let mut p = 1;
                    while let Some(&k) = w.get(p) {
                        if matches!(k, 1 | 3)
                            && let Some(&x) = w.get(p + 1)
                        {
                            out(x);
                        }
                        p += [1, 2, 4, 2, 2].get(k as usize).copied().unwrap_or(w.len());
                    }
                }
                _ => {}
            }
        }
    }
}

/// Every main-file offset one body section holds.
pub(super) fn section_locs(b: &Bodies, out: &mut Vec<u32>) {
    for e in &b.exprs {
        out.push(e.loc);
        let Some(&k) = EXPR_KINDS.get(usize::from(e.kind)) else { continue };
        for (f, w) in tokens(&layout(expr_shape(k, e.bits), e.cold), &e.words).unwrap_or_default() {
            match f {
                F::Loc => out.push(w[0]),
                F::Gen | F::GenT => {
                    out.push(w[gen_ctrl_words(f)]);
                    out.extend(gen_assocs(f, w).map(|a| a[2]));
                }
                F::Reqs => super::requires::reqs_locs(w, out),
                _ => {}
            }
        }
    }
    for s in &b.stmts {
        out.push(s.loc);
        let Some(&k) = STMT_KINDS.get(usize::from(s.kind)) else { continue };
        for (f, w) in tokens(&layout(stmt_shape(k), 0), &s.words).unwrap_or_default() {
            if f == F::Loc {
                out.push(w[0]);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The writer
// ---------------------------------------------------------------------------

/// What still needs a body walk.
#[derive(Clone, Copy)]
pub(super) enum Root {
    /// Function record, body.
    Body(u32, StmtId),
    /// Variable record, initializer.
    Init(u32, ExprId),
    /// Structured binding record, binding expression (WP-1343).
    Binding(u32, ExprId),
    /// Global parameter index, default argument.
    Default(u32, ExprId),
    /// Field record, bit-field width.
    FieldWidth(u32, ExprId),
    /// Field record, default member initializer.
    FieldInit(u32, ExprId),
    /// Constructor record, ctor-initializer position, initializer.
    CtorInit(u32, u32, ExprId),
    /// WP-1278: declaration record, attribute record, argument slot
    /// (`a`, `b`), the argument.
    Attr(u32, u32, u8, ExprId),
    /// Non-type template parameter record, default argument (WP-1279).
    TemplDefault(u32, ExprId),
    /// WP-1284: declaration record, constraint record, the constraint.
    Cons(u32, u32, ExprId),
    /// WP-1307: static assertion record, slot (0 the condition, 1 the
    /// message), the expression.
    Assert(u32, u8, ExprId),
    /// WP-1300: function record, exception words, a `noexcept` operand
    /// naming the function's parameters ([`super::exprs::LateNx`]).
    Noexcept(u32, u8, ExprId),
    /// WP-1310: a slot of the pattern mem-initializers
    /// ([`super::patinit`]), the expression.
    PatInit(u32, ExprId),
    /// WP-1310: a dependent enumerator's record, its initializer.
    EnumInit(u32, ExprId),
}

/// The writer's body state.
#[derive(Default)]
pub(super) struct BodyState {
    /// Parameters: global index; and (record, position) per index.
    pub(super) params: FxHashMap<DeclId, u32>,
    pub(super) param_pos: Vec<(u32, u32)>,
    pub(super) roots: Vec<Root>,
    /// Base-specifiers (global index) of carried class definitions:
    /// (class record, position).
    pub(super) bspec: FxHashMap<u32, (u32, u32)>,
    /// Field ctor-initializers to resolve once every record exists:
    /// (constructor record, initializer position, field).
    pub(super) init_fields: Vec<(u32, u32, DeclId)>,
    /// Requires-expression bodies (WP-1284).
    pub(super) req: super::requires::ReqState,
    /// How many expression sections of types or template arguments are
    /// being walked (WP-1300, [`Writer::texpr_section`]).
    pub(super) texpr: u32,
    emap: FxHashMap<ExprId, u32>,
    smap: FxHashMap<StmtId, u32>,
    cmap: FxHashMap<ConstValueId, u32>,
    strmap: FxHashMap<StringLitId, u32>,
}

/// A field value on the writer's side, before translation.
#[derive(Clone, Copy, Debug)]
enum Fv {
    Raw(u32),
    E(Option<ExprId>),
    S(Option<StmtId>),
    D(Option<DeclId>, Dc),
    Cv(Option<ConstValueId>),
    Str(StringLitId),
    Loc(Loc),
    Qt(QualType),
    EL(ListId),
    SL(ListId),
    DL(ListId, Dc),
    /// What unresolved name `e` found (sema's `ule_found`, WP-1342),
    /// written as an [`F::DL`] of [`Dc::Lookup`].
    Found(ExprId),
    QL(ListId),
    Nns(Option<NnsId>),
    AL(ListId),
    Path(Option<ListId>),
    Name(DeclName),
    TA(Option<kiln_ast::ArgListId>),
    /// A `_Generic`'s raw operand list; true for a type-name control.
    Gen(ListId, bool),
    /// A requires-expression's body and its requirements record.
    RB(DeclId),
    Reqs(u32),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Node {
    E(ExprId),
    S(StmtId),
}

struct Frame {
    node: Node,
    fields: Vec<Fv>,
    kids: Vec<Node>,
    next: usize,
}

pub(super) const OUTSIDE: &str = "a body referring to a declaration outside the image";
/// WP-1300: what an expression in a type may not name (the importer
/// builds types before parameters and bodies).
pub(super) const TEXPR_LOCAL: &str =
    "an expression in a type naming a parameter or local declaration";

impl Writer<'_, '_, '_> {
    /// Whether a function's body is carried as statements (an importer
    /// may inline or evaluate it).
    pub(super) fn carries_body(spec: FnSpec) -> bool {
        spec.is_inline() || spec.implicit_inline() || spec.constexpr_kind() != ConstexprKind::None
    }

    /// A raw slot value read as field `f`.
    fn fv(f: F, raw: u32) -> W<Fv> {
        const BAD: &str = "an expression the image does not carry";
        Ok(match f {
            F::Raw => Fv::Raw(raw),
            F::Bool if raw <= 1 => Fv::Raw(raw),
            F::Bool => return Err(BAD),
            F::E => Fv::E(Some(ExprId::from_raw(raw).ok_or(BAD)?)),
            F::OE => Fv::E(ExprId::from_raw(raw)),
            F::S => Fv::S(Some(StmtId::from_raw(raw).ok_or(BAD)?)),
            F::OS => Fv::S(StmtId::from_raw(raw)),
            F::D(dc) => Fv::D(Some(DeclId::from_raw(raw).ok_or(BAD)?), dc),
            F::OD(dc) => Fv::D(DeclId::from_raw(raw), dc),
            F::Cv => Fv::Cv(Some(ConstValueId::from_raw(raw).ok_or(BAD)?)),
            F::OCv => Fv::Cv(ConstValueId::from_raw(raw)),
            F::Str => Fv::Str(StringLitId::from_raw(raw).ok_or(BAD)?),
            F::Loc => Fv::Loc(Loc::from_raw(raw)),
            F::Qt => Fv::Qt(QualType::from_raw(raw).ok_or(BAD)?),
            F::EL => Fv::EL(ListId::from_raw(raw).ok_or(BAD)?),
            F::SL => Fv::SL(ListId::from_raw(raw).ok_or(BAD)?),
            F::DL(dc) => Fv::DL(ListId::from_raw(raw).ok_or(BAD)?, dc),
            F::QL => Fv::QL(ListId::from_raw(raw).ok_or(BAD)?),
            F::Path => Fv::Path(ListId::from_raw(raw)),
            F::Nns => Fv::Nns(NnsId::from_raw(raw)),
            F::AL => Fv::AL(ListId::from_raw(raw).ok_or(BAD)?),
            F::Name => Fv::Name(DeclName::from_raw(raw).ok_or(BAD)?),
            F::TA => Fv::TA(kiln_ast::ArgListId::from_raw(raw)),
            F::Gen | F::GenT => Fv::Gen(ListId::from_raw(raw).ok_or(BAD)?, f == F::GenT),
            F::RB => Fv::RB(DeclId::from_raw(raw).ok_or(BAD)?),
            F::Reqs => Fv::Reqs(raw),
        })
    }

    fn side_fv(&self, side: Side, node: Node, raw: u32) -> W<Vec<Fv>> {
        let ast = &self.s.ast;
        let (ex, st) = (&ast.exprs, &ast.stmts);
        let (e, s) = match node {
            Node::E(e) => (Some(e), None),
            Node::S(s) => (None, Some(s)),
        };
        let ee = |x: ExprId| Fv::E(Some(x));
        let ss = |x: StmtId| Fv::S(Some(x));
        let var = |x: Option<DeclId>| Fv::D(x, Dc::Var);
        Ok(match (side, e, s) {
            (Side::Arms, Some(e), _) => {
                let [t, f] = if ex.get(e).kind() == ExprKind::ChooseExpr {
                    let [_, l, r] = ex.choose_operands(e);
                    [l, r]
                } else {
                    ex.cond_arms(e)
                };
                vec![ee(t), ee(f)]
            }
            (Side::CompoundAssign, Some(e), _) => {
                let CompoundAssignExtra { rhs, lhs_ty, result_ty } = *ex.compound_assign(e);
                vec![ee(rhs), Fv::Qt(lhs_ty), Fv::Qt(result_ty)]
            }
            (Side::InitList, Some(e), _) => {
                let InitListExtra { array_filler, union_field, rbrace } = *ex.init_list(e);
                vec![Fv::E(array_filler), Fv::Loc(rbrace), Fv::D(union_field, Dc::Field)]
            }
            (Side::DeclRef, Some(e), _) => match (raw, ex.decl_ref_extra(e)) {
                (0, _) | (_, None) => {
                    vec![Fv::Nns(None), Fv::D(None, Dc::Value), Fv::TA(None)]
                }
                (_, Some(x)) => vec![
                    Fv::Nns(x.qualifier),
                    Fv::D(x.found_decl, Dc::Found),
                    Fv::TA(x.template_args),
                ],
            },
            (Side::If, _, Some(s)) => {
                let IfData { init, cond_var, cond, then, els } = *st.if_data(s);
                vec![Fv::S(init), var(cond_var), Fv::E(cond), Fv::S(then), Fv::S(els)]
            }
            (Side::Switch, _, Some(s)) => {
                let SwitchData { init, cond_var, cond, body, cases } = *st.switch_data(s);
                vec![Fv::S(init), var(cond_var), ee(cond), ss(body), Fv::SL(cases)]
            }
            (Side::Case, _, Some(s)) => {
                let CaseData { lhs, rhs, sub } = *st.case_data(s);
                vec![ee(lhs), Fv::E(rhs), ss(sub)]
            }
            (Side::Loop, _, Some(s)) => {
                let LoopData { init, cond_var, cond, inc, body } = *st.loop_data(s);
                vec![Fv::S(init), var(cond_var), Fv::E(cond), Fv::E(inc), Fv::S(body)]
            }
            (Side::RangeFor, _, Some(s)) => {
                let RangeForData { init, range, begin, end, cond, inc, loop_var, body, colon } =
                    *st.range_for_data(s);
                vec![
                    Fv::S(init),
                    var(Some(range)),
                    var(begin),
                    var(end),
                    Fv::E(cond),
                    Fv::E(inc),
                    var(Some(loop_var)),
                    ss(body),
                    Fv::Loc(colon),
                ]
            }
            (Side::New, Some(e), _) => {
                let x = ex.new_extra(e);
                vec![
                    Fv::Qt(x.allocated_ty),
                    Fv::E(x.array_size),
                    Fv::E(x.init),
                    Fv::Raw(u32::from(x.placement.is_some())),
                    Fv::EL(x.placement.unwrap_or(ListId::EMPTY)),
                    Fv::D(x.operator_new, Dc::Value),
                    Fv::D(x.operator_delete, Dc::Value),
                    Fv::Loc(x.type_loc),
                ]
            }
            _ => return Err("a malformed body"),
        })
    }

    fn slots_fv(&self, node: Node, shape: [Slot; 2], raw: [u32; 2]) -> W<Vec<Fv>> {
        let mut v = Vec::new();
        for (slot, r) in shape.into_iter().zip(raw) {
            match slot {
                Slot::Zero if r == 0 => {}
                Slot::Zero => return Err("an expression the image does not carry"),
                Slot::F(f) => v.push(Self::fv(f, r)?),
                Slot::Side(sd) => v.extend(self.side_fv(sd, node, r)?),
            }
        }
        Ok(v)
    }

    /// The cold annotations of expression `e` (its mask; the fields go
    /// to `v`, in mask order).
    fn cold_fields(&self, e: ExprId, v: &mut Vec<Fv>) -> u16 {
        let ast = &self.s.ast;
        let (c, t) = (&ast.exprs.cold, &ast.templates);
        let mut cold = 0u16;
        if let Some(&l) = c.end_loc.get(e) {
            cold |= COLD_END;
            v.push(Fv::Loc(l));
        }
        if let Some(&q) = c.cast_written.get(e) {
            cold |= COLD_WRITTEN;
            v.push(Fv::Qt(q));
        }
        if let Some(&l) = c.trait_rhs_loc.get(e) {
            cold |= COLD_RHS;
            v.push(Fv::Loc(l));
        }
        if let Some(&l) = t.expr_begin.get(e) {
            cold |= COLD_BEGIN;
            v.push(Fv::Loc(l));
        }
        if let Some(&l) = t.member_op_loc.get(e) {
            cold |= COLD_OP;
            v.push(Fv::Loc(l));
        }
        if let Some(r) = ast.exprs.default_arg_rewritten(e) {
            cold |= COLD_REWRITE;
            v.push(Fv::E(Some(r)));
        }
        if let Some(&l) = self.s.templ.fold_ellipsis.get(&e) {
            cold |= COLD_ELLIPSIS;
            v.push(Fv::Loc(l));
        }
        let (nns, targs) = (t.expr_nns.get(e).copied(), t.expr_targs.get(e).copied());
        if nns.is_some() || targs.is_some() {
            cold |= COLD_LOOKUP;
            v.extend([Fv::Nns(nns), Fv::TA(targs)]);
        }
        if let Some(&l) = t.template_kw.get(e) {
            cold |= COLD_TEMPLATE_KW;
            v.push(Fv::Loc(l));
        }
        if let Some(sv) = ast.exprs.source_loc_values(e)
            && matches!(
                ast.exprs.get(e).kind(),
                ExprKind::CXXDefaultArgExpr | ExprKind::CXXDefaultInitExpr
            )
        {
            cold |= COLD_SRCLOC;
            v.extend([
                Fv::Loc(sv.at),
                Fv::D(Some(sv.ctx), Dc::Ctx),
                Fv::Raw(u32::from(sv.location.is_some())),
            ]);
        }
        // The naming class, and what lookup found when that is not the
        // name's declarations (a using-shadow, WP-1342).
        if ast.exprs.get(e).kind() == ExprKind::UnresolvedLookupExpr
            && let Some(f) = self.s.ovl.ule_found.get(&e)
        {
            cold |= COLD_NAMING;
            v.push(Fv::D(f.naming, Dc::Ctx));
            if f.found != ast.unresolved_decls(e) {
                cold |= COLD_FOUND;
                v.push(Fv::Found(e));
            }
        }
        if ast.exprs.get(e).kind() == ExprKind::MemberExpr
            && let Some(&x) = c.member_extra.get(e)
        {
            cold |= COLD_MEMBER;
            v.extend([
                Fv::Nns(x.qualifier),
                Fv::D(x.found_decl, Dc::Found),
                Fv::TA(x.template_args),
            ]);
        }
        if let Some(&x) = c.array_trait.get(e) {
            cold |= COLD_ARRAY;
            v.extend([Fv::E(x.dim), Fv::Raw(x.value as u32), Fv::Raw((x.value >> 32) as u32)]);
        }
        if let Some(&p) = t.size_of_pack_origin.get(e) {
            cold |= COLD_PACK;
            v.push(Fv::D(Some(p), Dc::Pack));
        }
        cold
    }

    /// Header and fields of a node (no translation yet).
    fn fields(&self, node: Node) -> W<(u8, u8, u16, u16, Vec<Fv>)> {
        let ast = &self.s.ast;
        match node {
            Node::E(e) => {
                let n = *ast.exprs.get(e);
                let k = n.kind();
                let ki = EXPR_KINDS.iter().position(|&x| x == k).ok_or(expr_reason(k))?;
                // An unexpanded pack (WP-1279) occurs only in a pattern.
                if n.deps().intersects(ExprDeps::ERROR) {
                    return Err("a pack expansion or an invalid expression");
                }
                // Satisfaction is checked again by the importer, never
                // carried (WP-1284): a checked concept-id falls back.
                if k == ExprKind::ConceptSpecializationExpr
                    && (n.aux() != 0 || self.s.inst.concepts.expr_sat.contains_key(&e))
                {
                    return Err("a checked concept-id");
                }
                if k == ExprKind::RequiresExpr {
                    self.requires_ok(e)?;
                }
                if !expr_aux_ok(k, n.aux()) {
                    return Err("an expression the image does not carry");
                }
                let mut v = self.slots_fv(node, expr_shape(k, n.raw_bits()), [n.a(), n.b()])?;
                // A braced argument placeholder (`T(a, {b})`) lives in a
                // Sema side table the image does not carry.
                if matches!(k, ExprKind::CXXUnresolvedConstructExpr | ExprKind::ParenListExpr)
                    && ListId::from_raw(n.a()).is_some_and(|l| {
                        ast.lists.iter::<ExprId>(l).any(|x| self.s.is_braced_arg(x))
                    })
                {
                    return Err("a braced argument of a dependent construction");
                }
                let c = &ast.exprs.cold;
                let t = &ast.templates;
                if (c.member_extra.contains(e) && k != ExprKind::MemberExpr)
                    || c.designators.contains(e)
                    || c.syntactic_list.contains(e)
                    || (c.array_trait.contains(e) && k != ExprKind::ArrayTypeTraitExpr)
                    // A builtin call's own values, and a default argument
                    // or member initializer use's, are computed again
                    // (WP-1310).
                    || (c.source_loc.contains(e)
                        && !matches!(
                            k,
                            ExprKind::SourceLocExpr
                                | ExprKind::CXXDefaultArgExpr
                                | ExprKind::CXXDefaultInitExpr
                        ))
                    || c.union_field.contains(e)
                    || c.offsetof_pattern.contains(e)
                    || c.splice_args.contains(e)
                    || (t.size_of_pack_origin.contains(e) && k != ExprKind::IntegerLiteral)
                    || t.pseudo_dtor_name.contains(e)
                    || t.pseudo_dtor_first.contains(e)
                {
                    return Err("an expression annotation the image does not carry");
                }
                let cold = self.cold_fields(e, &mut v);
                if k == ExprKind::UnresolvedLookupExpr {
                    self.lookup_ok(e)?;
                }
                Ok((ki as u8, n.raw_bits(), u16::from(n.aux()), cold, v))
            }
            Node::S(s) => {
                let n = *ast.stmts.get(s);
                let k = n.kind();
                let ki = STMT_KINDS.iter().position(|&x| x == k).ok_or(stmt_reason(k))?;
                if n.flags() != 0 || !stmt_aux_ok(k, n.aux()) {
                    return Err(if k == StmtKind::CompoundStmt {
                        "a block under a floating-point pragma"
                    } else {
                        "a statement the image does not carry"
                    });
                }
                let v = self.slots_fv(node, stmt_shape(k), [n.a(), n.b()])?;
                Ok((ki as u8, 0, n.aux(), 0, v))
            }
        }
    }

    fn kids(&self, fields: &[Fv]) -> Vec<Node> {
        let lists = &self.s.ast.lists;
        let mut v = Vec::new();
        for f in fields {
            match *f {
                Fv::E(Some(e)) => v.push(Node::E(e)),
                Fv::S(Some(s)) => v.push(Node::S(s)),
                Fv::EL(l) => {
                    v.extend(lists.get(l).iter().filter_map(|&w| ExprId::from_raw(w)).map(Node::E));
                }
                Fv::SL(l) => {
                    v.extend(lists.get(l).iter().filter_map(|&w| StmtId::from_raw(w)).map(Node::S));
                }
                // An expression argument (`f<N + 1>(x)`).
                Fv::TA(Some(l)) => {
                    v.extend(self.s.ast.types.args(l).iter().filter_map(|a| match a.view() {
                        kiln_ast::types::ArgView::Expr { expr, .. } => Some(Node::E(expr)),
                        _ => None,
                    }));
                }
                // The control expression (not a type), then every
                // association's expression ([`ExprArena::generic_assocs`]).
                Fv::Gen(l, is_ty) => {
                    let w = lists.get(l);
                    if !is_ty {
                        v.extend(w.first().and_then(|&x| ExprId::from_raw(x)).map(Node::E));
                    }
                    let assocs = w.get(2..).unwrap_or(&[]).chunks_exact(3);
                    v.extend(assocs.filter_map(|c| ExprId::from_raw(c[2])).map(Node::E));
                }
                Fv::Reqs(i) => v.extend(self.reqs_kids(i).into_iter().map(Node::E)),
                _ => {}
            }
        }
        v
    }

    fn done(&self, n: Node) -> Option<u32> {
        match n {
            Node::E(e) => self.bs.emap.get(&e).copied(),
            Node::S(s) => self.bs.smap.get(&s).copied(),
        }
    }

    /// Writes `root` and everything it reaches, children first; returns
    /// its index. Iterative: a deep expression costs heap, not stack.
    fn walk(&mut self, root: Node) -> W<u32> {
        if let Some(i) = self.done(root) {
            return Ok(i);
        }
        let mut open: FxHashSet<Node> = FxHashSet::default();
        let mut stack: Vec<Frame> = Vec::new();
        let (fields, kids) = self.frame(root)?;
        open.insert(root);
        stack.push(Frame { node: root, fields, kids, next: 0 });
        while let Some(top) = stack.last_mut() {
            if top.next < top.kids.len() {
                let k = top.kids[top.next];
                top.next += 1;
                if self.done(k).is_some() {
                    continue;
                }
                if !open.insert(k) {
                    return Err("a cyclic body");
                }
                let (fields, kids) = self.frame(k)?;
                stack.push(Frame { node: k, fields, kids, next: 0 });
                continue;
            }
            let Some(f) = stack.pop() else { break };
            open.remove(&f.node);
            self.emit(f.node, &f.fields)?;
        }
        self.done(root).ok_or("a malformed body")
    }

    fn frame(&self, n: Node) -> W<(Vec<Fv>, Vec<Node>)> {
        let (.., fields) = self.fields(n)?;
        let kids = self.kids(&fields);
        Ok((fields, kids))
    }

    fn emit(&mut self, node: Node, _fields: &[Fv]) -> W<()> {
        // The header is recomputed (cheap) so a frame holds only fields.
        let (kind, bits, aux, cold, fields) = self.fields(node)?;
        let mut words = Vec::with_capacity(fields.len() + 2);
        for f in fields {
            self.word(f, &mut words)?;
        }
        let ast = &self.s.ast;
        match node {
            Node::E(e) => {
                let n = *ast.exprs.get(e);
                let ty = self.qref(n.ty())?;
                let loc = self.loc(n.loc())?;
                let i =
                    u32::try_from(self.img.bodies.exprs.len()).map_err(|_| "a body too large")?;
                self.img.bodies.exprs.push(ExprRec {
                    kind,
                    bits,
                    aux: aux as u8,
                    cold,
                    deps: n.deps().bits(),
                    ty,
                    loc,
                    words,
                });
                self.bs.emap.insert(e, i);
            }
            Node::S(s) => {
                let loc = self.loc(ast.stmts.get(s).loc())?;
                let i =
                    u32::try_from(self.img.bodies.stmts.len()).map_err(|_| "a body too large")?;
                self.img.bodies.stmts.push(StmtRec { kind, aux, loc, words });
                self.bs.smap.insert(s, i);
            }
        }
        Ok(())
    }

    fn word(&mut self, f: Fv, out: &mut Vec<u32>) -> W<()> {
        let lists = &self.s.ast.lists;
        match f {
            Fv::Raw(v) => out.push(v),
            Fv::E(e) => out.push(match e {
                None => NONE,
                Some(e) => *self.bs.emap.get(&e).ok_or("a malformed body")?,
            }),
            Fv::S(s) => out.push(match s {
                None => NONE,
                Some(s) => *self.bs.smap.get(&s).ok_or("a malformed body")?,
            }),
            Fv::D(d, dc) => out.push(match d {
                None => NONE,
                Some(d) => self.dref(d, dc)?,
            }),
            Fv::Cv(c) => out.push(match c {
                None => NONE,
                Some(c) => self.cnst_full(c)?.ok_or("a constant the image does not carry")?,
            }),
            Fv::Str(s) => out.push(self.string(s)),
            Fv::Name(n) => {
                // Not a declared name of the unit (a member found at
                // instantiation): not gated, like a local's.
                if let kiln_ast::names::DeclNameView::Operator(op) = self.s.ast.names.view(n) {
                    out.push(super::N_OP + u32::from(op as u8));
                } else {
                    let s = self.ident(n).ok_or("a dependent name not named by an identifier")?;
                    out.push(Self::name_ix(&mut self.pnames, &mut self.img.pnames, &s));
                }
            }
            Fv::TA(l) => self.targ_words(l, out)?,
            Fv::Loc(l) => out.push(self.loc(l)?),
            Fv::Qt(q) => {
                let q = self.qref(q)?;
                out.extend([q.ty, u32::from(q.quals)]);
            }
            Fv::EL(l) | Fv::SL(l) => {
                let items: Vec<u32> = lists.get(l).to_vec();
                out.push(items.len() as u32);
                for w in items {
                    let id = match f {
                        Fv::EL(_) => {
                            ExprId::from_raw(w).and_then(|e| self.bs.emap.get(&e).copied())
                        }
                        _ => StmtId::from_raw(w).and_then(|s| self.bs.smap.get(&s).copied()),
                    };
                    out.push(id.ok_or("a malformed body")?);
                }
            }
            Fv::DL(l, dc) => {
                let items: Vec<u32> = lists.get(l).to_vec();
                out.push(items.len() as u32);
                for w in items {
                    let d = DeclId::from_raw(w).ok_or("a malformed body")?;
                    out.push(self.dref(d, dc)?);
                }
            }
            Fv::Found(e) => {
                let ds = self.s.ovl.ule_found.get(&e).map(|f| f.found.clone()).unwrap_or_default();
                out.push(ds.len() as u32);
                for d in ds {
                    out.push(self.dref(d, Dc::Lookup)?);
                }
            }
            Fv::QL(l) => {
                let items: Vec<u32> = lists.get(l).to_vec();
                out.push(items.len() as u32);
                for w in items {
                    let q = self.qref(QualType::from_raw(w).ok_or("a malformed body")?)?;
                    out.extend([q.ty, u32::from(q.quals)]);
                }
            }
            Fv::Nns(n) => self.nns(n, out)?,
            Fv::Path(None) => out.push(0),
            Fv::Path(Some(l)) => {
                let items: Vec<u32> = lists.get(l).to_vec();
                out.push(items.len() as u32 + 1);
                for g in items {
                    let (rec, pos) = *self.bs.bspec.get(&g).ok_or(OUTSIDE)?;
                    out.extend([rec, pos]);
                }
            }
            Fv::Gen(l, is_ty) => self.generic_words(l, is_ty, out)?,
            Fv::RB(d) => out.push(self.req_body(d)?),
            Fv::Reqs(i) => self.reqs_words(i, out)?,
            Fv::AL(l) => {
                let items: Vec<u32> = lists.get(l).to_vec();
                out.push(items.len() as u32);
                for w in items {
                    let a = *self.s.ast.attrs.get(AttrId::from_raw(w).ok_or("a malformed body")?);
                    out.push(
                        attr_word(&a).ok_or("a statement attribute the image does not carry")?,
                    );
                    out.push(self.loc(a.loc)?);
                }
            }
        }
        Ok(())
    }

    /// The record of written expression `e`.
    /// The section of expression `e`, held by a type or a template
    /// argument (WP-1300): written with its own records, constants and
    /// strings, so the image's bodies (perhaps mid-walk) stay as they
    /// were. Its root is its last expression.
    pub(super) fn texpr_section(&mut self, e: ExprId) -> W<Bodies> {
        if self.bs.texpr >= 8 {
            return Err("an expression in a type nested too deeply");
        }
        let bodies = std::mem::take(&mut self.img.bodies);
        let bs = &mut self.bs;
        // The parameters stay visible (WP-1343): a section may name those
        // numbered so far, which the importer creates before the type of
        // their function.
        let saved = (
            std::mem::take(&mut bs.emap),
            std::mem::take(&mut bs.smap),
            std::mem::take(&mut bs.cmap),
            std::mem::take(&mut bs.strmap),
        );
        bs.texpr += 1;
        let r = self.walk(Node::E(e));
        let bs = &mut self.bs;
        bs.texpr -= 1;
        (bs.emap, bs.smap, bs.cmap, bs.strmap) = saved;
        let b = std::mem::replace(&mut self.img.bodies, bodies);
        let i = r?;
        if !b.stmts.is_empty() || i as usize + 1 != b.exprs.len() {
            return Err("a statement in an expression in a type");
        }
        Ok(b)
    }

    pub(super) fn expr_word(&self, e: ExprId) -> W<u32> {
        self.bs.emap.get(&e).copied().ok_or("a malformed body")
    }

    /// A `_Generic`'s operand list `l` as an [`F::Gen`] / [`F::GenT`]
    /// token.
    fn generic_words(&mut self, l: ListId, is_ty: bool, out: &mut Vec<u32>) -> W<()> {
        let items: Vec<u32> = self.s.ast.lists.get(l).to_vec();
        if items.len() < 2 || !(items.len() - 2).is_multiple_of(3) {
            return Err("a malformed body");
        }
        out.push(((items.len() - 2) / 3) as u32);
        let expr = |s: &Self, w: u32| {
            ExprId::from_raw(w).and_then(|e| s.bs.emap.get(&e).copied()).ok_or("a malformed body")
        };
        if is_ty {
            let q = self.qref(QualType::from_raw(items[0]).ok_or("a malformed body")?)?;
            out.extend([q.ty, u32::from(q.quals)]);
        } else {
            out.push(expr(self, items[0])?);
        }
        out.push(self.loc(Loc::from_raw(items[1]))?);
        for c in items[2..].chunks_exact(3) {
            match QualType::from_raw(c[0]) {
                None => out.extend([NONE, 0]),
                Some(t) => {
                    let q = self.qref(t)?;
                    out.extend([q.ty, u32::from(q.quals)]);
                }
            }
            out.push(self.loc(Loc::from_raw(c[1]))?);
            out.push(expr(self, c[2])?);
        }
        Ok(())
    }

    /// Explicit template arguments `l` as an [`F::TA`] token (WP-1279).
    fn targ_words(&mut self, l: Option<kiln_ast::ArgListId>, out: &mut Vec<u32>) -> W<()> {
        use kiln_ast::types::ArgView;
        let Some(l) = l else {
            out.push(0);
            return Ok(());
        };
        let args = self.s.ast.types.args(l).to_vec();
        out.push(args.len() as u32 + 1);
        for a in args {
            match a.view() {
                ArgView::Type(q) => {
                    if let Some((rb, k)) = self.invented_arg(q)? {
                        out.extend([2, rb, k]);
                    } else {
                        let q = self.qref(q)?;
                        out.extend([0, q.ty, u32::from(q.quals)]);
                    }
                }
                ArgView::Expr { expr, profile } => {
                    let e = *self.bs.emap.get(&expr).ok_or("a malformed body")?;
                    out.extend([1, e, u32::from(profile.is_some())]);
                }
                // A template (WP-1342), not in an expression section
                // (built before the records it names).
                ArgView::Template(_) | ArgView::TemplateExpansion { .. } if self.bs.texpr == 0 => {
                    match self.arg_rec(a)? {
                        super::ArgRec::Template(r, k) => out.extend([3, r, k]),
                        super::ArgRec::DepTemplate(t, k) => out.extend([4, t, k]),
                        _ => return Err("a malformed body"),
                    }
                }
                _ => {
                    return Err(
                        "a template argument the image does not carry: an explicit argument of a name in a body",
                    );
                }
            }
        }
        Ok(())
    }

    fn nns(&mut self, n: Option<NnsId>, out: &mut Vec<u32>) -> W<()> {
        let types = &self.s.ast.types;
        let mut chain = Vec::new();
        let mut cur = n;
        while let Some(id) = cur {
            if chain.len() > 1024 {
                return Err("a malformed qualifier");
            }
            let v = types.nns_view(id);
            cur = match v {
                Nns::Global => None,
                Nns::Namespace { prefix, .. }
                | Nns::Type { prefix, .. }
                | Nns::NamespaceAlias { prefix, .. }
                | Nns::Identifier { prefix, .. } => prefix,
                Nns::Splice(_) => return Err("a qualifier naming a splice"),
            };
            chain.push(v);
        }
        out.push(chain.len() as u32);
        for v in chain.into_iter().rev() {
            match v {
                Nns::Global => out.push(0),
                Nns::Namespace { ns, .. } => {
                    let w = self.dref(ns, Dc::Namespace)?;
                    out.extend([1, w]);
                }
                Nns::Type { ty, template_kw, .. } => {
                    let q = self.qref(ty)?;
                    out.extend([2, q.ty, u32::from(q.quals), u32::from(template_kw)]);
                }
                // S6: an alias of the image, over the namespace it
                // denotes (rebuilt from the alias).
                Nns::NamespaceAlias { alias, ns, .. } => {
                    let w = self
                        .recs
                        .get(&alias)
                        .copied()
                        .filter(|&r| {
                            matches!(self.img.decls[r as usize].payload, Payload::NsAlias { .. })
                        })
                        .filter(|_| self.s.ast.resolve_alias(alias) == ns)
                        .ok_or("a qualifier naming a namespace alias outside the image")?;
                    out.extend([3, w]);
                }
                // WP-1284: `T::x::`, looked up at the instantiation.
                Nns::Identifier { name, .. } => {
                    let s = self.s.ast.symbols().as_str(name).to_owned();
                    out.extend([4, Self::name_ix(&mut self.pnames, &mut self.img.pnames, &s)]);
                }
                Nns::Splice(_) => return Err("a malformed qualifier"),
            }
        }
        Ok(())
    }

    fn string(&mut self, s: StringLitId) -> u32 {
        if let Some(&i) = self.bs.strmap.get(&s) {
            return i;
        }
        let (enc, bytes) = self.s.ast.strings.get(s);
        let i = self.img.bodies.strings.len() as u32;
        self.img.bodies.strings.push((enc as u8, bytes.to_vec()));
        self.bs.strmap.insert(s, i);
        i
    }

    /// The constant record of `c`, or `None` for a value the image does
    /// not carry (an aggregate, a pointer into an object).
    pub(super) fn cnst(&mut self, c: ConstValueId) -> W<Option<u32>> {
        if let Some(&i) = self.bs.cmap.get(&c) {
            return Ok(Some(i));
        }
        let rec = match self.s.ast.consts.scalar(c) {
            Some(Scalar::Int { ty, signed, bits }) => {
                let (width, words) = (bits.width(), bits.words().to_vec());
                ConstRec::Int { ty: self.qref(ty)?, signed, width, words }
            }
            Some(Scalar::Bool { ty, value }) => ConstRec::Bool { ty: self.qref(ty)?, value },
            Some(Scalar::Float { ty, kind, bits }) => {
                ConstRec::Float { ty: self.qref(ty)?, kind: kind as u8, bits }
            }
            Some(Scalar::Null { ty, kind }) => {
                ConstRec::Null { ty: self.qref(ty)?, kind: kind as u8 }
            }
            _ => return Ok(None),
        };
        let i = self.img.bodies.consts.len() as u32;
        self.img.bodies.consts.push(rec);
        self.bs.cmap.insert(c, i);
        Ok(Some(i))
    }

    /// [`Self::cnst`], or a pointer or aggregate constant (WP-1343,
    /// [`super::aggconsts`]): a body's, whose record may name
    /// declarations; a variable's value stays recomputed instead.
    pub(super) fn cnst_full(&mut self, c: ConstValueId) -> W<Option<u32>> {
        if let Some(i) = self.cnst(c)? {
            return Ok(Some(i));
        }
        let Some(rec) = self.agg_const(c)? else { return Ok(None) };
        let i = self.img.bodies.consts.len() as u32;
        self.img.bodies.consts.push(rec);
        self.bs.cmap.insert(c, i);
        Ok(Some(i))
    }

    /// The declaration word of `d` as class `dc`; a local declaration
    /// gets its record on first use.
    pub(super) fn dref(&mut self, d: DeclId, dc: Dc) -> W<u32> {
        let w = if let Some(&r) = self.recs.get(&d) {
            r
        } else if let Some(&p) = self.bs.params.get(&d) {
            PARAM | p
        } else if d == self.s.ast.tu {
            LEX_TU
        } else if let Some(&p) = self.bs.req.params.get(&d) {
            // A requires-expression parameter its body declared (WP-1343:
            // a type in a requirement may name it).
            PARAM | super::requires::REQP | p
        } else if self.bs.texpr > 0 && (self.req_param_body(d).is_some() || self.local(d)) {
            return Err(TEXPR_LOCAL);
        } else if let Some(body) = self.req_param_body(d) {
            self.req_param(d, body)?
        } else if self.local(d) {
            self.local_rec(d)?
        } else if let Some(r) = self.spec_rec(d)? {
            // S4b: a function template specialization.
            r
        } else if let Some(r) = self.member_rec(d)? {
            // S4b: a member of a class template specialization.
            r
        } else if let Some(r) = self.global_op_stub(d)? {
            // WP-1320: an implicit global allocation function.
            r
        } else {
            return Err(OUTSIDE);
        };
        let np = self.bs.param_pos.len();
        if !dc_ok(&self.img, np, w, dc) {
            return Err(if dc == Dc::Local {
                "a local declaration the image does not carry"
            } else {
                OUTSIDE
            });
        }
        Ok(w)
    }

    /// A record for local variable or label `d`, or (in a precompiled
    /// header) block-scope function or `extern` declaration `d`
    /// ([`Writer::local_extern_rec`]).
    #[allow(clippy::too_many_lines)] // one branch per local kind
    pub(super) fn local_rec(&mut self, d: DeclId) -> W<u32> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        let kind = n.kind();
        let local_fn = kind == DeclKind::Function && self.carries_local_extern(d);
        if !local_fn
            && !matches!(
                kind,
                DeclKind::Var
                    | DeclKind::Label
                    | DeclKind::LifetimeExtendedTemporary
                    | DeclKind::StaticAssert
                    | DeclKind::Typedef
                    | DeclKind::TypeAlias
                    | DeclKind::Using
                    | DeclKind::UsingShadow
                    | DeclKind::UsingDirective
                    | DeclKind::UsingEnum
                    | DeclKind::NamespaceAlias
                    | DeclKind::Binding
            )
        {
            return Err(local_reason(kind));
        }
        if kind == DeclKind::Binding {
            // Its decomposition variable first, which records every
            // binding of it, this one too (WP-1343).
            let v = ast.decls.binding_decomp(d).ok_or("a malformed structured binding")?;
            self.dref(v, Dc::Var)?;
            return self.recs.get(&d).copied().ok_or("a malformed structured binding");
        }
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        let ctx = n.ctx().and_then(|c| self.recs.get(&c).copied()).ok_or(OUTSIDE)?;
        if !matches!(self.img.decls[ctx as usize].payload, Payload::Function { .. }) {
            return Err("a local declaration in a nested scope the image does not carry");
        }
        let allowed = LOCAL_FLAGS | DEP | DeclFlags::HAS_LEXICAL_CTX.bits();
        let mut extra = n.flags().bits() & !allowed;
        if extra & DeclFlags::INVALID.bits() != 0 {
            return Err("an invalid local declaration");
        }
        if extra & DeclFlags::HAS_ATTRS.bits() != 0 {
            // Carried as a top-level declaration's are (WP-1310).
            if !self.attrs_carried(d) {
                return Err(super::attrs::UNCARRIED);
            }
            extra &= !DeclFlags::HAS_ATTRS.bits();
        }
        if extra != 0 {
            return Err("a local declaration with flags the image does not carry");
        }
        let lexical = if n.has(DeclFlags::HAS_LEXICAL_CTX) {
            match ast.decls.lexical_ctx(d) {
                Some(c) if Some(c) == n.ctx() => NONE,
                _ => return Err("a local declaration with a lexical context"),
            }
        } else {
            NONE
        };
        if n.misc().local_extern()
            || n.misc().name_independent()
            || n.misc().access() != Access::None
        {
            return Err("a local extern or placeholder declaration");
        }
        let prev = match ast.decls.redecl(d).and_then(|r| r.prev) {
            None => NONE,
            Some(p) if self.carries_local_extern(d) => self.local_extern_prev(p)?,
            Some(_) => return Err("a local redeclaration"),
        };
        let name = match n.name() {
            x if x == DeclName::EMPTY => NONE,
            x => {
                // Not a declared name of the unit: a local never meets an
                // importer's declaration, so it is not gated ("ODR merging").
                let s = self.ident(x).ok_or("a local declaration not named by an identifier")?;
                Self::name_ix(&mut self.pnames, &mut self.img.pnames, &s)
            }
        };
        let loc = self.loc(n.loc())?;
        let flags = n.flags().bits() & (LOCAL_FLAGS | DEP);
        let o = ast.modules.owner(d);
        let owner = u8::from(o.purview) | (u8::from(o.exported) << 1);
        self.local_type_first(d)?;
        let i = u32::try_from(self.img.decls.len()).map_err(|_| "a body too large")?;
        let attrs = self.attr_recs(d, i)?;
        let payload = match kind {
            DeclKind::Label => Payload::Label,
            DeclKind::LifetimeExtendedTemporary => {
                let ld = *ast.decls.lifetime_extended(d).ok_or("a malformed temporary")?;
                if ld.value.is_some() {
                    return Err("a lifetime-extended temporary with a constant value");
                }
                let ty = n.ty().ok_or("a malformed temporary")?;
                let extending = self.recs.get(&ld.extending).copied().ok_or(OUTSIDE)?;
                Payload::Let { ty: self.qref(ty)?, extending, index: ld.mangling_index }
            }
            _ => self.local_payload(d, i)?,
        };
        if self.img.decls.len() != i as usize {
            return Err("a specialization named while its user is recorded");
        }
        self.img.decls.push(DeclRec {
            flags,
            owner,
            access: 0,
            name,
            ctx,
            lexical,
            loc,
            prev,
            payload,
        });
        self.recs.insert(d, i);
        self.attr_commit(i, attrs);
        if kind == DeclKind::Var
            && let Some(l) = self.s.ast.decls.decomp_bindings(d)
        {
            let bs: Vec<DeclId> = self.s.ast.lists.iter::<DeclId>(l).collect();
            for b in bs {
                self.binding_rec(b, i)?;
            }
        }
        Ok(i)
    }

    /// The record of structured binding `b` of decomposition variable
    /// record `v` (WP-1343): after its holding variable's.
    fn binding_rec(&mut self, b: DeclId, v: u32) -> W<u32> {
        // A pack (P1061R10) or one of its elements (implicit).
        if self.s.binding_packs.contains(&b)
            || self.s.binding_pack_elems.contains_key(&b)
            || self.s.ast.decls.get(b).has(DeclFlags::IMPLICIT)
        {
            return Err("a structured binding pack");
        }
        let holding = match self.s.ast.decls.binding_var(b) {
            Some(h) => self.dref(h, Dc::Var)?,
            None => NONE,
        };
        let ty = match self.s.ast.decls.get(b).ty() {
            Some(t) => Some(self.qref(t)?),
            None => None,
        };
        let ast = &self.s.ast;
        let n = ast.decls.get(b);
        let allowed = LOCAL_FLAGS | DEP | DeclFlags::HAS_LEXICAL_CTX.bits();
        if n.flags().bits() & !allowed != 0 {
            return Err("a local declaration with flags the image does not carry");
        }
        if n.has(DeclFlags::HAS_LEXICAL_CTX) && ast.decls.lexical_ctx(b) != n.ctx() {
            return Err("a local declaration with a lexical context");
        }
        if n.misc().name_independent() {
            return Err("a local extern or placeholder declaration");
        }
        let s = self.ident(n.name()).ok_or("a local declaration not named by an identifier")?;
        let name = Self::name_ix(&mut self.pnames, &mut self.img.pnames, &s);
        let loc = self.loc(n.loc())?;
        let flags = n.flags().bits() & (LOCAL_FLAGS | DEP);
        let o = ast.modules.owner(b);
        let owner = u8::from(o.purview) | (u8::from(o.exported) << 1);
        let ctx = self.img.decls[v as usize].ctx;
        let expr = ast.decls.binding_expr(b);
        let i = u32::try_from(self.img.decls.len()).map_err(|_| "a body too large")?;
        self.img.decls.push(DeclRec {
            flags,
            owner,
            access: 0,
            name,
            ctx,
            lexical: NONE,
            loc,
            prev: NONE,
            payload: Payload::Binding { ty, decomp: v, holding, expr: NONE },
        });
        self.recs.insert(b, i);
        if let Some(e) = expr {
            self.bs.roots.push(Root::Binding(i, e));
        }
        Ok(i)
    }

    /// Records local `d`'s type before `d`: naming a specialization may
    /// add its record. An alias's own type names the alias, so its
    /// underlying type instead.
    fn local_type_first(&mut self, d: DeclId) -> W<()> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        if matches!(n.kind(), DeclKind::Typedef | DeclKind::TypeAlias) {
            let td = *ast.decls.typedef(d).ok_or("a malformed typedef")?;
            self.qref(td.underlying)?;
        } else if let Some(t) = n.ty() {
            self.qref(t)?;
        }
        Ok(())
    }

    /// The payload of local `d` (record `i`) other than a label or a
    /// lifetime-extended temporary.
    fn local_payload(&mut self, d: DeclId, i: u32) -> W<Payload> {
        match self.s.ast.decls.get(d).kind() {
            DeclKind::Function => self.function(d, i),
            // A block-scope `static_assert` or alias (WP-1320: `declval`'s
            // and `__call_is_nt`'s in <type_traits>), as at namespace scope.
            // WP-1343: a block-scope using-declaration (its shadows are
            // recorded where a name refers to them) or using-directive,
            // as at namespace scope (S6).
            DeclKind::StaticAssert
            | DeclKind::Typedef
            | DeclKind::TypeAlias
            | DeclKind::Using
            | DeclKind::UsingShadow
            | DeclKind::UsingDirective
            | DeclKind::UsingEnum
            | DeclKind::NamespaceAlias => self.record(d, i),
            _ => self.var_payload(d, i),
        }
    }

    /// Walks every pending root and patches the records that point at it.
    pub(super) fn walk_roots(&mut self) -> W<()> {
        // `-fkiln-bmi-image-census`: the record each root belongs to, and
        // the top-level record whose roots are being walked (a local's
        // roots follow its function's).
        let mut owner: Option<DeclId> = None;
        let inv: FxHashMap<u32, DeclId> = if self.census.is_some() {
            self.recs.iter().map(|(&d, &i)| (i, d)).collect()
        } else {
            FxHashMap::default()
        };
        // In source order: the census's examples are the first, and the
        // specialization references a body names first are written (so
        // instantiated by the importer) in the order replay made them
        // (WP-1344).
        self.bs.roots.reverse();
        while let Some(r) = self.bs.roots.pop() {
            let rec = match r {
                Root::Body(rec, _)
                | Root::Init(rec, _)
                | Root::Binding(rec, _)
                | Root::FieldWidth(rec, _)
                | Root::FieldInit(rec, _)
                | Root::CtorInit(rec, _, _)
                | Root::Attr(rec, _, _, _)
                | Root::TemplDefault(rec, _)
                | Root::Cons(rec, _, _)
                | Root::Assert(rec, _, _)
                | Root::Noexcept(rec, _, _)
                | Root::EnumInit(rec, _) => rec,
                Root::PatInit(..) => NONE,
                Root::Default(p, _) => self.bs.param_pos.get(p as usize).map_or(NONE, |x| x.0),
            };
            if let Some(&d) = inv.get(&rec) {
                owner = Some(d);
            }
            if let Err(why) = self.walk_root(r) {
                match owner {
                    Some(d) if self.census.is_some() => self.census_fail(d, why, false),
                    _ if self.census.is_some() => {}
                    _ => return Err(why),
                }
            }
        }
        Ok(())
    }

    /// Walks one pending root ([`Writer::walk_roots`]).
    fn walk_root(&mut self, r: Root) -> W<()> {
        {
            match r {
                Root::Body(rec, s) => {
                    let i = self.walk(Node::S(s))?;
                    if let Payload::Function { body, .. } =
                        &mut self.img.decls[rec as usize].payload
                    {
                        *body = FnBody::Stmt(i);
                    }
                }
                Root::Init(rec, e) => {
                    let i = self.walk(Node::E(e))?;
                    if let Payload::Var { init, .. } = &mut self.img.decls[rec as usize].payload {
                        *init = i;
                    }
                }
                Root::Binding(rec, e) => {
                    let i = self.walk(Node::E(e))?;
                    if let Payload::Binding { expr, .. } = &mut self.img.decls[rec as usize].payload
                    {
                        *expr = i;
                    }
                }
                Root::Default(p, e) => {
                    let i = self.walk(Node::E(e))?;
                    let (rec, k) = self.bs.param_pos[p as usize];
                    if let Payload::Function { params, .. } =
                        &mut self.img.decls[rec as usize].payload
                    {
                        params[k as usize].default = i;
                    }
                }
                Root::FieldWidth(rec, e) => {
                    let i = self.walk(Node::E(e))?;
                    if let Payload::Field { width, .. } = &mut self.img.decls[rec as usize].payload
                    {
                        *width = i;
                    }
                }
                Root::EnumInit(rec, e) => {
                    let i = self.walk(Node::E(e))?;
                    if let Payload::Enumerator { init, .. } =
                        &mut self.img.decls[rec as usize].payload
                    {
                        *init = i;
                    }
                }
                Root::FieldInit(rec, e) => {
                    let i = self.walk(Node::E(e))?;
                    if let Payload::Field { init, .. } = &mut self.img.decls[rec as usize].payload {
                        *init = i;
                    }
                }
                Root::CtorInit(rec, k, e) => {
                    let i = self.walk(Node::E(e))?;
                    if let Payload::Function { inits, .. } =
                        &mut self.img.decls[rec as usize].payload
                    {
                        inits[k as usize].init = i;
                    }
                }
                Root::Attr(_, k, slot, e) => {
                    let i = self.walk(Node::E(e))?;
                    let r = &mut self.img.attrs.recs[k as usize];
                    if slot == 0 {
                        r.a = i;
                    } else {
                        r.b = i;
                    }
                }
                Root::TemplDefault(rec, e) => {
                    let i = self.walk(Node::E(e))?;
                    self.set_nttp_default(rec, i);
                }
                Root::Assert(rec, slot, e) => {
                    let i = self.walk(Node::E(e))?;
                    if let Payload::Assert { cond, msg } = &mut self.img.decls[rec as usize].payload
                    {
                        *if slot == 0 { cond } else { msg } = i;
                    }
                }
                Root::Noexcept(rec, exc, e) => {
                    let expr = self.walk(Node::E(e))?;
                    self.img.late_nx.push(super::exprs::LateNx { rec, exc, expr });
                }
                Root::Cons(_, k, e) => {
                    let i = self.walk(Node::E(e))?;
                    self.img.cons.recs[k as usize].expr = i;
                }
                Root::PatInit(k, e) => {
                    let i = self.walk(Node::E(e))?;
                    self.img.patinits.exprs[k as usize] = i;
                }
            }
        }
        Ok(())
    }

    /// A variable's record (top-level or local) at index `rec`.
    pub(super) fn var_payload(&mut self, d: DeclId, rec: u32) -> W<Payload> {
        let ast = &self.s.ast;
        let vd = *ast.decls.var(d).ok_or("a malformed variable")?;
        if !vd.templ.is_none() && !self.pattern_of(d, DeclKind::VarTemplate) {
            // WP-1307: an explicit or partial specialization is listed
            // with its template and arguments.
            match vd.templ.get() {
                Some((
                    kiln_ast::decl::TemplLinkKind::Specialization
                    | kiln_ast::decl::TemplLinkKind::PartialSpecialization,
                    _,
                )) => {
                    self.class_spec_entry(d, rec)?;
                }
                _ => return Err("a template"),
            }
        }
        let bits = vd.spec.bits();
        if var_spec(bits) != Some(vd.spec) {
            return Err("a variable with specifiers the image does not carry");
        }
        let ty = self.qref(ast.decls.get(d).ty().ok_or("a malformed variable")?)?;
        let eval = match vd.eval.state() {
            VarEvalState::NotEvaluated => NONE,
            VarEvalState::NotConstant => NOT_CONST,
            // An aggregate value is recomputed by the importer on use.
            VarEvalState::Value(c) => self.cnst(c)?.unwrap_or(NONE),
            VarEvalState::Evaluating => return Err("a variable under evaluation"),
        };
        if let Some(e) = vd.init {
            // WP-1344: a pattern's static data member's, as a section.
            if let Some(i) = self.static_init_section(d, e) {
                let spec = bits | super::specinit::INIT_SECT;
                return Ok(Payload::Var { ty, spec, init: i, eval });
            }
            self.bs.roots.push(Root::Init(rec, e));
        }
        Ok(Payload::Var { ty, spec: bits, init: NONE, eval })
    }
}

/// The flags a local record may carry.
pub(super) const LOCAL_FLAGS: u8 =
    DeclFlags::IMPLICIT.bits() | DeclFlags::USED.bits() | DeclFlags::REFERENCED.bits();

/// The census reason of a local declaration of kind `k` the image does
/// not carry (WP-1314: one per kind).
fn local_reason(k: DeclKind) -> &'static str {
    match k {
        DeclKind::CXXRecord | DeclKind::Enum => "a local class or enumeration",
        DeclKind::Using | DeclKind::UsingShadow | DeclKind::UsingDirective => {
            "a local using-declaration or using-directive"
        }
        DeclKind::Binding => "a local structured binding",
        // A default argument use of a function template
        // specialization's parameter (`f<char>()`): the specialization's
        // parameters have no record (WP-1343 remaining).
        DeclKind::ParmVar => {
            "a local declaration the image does not carry (a specialization's parameter)"
        }
        _ => "a local declaration the image does not carry",
    }
}

fn expr_reason(k: ExprKind) -> &'static str {
    use ExprKind as X;
    match k {
        X::CXXDeleteExpr => "a new or delete expression",
        X::CXXTypeidExpr | X::CXXDynamicCastExpr => "a typeid or dynamic_cast expression",
        X::CXXStdInitializerListExpr => "a std::initializer_list",
        X::CXXInheritedCtorInitExpr => "an inherited constructor",
        X::LambdaExpr => "a lambda expression",
        X::RequiresExpr => "a requires-expression or concept",
        X::CoawaitExpr | X::DependentCoawaitExpr | X::CoyieldExpr => "a coroutine",
        X::CXXReflectExpr | X::CXXSpliceExpr => "a reflection or splice",
        // WP-1314: the template expressions not carried yet, one census
        // reason each (the rest of the family is in [`EXPR_KINDS`]).
        X::UnresolvedMemberExpr => "a template expression (an unresolved member reference)",
        X::SubstNonTypeTemplateParmPackExpr | X::FunctionParmPackExpr => {
            "a template expression (a substituted parameter pack)"
        }
        X::PackIndexingExpr => "a template expression (pack indexing)",
        X::CXXExpansionSelectExpr => "a template expression (an expansion statement's element)",
        X::DesignatedInitExpr => {
            "an expression the image does not carry yet (a designated initializer)"
        }
        _ => "an expression the image does not carry yet",
    }
}

fn stmt_reason(k: StmtKind) -> &'static str {
    use StmtKind as K;
    match k {
        K::CXXTryStmt | K::CXXCatchStmt => "a try block",
        K::CoroutineBodyStmt | K::CoreturnStmt => "a coroutine",
        K::CXXExpansionStmt => "an expansion statement",
        K::GCCAsmStmt => "an asm statement",
        K::ContractAssertStmt => "a contract assertion",
        _ => "a statement the image does not carry yet",
    }
}

// ---------------------------------------------------------------------------
// The importer
// ---------------------------------------------------------------------------

/// What the importer built: the expression and statement ids of the
/// image's records (consecutive from the bases) and its constants.
pub(super) struct Built {
    ebase: u32,
    sbase: u32,
    pub(super) consts: Vec<ConstValueId>,
    /// The requires-expression bodies (WP-1284).
    pub(super) reqs: super::requires::ReqsBuilt,
}

impl Built {
    pub(super) fn expr(&self, i: u32) -> ExprId {
        ExprId::from_raw(self.ebase + i)
            .unwrap_or_else(|| unreachable!("expression ids start at 1"))
    }
    pub(super) fn stmt(&self, i: u32) -> StmtId {
        StmtId::from_raw(self.sbase + i).unwrap_or_else(|| unreachable!("statement ids start at 1"))
    }
}

impl Sema<'_, '_> {
    /// Builds the body records of `img` (after its declarations: `ids`
    /// are the records', `params` the parameters in image order).
    pub(super) fn image_bodies(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        params: &[DeclId],
        reqs: super::requires::ReqsBuilt,
        main: &super::Locs,
    ) -> Built {
        let b = &img.bodies;
        let (consts, strs) = self.image_consts_strs(img, b, tys, ids);
        let built = Built {
            ebase: self.ast.exprs.len() as u32 + 1,
            sbase: self.ast.stmts.len() as u32 + 1,
            consts,
            reqs,
        };
        // Values of every token, as the AST slot would hold them.
        let mut vals: Vec<u32> = Vec::new();
        for (i, e) in b.exprs.iter().enumerate() {
            self.image_expr_rec(img, tys, ids, &[], &built, &strs, main, e, i, &mut vals);
        }
        for (i, s) in b.stmts.iter().enumerate() {
            let k = STMT_KINDS[usize::from(s.kind)];
            let shape = stmt_shape(k);
            let fields = layout(shape, 0);
            let toks = tokens(&fields, &s.words).unwrap_or_default();
            vals.clear();
            for &(f, w) in &toks {
                let v = self.image_val(img, tys, ids, params, &built, &strs, main, f, w);
                vals.push(v);
            }
            let (a, bb, _) = self.image_slots(shape, &vals, Node::S(built.stmt(0)));
            let loc = main.get(s.loc);
            let id = self.ast.stmts.push(StmtNode::new(k, s.aux, loc, a, bb));
            debug_assert_eq!(id, built.stmt(i as u32));
        }
        built
    }

    /// The constants and strings of body section `b`.
    fn image_consts_strs(
        &mut self,
        img: &Image,
        b: &Bodies,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
    ) -> (Vec<ConstValueId>, Vec<StringLitId>) {
        let mut consts = Vec::with_capacity(b.consts.len());
        for c in &b.consts {
            let ty = self.image_ty(img, tys, ids, c.ty());
            let s = match c {
                ConstRec::Int { signed, width, words, .. } => IntBits::from_words(*width, words)
                    .map(|bits| Scalar::Int { ty, signed: *signed, bits }),
                ConstRec::Bool { value, .. } => Some(Scalar::Bool { ty, value: *value }),
                ConstRec::Float { kind, bits, .. } => {
                    FloatKind::from_u8(*kind).map(|kind| Scalar::Float { ty, kind, bits: *bits })
                }
                ConstRec::Null { kind, .. } => {
                    null_kind(*kind).map(|kind| Scalar::Null { ty, kind })
                }
                ConstRec::Ptr { .. } | ConstRec::Agg { .. } => None,
            };
            // A value the pools reject (checked, never trusted) becomes 0.
            let id = match s.map(|s| self.ast.consts.push_scalar(s)) {
                Some(Ok(id)) => id,
                None if matches!(c, ConstRec::Ptr { .. } | ConstRec::Agg { .. }) => {
                    let mut q = |s: &mut Self, t: QRef| s.image_ty(img, tys, ids, t);
                    match self.image_agg_const(c, ty, &consts, ids, &mut q) {
                        Some(id) => id,
                        None => self.int_const(ty, 0),
                    }
                }
                _ => self.int_const(ty, 0),
            };
            consts.push(id);
        }
        let mut strs = Vec::with_capacity(b.strings.len());
        for (enc, bytes) in &b.strings {
            let enc = string_encoding(*enc).unwrap_or(StringEncoding::Ordinary);
            let id = match self.ast.strings.try_intern(enc, bytes) {
                Ok(id) => id,
                Err(_) => {
                    self.ast.strings.try_intern(StringEncoding::Ordinary, &[]).unwrap_or_else(
                        |_| unreachable!("the empty ordinary string is always internable"),
                    )
                }
            };
            strs.push(id);
        }
        (consts, strs)
    }

    /// Builds expression record `e`, the `i`-th of its section.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)] // the importer's tables, one read per cold annotation
    fn image_expr_rec(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        params: &[DeclId],
        built: &Built,
        strs: &[StringLitId],
        main: &super::Locs,
        e: &ExprRec,
        i: usize,
        vals: &mut Vec<u32>,
    ) {
        let k = EXPR_KINDS[usize::from(e.kind)];
        let shape = expr_shape(k, e.bits);
        let fields = layout(shape, e.cold);
        let toks = tokens(&fields, &e.words).unwrap_or_default();
        vals.clear();
        for &(f, w) in &toks {
            let v = self.image_val(img, tys, ids, params, built, strs, main, f, w);
            vals.push(v);
        }
        let (a, bb, used) = self.image_slots(shape, vals, Node::E(built.expr(0)));
        let ty = self.image_ty(img, tys, ids, e.ty);
        let loc = main.get(e.loc);
        let mut deps = ExprDeps::NONE;
        for d in
            [ExprDeps::TYPE, ExprDeps::VALUE, ExprDeps::INSTANTIATION, ExprDeps::UNEXPANDED_PACK]
        {
            if e.deps & d.bits() != 0 {
                deps |= d;
            }
        }
        let node = ExprNode::from_raw(k, e.bits, deps, e.aux, ty, loc, a, bb);
        let id = self.ast.exprs.push(node);
        debug_assert_eq!(id, built.expr(i as u32));
        let mut rest = vals[used..].iter().copied();
        let mut next = || rest.next().unwrap_or(0);
        if e.cold & COLD_END != 0 {
            let _ = self.ast.exprs.cold.end_loc.insert(id, Loc::from_raw(next()));
        }
        if e.cold & COLD_WRITTEN != 0
            && let Some(q) = QualType::from_raw(next())
        {
            let _ = self.ast.exprs.cold.cast_written.insert(id, q);
        }
        if e.cold & COLD_RHS != 0 {
            let _ = self.ast.exprs.cold.trait_rhs_loc.insert(id, Loc::from_raw(next()));
        }
        if e.cold & COLD_BEGIN != 0 {
            let _ = self.ast.templates.expr_begin.insert(id, Loc::from_raw(next()));
        }
        if e.cold & COLD_OP != 0 {
            let _ = self.ast.templates.member_op_loc.insert(id, Loc::from_raw(next()));
        }
        // Only on a default-argument use (the copy precedes it, so it
        // is already built); anything else is dropped.
        let rewrite = if e.cold & COLD_REWRITE != 0 { ExprId::from_raw(next()) } else { None };
        if let Some(r) = rewrite
            && matches!(k, ExprKind::CXXDefaultArgExpr | ExprKind::CXXDefaultInitExpr)
        {
            let _ = self.ast.exprs.set_default_arg_rewritten(id, r);
        }
        if e.cold & COLD_ELLIPSIS != 0 {
            self.templ.fold_ellipsis.insert(id, Loc::from_raw(next()));
        }
        if e.cold & COLD_LOOKUP != 0 {
            let (nns, targs) = (NnsId::from_raw(next()), kiln_ast::ArgListId::from_raw(next()));
            if let Some(n) = nns {
                let _ = self.ast.templates.expr_nns.insert(id, n);
            }
            if let Some(t) = targs {
                let _ = self.ast.templates.expr_targs.insert(id, t);
            }
        }
        if e.cold & COLD_TEMPLATE_KW != 0 {
            let _ = self.ast.templates.template_kw.insert(id, Loc::from_raw(next()));
        }
        if e.cold & COLD_SRCLOC != 0 {
            let at = Loc::from_raw(next());
            let ctx = DeclId::from_raw(next());
            let location = next() != 0;
            if let Some(ctx) = ctx {
                self.srcloc.image_uses.push((id, at, ctx, location));
            }
        }
        if e.cold & COLD_NAMING != 0 {
            // What `build_overload_ref` keeps: the naming class, and what
            // lookup found (the decls, or the shadows naming them).
            let naming = DeclId::from_raw(next());
            let decls = self.ast.unresolved_decls(id);
            let found = if e.cold & COLD_FOUND != 0 {
                ListId::from_raw(next())
                    .map(|l| self.ast.lists.iter::<DeclId>(l).collect::<Vec<DeclId>>())
                    .filter(|f| f.len() == decls.len())
                    .unwrap_or(decls)
            } else {
                decls
            };
            self.ovl.ule_found.insert(id, crate::overload::UleFound { found, naming });
        }
        if e.cold & COLD_MEMBER != 0 {
            let qualifier = NnsId::from_raw(next());
            let found_decl = DeclId::from_raw(next());
            let template_args = kiln_ast::ArgListId::from_raw(next());
            if qualifier.is_some() || found_decl.is_some() || template_args.is_some() {
                let x = DeclRefExtra { qualifier, template_args, found_decl };
                let _ = self.ast.exprs.cold.member_extra.insert(id, x);
            }
        }
        if e.cold & COLD_ARRAY != 0 {
            let dim = ExprId::from_raw(next());
            let value = u64::from(next()) | (u64::from(next()) << 32);
            let x = kiln_ast::expr::ArrayTraitExtra { dim, value };
            let _ = self.ast.exprs.cold.array_trait.insert(id, x);
        }
        if e.cold & COLD_PACK != 0
            && let Some(p) = DeclId::from_raw(next())
        {
            let _ = self.ast.templates.size_of_pack_origin.insert(id, p);
        }
    }

    /// Builds expression section `b` of a type or a template argument
    /// (WP-1300, [`super::exprs`]): no statements or requires-expressions;
    /// a parameter is one built so far (WP-1343); returns its root, the
    /// last expression.
    pub(super) fn image_texpr_section(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        b: &Bodies,
        main: &super::Locs,
    ) -> Option<ExprId> {
        let (consts, strs) = self.image_consts_strs(img, b, tys, ids);
        let built = Built {
            ebase: self.ast.exprs.len() as u32 + 1,
            sbase: self.ast.stmts.len() as u32 + 1,
            consts,
            reqs: super::requires::ReqsBuilt::default(),
        };
        let mut vals: Vec<u32> = Vec::new();
        for (i, e) in b.exprs.iter().enumerate() {
            self.image_expr_rec(img, tys, ids, &[], &built, &strs, main, e, i, &mut vals);
        }
        let n = u32::try_from(b.exprs.len().checked_sub(1)?).ok()?;
        Some(built.expr(n))
    }

    /// One token's slot value.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)] // the importer's tables
    fn image_val(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        params: &[DeclId],
        built: &Built,
        strs: &[StringLitId],
        main: &super::Locs,
        f: F,
        w: &[u32],
    ) -> u32 {
        let decl = |s: &Self, w: u32| -> u32 {
            if w == NONE {
                return 0;
            }
            let d = if w == LEX_TU {
                Some(s.ast.tu)
            } else if w & PARAM != 0 && w & super::requires::REQP != 0 {
                let g = (w & !(PARAM | super::requires::REQP)) as usize;
                built.reqs.params.get(g).or_else(|| s.image_texprs.req_params.get(g)).copied()
            } else if w & PARAM != 0 {
                // A section's parameter (WP-1343): one built so far.
                let g = (w & !PARAM) as usize;
                params.get(g).or_else(|| s.image_texprs.params.get(g)).copied()
            } else {
                ids.get(w as usize).copied()
            };
            RawId::raw_opt(d)
        };
        let opt = |x: u32, base: u32| if x == NONE { 0 } else { base + x };
        match f {
            F::Raw | F::Bool => w[0],
            F::Path => {
                if w[0] == 0 {
                    0
                } else {
                    let v: Vec<u32> = w[1..]
                        .chunks(2)
                        .map(|p| {
                            let bases = ids
                                .get(p[0] as usize)
                                .and_then(|&c| self.ast.decls.record(c))
                                .map_or(ListId::EMPTY, |r| r.bases);
                            self.ast.lists.get(bases).get(p[1] as usize).copied().unwrap_or(0)
                        })
                        .collect();
                    self.ast.lists.push_raw(&v).raw()
                }
            }
            F::E | F::OE => opt(w[0], built.ebase),
            F::S | F::OS => opt(w[0], built.sbase),
            F::D(_) | F::OD(_) => decl(self, w[0]),
            F::Cv | F::OCv => {
                if w[0] == NONE {
                    0
                } else {
                    built.consts[w[0] as usize].raw()
                }
            }
            F::Str => strs[w[0] as usize].raw(),
            F::Name => {
                if let Some(o) = w[0].checked_sub(super::N_OP) {
                    u8::try_from(o)
                        .ok()
                        .and_then(OverloadedOperator::from_u8)
                        .map_or(DeclName::EMPTY, DeclName::operator)
                        .to_raw()
                } else {
                    let n = img.pnames.get(w[0] as usize).map_or("", String::as_str);
                    DeclName::ident(self.intern(n)).to_raw()
                }
            }
            F::TA => {
                if w[0] == 0 {
                    return 0;
                }
                let mut args = Vec::with_capacity(w.len() / 3);
                for a in w[1..].chunks(3) {
                    args.push(if a[0] == 0 {
                        let q = self.image_ty(img, tys, ids, QRef { ty: a[1], quals: a[2] as u8 });
                        kiln_ast::types::TemplateArg::ty(q)
                    } else if a[0] == 2 {
                        kiln_ast::types::TemplateArg::ty(built.reqs.invented(a[1], a[2]))
                    } else if a[0] == 3 {
                        let rec = super::ArgRec::Template(a[1], a[2]);
                        self.image_targ(ids, QualType::INT, &rec)
                    } else if a[0] == 4 {
                        let q = self.image_ty(img, tys, ids, QRef { ty: a[1], quals: 0 });
                        let rec = super::ArgRec::DepTemplate(a[1], a[2]);
                        self.image_targ(ids, q, &rec)
                    } else {
                        let e = ExprId::from_raw(built.ebase + a[1])
                            .unwrap_or_else(|| unreachable!("validated operand"));
                        let profile = (a[2] != 0).then(|| self.ast.profile_expr(e));
                        kiln_ast::types::TemplateArg::expr(e, profile)
                    });
                }
                self.ast.types.arg_list(&args).raw()
            }
            F::Loc => main.get(w[0]).to_raw(),
            F::RB => RawId::raw_opt(built.reqs.body(w[0])),
            F::Reqs => self.image_reqs(img, tys, ids, built, main, w),
            // The AST's raw list ([`AstBuilder::generic_selection`]):
            // control, its location, then (type, location, expression)
            // per association.
            F::Gen | F::GenT => {
                let mut v = Vec::with_capacity(w.len());
                let ctrl = match f {
                    F::GenT => {
                        self.image_ty(img, tys, ids, QRef { ty: w[1], quals: w[2] as u8 }).to_raw()
                    }
                    _ => built.ebase + w[1],
                };
                v.extend([ctrl, main.get(w[gen_ctrl_words(f)]).to_raw()]);
                for a in gen_assocs(f, w) {
                    let ty = if a[0] == NONE {
                        0
                    } else {
                        self.image_ty(img, tys, ids, QRef { ty: a[0], quals: a[1] as u8 }).to_raw()
                    };
                    v.extend([ty, main.get(a[2]).to_raw(), built.ebase + a[3]]);
                }
                self.ast.lists.push_raw(&v).raw()
            }
            F::Qt => self.image_ty(img, tys, ids, QRef { ty: w[0], quals: w[1] as u8 }).to_raw(),
            F::EL => {
                let v: Vec<u32> = w[1..].iter().map(|&x| built.ebase + x).collect();
                self.ast.lists.push_raw(&v).raw()
            }
            F::SL => {
                let v: Vec<u32> = w[1..].iter().map(|&x| built.sbase + x).collect();
                self.ast.lists.push_raw(&v).raw()
            }
            F::DL(_) => {
                let v: Vec<u32> = w[1..].iter().map(|&x| decl(self, x)).collect();
                self.ast.lists.push_raw(&v).raw()
            }
            F::QL => {
                let v: Vec<u32> = w[1..]
                    .chunks(2)
                    .map(|q| {
                        self.image_ty(img, tys, ids, QRef { ty: q[0], quals: q[1] as u8 }).to_raw()
                    })
                    .collect();
                self.ast.lists.push_raw(&v).raw()
            }
            F::Nns => {
                let mut cur: Option<NnsId> = None;
                let mut p = 1;
                while p < w.len() {
                    let n = match w[p] {
                        0 => Nns::Global,
                        1 => {
                            let ns = DeclId::from_raw(decl(self, w[p + 1]));
                            match ns {
                                Some(ns) => Nns::Namespace { prefix: cur, ns },
                                None => Nns::Global,
                            }
                        }
                        3 => match DeclId::from_raw(decl(self, w[p + 1])) {
                            Some(alias) => Nns::NamespaceAlias {
                                prefix: cur,
                                alias,
                                ns: self.ast.resolve_alias(alias),
                            },
                            None => Nns::Global,
                        },
                        4 => Nns::Identifier {
                            prefix: cur,
                            name: self.intern(
                                img.pnames.get(w[p + 1] as usize).map_or("", String::as_str),
                            ),
                        },
                        _ => Nns::Type {
                            prefix: cur,
                            ty: self.image_ty(
                                img,
                                tys,
                                ids,
                                QRef { ty: w[p + 1], quals: w[p + 2] as u8 },
                            ),
                            template_kw: w[p + 3] != 0,
                        },
                    };
                    p += [1, 2, 4, 2, 2][(w[p] as usize).min(4)];
                    cur = Some(self.ast.types.nns(n));
                }
                RawId::raw_opt(cur)
            }
            F::AL => {
                let v: Vec<u32> = w[1..]
                    .chunks(2)
                    .map(|a| {
                        let loc = main.get(a[1]);
                        let node = word_attr(a[0], loc)
                            .unwrap_or_else(|| unreachable!("validated attribute"));
                        self.ast.attrs.push(node).raw()
                    })
                    .collect();
                self.ast.lists.push_raw(&v).raw()
            }
        }
    }

    /// The two slot values from the token values; also how many tokens
    /// the slots used (the rest are cold annotations).
    #[allow(clippy::too_many_lines)] // one arm per field kind, kept together with `Fv`
    fn image_slots(&mut self, shape: [Slot; 2], vals: &[u32], _n: Node) -> (u32, u32, usize) {
        let mut p = 0usize;
        let mut out = [0u32; 2];
        for (i, slot) in shape.into_iter().enumerate() {
            let take = |p: &mut usize, n: usize| -> &[u32] {
                let s = vals.get(*p..*p + n).unwrap_or(&[]);
                *p += n;
                s
            };
            let e = |x: u32| ExprId::from_raw(x);
            let s = |x: u32| StmtId::from_raw(x);
            let d = |x: u32| DeclId::from_raw(x);
            let q = |x: u32| QualType::from_raw(x).unwrap_or(QualType::INT);
            out[i] = match slot {
                Slot::Zero => 0,
                Slot::F(_) => take(&mut p, 1).first().copied().unwrap_or(0),
                Slot::Side(sd) => {
                    let n = side_fields(sd).len();
                    let v = take(&mut p, n).to_vec();
                    if v.len() == n {
                        let ex = &mut self.ast.exprs;
                        let st = &mut self.ast.stmts;
                        let req_e =
                            |x: u32| e(x).unwrap_or_else(|| unreachable!("validated operand"));
                        let req_s =
                            |x: u32| s(x).unwrap_or_else(|| unreachable!("validated operand"));
                        match sd {
                            Side::Arms => ex.push_cond_arms(req_e(v[0]), req_e(v[1])),
                            Side::CompoundAssign => ex.push_compound_assign(CompoundAssignExtra {
                                rhs: req_e(v[0]),
                                lhs_ty: q(v[1]),
                                result_ty: q(v[2]),
                            }),
                            Side::InitList => ex.push_init_list(InitListExtra {
                                array_filler: e(v[0]),
                                union_field: d(v[2]),
                                rbrace: Loc::from_raw(v[1]),
                            }),
                            Side::DeclRef => {
                                let (qualifier, found_decl) = (NnsId::from_raw(v[0]), d(v[1]));
                                let template_args = kiln_ast::ArgListId::from_raw(v[2]);
                                if qualifier.is_none()
                                    && found_decl.is_none()
                                    && template_args.is_none()
                                {
                                    0
                                } else {
                                    ex.push_declref_extra(DeclRefExtra {
                                        qualifier,
                                        template_args,
                                        found_decl,
                                    })
                                }
                            }
                            Side::If => st.push_if(IfData {
                                init: s(v[0]),
                                cond_var: d(v[1]),
                                cond: e(v[2]),
                                then: s(v[3]),
                                els: s(v[4]),
                            }),
                            Side::Switch => st.push_switch(SwitchData {
                                init: s(v[0]),
                                cond_var: d(v[1]),
                                cond: req_e(v[2]),
                                body: req_s(v[3]),
                                cases: ListId::from_raw(v[4]).unwrap_or(ListId::EMPTY),
                            }),
                            Side::Case => st.push_case(CaseData {
                                lhs: req_e(v[0]),
                                rhs: e(v[1]),
                                sub: req_s(v[2]),
                            }),
                            Side::Loop => st.push_loop(LoopData {
                                init: s(v[0]),
                                cond_var: d(v[1]),
                                cond: e(v[2]),
                                inc: e(v[3]),
                                body: s(v[4]),
                            }),
                            Side::RangeFor => st.push_range_for(RangeForData {
                                init: s(v[0]),
                                range: d(v[1]).unwrap_or_else(|| unreachable!("validated operand")),
                                begin: d(v[2]),
                                end: d(v[3]),
                                cond: e(v[4]),
                                inc: e(v[5]),
                                loop_var: d(v[6])
                                    .unwrap_or_else(|| unreachable!("validated operand")),
                                body: req_s(v[7]),
                                colon: Loc::from_raw(v[8]),
                            }),
                            Side::New => ex.push_new_extra(NewExtra {
                                allocated_ty: q(v[0]),
                                array_size: e(v[1]),
                                init: e(v[2]),
                                placement: (v[3] != 0)
                                    .then(|| ListId::from_raw(v[4]).unwrap_or(ListId::EMPTY)),
                                operator_new: d(v[5]),
                                operator_delete: d(v[6]),
                                type_loc: Loc::from_raw(v[7]),
                            }),
                        }
                    } else {
                        0
                    }
                }
            };
        }
        (out[0], out[1], p)
    }

    /// Creates local label `name` of function `f` (as Sema's
    /// `label_decl` does).
    pub(super) fn image_label(&mut self, f: DeclId, name: DeclName, loc: Loc) -> DeclId {
        self.ast.decls.push(
            NewDecl {
                kind: DeclKind::Label,
                flags: DeclFlags::empty(),
                misc: DeclMisc::default(),
                name,
                ctx: Some(f),
                loc,
                ty: None,
            },
            AstPayload::None(0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::sample;
    use super::super::{decode, encode};
    use super::{ExprRec, NONE};

    /// The kind byte of `GenericSelectionExpr`.
    fn generic_kind() -> u8 {
        let k = kiln_ast::expr::ExprKind::GenericSelectionExpr;
        super::EXPR_KINDS.iter().position(|&x| x == k).unwrap() as u8
    }

    /// The sample image plus `_Generic(e0, int: e0, default: e2)` with
    /// `words` (the operand list) and selected expression `sel`; `bits`
    /// 0x10 is the type-name control form.
    fn with_generic(bits: u8, mut words: Vec<u32>, sel: u32) -> super::super::Image {
        let mut img = sample();
        let ty = img.bodies.exprs[0].ty;
        words.push(sel);
        img.bodies.exprs.push(ExprRec {
            kind: generic_kind(),
            bits,
            aux: 0,
            cold: 0,
            deps: 0,
            ty,
            loc: 111,
            words,
        });
        img
    }

    /// WP-1132: a `_Generic` record round-trips; its operand list and its
    /// selection are validated.
    #[test]
    fn generic_selection_records() {
        let int = sample().bodies.exprs[0].ty;
        let assocs = |d: [u32; 2]| vec![int.ty, 0, 112, 0, d[0], d[1], 113, 2];
        let expr_form = |ctrl: Vec<u32>, n: u32, d: [u32; 2], sel: u32| {
            let mut w = vec![n];
            w.extend(ctrl);
            w.extend(assocs(d));
            with_generic(0, w, sel)
        };
        let good = expr_form(vec![0, NONE], 2, [NONE, 0], 2);
        assert_eq!(decode(&encode(&good)), Ok(good));
        // Result-dependent: no selection.
        let dep = expr_form(vec![0, NONE], 2, [NONE, 0], NONE);
        assert_eq!(decode(&encode(&dep)), Ok(dep));
        // The type-name form.
        let mut w = vec![2, int.ty, 1, 110];
        w.extend(assocs([NONE, 0]));
        let ty_form = with_generic(0x10, w, 0);
        assert_eq!(decode(&encode(&ty_form)), Ok(ty_form));
        for bad in [
            // A selection that is not an association's expression.
            expr_form(vec![0, NONE], 2, [NONE, 0], 1),
            // Two `default`s.
            {
                let mut img = expr_form(vec![0, NONE], 2, [NONE, 0], 2);
                img.bodies.exprs[3].words[3] = NONE;
                img
            },
            // A `default` with qualifiers.
            expr_form(vec![0, NONE], 2, [NONE, 1], 2),
            // An association type the image does not hold.
            expr_form(vec![0, NONE], 2, [9999, 0], 2),
            // A control expression that does not precede the selection.
            expr_form(vec![3, NONE], 2, [NONE, 0], 2),
            // A count past the words.
            expr_form(vec![0, NONE], 3, [NONE, 0], 2),
        ] {
            assert!(decode(&encode(&bad)).is_err(), "{:?}", bad.bodies.exprs[3]);
        }
        // Every location reaches the importer's bound check.
        let mut offs = Vec::new();
        super::body_locs(&expr_form(vec![0, 114], 2, [NONE, 0], 2), &mut offs);
        assert!([111, 112, 113, 114].iter().all(|o| offs.contains(o)), "{offs:?}");
    }
}
