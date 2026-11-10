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
    ExprKind, ExprNode, InitListExtra, NonOdrUse, Radix, StorageDuration, StringKind, TraitKind,
    TypeTrait, UnaryOp,
};
use kiln_ast::names::OverloadedOperator;
use kiln_ast::stmt::{CaseData, IfData, LoopData, RangeForData, StmtKind, StmtNode, SwitchData};
use kiln_ast::strings::StringEncoding;
use kiln_ast::types::Nns;
use kiln_ast::{AttrId, ConstValueId, ExprId, ListId, NnsId, RawId, StmtId, StringLitId};
use kiln_source::fx::FxHashSet;

use super::{
    Access, ConstexprKind, DEP, DeclFlags, DeclId, DeclKind, DeclName, DeclRec, FileId, FnBody,
    FnSpec, FxHashMap, Image, LEX_TU, Loc, NONE, Payload, QRef, QualType, Rd, Res, Sema, W, Wr,
    Writer, bad, var_spec,
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
    Int { ty: QRef, signed: bool, width: u32, words: Vec<u64> },
    Bool { ty: QRef, value: bool },
    Float { ty: QRef, kind: u8, bits: u128 },
    Null { ty: QRef, kind: u8 },
}

impl ConstRec {
    fn ty(&self) -> QRef {
        match self {
            ConstRec::Int { ty, .. }
            | ConstRec::Bool { ty, .. }
            | ConstRec::Float { ty, .. }
            | ConstRec::Null { ty, .. } => *ty,
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
    pub(super) cold: u8,
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
/// `expr_begin`, `member_op_loc`, `default_arg_rewritten`, in this order
/// after the slots.
const COLD_END: u8 = 1;
const COLD_WRITTEN: u8 = 2;
const COLD_RHS: u8 = 4;
const COLD_BEGIN: u8 = 8;
const COLD_OP: u8 = 16;
/// The lifetime-extended copy of a default argument (WP-1149) or default
/// member initializer (WP-1177, spec/sema.md §7.6a): an expression field,
/// so the copy is written before its use.
const COLD_REWRITE: u8 = 32;
const COLD_ALL: u8 = 63;

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
    /// A member name of a dependent member access (S4): an identifier,
    /// as an index into the image's undeclared names (`pnames`).
    Name,
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
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    /// The slot is 0.
    Zero,
    F(F),
    Side(Side),
}

fn side_fields(s: Side) -> &'static [F] {
    use F::{D, E, Loc, Nns, OD, OE, OS, Qt, S, SL};
    match s {
        Side::Arms => &[E, E],
        Side::CompoundAssign => &[E, Qt, Qt],
        Side::InitList => &[OE, Loc],
        Side::DeclRef => &[Nns, OD(Dc::Found)],
        Side::If => &[OS, OD(Dc::Var), OE, OS, OS],
        Side::Switch => &[OS, OD(Dc::Var), E, S, SL],
        Side::Case => &[E, OE, S],
        Side::Loop => &[OS, OD(Dc::Var), OE, OE, OS],
        Side::RangeFor => &[OS, D(Dc::Var), OD(Dc::Var), OD(Dc::Var), OE, OE, D(Dc::Var), S, Loc],
    }
}

/// The expression kinds the image carries; the record's kind byte indexes
/// this table (append only).
pub(super) const EXPR_KINDS: [ExprKind; 49] = {
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
    ]
};

/// The statement kinds the image carries (append only).
pub(super) const STMT_KINDS: [StmtKind; 18] = {
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
        X::CallExpr | X::CXXOperatorCallExpr | X::CXXMemberCallExpr => [f(F::E), f(F::EL)],
        X::CXXUnresolvedConstructExpr => [f(F::EL), Z],
        X::CXXDependentScopeMemberExpr => [f(F::E), f(F::Name)],
        X::ImplicitCastExpr
        | X::CStyleCastExpr
        | X::CXXFunctionalCastExpr
        | X::CXXStaticCastExpr
        | X::CXXConstCastExpr
        | X::CXXReinterpretCastExpr => [f(F::E), f(F::Path)],
        X::InitListExpr => [f(F::EL), Sd(Side::InitList)],
        X::UnaryExprOrTypeTraitExpr if flag4 => [f(F::Qt), Z],
        X::UnaryOperator | X::ExprWithCleanups | X::UnaryExprOrTypeTraitExpr => [f(F::E), Z],
        X::TypeTraitExpr => [f(F::QL), f(F::Bool)],
        X::CXXNoexceptExpr => [f(F::E), f(F::Bool)],
        X::ConstantExpr => [f(F::E), f(F::OCv)],
        X::MaterializeTemporaryExpr => [f(F::E), f(F::OD(Dc::Let))],
        X::StmtExpr => [f(F::S), Z],
        X::CXXDefaultArgExpr => [f(F::D(Dc::Param)), f(F::D(Dc::Ctx))],
        X::AddrLabelExpr => [f(F::D(Dc::Label)), Z],
        X::MemberExpr => [f(F::E), f(F::D(Dc::Member))],
        X::CXXConstructExpr | X::CXXTemporaryObjectExpr => [f(F::D(Dc::Ctor)), f(F::EL)],
        X::CXXDefaultInitExpr => [f(F::D(Dc::Field)), f(F::D(Dc::Ctx))],
        X::CXXBindTemporaryExpr => [f(F::E), f(F::D(Dc::Dtor))],
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
        X::UnaryOperator => UnaryOp::from_u8(aux).is_some(),
        X::BinaryOperator | X::CompoundAssignOperator => BinaryOp::from_u8(aux).is_some(),
        X::ChooseExpr => aux <= 2,
        X::CXXOperatorCallExpr => OverloadedOperator::from_u8(aux).is_some(),
        X::ImplicitCastExpr
        | X::CStyleCastExpr
        | X::CXXFunctionalCastExpr
        | X::CXXStaticCastExpr
        | X::CXXConstCastExpr
        | X::CXXReinterpretCastExpr => CastKind::from_u8(aux).is_some(),
        X::UnaryExprOrTypeTraitExpr => TraitKind::from_u8(aux).is_some(),
        X::TypeTraitExpr => usize::from(aux) < TypeTrait::all().len(),
        X::ConstantExpr => aux <= 3,
        X::MaterializeTemporaryExpr => StorageDuration::from_u8(aux & 3).is_some() && aux >> 3 == 0,
        X::CXXThisExpr
        | X::ExprWithCleanups
        | X::CXXUnresolvedConstructExpr
        | X::CXXDependentScopeMemberExpr => aux <= 1,
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
fn layout(shape: [Slot; 2], cold: u8) -> Vec<F> {
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
            F::Path => match *words.get(p)? as usize {
                0 => 1,
                n => 1 + 2 * (n - 1),
            },
            F::Nns => {
                let n = *words.get(p)? as usize;
                let mut q = p + 1;
                for _ in 0..n {
                    q += match *words.get(q)? {
                        0 => 1,
                        1 => 2,
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
        for x in [e.kind, e.bits, e.aux, e.cold, e.deps] {
            w.u8(x);
        }
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
    b.exprs = decode_arena(r, 5 + 5 + 4 + 4, |r| {
        let (kind, bits, aux, cold, deps) = (r.u8()?, r.u8()?, r.u8()?, r.u8()?, r.u8()?);
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

/// Whether declaration word `w` names something of class `dc`.
pub(super) fn dc_ok(img: &Image, nparams: usize, w: u32, dc: Dc) -> bool {
    if w == LEX_TU {
        return dc == Dc::Ctx;
    }
    if w == NONE {
        return false;
    }
    if w & PARAM != 0 {
        return matches!(dc, Dc::Value | Dc::Param) && ((w & !PARAM) as usize) < nparams;
    }
    let Some(d) = img.decls.get(w as usize) else { return false };
    if dc == Dc::Found && matches!(d.payload, Payload::Template { kind: 0, .. }) {
        return true;
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
        Dc::Value | Dc::Found => {
            matches!(
                d.payload,
                Payload::Function { .. }
                    | Payload::Var { .. }
                    | Payload::Enumerator { .. }
                    | Payload::Implicit { .. }
                    | Payload::Nttp { .. }
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
        Dc::Local => matches!(d.payload, Payload::Var { .. }) && is_local(img, d),
        Dc::Label => matches!(d.payload, Payload::Label),
        Dc::Let => matches!(d.payload, Payload::Let { .. }),
        Dc::Param => false,
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
    let nparams = param_count(img);
    let (ne, ns) = (b.exprs.len(), b.stmts.len());
    for c in &b.consts {
        if !qref_ok(c.ty()) {
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
            F::D(dc) => dc_ok(img, nparams, w[0], dc),
            F::OD(dc) => w[0] == NONE || dc_ok(img, nparams, w[0], dc),
            F::Cv => (w[0] as usize) < b.consts.len(),
            F::OCv => opt(w[0], b.consts.len()),
            F::Str => (w[0] as usize) < b.strings.len(),
            F::Qt => w[1] <= 7 && qref_ok(QRef { ty: w[0], quals: w[1] as u8 }),
            F::EL => w[1..].iter().all(|&x| (x as usize) < e_lim),
            F::SL => w[1..].iter().all(|&x| (x as usize) < s_lim),
            F::DL(dc) => w[1..].iter().all(|&x| dc_ok(img, nparams, x, dc)),
            F::QL => {
                w[1..].chunks(2).all(|q| q[1] <= 7 && qref_ok(QRef { ty: q[0], quals: q[1] as u8 }))
            }
            F::AL => w[1..].chunks(2).all(|a| word_attr(a[0], Loc::INVALID).is_some()),
            F::Name => (w[0] as usize) < img.pnames.len(),
            F::Nns => {
                let mut p = 1;
                while p < w.len() {
                    let ok = match w[p] {
                        0 => true,
                        1 => dc_ok(img, nparams, w[p + 1], Dc::Namespace),
                        _ => {
                            w[p + 3] <= 1
                                && w[p + 2] <= 7
                                && qref_ok(QRef { ty: w[p + 1], quals: w[p + 2] as u8 })
                        }
                    };
                    if !ok {
                        return false;
                    }
                    p += [1, 2, 4][w[p] as usize];
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
            || (e.cold & COLD_REWRITE != 0
                && !matches!(k, ExprKind::CXXDefaultArgExpr | ExprKind::CXXDefaultInitExpr))
            || e.deps & !ExprDeps::TYPE_VALUE_INST.bits() != 0
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
    }
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
            Payload::Var { init, eval, .. } => {
                if *init != NONE && *init as usize >= ne {
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

/// Every main-file offset the bodies hold (for the importer's bound
/// check).
pub(super) fn body_locs(img: &Image, out: &mut Vec<u32>) {
    let b = &img.bodies;
    for e in &b.exprs {
        out.push(e.loc);
        let Some(&k) = EXPR_KINDS.get(usize::from(e.kind)) else { continue };
        for (f, w) in tokens(&layout(expr_shape(k, e.bits), e.cold), &e.words).unwrap_or_default() {
            if f == F::Loc {
                out.push(w[0]);
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
    /// Global parameter index, default argument.
    Default(u32, ExprId),
    /// Field record, bit-field width.
    FieldWidth(u32, ExprId),
    /// Field record, default member initializer.
    FieldInit(u32, ExprId),
    /// Constructor record, ctor-initializer position, initializer.
    CtorInit(u32, u32, ExprId),
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
    QL(ListId),
    Nns(Option<NnsId>),
    AL(ListId),
    Path(Option<ListId>),
    Name(DeclName),
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

const OUTSIDE: &str = "a body referring to a declaration outside the image";

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
                if union_field.is_some() {
                    return Err("a union initializer");
                }
                vec![Fv::E(array_filler), Fv::Loc(rbrace)]
            }
            (Side::DeclRef, Some(e), _) => match (raw, ex.decl_ref_extra(e)) {
                (0, _) | (_, None) => vec![Fv::Nns(None), Fv::D(None, Dc::Value)],
                (_, Some(x)) => {
                    if x.template_args.is_some() {
                        return Err("a template argument list");
                    }
                    vec![Fv::Nns(x.qualifier), Fv::D(x.found_decl, Dc::Found)]
                }
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

    /// Header and fields of a node (no translation yet).
    fn fields(&self, node: Node) -> W<(u8, u8, u16, u8, Vec<Fv>)> {
        let ast = &self.s.ast;
        match node {
            Node::E(e) => {
                let n = *ast.exprs.get(e);
                let k = n.kind();
                let ki = EXPR_KINDS.iter().position(|&x| x == k).ok_or(expr_reason(k))?;
                if n.deps().intersects(ExprDeps::UNEXPANDED_PACK.with(ExprDeps::ERROR)) {
                    return Err("a pack expansion or an invalid expression");
                }
                if !expr_aux_ok(k, n.aux()) {
                    return Err("an expression the image does not carry");
                }
                let mut v = self.slots_fv(node, expr_shape(k, n.raw_bits()), [n.a(), n.b()])?;
                // A braced argument placeholder (`T(a, {b})`) lives in a
                // Sema side table the image does not carry.
                if k == ExprKind::CXXUnresolvedConstructExpr
                    && ListId::from_raw(n.a()).is_some_and(|l| {
                        ast.lists.iter::<ExprId>(l).any(|x| self.s.is_braced_arg(x))
                    })
                {
                    return Err("a braced argument of a dependent construction");
                }
                let c = &ast.exprs.cold;
                let t = &ast.templates;
                if c.member_extra.contains(e)
                    || c.designators.contains(e)
                    || c.syntactic_list.contains(e)
                    || c.array_trait.contains(e)
                    || c.source_loc.contains(e)
                    || c.union_field.contains(e)
                    || c.offsetof_pattern.contains(e)
                    || c.splice_args.contains(e)
                    || t.expr_targs.contains(e)
                    || t.expr_nns.contains(e)
                    || t.size_of_pack_origin.contains(e)
                    || t.pseudo_dtor_name.contains(e)
                    || t.pseudo_dtor_first.contains(e)
                    || t.template_kw.contains(e)
                {
                    return Err("an expression annotation the image does not carry");
                }
                let mut cold = 0u8;
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
                Some(c) => self.cnst(c)?.ok_or("a constant the image does not carry")?,
            }),
            Fv::Str(s) => out.push(self.string(s)),
            Fv::Name(n) => {
                // Not a declared name of the unit (a member found at
                // instantiation): not gated, like a local's.
                let s = self.ident(n).ok_or("a dependent member not named by an identifier")?;
                out.push(Self::name_ix(&mut self.pnames, &mut self.img.pnames, &s));
            }
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
                Nns::Namespace { prefix, .. } | Nns::Type { prefix, .. } => prefix,
                _ => return Err("a qualifier naming a namespace alias, dependent name or splice"),
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
                _ => return Err("a malformed qualifier"),
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

    /// The declaration word of `d` as class `dc`; a local declaration
    /// gets its record on first use.
    fn dref(&mut self, d: DeclId, dc: Dc) -> W<u32> {
        let w = if let Some(&r) = self.recs.get(&d) {
            r
        } else if let Some(&p) = self.bs.params.get(&d) {
            PARAM | p
        } else if d == self.s.ast.tu {
            LEX_TU
        } else if self.local(d) {
            self.local_rec(d)?
        } else if let Some(r) = self.spec_rec(d)? {
            // S4b: a function template specialization.
            r
        } else if let Some(r) = self.member_rec(d)? {
            // S4b: a member of a class template specialization.
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

    /// A record for local variable or label `d`.
    fn local_rec(&mut self, d: DeclId) -> W<u32> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        let kind = n.kind();
        if !matches!(kind, DeclKind::Var | DeclKind::Label | DeclKind::LifetimeExtendedTemporary) {
            return Err("a local declaration the image does not carry");
        }
        let ctx = n.ctx().and_then(|c| self.recs.get(&c).copied()).ok_or(OUTSIDE)?;
        if !matches!(self.img.decls[ctx as usize].payload, Payload::Function { .. }) {
            return Err("a local declaration the image does not carry");
        }
        let allowed = LOCAL_FLAGS | DEP | DeclFlags::HAS_LEXICAL_CTX.bits();
        if n.flags().bits() & !allowed != 0 {
            return Err("an invalid or attributed local declaration");
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
        if ast.decls.redecl(d).and_then(|r| r.prev).is_some() {
            return Err("a local redeclaration");
        }
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
        // The type first: naming a specialization may add its record.
        if let Some(t) = n.ty() {
            self.qref(t)?;
        }
        let i = u32::try_from(self.img.decls.len()).map_err(|_| "a body too large")?;
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
            _ => self.var_payload(d, i)?,
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
            prev: NONE,
            payload,
        });
        self.recs.insert(d, i);
        Ok(i)
    }

    /// Walks every pending root and patches the records that point at it.
    pub(super) fn walk_roots(&mut self) -> W<()> {
        while let Some(r) = self.bs.roots.pop() {
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
            }
        }
        Ok(())
    }

    /// A variable's record (top-level or local) at index `rec`.
    pub(super) fn var_payload(&mut self, d: DeclId, rec: u32) -> W<Payload> {
        let ast = &self.s.ast;
        let vd = *ast.decls.var(d).ok_or("a malformed variable")?;
        if !vd.templ.is_none() {
            return Err("a template");
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
            self.bs.roots.push(Root::Init(rec, e));
        }
        Ok(Payload::Var { ty, spec: bits, init: NONE, eval })
    }
}

/// The flags a local record may carry.
pub(super) const LOCAL_FLAGS: u8 =
    DeclFlags::IMPLICIT.bits() | DeclFlags::USED.bits() | DeclFlags::REFERENCED.bits();

fn expr_reason(k: ExprKind) -> &'static str {
    use ExprKind as X;
    match k {
        X::CXXNewExpr | X::CXXDeleteExpr => "a new or delete expression",
        X::CXXTypeidExpr | X::CXXDynamicCastExpr => "a typeid or dynamic_cast expression",
        X::CXXStdInitializerListExpr => "a std::initializer_list",
        X::CXXInheritedCtorInitExpr => "an inherited constructor",
        X::CXXPseudoDestructorExpr => "a pseudo-destructor call",
        X::LambdaExpr => "a lambda expression",
        X::RequiresExpr | X::ConceptSpecializationExpr => "a requires-expression or concept",
        X::CoawaitExpr | X::DependentCoawaitExpr | X::CoyieldExpr => "a coroutine",
        X::CXXReflectExpr | X::CXXSpliceExpr => "a reflection or splice",
        X::UnresolvedLookupExpr
        | X::UnresolvedMemberExpr
        | X::DependentScopeDeclRefExpr
        | X::CXXDependentScopeMemberExpr
        | X::SubstNonTypeTemplateParmExpr
        | X::SubstNonTypeTemplateParmPackExpr
        | X::FunctionParmPackExpr
        | X::PackExpansionExpr
        | X::CXXFoldExpr
        | X::PackIndexingExpr
        | X::SizeOfPackExpr
        | X::CXXUnresolvedConstructExpr
        | X::CXXExpansionSelectExpr => "a template expression",
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
    #[allow(clippy::too_many_lines)] // one arm per side record, kept with the schema
    pub(super) fn image_bodies(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        params: &[DeclId],
        main: FileId,
    ) -> Built {
        let b = &img.bodies;
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
            };
            // A value the pools reject (checked, never trusted) becomes 0.
            let id = match s.map(|s| self.ast.consts.push_scalar(s)) {
                Some(Ok(id)) => id,
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
        let built = Built {
            ebase: self.ast.exprs.len() as u32 + 1,
            sbase: self.ast.stmts.len() as u32 + 1,
            consts,
        };
        // Values of every token, as the AST slot would hold them.
        let mut vals: Vec<u32> = Vec::new();
        for (i, e) in b.exprs.iter().enumerate() {
            let k = EXPR_KINDS[usize::from(e.kind)];
            let shape = expr_shape(k, e.bits);
            let fields = layout(shape, e.cold);
            let toks = tokens(&fields, &e.words).unwrap_or_default();
            vals.clear();
            for &(f, w) in &toks {
                let v = self.image_val(img, tys, ids, params, &built, &strs, main, f, w);
                vals.push(v);
            }
            let (a, bb, used) = self.image_slots(shape, &vals, Node::E(built.expr(0)));
            let ty = self.image_ty(img, tys, ids, e.ty);
            let loc = if e.loc == NONE { Loc::INVALID } else { self.diags.sm().loc(main, e.loc) };
            let mut deps = ExprDeps::NONE;
            for d in [ExprDeps::TYPE, ExprDeps::VALUE, ExprDeps::INSTANTIATION] {
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
            if e.cold & COLD_REWRITE != 0
                && matches!(k, ExprKind::CXXDefaultArgExpr | ExprKind::CXXDefaultInitExpr)
                && let Some(r) = ExprId::from_raw(next())
            {
                let _ = self.ast.exprs.set_default_arg_rewritten(id, r);
            }
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
            let loc = if s.loc == NONE { Loc::INVALID } else { self.diags.sm().loc(main, s.loc) };
            let id = self.ast.stmts.push(StmtNode::new(k, s.aux, loc, a, bb));
            debug_assert_eq!(id, built.stmt(i as u32));
        }
        built
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
        main: FileId,
        f: F,
        w: &[u32],
    ) -> u32 {
        let decl = |s: &Self, w: u32| -> u32 {
            if w == NONE {
                return 0;
            }
            let d = if w == LEX_TU {
                Some(s.ast.tu)
            } else if w & PARAM != 0 {
                params.get((w & !PARAM) as usize).copied()
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
                let n = img.pnames.get(w[0] as usize).map_or("", String::as_str);
                DeclName::ident(self.intern(n)).to_raw()
            }
            F::Loc => {
                if w[0] == NONE {
                    Loc::INVALID.to_raw()
                } else {
                    self.diags.sm().loc(main, w[0]).to_raw()
                }
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
                    p += [1, 2, 4][(w[p] as usize).min(2)];
                    cur = Some(self.ast.types.nns(n));
                }
                RawId::raw_opt(cur)
            }
            F::AL => {
                let v: Vec<u32> = w[1..]
                    .chunks(2)
                    .map(|a| {
                        let loc = if a[1] == NONE {
                            Loc::INVALID
                        } else {
                            self.diags.sm().loc(main, a[1])
                        };
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
                                union_field: None,
                                rbrace: Loc::from_raw(v[1]),
                            }),
                            Side::DeclRef => {
                                let (qualifier, found_decl) = (NnsId::from_raw(v[0]), d(v[1]));
                                if qualifier.is_none() && found_decl.is_none() {
                                    0
                                } else {
                                    ex.push_declref_extra(DeclRefExtra {
                                        qualifier,
                                        template_args: None,
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
