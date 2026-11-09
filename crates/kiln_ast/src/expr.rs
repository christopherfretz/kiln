//! Expressions: the 20-byte [`ExprNode`] header and side records
//! (spec/ast.md §5).
//!
//! Nodes are created by the `AstBuilder`, which computes dependence and
//! value category; this module owns storage, the operator/cast enums and
//! typed accessors.
#![warn(missing_docs)]

use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::ids::{
    ArgListId, ConstValueId, DeclId, ExprId, LambdaId, ListId, NnsId, QualType, RawId, StmtId,
    StringLitId,
};
use crate::lists::IdLists;
use crate::mem::{HeapSize, MemReport, VecDims};
use crate::slot::{AlreadySet, ColdMap};

/// `ConstantExpr` aux bit: a nested immediate invocation (no wrapper in clang).
const CONSTANT_ELIDED: u8 = 1;
/// `ConstantExpr` aux bit: the cached value is not clang's (not dumped).
const CONSTANT_VALUE_HIDDEN: u8 = 2;

/// Expression kinds (spec/ast.md §5.3; clang's class names without the
/// `Expr`/`Operator` suffix where the dump adds it). The numbering is
/// stable.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
#[allow(missing_docs, reason = "named after clang's Stmt classes; see spec/ast.md §5.3")]
pub enum ExprKind {
    // literals
    IntegerLiteral,
    FloatingLiteral,
    CharacterLiteral,
    StringLiteral,
    CXXBoolLiteralExpr,
    CXXNullPtrLiteralExpr,
    UserDefinedLiteral,
    ImaginaryLiteral,
    EmbedExpr,
    GNUNullExpr,
    // names
    DeclRefExpr,
    MemberExpr,
    CXXThisExpr,
    PredefinedExpr,
    SourceLocExpr,
    UnresolvedLookupExpr,
    UnresolvedMemberExpr,
    DependentScopeDeclRefExpr,
    CXXDependentScopeMemberExpr,
    SubstNonTypeTemplateParmExpr,
    SubstNonTypeTemplateParmPackExpr,
    FunctionParmPackExpr,
    // operators
    UnaryOperator,
    BinaryOperator,
    CompoundAssignOperator,
    ConditionalOperator,
    BinaryConditionalOperator,
    ArraySubscriptExpr,
    ParenExpr,
    CallExpr,
    CXXMemberCallExpr,
    CXXOperatorCallExpr,
    CXXRewrittenBinaryOperator,
    ParenListExpr,
    // casts
    ImplicitCastExpr,
    CStyleCastExpr,
    CXXFunctionalCastExpr,
    CXXStaticCastExpr,
    CXXConstCastExpr,
    CXXReinterpretCastExpr,
    CXXDynamicCastExpr,
    BuiltinBitCastExpr,
    // initialization
    InitListExpr,
    ImplicitValueInitExpr,
    /// An initializer-clause with designators in a dependent
    /// `InitListExpr` (template patterns only, §5.6).
    DesignatedInitExpr,
    CXXConstructExpr,
    CXXTemporaryObjectExpr,
    CXXInheritedCtorInitExpr,
    CXXDefaultArgExpr,
    CXXDefaultInitExpr,
    CXXScalarValueInitExpr,
    CXXParenListInitExpr,
    ArrayInitLoopExpr,
    ArrayInitIndexExpr,
    CXXStdInitializerListExpr,
    CompoundLiteralExpr,
    /// An element of a [`ExprKind::DesignatedInitUpdateExpr`] updater
    /// that keeps the base's initialization.
    NoInitExpr,
    /// A subobject initialized by an expression, then partly
    /// re-initialized by a designator into it ([dcl.init.aggr]/4 with
    /// C99 designators; clang's `DesignatedInitUpdateExpr`): the base
    /// and an updater `InitListExpr` whose `NoInitExpr` elements keep
    /// the base's values.
    DesignatedInitUpdateExpr,
    // temporaries
    MaterializeTemporaryExpr,
    CXXBindTemporaryExpr,
    ExprWithCleanups,
    OpaqueValueExpr,
    // memory
    CXXNewExpr,
    CXXDeleteExpr,
    CXXPseudoDestructorExpr,
    // exceptions / RTTI
    CXXThrowExpr,
    CXXNoexceptExpr,
    CXXTypeidExpr,
    // traits
    UnaryExprOrTypeTraitExpr,
    TypeTraitExpr,
    ArrayTypeTraitExpr,
    ExpressionTraitExpr,
    OffsetOfExpr,
    SizeOfPackExpr,
    RequiresExpr,
    ConceptSpecializationExpr,
    // lambdas, packs
    LambdaExpr,
    PackExpansionExpr,
    CXXFoldExpr,
    PackIndexingExpr,
    CXXUnresolvedConstructExpr,
    // coroutines
    CoawaitExpr,
    DependentCoawaitExpr,
    CoyieldExpr,
    // constant
    ConstantExpr,
    // GNU / builtins
    StmtExpr,
    AddrLabelExpr,
    ChooseExpr,
    VAArgExpr,
    AtomicExpr,
    ShuffleVectorExpr,
    ConvertVectorExpr,
    ExtVectorElementExpr,
    // reflection
    CXXReflectExpr,
    CXXSpliceExpr,
    // recovery
    RecoveryExpr,
    /// The element an expansion statement's for-range-declaration is
    /// initialized from ([stmt.expand], P1306R5; template patterns
    /// only): `aux` is the statement's form ([`crate::stmt::ExpansionForm`]),
    /// `a` the depth of its synthetic template level. Each expansion
    /// replaces it by its element (spec/sema.md "Expansion statements").
    CXXExpansionSelectExpr,
    // semantic rewrites
    /// clang's `PseudoObjectExpr`: a syntactic form (as written, for the
    /// dump and diagnostics) and the semantic expressions that compute it
    /// (`OpaqueValueExpr`s bound before use, then the result); the
    /// synthesized three-way comparison ([class.spaceship]/1).
    PseudoObjectExpr,
    /// C11 `_Generic(ctrl, T: e, ..., default: e)` in C++ (clang's
    /// `GenericSelectionExpr`, an extension): `a` is a raw list (see
    /// [`ExprArena::generic_control`] / [`ExprArena::generic_assocs`]),
    /// `b` the raw id of the selected association expression (0 while
    /// the selection is result-dependent). [`ExprFlags::ARG_IS_TYPE`]
    /// marks the type-operand form (`_Generic(type-name, ...)`).
    GenericSelectionExpr,
}

/// The controlling operand of a `GenericSelectionExpr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenericControl {
    /// An expression, as written (sema lvalue-converts a copy for the
    /// match; the written operand is kept, unevaluated).
    Expr(ExprId),
    /// A type-name (clang's C2y extension), matched as written.
    Type(QualType, Loc),
}

/// One association of a `GenericSelectionExpr`: `ty` is `None` for
/// `default`; `ty_loc` is the type's (or the `default` keyword's) location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenericAssoc {
    /// The association type; `None` for `default`.
    pub ty: Option<QualType>,
    /// The location of the type-name or of `default`.
    pub ty_loc: Loc,
    /// The association expression.
    pub expr: ExprId,
}

impl ExprKind {
    /// True for the cast kinds (`aux` is a [`CastKind`]).
    pub const fn is_cast(self) -> bool {
        matches!(
            self,
            ExprKind::ImplicitCastExpr
                | ExprKind::CStyleCastExpr
                | ExprKind::CXXFunctionalCastExpr
                | ExprKind::CXXStaticCastExpr
                | ExprKind::CXXConstCastExpr
                | ExprKind::CXXReinterpretCastExpr
                | ExprKind::CXXDynamicCastExpr
                | ExprKind::BuiltinBitCastExpr
        )
    }

    /// True for explicit casts.
    pub const fn is_explicit_cast(self) -> bool {
        self.is_cast() && !matches!(self, ExprKind::ImplicitCastExpr)
    }
}

/// Value category ([basic.lval]), 2 bits of `ExprNode.bits`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[repr(u8)]
pub enum ValueCategory {
    /// prvalue (not printed by the dump).
    #[default]
    PRValue = 0,
    /// lvalue.
    LValue = 1,
    /// xvalue.
    XValue = 2,
}

impl ValueCategory {
    /// lvalue or xvalue.
    pub const fn is_glvalue(self) -> bool {
        !matches!(self, ValueCategory::PRValue)
    }
}

/// Object kind (2 bits of `ExprNode.bits`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[repr(u8)]
pub enum ObjectKind {
    /// An ordinary object.
    #[default]
    Ordinary = 0,
    /// A bit-field (the `FieldDecl` is reachable from the `MemberExpr`).
    BitField = 1,
    /// A vector element.
    VectorComponent = 2,
}

/// The 4 per-kind flag bits of `ExprNode.bits` (bits 4..=7).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct ExprFlags(u8);

impl ExprFlags {
    /// No flags.
    pub const NONE: ExprFlags = ExprFlags(0);
    /// `ImplicitCastExpr`: an earlier step of an explicit cast.
    pub const PART_OF_EXPLICIT_CAST: ExprFlags = ExprFlags(1 << 4);
    /// `MemberExpr`: `->`.
    pub const ARROW: ExprFlags = ExprFlags(1 << 4);
    /// `UnaryOperator`: cannot overflow (dump `cannot overflow`).
    pub const CANNOT_OVERFLOW: ExprFlags = ExprFlags(1 << 4);
    /// `IntegerLiteral`: wider than 64 bits; `a` is a `ConstValueId`.
    pub const WIDE: ExprFlags = ExprFlags(1 << 4);
    /// `UnaryExprOrTypeTraitExpr`, `CXXTypeidExpr`: the operand is a type
    /// (`a` = raw `QualType`).
    pub const ARG_IS_TYPE: ExprFlags = ExprFlags(1 << 4);
    /// `DeclRefExpr`: refers to an enclosing variable or capture.
    pub const REFERS_TO_ENCLOSING: ExprFlags = ExprFlags(1 << 5);
    /// `CallExpr`: the callee was found by ADL.
    pub const ADL: ExprFlags = ExprFlags(1 << 5);
    /// `DeclRefExpr` / `MemberExpr` / `CXXConstructExpr` /
    /// `CXXTemporaryObjectExpr`: overload resolution chose among several
    /// candidates.
    pub const HAD_MULTIPLE_CANDIDATES: ExprFlags = ExprFlags(1 << 6);
    /// `CallExpr` of `__builtin_assume`: the assumption has possible side
    /// effects, so it is never evaluated and emits nothing (clang
    /// `BI__builtin_assume` under `HasSideEffects`; WP-805).
    pub const ASSUME_IGNORED: ExprFlags = ExprFlags(1 << 6);
    /// `SubstNonTypeTemplateParmExpr`: the parameter has (deduced)
    /// reference type (clang `isReferenceParameter`).
    pub const REFERENCE_PARAMETER: ExprFlags = ExprFlags(1 << 4);
    /// `CXXNewExpr` / `CXXDeleteExpr`: `::new` / `::delete` (dump `global`).
    pub const GLOBAL: ExprFlags = ExprFlags(1 << 4);
    /// `CXXNewExpr`: an array new; `CXXDeleteExpr`: deletes an array
    /// (`delete[]`, or `delete` of a pointer to array; dump `array`).
    pub const ARRAY_FORM: ExprFlags = ExprFlags(1 << 5);
    /// `CXXNewExpr`: the allocation function takes `std::align_val_t`.
    pub const PASS_ALIGN: ExprFlags = ExprFlags(1 << 6);
    /// `CXXDeleteExpr`: written `delete[]`.
    pub const ARRAY_AS_WRITTEN: ExprFlags = ExprFlags(1 << 6);
    /// `DeclRefExpr`: names an immediate function where that made the
    /// enclosing function immediate (P2564; dump `immediate-escalating`).
    pub const IMMEDIATE_ESCALATING: ExprFlags = ExprFlags(1 << 7);

    /// The raw bits (already shifted into bits 4..=7).
    pub const fn bits(self) -> u8 {
        self.0
    }
    /// The union.
    #[must_use]
    pub const fn with(self, f: ExprFlags) -> Self {
        ExprFlags(self.0 | f.0)
    }
}

/// Expression dependence (spec/ast.md §5.1; [temp.dep.expr],
/// [temp.dep.constexpr]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct ExprDeps(u8);

impl ExprDeps {
    /// No dependence.
    pub const NONE: ExprDeps = ExprDeps(0);
    /// Type-dependent.
    pub const TYPE: ExprDeps = ExprDeps(1);
    /// Value-dependent.
    pub const VALUE: ExprDeps = ExprDeps(2);
    /// Instantiation-dependent.
    pub const INSTANTIATION: ExprDeps = ExprDeps(4);
    /// Contains an unexpanded parameter pack.
    pub const UNEXPANDED_PACK: ExprDeps = ExprDeps(8);
    /// Contains an error (recovery expression or error type).
    pub const ERROR: ExprDeps = ExprDeps(16);
    /// Type-, value- and instantiation-dependent.
    pub const TYPE_VALUE_INST: ExprDeps = ExprDeps(7);
    /// Value- and instantiation-dependent.
    pub const VALUE_INST: ExprDeps = ExprDeps(6);

    /// The raw bits.
    pub const fn bits(self) -> u8 {
        self.0
    }
    /// True if every bit of `d` is set.
    pub const fn contains(self, d: ExprDeps) -> bool {
        self.0 & d.0 == d.0
    }
    /// True if any bit of `d` is set.
    pub const fn intersects(self, d: ExprDeps) -> bool {
        self.0 & d.0 != 0
    }
    /// The union.
    #[must_use]
    pub const fn with(self, d: ExprDeps) -> Self {
        ExprDeps(self.0 | d.0)
    }
    /// The bits not in `d`.
    #[must_use]
    pub const fn without(self, d: ExprDeps) -> Self {
        ExprDeps(self.0 & !d.0)
    }
    /// True if no bit is set.
    pub const fn is_none(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for ExprDeps {
    type Output = ExprDeps;
    fn bitor(self, rhs: ExprDeps) -> ExprDeps {
        ExprDeps(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for ExprDeps {
    fn bitor_assign(&mut self, rhs: ExprDeps) {
        self.0 |= rhs.0;
    }
}

macro_rules! u8_enum {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $v:ident = $s:literal,)* }) => {
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        #[repr(u8)]
        pub enum $name { $($(#[$vm])* $v,)* }
        impl $name {
            const ALL: &'static [$name] = &[$($name::$v,)*];
            /// The variant with discriminant `v`.
            pub fn from_u8(v: u8) -> Option<Self> {
                Self::ALL.get(usize::from(v)).copied()
            }
            /// The spelling used by the dump.
            pub const fn spelling(self) -> &'static str {
                match self { $($name::$v => $s,)* }
            }
        }
    };
}

u8_enum!(
    /// Unary operators (clang's `UnaryOperatorKind`).
    UnaryOp {
        /// `x++`.
        PostInc = "++",
        /// `x--`.
        PostDec = "--",
        /// `++x`.
        PreInc = "++",
        /// `--x`.
        PreDec = "--",
        /// `&x`.
        AddrOf = "&",
        /// `*x`.
        Deref = "*",
        /// `+x`.
        Plus = "+",
        /// `-x`.
        Minus = "-",
        /// `~x`.
        Not = "~",
        /// `!x`.
        LNot = "!",
        /// `__real x`.
        Real = "__real",
        /// `__imag x`.
        Imag = "__imag",
        /// `__extension__ x`.
        Extension = "__extension__",
        /// `co_await x` (non-overloaded).
        Coawait = "co_await",
    }
);

impl UnaryOp {
    /// `x++` / `x--`.
    pub const fn is_postfix(self) -> bool {
        matches!(self, UnaryOp::PostInc | UnaryOp::PostDec)
    }
    /// `++` / `--` in either position.
    pub const fn is_inc_dec(self) -> bool {
        matches!(self, UnaryOp::PostInc | UnaryOp::PostDec | UnaryOp::PreInc | UnaryOp::PreDec)
    }
}

u8_enum!(
    /// Binary operators (clang's `BinaryOperatorKind`).
    BinaryOp {
        /// `.*`.
        PtrMemD = ".*",
        /// `->*`.
        PtrMemI = "->*",
        /// `*`.
        Mul = "*",
        /// `/`.
        Div = "/",
        /// `%`.
        Rem = "%",
        /// `+`.
        Add = "+",
        /// `-`.
        Sub = "-",
        /// `<<`.
        Shl = "<<",
        /// `>>`.
        Shr = ">>",
        /// `<=>`.
        Cmp = "<=>",
        /// `<`.
        LT = "<",
        /// `>`.
        GT = ">",
        /// `<=`.
        LE = "<=",
        /// `>=`.
        GE = ">=",
        /// `==`.
        EQ = "==",
        /// `!=`.
        NE = "!=",
        /// `&`.
        And = "&",
        /// `^`.
        Xor = "^",
        /// `|`.
        Or = "|",
        /// `&&`.
        LAnd = "&&",
        /// `||`.
        LOr = "||",
        /// `=`.
        Assign = "=",
        /// `*=`.
        MulAssign = "*=",
        /// `/=`.
        DivAssign = "/=",
        /// `%=`.
        RemAssign = "%=",
        /// `+=`.
        AddAssign = "+=",
        /// `-=`.
        SubAssign = "-=",
        /// `<<=`.
        ShlAssign = "<<=",
        /// `>>=`.
        ShrAssign = ">>=",
        /// `&=`.
        AndAssign = "&=",
        /// `^=`.
        XorAssign = "^=",
        /// `|=`.
        OrAssign = "|=",
        /// `,`.
        Comma = ",",
    }
);

impl BinaryOp {
    /// `=` and the compound assignments.
    pub const fn is_assignment(self) -> bool {
        (self as u8) >= (BinaryOp::Assign as u8) && (self as u8) <= (BinaryOp::OrAssign as u8)
    }
    /// The compound assignments (`CompoundAssignOperator`).
    pub const fn is_compound_assignment(self) -> bool {
        self.is_assignment() && !matches!(self, BinaryOp::Assign)
    }
    /// `.*` / `->*`.
    pub const fn is_ptr_mem(self) -> bool {
        matches!(self, BinaryOp::PtrMemD | BinaryOp::PtrMemI)
    }
}

u8_enum!(
    /// Cast kinds (spec/ast.md §5.4; clang's `CastKind`, C++ subset, stable
    /// numbering).
    CastKind {
        /// Dependent cast.
        Dependent = "Dependent",
        /// Qualification / noexcept / identity.
        NoOp = "NoOp",
        /// [conv.lval].
        LValueToRValue = "LValueToRValue",
        /// `__builtin_bit_cast` on an lvalue.
        LValueToRValueBitCast = "LValueToRValueBitCast",
        /// `reinterpret_cast<T&>`.
        LValueBitCast = "LValueBitCast",
        /// Pointer bit cast.
        BitCast = "BitCast",
        /// [conv.array].
        ArrayToPointerDecay = "ArrayToPointerDecay",
        /// [conv.func].
        FunctionToPointerDecay = "FunctionToPointerDecay",
        /// Builtin function to function pointer.
        BuiltinFnToFnPtr = "BuiltinFnToFnPtr",
        /// Null pointer constant to pointer.
        NullToPointer = "NullToPointer",
        /// Null pointer constant to member pointer.
        NullToMemberPointer = "NullToMemberPointer",
        /// Integer to pointer.
        IntegralToPointer = "IntegralToPointer",
        /// Pointer to integer.
        PointerToIntegral = "PointerToIntegral",
        /// Pointer to `bool`.
        PointerToBoolean = "PointerToBoolean",
        /// [conv.integral].
        IntegralCast = "IntegralCast",
        /// Integer to `bool`.
        IntegralToBoolean = "IntegralToBoolean",
        /// `bool` to signed integer (-1).
        BooleanToSignedIntegral = "BooleanToSignedIntegral",
        /// [conv.fpint] integer to floating.
        IntegralToFloating = "IntegralToFloating",
        /// [conv.fpint] floating to integer.
        FloatingToIntegral = "FloatingToIntegral",
        /// Floating to `bool`.
        FloatingToBoolean = "FloatingToBoolean",
        /// [conv.double].
        FloatingCast = "FloatingCast",
        /// Derived-to-base.
        DerivedToBase = "DerivedToBase",
        /// Derived-to-base, null check elided.
        UncheckedDerivedToBase = "UncheckedDerivedToBase",
        /// Base-to-derived.
        BaseToDerived = "BaseToDerived",
        /// `dynamic_cast`.
        Dynamic = "Dynamic",
        /// Member pointer, derived to base.
        DerivedToBaseMemberPointer = "DerivedToBaseMemberPointer",
        /// Member pointer, base to derived.
        BaseToDerivedMemberPointer = "BaseToDerivedMemberPointer",
        /// Member pointer to `bool`.
        MemberPointerToBoolean = "MemberPointerToBoolean",
        /// `reinterpret_cast` of member pointers.
        ReinterpretMemberPointer = "ReinterpretMemberPointer",
        /// User-defined conversion function.
        UserDefinedConversion = "UserDefinedConversion",
        /// Converting constructor.
        ConstructorConversion = "ConstructorConversion",
        /// To `void`.
        ToVoid = "ToVoid",
        /// GNU cast to union.
        ToUnion = "ToUnion",
        /// Scalar to vector.
        VectorSplat = "VectorSplat",
        /// To `_Atomic`.
        NonAtomicToAtomic = "NonAtomicToAtomic",
        /// From `_Atomic`.
        AtomicToNonAtomic = "AtomicToNonAtomic",
        /// Complex conversions.
        IntegralRealToComplex = "IntegralRealToComplex",
        /// Complex conversions.
        FloatingRealToComplex = "FloatingRealToComplex",
        /// Complex conversions.
        IntegralComplexCast = "IntegralComplexCast",
        /// Complex conversions.
        FloatingComplexCast = "FloatingComplexCast",
        /// Complex conversions.
        IntegralComplexToReal = "IntegralComplexToReal",
        /// Complex conversions.
        FloatingComplexToReal = "FloatingComplexToReal",
        /// Complex conversions.
        IntegralComplexToBoolean = "IntegralComplexToBoolean",
        /// Complex conversions.
        FloatingComplexToBoolean = "FloatingComplexToBoolean",
        /// Complex conversions.
        IntegralComplexToFloatingComplex = "IntegralComplexToFloatingComplex",
        /// Complex conversions.
        FloatingComplexToIntegralComplex = "FloatingComplexToIntegralComplex",
    }
);

u8_enum!(
    /// Character literal kinds ([lex.ccon]).
    CharKind {
        /// `'a'`.
        Ascii = "ascii",
        /// `L'a'`.
        Wide = "wide",
        /// `u8'a'`.
        Utf8 = "utf8",
        /// `u'a'`.
        Utf16 = "utf16",
        /// `U'a'`.
        Utf32 = "utf32",
    }
);

u8_enum!(
    /// String literal kinds ([lex.string]; clang's `StringLiteralKind`).
    StringKind {
        /// `"a"`.
        Ordinary = "ordinary",
        /// `L"a"`.
        Wide = "wide",
        /// `u8"a"`.
        Utf8 = "utf8",
        /// `u"a"`.
        Utf16 = "utf16",
        /// `U"a"`.
        Utf32 = "utf32",
        /// An unevaluated string ([lex.string.uneval]: `static_assert`,
        /// `deprecated`, ...).
        Unevaluated = "unevaluated",
    }
);

u8_enum!(
    /// `UnaryExprOrTypeTraitExpr` kinds.
    TraitKind {
        /// `sizeof`.
        SizeOf = "sizeof",
        /// `alignof` / `_Alignof`.
        AlignOf = "alignof",
        /// `__alignof` (preferred alignment).
        PreferredAlignOf = "__alignof",
        /// `__datasizeof`.
        DataSizeOf = "__datasizeof",
        /// `vec_step` (`OpenCL`).
        VecStep = "vec_step",
    }
);

u8_enum!(
    /// Why a `DeclRefExpr`/`MemberExpr` is not an odr-use
    /// ([basic.def.odr]/5).
    NonOdrUse {
        /// It is an odr-use (or potentially one).
        None = "",
        /// In an unevaluated operand.
        Unevaluated = "non_odr_use_unevaluated",
        /// Constant: lvalue-to-rvalue on a usable-in-constant-expressions
        /// variable.
        Constant = "non_odr_use_constant",
        /// A discarded-value expression.
        Discarded = "non_odr_use_discarded",
    }
);

u8_enum!(
    /// Integer literal radix (for printing as written).
    Radix {
        /// Decimal.
        Decimal = "10",
        /// Hexadecimal.
        Hex = "16",
        /// Octal.
        Octal = "8",
        /// Binary.
        Binary = "2",
    }
);

u8_enum!(
    /// What a `CXXConstructExpr` / `CXXInheritedCtorInitExpr` constructs
    /// (clang's `CXXConstructionKind`; 2 bits of `aux`).
    ConstructionKind {
        /// A complete object.
        Complete = "complete",
        /// A non-virtual base subobject.
        NonVirtualBase = "non-virtual base",
        /// A virtual base subobject.
        VirtualBase = "virtual base",
        /// A delegating constructor's target ([class.base.init]/6).
        Delegating = "delegating",
    }
);

u8_enum!(
    /// The storage duration of a materialized temporary
    /// ([class.temporary]/6; clang's `StorageDuration` minus `Dynamic`;
    /// 2 bits of `MaterializeTemporaryExpr.aux`).
    StorageDuration {
        /// Destroyed at the end of the full-expression (not extended).
        FullExpression = "full expression",
        /// Lifetime-extended by an automatic reference.
        Automatic = "automatic",
        /// Lifetime-extended by a static-storage reference.
        Static = "static",
        /// Lifetime-extended by a thread-storage reference.
        Thread = "thread",
    }
);

u8_enum!(
    /// The clang type-trait keywords (spec/sema-templates.md §9.1, §9.4):
    /// the `aux` of `TypeTraitExpr`, `ArrayTypeTraitExpr` and
    /// `ExpressionTraitExpr`. Spellings are the keywords. The clang
    /// `ALIAS` keywords `__is_same_as` and `__is_literal_type` have rows
    /// for `__has_builtin`, but a built expression names the aliased
    /// trait (`__is_same`, `__is_literal`), as clang prints it;
    /// `__is_convertible_to` is a trait of its own. Arity, status and
    /// the evaluator live in `kiln_sema::templ::builtins::TABLE`.
    TypeTrait {
        /// `__has_nothrow_assign`.
        HasNothrowAssign = "__has_nothrow_assign",
        /// `__has_nothrow_constructor`.
        HasNothrowConstructor = "__has_nothrow_constructor",
        /// `__has_nothrow_copy`.
        HasNothrowCopy = "__has_nothrow_copy",
        /// `__has_nothrow_move_assign`.
        HasNothrowMoveAssign = "__has_nothrow_move_assign",
        /// `__has_trivial_assign`.
        HasTrivialAssign = "__has_trivial_assign",
        /// `__has_trivial_constructor`.
        HasTrivialDefaultConstructor = "__has_trivial_constructor",
        /// `__has_trivial_copy`.
        HasTrivialCopy = "__has_trivial_copy",
        /// `__has_trivial_destructor`.
        HasTrivialDestructor = "__has_trivial_destructor",
        /// `__has_trivial_move_assign`.
        HasTrivialMoveAssign = "__has_trivial_move_assign",
        /// `__has_trivial_move_constructor`.
        HasTrivialMoveConstructor = "__has_trivial_move_constructor",
        /// `__has_unique_object_representations`.
        HasUniqueObjectRepresentations = "__has_unique_object_representations",
        /// `__has_virtual_destructor`.
        HasVirtualDestructor = "__has_virtual_destructor",
        /// `__is_abstract`.
        IsAbstract = "__is_abstract",
        /// `__is_aggregate`.
        IsAggregate = "__is_aggregate",
        /// `__is_arithmetic`.
        IsArithmetic = "__is_arithmetic",
        /// `__is_array`.
        IsArray = "__is_array",
        /// `__is_bounded_array`.
        IsBoundedArray = "__is_bounded_array",
        /// `__is_class`.
        IsClass = "__is_class",
        /// `__is_complete_type`.
        IsCompleteType = "__is_complete_type",
        /// `__is_compound`.
        IsCompound = "__is_compound",
        /// `__is_const`.
        IsConst = "__is_const",
        /// `__is_destructible`.
        IsDestructible = "__is_destructible",
        /// `__is_empty`.
        IsEmpty = "__is_empty",
        /// `__is_enum`.
        IsEnum = "__is_enum",
        /// `__is_final`.
        IsFinal = "__is_final",
        /// `__is_floating_point`.
        IsFloatingPoint = "__is_floating_point",
        /// `__is_function`.
        IsFunction = "__is_function",
        /// `__is_fundamental`.
        IsFundamental = "__is_fundamental",
        /// `__is_integral`.
        IsIntegral = "__is_integral",
        /// `__is_literal`.
        IsLiteral = "__is_literal",
        /// `__is_literal_type` (an alias of `__is_literal`).
        IsLiteralType = "__is_literal_type",
        /// `__is_lvalue_reference`.
        IsLvalueReference = "__is_lvalue_reference",
        /// `__is_member_function_pointer`.
        IsMemberFunctionPointer = "__is_member_function_pointer",
        /// `__is_member_object_pointer`.
        IsMemberObjectPointer = "__is_member_object_pointer",
        /// `__is_member_pointer`.
        IsMemberPointer = "__is_member_pointer",
        /// `__is_nothrow_destructible`.
        IsNothrowDestructible = "__is_nothrow_destructible",
        /// `__is_nullptr`.
        IsNullptr = "__is_nullptr",
        /// `__is_object`.
        IsObject = "__is_object",
        /// `__is_pod`.
        IsPod = "__is_pod",
        /// `__is_pointer`.
        IsPointer = "__is_pointer",
        /// `__is_polymorphic`.
        IsPolymorphic = "__is_polymorphic",
        /// `__is_reference`.
        IsReference = "__is_reference",
        /// `__is_referenceable`.
        IsReferenceable = "__is_referenceable",
        /// `__is_rvalue_reference`.
        IsRvalueReference = "__is_rvalue_reference",
        /// `__is_scalar`.
        IsScalar = "__is_scalar",
        /// `__is_scoped_enum`.
        IsScopedEnum = "__is_scoped_enum",
        /// `__is_signed`.
        IsSigned = "__is_signed",
        /// `__is_standard_layout`.
        IsStandardLayout = "__is_standard_layout",
        /// `__is_trivial`.
        IsTrivial = "__is_trivial",
        /// `__is_trivially_copyable`.
        IsTriviallyCopyable = "__is_trivially_copyable",
        /// `__is_trivially_destructible`.
        IsTriviallyDestructible = "__is_trivially_destructible",
        /// `__is_trivially_equality_comparable`.
        IsTriviallyEqualityComparable = "__is_trivially_equality_comparable",
        /// `__is_trivially_relocatable`.
        IsTriviallyRelocatable = "__is_trivially_relocatable",
        /// `__is_unbounded_array`.
        IsUnboundedArray = "__is_unbounded_array",
        /// `__is_union`.
        IsUnion = "__is_union",
        /// `__is_unsigned`.
        IsUnsigned = "__is_unsigned",
        /// `__is_void`.
        IsVoid = "__is_void",
        /// `__is_volatile`.
        IsVolatile = "__is_volatile",
        /// `__can_pass_in_regs`.
        CanPassInRegs = "__can_pass_in_regs",
        /// `__is_bitwise_cloneable`.
        IsBitwiseCloneable = "__is_bitwise_cloneable",
        /// `__builtin_is_implicit_lifetime` (clang 20+).
        IsImplicitLifetime = "__builtin_is_implicit_lifetime",
        /// `__builtin_is_cpp_trivially_relocatable` (clang 21+).
        IsCppTriviallyRelocatable = "__builtin_is_cpp_trivially_relocatable",
        /// `__is_base_of`.
        IsBaseOf = "__is_base_of",
        /// `__builtin_is_virtual_base_of` (clang 20+).
        IsVirtualBaseOf = "__builtin_is_virtual_base_of",
        /// `__is_same`.
        IsSame = "__is_same",
        /// `__is_same_as` (an alias of `__is_same`).
        IsSameAs = "__is_same_as",
        /// `__is_assignable`.
        IsAssignable = "__is_assignable",
        /// `__is_convertible`.
        IsConvertible = "__is_convertible",
        /// `__is_convertible_to` (clang's own trait, not an alias).
        IsConvertibleTo = "__is_convertible_to",
        /// `__is_layout_compatible`.
        IsLayoutCompatible = "__is_layout_compatible",
        /// `__is_nothrow_assignable`.
        IsNothrowAssignable = "__is_nothrow_assignable",
        /// `__is_nothrow_convertible`.
        IsNothrowConvertible = "__is_nothrow_convertible",
        /// `__is_pointer_interconvertible_base_of`.
        IsPointerInterconvertibleBaseOf = "__is_pointer_interconvertible_base_of",
        /// `__is_trivially_assignable`.
        IsTriviallyAssignable = "__is_trivially_assignable",
        /// `__reference_binds_to_temporary`.
        ReferenceBindsToTemporary = "__reference_binds_to_temporary",
        /// `__reference_constructs_from_temporary`.
        ReferenceConstructsFromTemporary = "__reference_constructs_from_temporary",
        /// `__reference_converts_from_temporary`.
        ReferenceConvertsFromTemporary = "__reference_converts_from_temporary",
        /// `__is_constructible`.
        IsConstructible = "__is_constructible",
        /// `__is_nothrow_constructible`.
        IsNothrowConstructible = "__is_nothrow_constructible",
        /// `__is_trivially_constructible`.
        IsTriviallyConstructible = "__is_trivially_constructible",
        /// `__array_rank` (`ArrayTypeTraitExpr`).
        ArrayRank = "__array_rank",
        /// `__array_extent` (`ArrayTypeTraitExpr`).
        ArrayExtent = "__array_extent",
        /// `__is_lvalue_expr` (`ExpressionTraitExpr`).
        IsLvalueExpr = "__is_lvalue_expr",
        /// `__is_rvalue_expr` (`ExpressionTraitExpr`).
        IsRvalueExpr = "__is_rvalue_expr",
    }
);

impl TypeTrait {
    /// Every trait, in discriminant order.
    pub fn all() -> &'static [TypeTrait] {
        Self::ALL
    }

    /// The trait spelled `name`, if any.
    pub fn from_name(name: &str) -> Option<TypeTrait> {
        if !name.starts_with("__") {
            return None;
        }
        Self::ALL.iter().copied().find(|t| t.spelling() == name)
    }
}

/// The flags of a `CXXConstructExpr` / `CXXTemporaryObjectExpr`
/// (sema-class.md §5.1), packed into `aux`: bits 0-1 the
/// [`ConstructionKind`], bit 2 `list`, bit 3 `std_init_list`, bit 4
/// `zeroing`, bit 5 `immediate_escalating`. "Had multiple candidates" is
/// [`ExprFlags::HAD_MULTIPLE_CANDIDATES`]. There is no `elidable` flag
/// (C++17 guaranteed elision).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "independent flags mirroring clang's dump keys")]
pub struct ConstructInfo {
    /// What is constructed.
    pub kind: ConstructionKind,
    /// List-initialization (`T{...}`, `= {...}`).
    pub list: bool,
    /// Initialization from a `std::initializer_list` ([dcl.init.list]/3.7).
    pub std_init_list: bool,
    /// Zero-initialization precedes the constructor call
    /// ([dcl.init.general]/9.1.2, value-initialization).
    pub zeroing: bool,
    /// The call is immediate-escalating ([expr.const]/18).
    pub immediate_escalating: bool,
}

impl ConstructInfo {
    /// A plain complete-object construction.
    pub const COMPLETE: ConstructInfo = ConstructInfo {
        kind: ConstructionKind::Complete,
        list: false,
        std_init_list: false,
        zeroing: false,
        immediate_escalating: false,
    };

    /// The `aux` encoding.
    pub const fn to_aux(self) -> u8 {
        self.kind as u8
            | (self.list as u8) << 2
            | (self.std_init_list as u8) << 3
            | (self.zeroing as u8) << 4
            | (self.immediate_escalating as u8) << 5
    }

    /// Decodes [`ConstructInfo::to_aux`].
    pub fn from_aux(aux: u8) -> Self {
        ConstructInfo {
            kind: ConstructionKind::from_u8(aux & 3).expect(
                "invariant: ConstructInfo::to_aux encodes every ConstructionKind in 2 bits",
            ),
            list: aux & (1 << 2) != 0,
            std_init_list: aux & (1 << 3) != 0,
            zeroing: aux & (1 << 4) != 0,
            immediate_escalating: aux & (1 << 5) != 0,
        }
    }
}

/// The operand of a `CXXTypeidExpr`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TypeidOperand {
    /// `typeid(T)`.
    Type(QualType),
    /// `typeid(expr)`.
    Expr(ExprId),
}

/// The 20-byte expression header (spec/ast.md §5.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ExprNode {
    kind: ExprKind,
    bits: u8,
    deps: ExprDeps,
    aux: u8,
    ty: QualType,
    loc: Loc,
    a: u32,
    b: u32,
}

impl ExprNode {
    /// A header (used by the builder after computing `deps` and `vk`).
    #[allow(clippy::too_many_arguments, reason = "one argument per header field")]
    pub(crate) fn new(
        kind: ExprKind,
        vk: ValueCategory,
        ok: ObjectKind,
        flags: ExprFlags,
        deps: ExprDeps,
        aux: u8,
        ty: QualType,
        loc: Loc,
        a: u32,
        b: u32,
    ) -> Self {
        let bits = vk as u8 | ((ok as u8) << 2) | flags.0;
        ExprNode { kind, bits, deps, aux, ty, loc, a, b }
    }
    /// A header from its stored fields, for the module image's loader
    /// (`kiln_sema` `bmi_body`, spec/modules.md §11.11 "Part 2a"), which
    /// validated them; `bits` is [`ExprNode::raw_bits`].
    #[allow(clippy::too_many_arguments, reason = "one argument per header field")]
    pub const fn from_raw(
        kind: ExprKind,
        bits: u8,
        deps: ExprDeps,
        aux: u8,
        ty: QualType,
        loc: Loc,
        a: u32,
        b: u32,
    ) -> Self {
        ExprNode { kind, bits, deps, aux, ty, loc, a, b }
    }
    /// The packed value category (bits 0-1), object kind (bits 2-3) and
    /// per-kind flags (bits 4-7), as stored (the module image).
    pub const fn raw_bits(&self) -> u8 {
        self.bits
    }
    /// The kind.
    pub const fn kind(&self) -> ExprKind {
        self.kind
    }
    /// The value category.
    pub const fn value_category(&self) -> ValueCategory {
        match self.bits & 3 {
            1 => ValueCategory::LValue,
            2 => ValueCategory::XValue,
            _ => ValueCategory::PRValue,
        }
    }
    /// The object kind.
    pub const fn object_kind(&self) -> ObjectKind {
        match (self.bits >> 2) & 3 {
            1 => ObjectKind::BitField,
            2 => ObjectKind::VectorComponent,
            _ => ObjectKind::Ordinary,
        }
    }
    /// True if the per-kind flag `f` is set.
    pub const fn has(&self, f: ExprFlags) -> bool {
        self.bits & f.0 == f.0 && f.0 != 0
    }
    /// The dependence bits.
    pub const fn deps(&self) -> ExprDeps {
        self.deps
    }
    /// Operator / cast kind / radix / trait kind.
    pub const fn aux(&self) -> u8 {
        self.aux
    }
    /// The type (never a reference type).
    pub const fn ty(&self) -> QualType {
        self.ty
    }
    /// The operator / start location.
    pub const fn loc(&self) -> Loc {
        self.loc
    }
    /// Operand word `a`.
    pub const fn a(&self) -> u32 {
        self.a
    }
    /// Operand word `b`.
    pub const fn b(&self) -> u32 {
        self.b
    }
    /// True if type-dependent.
    pub const fn is_type_dependent(&self) -> bool {
        self.deps.contains(ExprDeps::TYPE)
    }
    /// True if value-dependent.
    pub const fn is_value_dependent(&self) -> bool {
        self.deps.contains(ExprDeps::VALUE)
    }
}

/// Side record of a `DeclRefExpr` with a qualifier, explicit template
/// arguments or a found declaration different from the referenced one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct DeclRefExtra {
    /// The nested-name-specifier as written.
    pub qualifier: Option<NnsId>,
    /// Explicit template arguments as written.
    pub template_args: Option<ArgListId>,
    /// The declaration lookup found (a `UsingShadow`, ...) when it differs.
    pub found_decl: Option<DeclId>,
}

/// Side record of a `CompoundAssignOperator` (spec/ast.md §5.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CompoundAssignExtra {
    /// The right operand.
    pub rhs: ExprId,
    /// `ComputeLHSTy`: the type the left operand is converted to.
    pub lhs_ty: QualType,
    /// `ComputeResultTy`: the type of the computation.
    pub result_ty: QualType,
}

/// One designator of a `DesignatedInitExpr` ([dcl.init.aggr]/3.1;
/// clang's `DesignatedInitExpr::Designator`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Designator {
    /// `.name`; `loc` is the name.
    Field {
        /// The member name.
        name: Symbol,
        /// The name's location.
        loc: Loc,
    },
    /// `[index]` (an extension in C++); `loc` is the `[`.
    Index {
        /// The index expression.
        index: ExprId,
        /// The `[`.
        loc: Loc,
    },
    /// `[first ... last]` (a GNU extension); `loc` is the `[`.
    Range {
        /// The first index.
        first: ExprId,
        /// The last index (inclusive).
        last: ExprId,
        /// The `[`.
        loc: Loc,
        /// The `...`.
        ellipsis: Loc,
    },
}

/// One part of a `__builtin_offsetof` designator (clang
/// `Sema::OffsetOfComponent`). A value-dependent `OffsetOfExpr` keeps
/// its operand type and designator as written for instantiation
/// ([`ColdExprData::offsetof_pattern`]).
#[derive(Clone, Copy, Debug)]
pub enum OffsetofComponent {
    /// A member name: the leading identifier (`start` = `end` = its
    /// location) or `.identifier` (`start` is the `.`).
    Field {
        /// The member name.
        name: Symbol,
        /// The `.`, or the name for the leading identifier.
        start: Loc,
        /// The name.
        end: Loc,
    },
    /// `[ index ]`.
    Index {
        /// The index expression.
        index: ExprId,
        /// The `[`.
        start: Loc,
        /// The `]`.
        end: Loc,
    },
}

/// The meaning of the accessor of an ext-vector element access `v.xyzw`
/// (clang `CheckExtVectorComponent` and
/// `ExtVectorElementExpr::getEncodedElementAccess`; spec/sema.md §5.8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ExtVectorAccess {
    /// A valid accessor: the selected lanes in order (a lane of `hi` or
    /// `odd` on an odd-sized vector may be past its end and is then
    /// undefined), and whether a lane repeats (the result cannot be
    /// assigned to).
    Lanes {
        /// The selected lane indices.
        lanes: Vec<u32>,
        /// Whether some lane is selected twice.
        repeated: bool,
    },
    /// `illegal vector component name '{c}'`: the first character that
    /// is not from the set the accessor started with, and its byte
    /// offset in the accessor (clang 22 diagnoses it there).
    Illegal(char, u32),
    /// `vector component access exceeds type`.
    Exceeds,
}

/// Checks accessor `name` against an ext vector of `count` lanes: the
/// point set `xyzw`, the color set `rgba`, `s`/`S` followed by hex
/// digits, or one of `hi`, `lo`, `even`, `odd` (clang 19).
pub fn ext_vector_access(name: &str, count: u32) -> ExtVectorAccess {
    fn point(c: u8) -> Option<u32> {
        match c {
            b'x' | b'r' => Some(0),
            b'y' | b'g' => Some(1),
            b'z' | b'b' => Some(2),
            b'w' | b'a' => Some(3),
            _ => None,
        }
    }
    fn numeric(c: u8) -> Option<u32> {
        match c {
            b'0'..=b'9' => Some(u32::from(c - b'0')),
            b'a'..=b'f' => Some(u32::from(c - b'a') + 10),
            b'A'..=b'F' => Some(u32::from(c - b'A') + 10),
            _ => None,
        }
    }
    let rgba = |c: u8| matches!(c, b'r' | b'g' | b'b' | b'a');
    let b = name.as_bytes();
    let half = matches!(name, "hi" | "lo" | "even" | "odd");
    let hex = b.len() > 1 && matches!(b[0], b's' | b'S');
    let mut seen = [false; 16];
    let mut repeated = false;
    let mut i = 0;
    if half {
    } else if !hex && b.first().copied().and_then(point).is_some() {
        let has_rgba = rgba(b[0]);
        while let Some(idx) = b.get(i).copied().and_then(point) {
            if rgba(b[i]) != has_rgba {
                break;
            }
            repeated |= seen[idx as usize];
            seen[idx as usize] = true;
            i += 1;
        }
    } else {
        i = usize::from(hex);
        while let Some(idx) = b.get(i).copied().and_then(numeric) {
            repeated |= seen[idx as usize];
            seen[idx as usize] = true;
            i += 1;
        }
    }
    if !half && i < b.len() {
        let at = u32::try_from(i).unwrap_or(u32::MAX);
        return ExtVectorAccess::Illegal(name[i..].chars().next().unwrap_or('?'), at);
    }
    let idx = |c: u8| if hex { numeric(c) } else { point(c) };
    let comps = &b[usize::from(hex)..];
    if !half && comps.iter().any(|&c| idx(c).is_none_or(|x| x >= count)) {
        return ExtVectorAccess::Exceeds;
    }
    let lanes = if half {
        let size = count.div_ceil(2);
        (0..size)
            .map(|i| match name {
                "hi" => size + i,
                "lo" => i,
                "even" => 2 * i,
                _ => 2 * i + 1,
            })
            .collect()
    } else {
        comps.iter().filter_map(|&c| idx(c)).collect()
    };
    ExtVectorAccess::Lanes { lanes, repeated }
}

/// Side record of an `InitListExpr` (semantic form, §5.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct InitListExtra {
    /// Initializer for the elements past the explicit ones.
    pub array_filler: Option<ExprId>,
    /// The initialized member of a union.
    pub union_field: Option<DeclId>,
    /// The `}`.
    pub rbrace: Loc,
}

/// How a `CXXNewExpr`'s initializer was written (its `aux`; clang's
/// `CXXNewInitializationStyle`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum NewInitStyle {
    /// No initializer: default-initialization.
    None = 0,
    /// `new T(args)`, including `new T()`.
    Parens = 1,
    /// `new T{args}`.
    Braces = 2,
}

impl NewInitStyle {
    /// The style with raw value `v`.
    pub const fn from_u8(v: u8) -> Option<NewInitStyle> {
        match v {
            0 => Some(NewInitStyle::None),
            1 => Some(NewInitStyle::Parens),
            2 => Some(NewInitStyle::Braces),
            _ => None,
        }
    }
}

/// Side record of a `CXXNewExpr` (spec/ast.md §5.7; [expr.new]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NewExtra {
    /// The allocated type: the type of the created object, or for an array
    /// new the element type (`int[4]` for `new int[n][4]`; clang's
    /// `getAllocatedType`).
    pub allocated_ty: QualType,
    /// The first array bound, converted to `size_t` (synthesized from the
    /// initializer for `new int[]{...}`); `None` for a non-array new.
    pub array_size: Option<ExprId>,
    /// The semantic initializer (`InitListExpr`, `ImplicitValueInitExpr`,
    /// or the converted expression); `None` for default-initialization.
    pub init: Option<ExprId>,
    /// The converted placement arguments (a `ListId` of `ExprId`s).
    pub placement: Option<ListId>,
    /// The allocation function.
    pub operator_new: Option<DeclId>,
    /// The matching deallocation function (called if initialization
    /// throws).
    pub operator_delete: Option<DeclId>,
    /// The start of the type-id (after any placement arguments).
    pub type_loc: Loc,
}

/// Cold per-expression data.
#[derive(Clone, Debug)]
pub struct ColdExprData {
    /// The closing token of kinds that end in one and have no room for it
    /// (`CallExpr` `)`, `UnaryExprOrTypeTraitExpr` `)`, `CStyleCastExpr`
    /// `)` before the operand, ...).
    pub end_loc: ColdMap<ExprId, Loc>,
    /// `MemberExpr` qualifier / template args / found decl.
    pub member_extra: ColdMap<ExprId, DeclRefExtra>,
    /// The designators of a `DesignatedInitExpr` (§5.6), outermost
    /// first.
    pub designators: ColdMap<ExprId, Box<[Designator]>>,
    /// The written form of a semantic `InitListExpr` with designators
    /// built in a template pattern: a dependent `InitListExpr` whose
    /// designated clauses are `DesignatedInitExpr`s, which instantiation
    /// rebuilds from (clang's syntactic form, kept only in templates;
    /// §5.6).
    pub syntactic_list: ColdMap<ExprId, ExprId>,
    /// `ArrayTypeTraitExpr`: the dimension operand of `__array_extent`
    /// (unevaluated by codegen; kept for substitution) and the value.
    pub array_trait: ColdMap<ExprId, ArrayTraitExtra>,
    /// `TypeTraitExpr`: the location of the second type argument of a
    /// binary trait (clang reports `__is_base_of`'s incomplete derived
    /// type there); `VAArgExpr`: the begin location of the written type
    /// (clang's `TypeLoc::getBeginLoc`, where its checks report).
    pub trait_rhs_loc: ColdMap<ExprId, Loc>,
    /// The values of the source-location builtins at a use (§5.8): for a
    /// `SourceLocExpr` its own location and context, for a
    /// `CXXDefaultArgExpr` / `CXXDefaultInitExpr` whose initializer holds
    /// one, the location and context of the use. Computed by sema (which
    /// owns the source manager and the function-name printer).
    pub source_loc: ColdMap<ExprId, SourceLocValues>,
    /// The value of an immediate invocation (`ConstantExpr`) in a default
    /// argument or default member initializer, which has none of its own
    /// (it is evaluated at each use, §5.10): sema's evaluation at a use
    /// without source-location values, which every such use shares
    /// (codegen reads it instead of evaluating the call again, WP-1129).
    pub default_imm: ColdMap<ExprId, ConstValueId>,
    /// The initialized member of a union's `CXXParenListInitExpr`
    /// (clang `getInitializedFieldInUnion`).
    pub union_field: ColdMap<ExprId, DeclId>,
    /// The type as written of a named cast or functional cast, when the
    /// dump cannot rebuild it from the node (clang `getTypeAsWritten`): a
    /// non-class `CI{1}` / `static_cast<CI>(x)` whose prvalue drops the
    /// qualifiers and sugar of `CI`, a reference named by an alias.
    pub cast_written: ColdMap<ExprId, QualType>,
    /// A value-dependent `OffsetOfExpr`: the operand type and the
    /// designator as written, rebuilt at instantiation (clang keeps its
    /// `OffsetOfNode`s; spec/sema.md §5.8).
    pub offsetof_pattern: ColdMap<ExprId, (QualType, Box<[OffsetofComponent]>)>,
    /// A dependent `CXXSpliceExpr` written as a template-id,
    /// `template [: r :] < args >` (P2996 [expr.prim.splice]): the
    /// template arguments as written.
    pub splice_args: ColdMap<ExprId, ArgListId>,
    /// A `CXXDefaultArgExpr` whose default argument was rebuilt for this
    /// use (clang `CXXDefaultArgExpr::getRewrittenExpr`): in a
    /// lifetime-extending context its temporaries are its own, fresh
    /// `MaterializeTemporaryExpr`s that the extending declaration
    /// extends, where the parameter's shared default argument keeps
    /// those of every other call (spec/sema.md §7.6a).
    pub default_arg_rewritten: ColdMap<ExprId, ExprId>,
}

/// The predefined identifier a `PredefinedExpr` names ([dcl.fct.def.general]/8
/// and the GNU spellings; clang `PredefinedIdentKind`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum PredefinedKind {
    /// `__func__`: the function's unqualified name.
    Func,
    /// `__FUNCTION__`: the same as `__func__` outside MSVC mode.
    Function,
    /// `__PRETTY_FUNCTION__`: the signature with qualified name and
    /// template arguments.
    PrettyFunction,
}

impl PredefinedKind {
    /// Every kind, in `repr` order.
    pub const ALL: [PredefinedKind; 3] =
        [PredefinedKind::Func, PredefinedKind::Function, PredefinedKind::PrettyFunction];

    /// The identifier (clang `getIdentKindName`).
    pub const fn spelling(self) -> &'static str {
        match self {
            PredefinedKind::Func => "__func__",
            PredefinedKind::Function => "__FUNCTION__",
            PredefinedKind::PrettyFunction => "__PRETTY_FUNCTION__",
        }
    }

    fn from_aux(a: u8) -> PredefinedKind {
        Self::ALL.get(usize::from(a)).copied().unwrap_or(PredefinedKind::Func)
    }
}

/// The builtin a `SourceLocExpr` names (clang `SourceLocIdentKind`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum SourceLocKind {
    /// `__builtin_FILE()`: `const char *`.
    File,
    /// `__builtin_FILE_NAME()`: the last path component, `const char *`.
    FileName,
    /// `__builtin_FUNCTION()`: `const char *`.
    Function,
    /// `__builtin_LINE()`: `unsigned int`.
    Line,
    /// `__builtin_COLUMN()`: `unsigned int`.
    Column,
    /// `__builtin_source_location()`: `const std::source_location::__impl *`.
    SourceLocStruct,
}

impl SourceLocKind {
    /// Every kind, in `repr` order.
    pub const ALL: [SourceLocKind; 6] = [
        SourceLocKind::File,
        SourceLocKind::FileName,
        SourceLocKind::Function,
        SourceLocKind::Line,
        SourceLocKind::Column,
        SourceLocKind::SourceLocStruct,
    ];

    /// The builtin's name.
    pub const fn spelling(self) -> &'static str {
        match self {
            SourceLocKind::File => "__builtin_FILE",
            SourceLocKind::FileName => "__builtin_FILE_NAME",
            SourceLocKind::Function => "__builtin_FUNCTION",
            SourceLocKind::Line => "__builtin_LINE",
            SourceLocKind::Column => "__builtin_COLUMN",
            SourceLocKind::SourceLocStruct => "__builtin_source_location",
        }
    }

    fn from_aux(a: u8) -> SourceLocKind {
        Self::ALL.get(usize::from(a)).copied().unwrap_or(SourceLocKind::File)
    }
}

/// The values every source-location builtin has at one location in one
/// context (clang `SourceLocExpr::EvaluateInContext`): the pointers are
/// interned [`Scalar::Pointer`](crate::consts::Scalar) values into string
/// literal objects (and, for `location`, into the frozen
/// `std::source_location::__impl` object).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SourceLocValues {
    /// `__builtin_FILE()`.
    pub file: ConstValueId,
    /// `__builtin_FILE_NAME()`.
    pub file_name: ConstValueId,
    /// `__builtin_FUNCTION()`.
    pub function: ConstValueId,
    /// `__builtin_LINE()`.
    pub line: u32,
    /// `__builtin_COLUMN()`.
    pub column: u32,
    /// `__builtin_source_location()`, when the use needs it.
    pub location: Option<ConstValueId>,
}

/// The cold record of an `ArrayTypeTraitExpr` (§9.4).
#[derive(Clone, Copy, Debug)]
pub struct ArrayTraitExtra {
    /// The converted dimension of `__array_extent(T, I)`.
    pub dim: Option<ExprId>,
    /// The value (0 while value-dependent).
    pub value: u64,
}

impl Default for ColdExprData {
    fn default() -> Self {
        Self {
            end_loc: ColdMap::new("end_loc"),
            member_extra: ColdMap::new("member_extra"),
            designators: ColdMap::new("designators"),
            syntactic_list: ColdMap::new("syntactic_list"),
            array_trait: ColdMap::new("array_trait"),
            trait_rhs_loc: ColdMap::new("trait_rhs_loc"),
            source_loc: ColdMap::new("source_loc"),
            default_imm: ColdMap::new("default_imm"),
            union_field: ColdMap::new("union_field"),
            cast_written: ColdMap::new("cast_written"),
            offsetof_pattern: ColdMap::new("offsetof_pattern"),
            splice_args: ColdMap::new("splice_args"),
            default_arg_rewritten: ColdMap::new("default_arg_rewritten"),
        }
    }
}

/// Expression headers and side records (spec/ast.md §5). Index 0 of every
/// vector is a dummy.
#[derive(Debug)]
pub struct ExprArena {
    nodes: Vec<ExprNode>,
    cond_arms: Vec<[ExprId; 2]>,
    declref_extras: Vec<DeclRefExtra>,
    compound_assigns: Vec<CompoundAssignExtra>,
    init_lists: Vec<InitListExtra>,
    news: Vec<NewExtra>,
    /// Lambda side records (`LambdaExpr.a`, `RecordData.lambda`).
    pub lambdas: crate::lambda::LambdaArena,
    /// Cold per-expression data.
    pub cold: ColdExprData,
}

impl Default for ExprArena {
    fn default() -> Self {
        let qt = QualType::unqual(crate::ids::TypeId::new(1));
        let dummy_e = ExprId::new(ExprId::MAX_INDEX);
        Self {
            nodes: vec![ExprNode::new(
                ExprKind::RecoveryExpr,
                ValueCategory::PRValue,
                ObjectKind::Ordinary,
                ExprFlags::NONE,
                ExprDeps::ERROR,
                0,
                qt,
                Loc::INVALID,
                0,
                0,
            )],
            cond_arms: vec![[dummy_e; 2]],
            declref_extras: vec![DeclRefExtra::default()],
            compound_assigns: vec![CompoundAssignExtra { rhs: dummy_e, lhs_ty: qt, result_ty: qt }],
            init_lists: vec![InitListExtra::default()],
            news: vec![NewExtra {
                allocated_ty: qt,
                array_size: None,
                init: None,
                placement: None,
                operator_new: None,
                operator_delete: None,
                type_loc: Loc::INVALID,
            }],
            lambdas: crate::lambda::LambdaArena::default(),
            cold: ColdExprData::default(),
        }
    }
}

fn push_side<T>(v: &mut Vec<T>, x: T) -> u32 {
    let i = v.len();
    assert!(i < u32::MAX as usize, "expression side-record space exhausted");
    v.push(x);
    i as u32
}

/// `CXXReflectExpr`'s `aux` tag for a dependent qualified-name operand.
const REFLECT_DEPENDENT_NAME: u8 = crate::reflect::Reflectee::DEPENDENT_NAME_TAG;

impl ExprArena {
    /// An arena holding only the dummies.
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of expressions.
    pub fn len(&self) -> usize {
        self.nodes.len() - 1
    }

    /// True if there are no expressions.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The header of `e`.
    pub fn get(&self, e: ExprId) -> &ExprNode {
        &self.nodes[e.index() as usize]
    }

    /// Appends a record (the builder; the module image's loader).
    pub fn push(&mut self, n: ExprNode) -> ExprId {
        let id = ExprId::new(self.nodes.len() as u32);
        self.nodes.push(n);
        id
    }

    /// Appends a record (the builder; the module image's loader).
    pub fn push_cond_arms(&mut self, then: ExprId, els: ExprId) -> u32 {
        push_side(&mut self.cond_arms, [then, els])
    }
    /// Appends a record (the builder; the module image's loader).
    pub fn push_declref_extra(&mut self, x: DeclRefExtra) -> u32 {
        push_side(&mut self.declref_extras, x)
    }
    /// Appends a record (the builder; the module image's loader).
    pub fn push_compound_assign(&mut self, x: CompoundAssignExtra) -> u32 {
        push_side(&mut self.compound_assigns, x)
    }
    /// Appends a record (the builder; the module image's loader).
    pub fn push_init_list(&mut self, x: InitListExtra) -> u32 {
        push_side(&mut self.init_lists, x)
    }
    pub(crate) fn push_new_extra(&mut self, x: NewExtra) -> u32 {
        push_side(&mut self.news, x)
    }

    fn expect_kind(&self, e: ExprId, k: ExprKind) -> &ExprNode {
        // Invariant (WP-145): accessors are called on the kind they read;
        // a mismatch is a Sema/AST bug, never input (spec/ast.md).
        let n = self.get(e);
        assert!(n.kind == k, "expected {k:?}, found {:?}", n.kind);
        n
    }

    /// The value of an `IntegerLiteral` of at most 64 bits (`None` for
    /// wider literals, whose `a` is a `ConstValueId`).
    pub fn int_value(&self, e: ExprId) -> Option<u64> {
        let n = self.expect_kind(e, ExprKind::IntegerLiteral);
        (!n.has(ExprFlags::WIDE)).then(|| u64::from(n.a) | (u64::from(n.b) << 32))
    }

    /// The value of a wide `IntegerLiteral` or a `FloatingLiteral`.
    pub fn literal_const(&self, e: ExprId) -> Option<ConstValueId> {
        let n = self.get(e);
        match n.kind {
            ExprKind::FloatingLiteral => ConstValueId::from_raw(n.a),
            ExprKind::IntegerLiteral if n.has(ExprFlags::WIDE) => ConstValueId::from_raw(n.a),
            _ => None,
        }
    }

    /// The string of a `StringLiteral`.
    pub fn string_lit(&self, e: ExprId) -> StringLitId {
        StringLitId::new(self.expect_kind(e, ExprKind::StringLiteral).a)
    }

    /// The referenced declaration of a `DeclRefExpr` or `MemberExpr`.
    pub fn decl(&self, e: ExprId) -> DeclId {
        let n = self.get(e);
        match n.kind {
            ExprKind::DeclRefExpr => DeclId::new(n.a),
            ExprKind::MemberExpr => DeclId::new(n.b),
            k => panic!("invariant: decl() is called on a DeclRefExpr or MemberExpr, found {k:?}"),
        }
    }

    /// The extras of a `DeclRefExpr` (qualifier, template args, found
    /// decl) or `MemberExpr`.
    pub fn decl_ref_extra(&self, e: ExprId) -> Option<&DeclRefExtra> {
        let n = self.get(e);
        match n.kind {
            ExprKind::DeclRefExpr if n.b != 0 => Some(&self.declref_extras[n.b as usize]),
            ExprKind::MemberExpr => self.cold.member_extra.get(e),
            _ => None,
        }
    }

    /// The operator of a `UnaryOperator`.
    pub fn unary_op(&self, e: ExprId) -> UnaryOp {
        UnaryOp::from_u8(self.expect_kind(e, ExprKind::UnaryOperator).aux)
            .expect("invariant: the builder stores a valid opcode in aux")
    }

    /// The operator of a `BinaryOperator` / `CompoundAssignOperator`.
    pub fn binary_op(&self, e: ExprId) -> BinaryOp {
        let n = self.get(e);
        assert!(
            matches!(n.kind, ExprKind::BinaryOperator | ExprKind::CompoundAssignOperator),
            "expected a binary operator, found {:?}",
            n.kind
        );
        BinaryOp::from_u8(n.aux).expect("invariant: the builder stores a valid opcode in aux")
    }

    /// The side record of a `CompoundAssignOperator`.
    pub fn compound_assign(&self, e: ExprId) -> &CompoundAssignExtra {
        &self.compound_assigns[self.expect_kind(e, ExprKind::CompoundAssignOperator).b as usize]
    }

    /// The cast kind of a cast expression.
    pub fn cast_kind(&self, e: ExprId) -> CastKind {
        let n = self.get(e);
        assert!(n.kind.is_cast(), "expected a cast, found {:?}", n.kind);
        CastKind::from_u8(n.aux).expect("invariant: the builder stores a valid CastKind in aux")
    }

    /// The type as written of a named or functional cast, when it is not
    /// the node's type (plus `&`/`&&` for a glvalue); see
    /// [`ColdExprData::cast_written`].
    pub fn cast_written_type(&self, e: ExprId) -> Option<QualType> {
        self.cold.cast_written.get(e).copied()
    }

    /// The base path (`ListId` of base-spec indices) of a derived-to-base /
    /// base-to-derived cast.
    pub fn cast_path(&self, e: ExprId) -> Option<ListId> {
        let n = self.get(e);
        assert!(n.kind.is_cast(), "expected a cast, found {:?}", n.kind);
        ListId::from_raw(n.b)
    }

    /// The then/else arms of a `ConditionalOperator`.
    pub fn cond_arms(&self, e: ExprId) -> [ExprId; 2] {
        self.cond_arms[self.expect_kind(e, ExprKind::ConditionalOperator).b as usize]
    }

    /// The condition and the two operands of a `ChooseExpr`.
    pub fn choose_operands(&self, e: ExprId) -> [ExprId; 3] {
        let n = self.expect_kind(e, ExprKind::ChooseExpr);
        let [l, r] = self.cond_arms[n.b as usize];
        [ExprId::new(n.a), l, r]
    }

    /// The operand a `ChooseExpr`'s constant condition selected; `None`
    /// while the condition is dependent.
    pub fn choose_chosen(&self, e: ExprId) -> Option<ExprId> {
        let n = self.expect_kind(e, ExprKind::ChooseExpr);
        let [l, r] = self.cond_arms[n.b as usize];
        match n.aux {
            1 => Some(l),
            2 => Some(r),
            _ => None,
        }
    }

    /// The operand a `ChooseExpr` or `GenericSelectionExpr` resolved to
    /// (both are transparent once resolved); `None` for other kinds and
    /// while the selection is dependent.
    pub fn selected_operand(&self, e: ExprId) -> Option<ExprId> {
        match self.get(e).kind {
            ExprKind::ChooseExpr => self.choose_chosen(e),
            ExprKind::GenericSelectionExpr => ExprId::from_raw(self.get(e).b),
            _ => None,
        }
    }

    /// The controlling operand of a `GenericSelectionExpr`.
    pub fn generic_control(&self, e: ExprId, lists: &IdLists) -> GenericControl {
        let n = self.expect_kind(e, ExprKind::GenericSelectionExpr);
        let w = lists.get(ListId::new(n.a));
        if n.has(ExprFlags::ARG_IS_TYPE) {
            let q = QualType::from_raw(w[0]).unwrap_or(QualType::ERROR);
            GenericControl::Type(q, Loc::from_raw(w[1]))
        } else {
            GenericControl::Expr(ExprId::new(w[0]))
        }
    }

    /// The associations of a `GenericSelectionExpr`, in source order.
    pub fn generic_assocs(&self, e: ExprId, lists: &IdLists) -> Vec<GenericAssoc> {
        let n = self.expect_kind(e, ExprKind::GenericSelectionExpr);
        lists.get(ListId::new(n.a))[2..]
            .chunks_exact(3)
            .map(|c| GenericAssoc {
                ty: QualType::from_raw(c[0]),
                ty_loc: Loc::from_raw(c[1]),
                expr: ExprId::new(c[2]),
            })
            .collect()
    }

    /// The side record of an `InitListExpr`.
    pub fn init_list(&self, e: ExprId) -> &InitListExtra {
        &self.init_lists[self.expect_kind(e, ExprKind::InitListExpr).b as usize]
    }

    /// The kind of a `CharacterLiteral`.
    pub fn char_kind(&self, e: ExprId) -> CharKind {
        CharKind::from_u8(self.expect_kind(e, ExprKind::CharacterLiteral).aux)
            .expect("invariant: the builder stores a valid CharKind in aux")
    }

    /// The code point of a `CharacterLiteral`.
    pub fn char_value(&self, e: ExprId) -> u32 {
        self.expect_kind(e, ExprKind::CharacterLiteral).a
    }

    /// The kind of a `StringLiteral`.
    pub fn string_kind(&self, e: ExprId) -> StringKind {
        StringKind::from_u8(self.expect_kind(e, ExprKind::StringLiteral).aux)
            .expect("invariant: the builder stores a valid StringKind in aux")
    }

    /// The value of a `CXXBoolLiteralExpr`.
    pub fn bool_value(&self, e: ExprId) -> bool {
        self.expect_kind(e, ExprKind::CXXBoolLiteralExpr).a != 0
    }

    /// The radix of an `IntegerLiteral` as written.
    pub fn radix(&self, e: ExprId) -> Radix {
        Radix::from_u8(self.expect_kind(e, ExprKind::IntegerLiteral).aux)
            .expect("invariant: the builder stores a valid Radix in aux")
    }

    /// Why a `DeclRefExpr` / `MemberExpr` is not an odr-use.
    pub fn non_odr_use(&self, e: ExprId) -> NonOdrUse {
        let n = self.get(e);
        assert!(
            matches!(n.kind, ExprKind::DeclRefExpr | ExprKind::MemberExpr),
            "expected DeclRefExpr or MemberExpr, found {:?}",
            n.kind
        );
        NonOdrUse::from_u8(n.aux).expect("invariant: the builder stores a valid NonOdrUse in aux")
    }

    /// Records why a `DeclRefExpr` / `MemberExpr` is not an odr-use (sema's
    /// end-of-full-expression odr-use pass, [basic.def.odr]/5; clang's
    /// `MaybeODRUseExprs`). Only `None` → non-`None` is allowed.
    ///
    /// # Panics
    /// If `e` is not a `DeclRefExpr` / `MemberExpr`.
    pub fn set_non_odr_use(&mut self, e: ExprId, v: NonOdrUse) -> Result<(), AlreadySet> {
        let n = &mut self.nodes[e.index() as usize];
        assert!(
            matches!(n.kind, ExprKind::DeclRefExpr | ExprKind::MemberExpr),
            "expected DeclRefExpr or MemberExpr, found {:?}",
            n.kind
        );
        if n.aux != NonOdrUse::None as u8 {
            return Err(AlreadySet { slot: "DeclRefExpr.non_odr_use", existing: u32::from(n.aux) });
        }
        n.aux = v as u8;
        Ok(())
    }

    /// Records the explicit template arguments `targs` written on the
    /// member access `e` (`b.f<int>`, clang's `hasExplicitTemplateArgs`),
    /// known only once the call resolved the member template. Write-once.
    ///
    /// # Panics
    /// If `e` is not a `MemberExpr`.
    pub fn set_member_template_args(
        &mut self,
        e: ExprId,
        targs: ArgListId,
    ) -> Result<(), AlreadySet> {
        let kind = self.nodes[e.index() as usize].kind;
        assert!(kind == ExprKind::MemberExpr, "expected MemberExpr, found {kind:?}");
        match self.cold.member_extra.get(e).copied() {
            Some(x) if x.template_args.is_some() => {
                Err(AlreadySet { slot: "MemberExpr.template_args", existing: 0 })
            }
            Some(x) => {
                self.cold.member_extra.refine(e, DeclRefExtra { template_args: Some(targs), ..x });
                Ok(())
            }
            None => self.cold.member_extra.insert(
                e,
                DeclRefExtra { qualifier: None, template_args: Some(targs), found_decl: None },
            ),
        }
    }

    /// Records the nested-name-specifier `nns` written before the
    /// template-id that `DeclRefExpr` `e` names (`N::v<int>`, a variable
    /// template specialization), known only once the parser has the
    /// expression. Write-once; `e` must carry extras.
    ///
    /// # Panics
    /// If `e` is not a `DeclRefExpr`.
    pub fn set_decl_ref_qualifier(&mut self, e: ExprId, nns: NnsId) -> Result<(), AlreadySet> {
        let n = self.nodes[e.index() as usize];
        assert!(n.kind == ExprKind::DeclRefExpr, "expected DeclRefExpr, found {:?}", n.kind);
        let slot = "DeclRefExpr.qualifier";
        let Some(x) = self.declref_extras.get_mut(n.b as usize).filter(|_| n.b != 0) else {
            return Err(AlreadySet { slot, existing: 0 });
        };
        if x.qualifier.is_some() {
            return Err(AlreadySet { slot, existing: 0 });
        }
        x.qualifier = Some(nns);
        Ok(())
    }

    /// Marks the call `e` (`CallExpr` or `CXXOperatorCallExpr`) as calling
    /// a function found only by argument-dependent lookup (clang's
    /// `UsesADL`, dumped `adl`).
    pub fn set_call_adl(&mut self, e: ExprId) {
        let n = &mut self.nodes[e.index() as usize];
        assert!(
            matches!(n.kind, ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr),
            "expected a call, found {:?}",
            n.kind
        );
        n.bits |= ExprFlags::ADL.0;
    }

    /// Marks the `UnresolvedLookupExpr` `e` as using argument-dependent
    /// lookup: an unqualified template-id followed by `(`
    /// ([basic.lookup.argdep]/1, [temp.names]/3). Write-once.
    pub fn set_adl(&mut self, e: ExprId) -> Result<(), AlreadySet> {
        let n = &mut self.nodes[e.index() as usize];
        assert!(
            n.kind == ExprKind::UnresolvedLookupExpr,
            "expected UnresolvedLookupExpr, found {:?}",
            n.kind
        );
        if n.aux != 0 {
            return Err(AlreadySet {
                slot: "UnresolvedLookupExpr.adl",
                existing: u32::from(n.aux),
            });
        }
        n.aux = 1;
        Ok(())
    }

    /// Marks the glvalue `e` as designating a bit-field
    /// ([`ObjectKind::BitField`]): sema propagates the marking from a
    /// member access through `=`, compound assignment, prefix `++`/`--`,
    /// `?:` and `,` ([expr.ass]/1, [expr.pre.incr]/1, [expr.cond]/7.1,
    /// [expr.comma]/1), which the builder cannot see. Monotone.
    pub fn mark_bitfield(&mut self, e: ExprId) {
        let n = &mut self.nodes[e.index() as usize];
        assert!(n.bits & 3 != 0, "only a glvalue can designate a bit-field");
        n.bits = (n.bits & !(3 << 2)) | ((ObjectKind::BitField as u8) << 2);
    }

    /// Marks the glvalue `e` as designating a vector component
    /// ([`ObjectKind::VectorComponent`]), propagated like
    /// [`Self::mark_bitfield`] (clang copies the object kind of the
    /// left operand of `=`, of a compound assignment and of prefix
    /// `++`/`--`, and of the right operand of `,`). Monotone.
    pub fn mark_vector_component(&mut self, e: ExprId) {
        let n = &mut self.nodes[e.index() as usize];
        assert!(n.bits & 3 != 0, "only a glvalue can designate a vector component");
        n.bits = (n.bits & !(3 << 2)) | ((ObjectKind::VectorComponent as u8) << 2);
    }

    /// Retypes the string literal `e` that initializes a character array
    /// (and the parentheses around it) to the array type `ty`, as a
    /// prvalue ([dcl.init.string]; clang's `updateStringLiteralType`).
    ///
    /// # Panics
    /// If `e` is not a `StringLiteral` inside zero or more `ParenExpr`s
    /// and GNU `__extension__`s (clang `IgnoreParensSingleStep`).
    pub fn update_string_init_type(&mut self, e: ExprId, ty: QualType) {
        let mut e = e;
        loop {
            let n = &mut self.nodes[e.index() as usize];
            n.ty = ty;
            n.bits &= !3;
            match n.kind {
                ExprKind::StringLiteral => return,
                ExprKind::ParenExpr => e = ExprId::new(n.a),
                ExprKind::UnaryOperator if n.aux == UnaryOp::Extension as u8 => {
                    e = ExprId::new(n.a);
                }
                k => panic!(
                    "invariant: update_string_init_type is called on a (parenthesized or __extension__) StringLiteral, found {k:?}"
                ),
            }
        }
    }

    /// Extends the lifetime of the full-expression temporary `e` (a
    /// `MaterializeTemporaryExpr`) to that of a reference
    /// ([class.temporary]/6; clang's `setExtendingDecl`): `extended_by` is
    /// its `LifetimeExtendedTemporary` and `storage` the extending
    /// declaration's storage duration. Sema extends after the initializer
    /// is built, as clang does (spec/sema-class.md §5.4, §8.2). Write-once.
    ///
    /// # Errors
    /// If `e` is already extended.
    ///
    /// # Panics
    /// If `e` is not a `MaterializeTemporaryExpr` or `storage` is
    /// `FullExpression`.
    pub fn extend_temporary(
        &mut self,
        e: ExprId,
        extended_by: DeclId,
        storage: StorageDuration,
    ) -> Result<(), AlreadySet> {
        assert!(storage != StorageDuration::FullExpression, "extension needs a storage duration");
        let n = &mut self.nodes[e.index() as usize];
        assert!(n.kind == ExprKind::MaterializeTemporaryExpr, "not a MaterializeTemporaryExpr");
        if n.b != 0 {
            return Err(AlreadySet { slot: "MaterializeTemporaryExpr.extended_by", existing: n.b });
        }
        n.aux = (n.aux & !3) | storage as u8;
        n.b = extended_by.raw();
        Ok(())
    }

    /// Marks the `ImplicitCastExpr` `e` as part of an explicit cast
    /// (`part_of_explicit_cast`): sema builds the implicit conversion
    /// sequence of a C-style or functional cast first and marks it
    /// afterwards ([expr.cast]/4). Monotone.
    ///
    /// # Panics
    /// If `e` is not an `ImplicitCastExpr`.
    pub fn mark_part_of_explicit_cast(&mut self, e: ExprId) {
        let n = &mut self.nodes[e.index() as usize];
        assert!(n.kind == ExprKind::ImplicitCastExpr, "not an ImplicitCastExpr");
        n.bits |= ExprFlags::PART_OF_EXPLICIT_CAST.0;
    }

    fn expect_call(&self, e: ExprId) -> &ExprNode {
        let n = self.get(e);
        assert!(
            matches!(
                n.kind,
                ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr
            ),
            "expected a call, found {:?}",
            n.kind
        );
        n
    }

    /// The callee of a `CallExpr` or `CXXMemberCallExpr` (for the latter a
    /// `MemberExpr`, or a `.*`/`->*` `BinaryOperator`).
    pub fn callee(&self, e: ExprId) -> ExprId {
        ExprId::new(self.expect_call(e).a)
    }

    /// The arguments of a `CallExpr` or `CXXMemberCallExpr` (a `ListId` of
    /// `ExprId`s; the implied object argument is not among them).
    pub fn call_args(&self, e: ExprId) -> ListId {
        ListId::new(self.expect_call(e).b)
    }

    /// The operator of a `CXXOperatorCallExpr` (spec/sema-overload.md §8).
    pub fn operator_call_op(&self, e: ExprId) -> crate::names::OverloadedOperator {
        let n = self.expect_kind(e, ExprKind::CXXOperatorCallExpr);
        crate::names::OverloadedOperator::from_u8(n.aux)
            .expect("invariant: the builder stores a valid OverloadedOperator in aux")
    }

    /// The operand of a `CXXRewrittenBinaryOperator`: its semantic form
    /// (spec/sema-overload.md §5.2).
    pub fn rewritten_semantic(&self, e: ExprId) -> ExprId {
        ExprId::new(self.expect_kind(e, ExprKind::CXXRewrittenBinaryOperator).a)
    }

    /// The written operator of a `CXXRewrittenBinaryOperator` and its list
    /// of the two operands as written.
    pub fn rewritten_written(&self, e: ExprId) -> (BinaryOp, ListId) {
        let n = self.expect_kind(e, ExprKind::CXXRewrittenBinaryOperator);
        (
            BinaryOp::from_u8(n.aux).expect("invariant: the builder stores a valid opcode in aux"),
            ListId::new(n.b),
        )
    }

    /// The implied object argument of a `CXXMemberCallExpr`: the base of
    /// its `MemberExpr` callee, or the left operand of a `.*`/`->*`
    /// callee (sema-class.md §5.1; for `->` a pointer). Parentheses
    /// around the callee, as in `(o.f)()` or `(o.*pm)()`, are looked
    /// through (clang's `getImplicitObjectArgument`).
    pub fn member_call_object(&self, e: ExprId) -> ExprId {
        let mut callee = self.get(ExprId::new(self.expect_kind(e, ExprKind::CXXMemberCallExpr).a));
        while callee.kind == ExprKind::ParenExpr {
            callee = self.get(ExprId::new(callee.a));
        }
        assert!(
            matches!(callee.kind, ExprKind::MemberExpr | ExprKind::BinaryOperator),
            "CXXMemberCallExpr callee is {:?}",
            callee.kind
        );
        ExprId::new(callee.a)
    }

    fn expect_construct(&self, e: ExprId) -> &ExprNode {
        let n = self.get(e);
        assert!(
            matches!(n.kind, ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr),
            "expected a construct expression, found {:?}",
            n.kind
        );
        n
    }

    /// The constructor called by a `CXXConstructExpr`,
    /// `CXXTemporaryObjectExpr` or `CXXInheritedCtorInitExpr`.
    pub fn constructor(&self, e: ExprId) -> DeclId {
        let n = self.get(e);
        assert!(
            matches!(
                n.kind,
                ExprKind::CXXConstructExpr
                    | ExprKind::CXXTemporaryObjectExpr
                    | ExprKind::CXXInheritedCtorInitExpr
            ),
            "{:?} calls no constructor",
            n.kind
        );
        DeclId::new(n.a)
    }

    /// The arguments of a `CXXConstructExpr` / `CXXTemporaryObjectExpr`.
    pub fn construct_args(&self, e: ExprId) -> ListId {
        ListId::new(self.expect_construct(e).b)
    }

    /// The flags of a `CXXConstructExpr` / `CXXTemporaryObjectExpr`.
    pub fn construct_info(&self, e: ExprId) -> ConstructInfo {
        ConstructInfo::from_aux(self.expect_construct(e).aux)
    }

    /// The construction kind of a `CXXInheritedCtorInitExpr` and whether the
    /// constructor is inherited from a virtual base.
    pub fn inherited_ctor_info(&self, e: ExprId) -> (ConstructionKind, bool) {
        let aux = self.expect_kind(e, ExprKind::CXXInheritedCtorInitExpr).aux;
        (
            ConstructionKind::from_u8(aux & 3)
                .expect("invariant: the builder encodes every ConstructionKind in 2 bits"),
            aux & 4 != 0,
        )
    }

    /// The storage duration of a `MaterializeTemporaryExpr`.
    pub fn temporary_storage(&self, e: ExprId) -> StorageDuration {
        let aux = self.expect_kind(e, ExprKind::MaterializeTemporaryExpr).aux;
        StorageDuration::from_u8(aux & 3)
            .expect("invariant: the builder encodes every StorageDuration in 2 bits")
    }

    /// True if a `MaterializeTemporaryExpr` is bound to an lvalue
    /// reference (it is then an lvalue, else an xvalue).
    pub fn bound_to_lvalue_ref(&self, e: ExprId) -> bool {
        self.expect_kind(e, ExprKind::MaterializeTemporaryExpr).aux & 4 != 0
    }

    /// The `LifetimeExtendedTemporary` declaration of a lifetime-extended
    /// `MaterializeTemporaryExpr` (its `extending` field is the extending
    /// declaration).
    pub fn lifetime_extended_temporary(&self, e: ExprId) -> Option<DeclId> {
        DeclId::from_raw(self.expect_kind(e, ExprKind::MaterializeTemporaryExpr).b)
    }

    /// The destructor of a `CXXBindTemporaryExpr`.
    pub fn bound_destructor(&self, e: ExprId) -> DeclId {
        DeclId::new(self.expect_kind(e, ExprKind::CXXBindTemporaryExpr).b)
    }

    /// True if the cleanups of an `ExprWithCleanups` have side effects.
    pub fn cleanups_have_side_effects(&self, e: ExprId) -> bool {
        self.expect_kind(e, ExprKind::ExprWithCleanups).aux & 1 != 0
    }

    /// The field of a `CXXDefaultInitExpr` (its initializer is the
    /// field's shared default member initializer).
    pub fn default_init_field(&self, e: ExprId) -> DeclId {
        DeclId::new(self.expect_kind(e, ExprKind::CXXDefaultInitExpr).a)
    }

    /// The parameter of a `CXXDefaultArgExpr` (its initializer is the
    /// parameter's shared default argument).
    pub fn default_arg_param(&self, e: ExprId) -> DeclId {
        DeclId::new(self.expect_kind(e, ExprKind::CXXDefaultArgExpr).a)
    }

    /// The default argument rebuilt for the `CXXDefaultArgExpr` `e`, if
    /// any ([`ColdExprData::default_arg_rewritten`]).
    pub fn default_arg_rewritten(&self, e: ExprId) -> Option<ExprId> {
        self.cold.default_arg_rewritten.get(e).copied()
    }

    /// Records `rewritten` as the default argument of the
    /// `CXXDefaultArgExpr` `e` (sema, in a lifetime-extending context).
    /// Write-once.
    ///
    /// # Errors
    /// If `e` already has one.
    pub fn set_default_arg_rewritten(
        &mut self,
        e: ExprId,
        rewritten: ExprId,
    ) -> Result<(), AlreadySet> {
        let _ = self.expect_kind(e, ExprKind::CXXDefaultArgExpr);
        self.cold.default_arg_rewritten.insert(e, rewritten)
    }

    /// A copy of `e` whose children ([`ExprArena::children`] order) are
    /// `kids`, with `e`'s side records and cold data, for rebuilding the
    /// path from a shared initializer down to the temporaries that must
    /// be fresh (spec/sema.md §7.6a: a default argument rebuilt for a
    /// lifetime-extending context). `None` for a kind it does not copy,
    /// or when `kids` does not match `e`'s children. A copied
    /// `MaterializeTemporaryExpr` keeps `e`'s storage: callers copy
    /// only full-expression ones.
    pub fn clone_with_children(
        &mut self,
        e: ExprId,
        lists: &mut IdLists,
        kids: &[ExprId],
    ) -> Option<ExprId> {
        let n = *self.get(e);
        let old = self.children(e, lists);
        if old.len() != kids.len() {
            return None;
        }
        let mut c = n;
        match n.kind {
            ExprKind::MemberExpr
            | ExprKind::UnaryOperator
            | ExprKind::ParenExpr
            | ExprKind::MaterializeTemporaryExpr
            | ExprKind::CXXBindTemporaryExpr
            | ExprKind::CXXStdInitializerListExpr => c.a = kids.first()?.raw(),
            k if k.is_cast() => c.a = kids.first()?.raw(),
            ExprKind::BinaryOperator | ExprKind::ArraySubscriptExpr => {
                c.a = kids.first()?.raw();
                c.b = kids.get(1)?.raw();
            }
            ExprKind::ConditionalOperator => {
                c.a = kids.first()?.raw();
                c.b = self.push_cond_arms(*kids.get(1)?, *kids.get(2)?);
            }
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                c.b = lists.push(kids).raw();
            }
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr => {
                let (args, callee) =
                    if n.a == 0 { (kids, None) } else { (&kids[1..], kids.first()) };
                c.a = callee.map_or(0, |x| x.raw());
                c.b = lists.push(args).raw();
            }
            ExprKind::CXXParenListInitExpr => {
                let (args, filler) = if n.b == 0 {
                    (kids, None)
                } else {
                    kids.split_last().map(|(f, a)| (a, Some(f)))?
                };
                c.a = lists.push(args).raw();
                c.b = filler.map_or(0, |x| x.raw());
            }
            ExprKind::InitListExpr => {
                let mut extra = self.init_lists[n.b as usize];
                let (inits, filler) = match extra.array_filler {
                    None => (kids, None),
                    Some(_) => kids.split_last().map(|(f, a)| (a, Some(*f)))?,
                };
                extra.array_filler = filler;
                c.a = lists.push(inits).raw();
                c.b = self.push_init_list(extra);
            }
            ExprKind::CXXDefaultArgExpr => {}
            _ => return None,
        }
        let id = self.push(c);
        let cold = &mut self.cold;
        if let Some(&l) = cold.end_loc.get(e) {
            let _ = cold.end_loc.insert(id, l);
        }
        if let Some(&x) = cold.member_extra.get(e) {
            let _ = cold.member_extra.insert(id, x);
        }
        if let Some(&x) = cold.source_loc.get(e) {
            let _ = cold.source_loc.insert(id, x);
        }
        if let Some(&x) = cold.union_field.get(e) {
            let _ = cold.union_field.insert(id, x);
        }
        if let Some(&x) = cold.cast_written.get(e) {
            let _ = cold.cast_written.insert(id, x);
        }
        if let Some(&x) = cold.trait_rhs_loc.get(e) {
            let _ = cold.trait_rhs_loc.insert(id, x);
        }
        Some(id)
    }

    /// The identifier a `PredefinedExpr` names.
    pub fn predefined_kind(&self, e: ExprId) -> PredefinedKind {
        PredefinedKind::from_aux(self.expect_kind(e, ExprKind::PredefinedExpr).aux)
    }

    /// The `StringLiteral` holding a `PredefinedExpr`'s value (clang
    /// `getFunctionName`); `None` in a dependent context.
    pub fn predefined_name(&self, e: ExprId) -> Option<ExprId> {
        ExprId::from_raw(self.expect_kind(e, ExprKind::PredefinedExpr).a)
    }

    /// The builtin a `SourceLocExpr` names.
    pub fn source_loc_kind(&self, e: ExprId) -> SourceLocKind {
        SourceLocKind::from_aux(self.expect_kind(e, ExprKind::SourceLocExpr).aux)
    }

    /// The declaration context a `SourceLocExpr` appears in (clang
    /// `getParentContext`).
    pub fn source_loc_context(&self, e: ExprId) -> DeclId {
        DeclId::new(self.expect_kind(e, ExprKind::SourceLocExpr).a)
    }

    /// The source-location values of `e` (a `SourceLocExpr`, or a default
    /// argument or member initializer use that holds one), if sema
    /// computed them.
    pub fn source_loc_values(&self, e: ExprId) -> Option<&SourceLocValues> {
        self.cold.source_loc.get(e)
    }

    /// Records the source-location values of `e` (once).
    ///
    /// # Errors
    /// If they are already recorded.
    pub fn set_source_loc_values(
        &mut self,
        e: ExprId,
        v: SourceLocValues,
    ) -> Result<(), AlreadySet> {
        self.cold.source_loc.insert(e, v)
    }

    /// The value sema found for immediate invocation `e` of a default
    /// argument or default member initializer at its uses without
    /// source-location values (`ColdExprData::default_imm`).
    pub fn default_immediate_value(&self, e: ExprId) -> Option<ConstValueId> {
        self.cold.default_imm.get(e).copied()
    }

    /// Records [`ExprArena::default_immediate_value`] (the first one wins:
    /// every such use evaluates the same expression in the same context).
    pub fn set_default_immediate_value(&mut self, e: ExprId, v: ConstValueId) {
        let _ = self.cold.default_imm.insert(e, v);
    }

    /// Replaces the recorded source-location values of `e`: a function's
    /// pretty name is final only once its return type is deduced (clang
    /// computes `SourceLocExpr` values lazily, after the body). `false`
    /// if none were recorded.
    pub fn refine_source_loc_values(&mut self, e: ExprId, v: SourceLocValues) -> bool {
        self.cold.source_loc.refine(e, v)
    }

    /// The declaration context in which a `CXXDefaultInitExpr` /
    /// `CXXDefaultArgExpr` is used.
    pub fn used_context(&self, e: ExprId) -> DeclId {
        let n = self.get(e);
        assert!(
            matches!(n.kind, ExprKind::CXXDefaultInitExpr | ExprKind::CXXDefaultArgExpr),
            "{:?} has no used context",
            n.kind
        );
        DeclId::new(n.b)
    }

    /// True if a `CXXThisExpr` is implicit (a member named without `this->`).
    pub fn this_is_implicit(&self, e: ExprId) -> bool {
        self.expect_kind(e, ExprKind::CXXThisExpr).aux & 1 != 0
    }

    /// The operation of an `AtomicExpr`: its builtin id
    /// ([`crate::decl::builtin::atomic`]).
    pub fn atomic_op(&self, e: ExprId) -> u32 {
        u32::from(self.expect_kind(e, ExprKind::AtomicExpr).aux)
    }

    /// The operands of an `AtomicExpr` in clang's order: `ptr`, `order`,
    /// then (as the form has them) `val1`, `order_fail`, `val2`, `weak`.
    pub fn atomic_subs(&self, e: ExprId) -> ListId {
        ListId::new(self.expect_kind(e, ExprKind::AtomicExpr).a)
    }

    /// The operands of a `ShuffleVectorExpr`: the two vectors, then the
    /// indices, as written.
    pub fn shuffle_subs(&self, e: ExprId) -> ListId {
        ListId::new(self.expect_kind(e, ExprKind::ShuffleVectorExpr).a)
    }

    /// The folded indices of a `ShuffleVectorExpr` (`u32::MAX` for a
    /// poison lane), `None` while one is value-dependent.
    pub fn shuffle_mask<'l>(&self, e: ExprId, lists: &'l IdLists) -> Option<&'l [u32]> {
        let n = self.expect_kind(e, ExprKind::ShuffleVectorExpr);
        (n.aux & 1 != 0).then(|| lists.get(ListId::new(n.b)))
    }

    /// The explicit initializers of a `CXXParenListInitExpr`.
    pub fn paren_list_inits(&self, e: ExprId) -> ListId {
        ListId::new(self.expect_kind(e, ExprKind::CXXParenListInitExpr).a)
    }

    /// The array filler of a `CXXParenListInitExpr`.
    pub fn paren_list_filler(&self, e: ExprId) -> Option<ExprId> {
        ExprId::from_raw(self.expect_kind(e, ExprKind::CXXParenListInitExpr).b)
    }

    /// The initialized member of a union's `CXXParenListInitExpr`.
    pub fn paren_list_union_field(&self, e: ExprId) -> Option<DeclId> {
        self.expect_kind(e, ExprKind::CXXParenListInitExpr);
        self.cold.union_field.get(e).copied()
    }

    /// The common expression (an `OpaqueValueExpr`) and element
    /// initializer of an `ArrayInitLoopExpr`.
    pub fn array_init_loop(&self, e: ExprId) -> [ExprId; 2] {
        let n = self.expect_kind(e, ExprKind::ArrayInitLoopExpr);
        [ExprId::new(n.a), ExprId::new(n.b)]
    }

    /// The base and updater of a `DesignatedInitUpdateExpr`.
    pub fn designated_init_update(&self, e: ExprId) -> [ExprId; 2] {
        let n = self.expect_kind(e, ExprKind::DesignatedInitUpdateExpr);
        [ExprId::new(n.a), ExprId::new(n.b)]
    }

    /// The source expression of an `OpaqueValueExpr` (shared: not one of
    /// its [`ExprArena::children`]).
    pub fn opaque_source(&self, e: ExprId) -> Option<ExprId> {
        ExprId::from_raw(self.expect_kind(e, ExprKind::OpaqueValueExpr).a)
    }

    /// The operand of a `CXXTypeidExpr`.
    pub fn typeid_operand(&self, e: ExprId) -> TypeidOperand {
        let n = self.expect_kind(e, ExprKind::CXXTypeidExpr);
        if n.has(ExprFlags::ARG_IS_TYPE) {
            TypeidOperand::Type(
                QualType::from_raw(n.a)
                    .expect("invariant: an ARG_IS_TYPE typeid stores a QualType (never 0) in a"),
            )
        } else {
            TypeidOperand::Expr(ExprId::new(n.a))
        }
    }

    /// True if the operand of a `CXXTypeidExpr` is potentially evaluated
    /// (a glvalue of polymorphic class type, [expr.typeid]/4).
    pub fn typeid_potentially_evaluated(&self, e: ExprId) -> bool {
        self.expect_kind(e, ExprKind::CXXTypeidExpr).aux & 1 != 0
    }

    /// The initializer of a `DesignatedInitExpr`.
    pub fn designated_init(&self, e: ExprId) -> ExprId {
        ExprId::new(self.expect_kind(e, ExprKind::DesignatedInitExpr).a)
    }

    /// The designators of a `DesignatedInitExpr`, outermost first.
    pub fn designators(&self, e: ExprId) -> &[Designator] {
        self.expect_kind(e, ExprKind::DesignatedInitExpr);
        self.cold.designators.get(e).map_or(&[], |d| &d[..])
    }

    /// The index expressions of a `DesignatedInitExpr`'s array
    /// designators, in order (both bounds of a range).
    pub fn designated_indices(&self, e: ExprId) -> impl Iterator<Item = ExprId> + '_ {
        self.cold.designators.get(e).into_iter().flat_map(|d| {
            d.iter()
                .flat_map(|x| match *x {
                    Designator::Index { index, .. } => [Some(index), None],
                    Designator::Range { first, last, .. } => [Some(first), Some(last)],
                    Designator::Field { .. } => [None, None],
                })
                .flatten()
        })
    }

    /// The written form of a semantic `InitListExpr` built in a template
    /// pattern from a list with designators (§5.6).
    pub fn syntactic_list(&self, e: ExprId) -> Option<ExprId> {
        self.cold.syntactic_list.get(e).copied()
    }

    /// Records `syntactic` as the written form of `semantic` (§5.6).
    pub fn set_syntactic_list(&mut self, semantic: ExprId, syntactic: ExprId) {
        let _ = self.cold.syntactic_list.insert(semantic, syntactic);
    }

    /// The explicit initializers of an `InitListExpr` (a `ListId` of
    /// `ExprId`s; the filler is in [`ExprArena::init_list`]).
    pub fn init_list_inits(&self, e: ExprId) -> ListId {
        ListId::new(self.expect_kind(e, ExprKind::InitListExpr).a)
    }

    /// The kind of a `UnaryExprOrTypeTraitExpr`.
    pub fn trait_kind(&self, e: ExprId) -> TraitKind {
        TraitKind::from_u8(self.expect_kind(e, ExprKind::UnaryExprOrTypeTraitExpr).aux)
            .expect("invariant: the builder stores a valid TraitKind in aux")
    }

    /// The type operand of a `UnaryExprOrTypeTraitExpr` (`None` for an
    /// expression operand, which is its only child).
    pub fn trait_type_arg(&self, e: ExprId) -> Option<QualType> {
        let n = self.expect_kind(e, ExprKind::UnaryExprOrTypeTraitExpr);
        n.has(ExprFlags::ARG_IS_TYPE).then(|| QualType::from_raw(n.a)).flatten()
    }

    /// The `va_list` operand, the written type and the written type's
    /// begin location of a `VAArgExpr`.
    pub fn va_arg(&self, e: ExprId) -> (ExprId, QualType, Loc) {
        let n = self.expect_kind(e, ExprKind::VAArgExpr);
        let ty_loc = self.cold.trait_rhs_loc.get(e).copied().unwrap_or(n.loc);
        (
            ExprId::new(n.a),
            QualType::from_raw(n.b)
                .expect("invariant: a VAArgExpr stores its written QualType (never 0) in b"),
            ty_loc,
        )
    }

    /// The (unevaluated) operand of a `CXXNoexceptExpr`.
    pub fn noexcept_operand(&self, e: ExprId) -> ExprId {
        ExprId::new(self.expect_kind(e, ExprKind::CXXNoexceptExpr).a)
    }

    /// The value of a `CXXNoexceptExpr`: `true` unless its operand is
    /// potentially-throwing ([expr.unary.noexcept]/3).
    pub fn noexcept_value(&self, e: ExprId) -> bool {
        self.expect_kind(e, ExprKind::CXXNoexceptExpr).b != 0
    }

    /// The trait of a `TypeTraitExpr`, `ArrayTypeTraitExpr` or
    /// `ExpressionTraitExpr`.
    pub fn type_trait(&self, e: ExprId) -> TypeTrait {
        let n = self.get(e);
        debug_assert!(matches!(
            n.kind,
            ExprKind::TypeTraitExpr | ExprKind::ArrayTypeTraitExpr | ExprKind::ExpressionTraitExpr
        ));
        TypeTrait::from_u8(n.aux).expect("invariant: the builder stores a valid TypeTrait in aux")
    }

    /// The type operands of a `TypeTraitExpr` (a `ListId` of `QualType`s).
    pub fn type_trait_args(&self, e: ExprId) -> ListId {
        ListId::new(self.expect_kind(e, ExprKind::TypeTraitExpr).a)
    }

    /// The location of the second type operand of a binary
    /// `TypeTraitExpr` (where `__is_base_of`'s incomplete-type error
    /// points).
    pub fn type_trait_rhs_loc(&self, e: ExprId) -> Option<Loc> {
        self.expect_kind(e, ExprKind::TypeTraitExpr);
        self.cold.trait_rhs_loc.get(e).copied()
    }

    /// The operand of an `ExpressionTraitExpr`.
    pub fn expression_trait_operand(&self, e: ExprId) -> ExprId {
        ExprId::new(self.expect_kind(e, ExprKind::ExpressionTraitExpr).a)
    }

    /// The value of a `TypeTraitExpr` or `ExpressionTraitExpr` (`false`
    /// while value-dependent).
    pub fn type_trait_value(&self, e: ExprId) -> bool {
        let n = self.get(e);
        debug_assert!(matches!(n.kind, ExprKind::TypeTraitExpr | ExprKind::ExpressionTraitExpr));
        n.b != 0
    }

    /// The operand of a `CXXReflectExpr` as written (spec/reflection-repr.md
    /// §2): decoded from the node, so it is the dependent operand while
    /// the expression is value-dependent. A `BaseSpec` operand (only ever
    /// built from an interned value) reads as `Null` here; use
    /// [`crate::Ast::reflect_operand`] for the interned entity.
    pub fn reflect_operand(&self, e: ExprId) -> crate::reflect::Reflectee {
        let n = self.expect_kind(e, ExprKind::CXXReflectExpr);
        crate::reflect::Reflectee::decode(n.aux, n.a).unwrap_or(crate::reflect::Reflectee::Null)
    }

    /// The dependent qualified name a `CXXReflectExpr` reflects (`^^T::x`
    /// before instantiation), if that is its operand.
    pub fn reflect_dependent_name(&self, e: ExprId) -> Option<ExprId> {
        let n = self.expect_kind(e, ExprKind::CXXReflectExpr);
        (n.aux == crate::reflect::Reflectee::DEPENDENT_NAME_TAG).then(|| ExprId::new(n.a))
    }

    /// The value of a `CXXReflectExpr`: `None` while dependent.
    pub fn reflect_value(&self, e: ExprId) -> Option<crate::ids::ReflectionId> {
        let n = self.expect_kind(e, ExprKind::CXXReflectExpr);
        crate::ids::ReflectionId::try_new(n.b)
    }

    /// The operand of a `CXXSpliceExpr`.
    pub fn splice_operand(&self, e: ExprId) -> ExprId {
        ExprId::new(self.expect_kind(e, ExprKind::CXXSpliceExpr).a)
    }

    /// The template arguments of a `CXXSpliceExpr` written as a
    /// template-id (`template [: r :] < args >`); `None` otherwise.
    pub fn splice_template_args(&self, e: ExprId) -> Option<ArgListId> {
        self.expect_kind(e, ExprKind::CXXSpliceExpr);
        self.cold.splice_args.get(e).copied()
    }

    /// The object expression of a member `CXXSpliceExpr` and whether it
    /// was written with `->`; `None` for an expression splice.
    pub fn splice_base(&self, e: ExprId) -> Option<(ExprId, bool)> {
        let n = self.expect_kind(e, ExprKind::CXXSpliceExpr);
        ExprId::try_new(n.b).map(|b| (b, n.aux & 1 != 0))
    }

    /// The queried type of an `ArrayTypeTraitExpr`.
    pub fn array_trait_type(&self, e: ExprId) -> QualType {
        QualType::from_raw(self.expect_kind(e, ExprKind::ArrayTypeTraitExpr).a)
            .expect("invariant: an ArrayTypeTraitExpr stores its QualType (never 0) in a")
    }

    /// The cold record of an `ArrayTypeTraitExpr`.
    pub fn array_trait(&self, e: ExprId) -> ArrayTraitExtra {
        self.expect_kind(e, ExprKind::ArrayTypeTraitExpr);
        self.cold.array_trait.get(e).copied().unwrap_or(ArrayTraitExtra { dim: None, value: 0 })
    }

    /// The side record of a `CXXNewExpr`.
    pub fn new_extra(&self, e: ExprId) -> &NewExtra {
        &self.news[self.expect_kind(e, ExprKind::CXXNewExpr).a as usize]
    }

    /// The initialization style of a `CXXNewExpr`.
    pub fn new_init_style(&self, e: ExprId) -> NewInitStyle {
        NewInitStyle::from_u8(self.expect_kind(e, ExprKind::CXXNewExpr).aux)
            .expect("invariant: the builder stores a valid NewInitStyle in aux")
    }

    /// The operand of a `CXXDeleteExpr` (converted to a pointer prvalue).
    pub fn delete_operand(&self, e: ExprId) -> ExprId {
        ExprId::new(self.expect_kind(e, ExprKind::CXXDeleteExpr).a)
    }

    /// The deallocation function of a `CXXDeleteExpr` (`None` when the
    /// operand is erroneous).
    pub fn operator_delete(&self, e: ExprId) -> Option<DeclId> {
        DeclId::from_raw(self.expect_kind(e, ExprKind::CXXDeleteExpr).b)
    }

    /// Whether a `CXXDeleteExpr`'s deallocation function takes the size
    /// (aux bit 0) and the alignment (aux bit 1).
    pub fn delete_passes(&self, e: ExprId) -> (bool, bool) {
        let aux = self.expect_kind(e, ExprKind::CXXDeleteExpr).aux;
        (aux & 1 != 0, aux & 2 != 0)
    }

    /// The single operand of a unary operator, cast, `ParenExpr`,
    /// `ConstantExpr`, `MemberExpr` (its base), `MaterializeTemporaryExpr`,
    /// `CXXBindTemporaryExpr`, `CXXStdInitializerListExpr` (its backing
    /// array), `ExprWithCleanups`, `PackExpansionExpr` (its pattern) or
    /// `ImaginaryLiteral` (its real literal).
    pub fn operand(&self, e: ExprId) -> ExprId {
        let n = self.get(e);
        assert!(
            n.kind.is_cast()
                || matches!(
                    n.kind,
                    ExprKind::UnaryOperator
                        | ExprKind::ParenExpr
                        | ExprKind::ConstantExpr
                        | ExprKind::MemberExpr
                        | ExprKind::MaterializeTemporaryExpr
                        | ExprKind::CXXBindTemporaryExpr
                        | ExprKind::CXXStdInitializerListExpr
                        | ExprKind::ExprWithCleanups
                        | ExprKind::ConvertVectorExpr
                        | ExprKind::ExtVectorElementExpr
                        | ExprKind::PackExpansionExpr
                        | ExprKind::ImaginaryLiteral
                ),
            "{:?} has no single operand",
            n.kind
        );
        ExprId::new(n.a)
    }

    /// The operands of a `BinaryOperator` or `ArraySubscriptExpr` (for a
    /// `CompoundAssignOperator` the right operand is in its side record).
    pub fn operands(&self, e: ExprId) -> [ExprId; 2] {
        let n = self.get(e);
        assert!(
            matches!(n.kind, ExprKind::BinaryOperator | ExprKind::ArraySubscriptExpr),
            "{:?} is not a binary expression",
            n.kind
        );
        [ExprId::new(n.a), ExprId::new(n.b)]
    }

    /// The replacement expression of a `SubstNonTypeTemplateParmExpr`.
    pub fn subst_replacement(&self, e: ExprId) -> ExprId {
        ExprId::new(self.expect_kind(e, ExprKind::SubstNonTypeTemplateParmExpr).b)
    }

    /// The `NonTypeTemplateParmDecl` a `SubstNonTypeTemplateParmExpr`
    /// replaced.
    pub fn subst_parameter(&self, e: ExprId) -> DeclId {
        DeclId::new(self.expect_kind(e, ExprKind::SubstNonTypeTemplateParmExpr).a)
    }

    /// The parameter pack of a `SubstNonTypeTemplateParmPackExpr`.
    pub fn subst_pack_parameter(&self, e: ExprId) -> DeclId {
        DeclId::new(self.expect_kind(e, ExprKind::SubstNonTypeTemplateParmPackExpr).a)
    }

    /// The argument pack of a `SubstNonTypeTemplateParmPackExpr`.
    pub fn subst_pack_args(&self, e: ExprId) -> ArgListId {
        let b = self.expect_kind(e, ExprKind::SubstNonTypeTemplateParmPackExpr).b;
        ArgListId::try_new(b).unwrap_or(ArgListId::EMPTY)
    }

    /// The pattern's parameter pack of a `FunctionParmPackExpr`.
    pub fn parm_pack_parameter(&self, e: ExprId) -> DeclId {
        DeclId::new(self.expect_kind(e, ExprKind::FunctionParmPackExpr).a)
    }

    /// The expanded `ParmVar`s (a list of `DeclId`) of a
    /// `FunctionParmPackExpr`.
    pub fn parm_pack_params(&self, e: ExprId) -> ListId {
        let b = self.expect_kind(e, ExprKind::FunctionParmPackExpr).b;
        ListId::from_raw(b).unwrap_or(ListId::EMPTY)
    }

    /// The cached value of a `ConstantExpr`.
    pub fn constant_value(&self, e: ExprId) -> Option<ConstValueId> {
        ConstValueId::from_raw(self.expect_kind(e, ExprKind::ConstantExpr).b)
    }

    /// Stores the value of a `ConstantExpr` (once; sema, from
    /// `Oven::evaluate`, §5.5).
    ///
    /// # Panics
    /// If `e` is not a `ConstantExpr`.
    pub fn set_constant_value(&mut self, e: ExprId, v: ConstValueId) -> Result<(), AlreadySet> {
        let n = &mut self.nodes[e.index() as usize];
        assert!(n.kind == ExprKind::ConstantExpr, "expected ConstantExpr, found {:?}", n.kind);
        assert!(!n.deps.contains(ExprDeps::VALUE), "value-dependent ConstantExpr has no value");
        if n.b != 0 {
            return Err(AlreadySet { slot: "ConstantExpr.value", existing: n.b });
        }
        n.b = v.raw();
        Ok(())
    }

    /// Whether the `ConstantExpr` `e` is an immediate invocation nested in
    /// another one: clang removes its wrapper ([expr.const]/16; the dump
    /// shows the operand in its place).
    pub fn constant_elided(&self, e: ExprId) -> bool {
        self.expect_kind(e, ExprKind::ConstantExpr).aux & CONSTANT_ELIDED != 0
    }

    /// Whether the cached value of the `ConstantExpr` `e` is kiln's own:
    /// clang leaves the immediate invocations of a constant-initialized
    /// variable's initializer unevaluated (the dump omits the value).
    pub fn constant_value_hidden(&self, e: ExprId) -> bool {
        self.expect_kind(e, ExprKind::ConstantExpr).aux & CONSTANT_VALUE_HIDDEN != 0
    }

    /// Marks `e` instantiation-dependent: a source-location value naming
    /// the enclosing function in a dependent context (clang's
    /// `SourceLocExpr` and the used-context rebuild of `CXXDefaultArgExpr`)
    /// is recomputed when the template is instantiated. Call it before
    /// any parent of `e` is built (parents copy their children's bits).
    pub fn mark_instantiation_dependent(&mut self, e: ExprId) {
        self.nodes[e.index() as usize].deps |= ExprDeps::INSTANTIATION;
    }

    /// Marks `e` type-, value- and instantiation-dependent: a variable
    /// captured by copy in a lambda whose explicit object parameter has a
    /// dependent type ([temp.dep.expr]/3, P0847) has a type that depends
    /// on the specialization. Call it before any parent of `e` is built.
    pub fn mark_type_dependent(&mut self, e: ExprId) {
        self.nodes[e.index() as usize].deps |= ExprDeps::TYPE_VALUE_INST;
    }

    /// Marks the `DeclRefExpr` or construction `e` as immediate-escalating
    /// (P2564): it made the enclosing function immediate.
    ///
    /// # Panics
    /// If `e` is neither a `DeclRefExpr` nor a construction.
    pub fn set_immediate_escalating(&mut self, e: ExprId) {
        let n = &mut self.nodes[e.index() as usize];
        match n.kind {
            ExprKind::DeclRefExpr => n.bits |= ExprFlags::IMMEDIATE_ESCALATING.bits(),
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                n.aux |= 1 << 5;
            }
            k => panic!("invariant: only a DeclRefExpr or a construction escalates, found {k:?}"),
        }
    }

    /// Marks the `ConstantExpr` `e` as a nested immediate invocation
    /// ([`ExprArena::constant_elided`]).
    ///
    /// # Panics
    /// If `e` is not a `ConstantExpr`.
    pub fn set_constant_elided(&mut self, e: ExprId) {
        let n = &mut self.nodes[e.index() as usize];
        assert!(n.kind == ExprKind::ConstantExpr, "expected ConstantExpr, found {:?}", n.kind);
        n.aux |= CONSTANT_ELIDED;
    }

    /// Marks the value of the `ConstantExpr` `e` as kiln's own
    /// ([`ExprArena::constant_value_hidden`]).
    ///
    /// # Panics
    /// If `e` is not a `ConstantExpr`.
    pub fn set_constant_value_hidden(&mut self, e: ExprId) {
        let n = &mut self.nodes[e.index() as usize];
        assert!(n.kind == ExprKind::ConstantExpr, "expected ConstantExpr, found {:?}", n.kind);
        n.aux |= CONSTANT_VALUE_HIDDEN;
    }

    /// The satisfaction of a non-dependent `ConceptSpecializationExpr` or
    /// `RequiresExpr` (spec/sema-templates.md §8.1, §8.5; kept in `aux`:
    /// 0 not computed, 1 false, 2 true). `None` while dependent or not yet
    /// checked.
    pub fn satisfied(&self, e: ExprId) -> Option<bool> {
        let n = self.get(e);
        debug_assert!(matches!(
            n.kind,
            ExprKind::ConceptSpecializationExpr | ExprKind::RequiresExpr
        ));
        match n.aux {
            1 => Some(false),
            2 => Some(true),
            _ => None,
        }
    }

    /// Records the satisfaction of a `ConceptSpecializationExpr` or
    /// `RequiresExpr` (write-once, spec/ast.md §8.4).
    ///
    /// # Errors
    /// If a value was already recorded.
    ///
    /// # Panics
    /// If `e` is neither kind.
    pub fn set_satisfied(&mut self, e: ExprId, v: bool) -> Result<(), AlreadySet> {
        let n = &mut self.nodes[e.index() as usize];
        assert!(
            matches!(n.kind, ExprKind::ConceptSpecializationExpr | ExprKind::RequiresExpr),
            "expected a constraint expression, found {:?}",
            n.kind
        );
        if n.aux != 0 {
            return Err(AlreadySet { slot: "satisfied", existing: u32::from(n.aux) });
        }
        n.aux = if v { 2 } else { 1 };
        Ok(())
    }

    /// The object, `->` flag and destroyed type of a
    /// `CXXPseudoDestructorExpr`.
    pub fn pseudo_destructor(&self, e: ExprId) -> (ExprId, bool, QualType) {
        let n = self.expect_kind(e, ExprKind::CXXPseudoDestructorExpr);
        (
            ExprId::new(n.a),
            n.aux & 1 != 0,
            QualType::from_raw(n.b).expect(
                "invariant: a CXXPseudoDestructorExpr stores its destroyed QualType (never 0) in b",
            ),
        )
    }

    /// True if the destructor name of a `CXXPseudoDestructorExpr` was
    /// qualified (`p->T::~T`; [`crate::builder::AstBuilder::pseudo_destructor_q`]).
    pub fn pseudo_destructor_qualified(&self, e: ExprId) -> bool {
        self.expect_kind(e, ExprKind::CXXPseudoDestructorExpr).aux & 2 != 0
    }

    /// The closing-token location of `e`, if stored.
    pub fn end_loc(&self, e: ExprId) -> Option<Loc> {
        let n = self.get(e);
        match n.kind {
            ExprKind::ParenExpr => Some(Loc::from_raw(n.b)),
            ExprKind::InitListExpr => Some(self.init_lists[n.b as usize].rbrace),
            _ => self.cold.end_loc.get(e).copied(),
        }
    }
    /// Whether `e`, a `return` operand, is erroneous in itself (clang
    /// 22 drops such a `return`, `ActOnReturnStmt` getting `ExprError`),
    /// as opposed to a valid operand whose conversion to the return type
    /// failed (a `RecoveryExpr` over error-free operands, which clang keeps
    /// and which fails evaluation without a note).
    pub fn is_erroneous_operand(&self, e: ExprId, lists: &IdLists) -> bool {
        let n = self.get(e);
        if n.kind() != ExprKind::RecoveryExpr {
            return n.deps().contains(ExprDeps::ERROR);
        }
        let kids = self.children(e, lists);
        kids.is_empty()
            || kids.iter().any(|&c| {
                let k = self.get(c);
                k.kind() == ExprKind::RecoveryExpr || k.deps().contains(ExprDeps::ERROR)
            })
    }

    /// The children of `PseudoObjectExpr` `n`: its semantic expressions,
    /// each bound `OpaqueValueExpr` by its source (so every subexpression
    /// is visited once); the syntactic form shares the sources and is not
    /// a child.
    fn pseudo_children(&self, n: &ExprNode, lists: &IdLists, v: &mut impl Extend<ExprId>) {
        v.extend(lists.iter::<ExprId>(ListId::new(n.a)).skip(1).map(|c| {
            let src = (self.get(c).kind == ExprKind::OpaqueValueExpr)
                .then(|| ExprId::from_raw(self.get(c).a))
                .flatten();
            src.unwrap_or(c)
        }));
    }

    /// The child expressions of `e` in dump order (§11): the expressions
    /// `e` owns, as clang's `Stmt::children`. Shared subexpressions are
    /// not children (the dump prints them, §11): the source of an
    /// `OpaqueValueExpr`, the default member initializer of a
    /// `CXXDefaultInitExpr` and the default argument of a
    /// `CXXDefaultArgExpr`.
    pub fn children(&self, e: ExprId, lists: &IdLists) -> Vec<ExprId> {
        let mut v = Vec::new();
        self.children_into(e, lists, &mut v);
        v
    }

    /// [`ExprArena::children`], appended to `v` (no allocation once `v`
    /// has grown: the CFG builder's walk, spec/sema.md §7.5).
    pub fn children_into(&self, e: ExprId, lists: &IdLists, v: &mut Vec<ExprId>) {
        self.children_each(e, lists, v);
    }

    /// The first of [`ExprArena::children`], allocating nothing (WP-645).
    pub fn first_child(&self, e: ExprId, lists: &IdLists) -> Option<ExprId> {
        let mut first = FirstChild(None);
        self.children_each(e, lists, &mut first);
        first.0
    }

    /// A `GenericSelectionExpr`'s children: the controlling expression
    /// (not a type operand), then every association's expression.
    fn generic_children(n: &ExprNode, lists: &IdLists, v: &mut impl Extend<ExprId>) {
        let w = lists.get(ListId::new(n.a));
        if !n.has(ExprFlags::ARG_IS_TYPE) {
            v.extend(ExprId::from_raw(w[0]));
        }
        v.extend(w[2..].chunks_exact(3).filter_map(|c| ExprId::from_raw(c[2])));
    }

    /// [`ExprArena::children`] fed to `v` in order.
    fn children_each(&self, e: ExprId, lists: &IdLists, v: &mut impl Extend<ExprId>) {
        let n = self.get(e);
        let id = |w: u32| ExprId::from_raw(w);
        match n.kind {
            ExprKind::MemberExpr
            | ExprKind::UnaryOperator
            | ExprKind::ParenExpr
            | ExprKind::ConstantExpr
            | ExprKind::MaterializeTemporaryExpr
            | ExprKind::CXXBindTemporaryExpr
            | ExprKind::CXXStdInitializerListExpr
            | ExprKind::ExprWithCleanups
            | ExprKind::ConvertVectorExpr
            | ExprKind::ExtVectorElementExpr
            | ExprKind::ImaginaryLiteral
            | ExprKind::PredefinedExpr => v.extend(id(n.a)),
            ExprKind::CXXTypeidExpr if !n.has(ExprFlags::ARG_IS_TYPE) => v.extend(id(n.a)),
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                v.extend(lists.iter::<ExprId>(ListId::new(n.b)));
            }
            ExprKind::CXXParenListInitExpr => {
                v.extend(lists.iter::<ExprId>(ListId::new(n.a)));
                v.extend(id(n.b));
            }
            ExprKind::ArrayInitLoopExpr | ExprKind::DesignatedInitUpdateExpr => {
                v.extend(id(n.a));
                v.extend(id(n.b));
            }
            ExprKind::SubstNonTypeTemplateParmExpr => v.extend(id(n.b)),
            ExprKind::CXXSpliceExpr => v.extend([n.b, n.a].into_iter().filter_map(id)),
            ExprKind::CXXReflectExpr if n.aux == REFLECT_DEPENDENT_NAME => v.extend(id(n.a)),
            k if k.is_cast() => v.extend(id(n.a)),
            ExprKind::UnaryExprOrTypeTraitExpr if !n.has(ExprFlags::ARG_IS_TYPE) => {
                v.extend(id(n.a));
            }
            // `CXXDependentScopeMemberExpr` and `PackExpansionExpr` are
            // template expressions (spec/sema-templates.md §2.4).
            ExprKind::CXXNoexceptExpr
            | ExprKind::VAArgExpr
            | ExprKind::ExpressionTraitExpr
            | ExprKind::CXXThrowExpr
            | ExprKind::CompoundLiteralExpr
            | ExprKind::CXXDeleteExpr
            | ExprKind::CXXDependentScopeMemberExpr
            | ExprKind::CXXPseudoDestructorExpr
            | ExprKind::PackExpansionExpr
            | ExprKind::DependentCoawaitExpr
            | ExprKind::CXXRewrittenBinaryOperator => v.extend(id(n.a)),
            ExprKind::BinaryOperator | ExprKind::ArraySubscriptExpr | ExprKind::CXXFoldExpr => {
                v.extend(id(n.a));
                v.extend(id(n.b));
            }
            ExprKind::CompoundAssignOperator => {
                v.extend(id(n.a));
                v.extend([self.compound_assigns[n.b as usize].rhs]);
            }
            ExprKind::ConditionalOperator | ExprKind::ChooseExpr => {
                v.extend(id(n.a));
                v.extend(self.cond_arms[n.b as usize]);
            }
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr => {
                v.extend(id(n.a));
                v.extend(lists.iter::<ExprId>(ListId::new(n.b)));
            }
            ExprKind::InitListExpr => {
                v.extend(lists.iter::<ExprId>(ListId::new(n.a)));
                v.extend(self.init_lists[n.b as usize].array_filler);
            }
            ExprKind::DesignatedInitExpr => {
                v.extend(id(n.a));
                v.extend(self.designated_indices(e));
            }
            ExprKind::RecoveryExpr => v.extend(lists.iter::<ExprId>(ListId::new(n.a))),
            ExprKind::LambdaExpr => {
                if let Some(l) = self.lambdas.get(LambdaId::new(n.a)).inits {
                    v.extend(lists.iter::<ExprId>(l));
                }
            }
            ExprKind::CXXNewExpr => {
                let x = &self.news[n.a as usize];
                v.extend(x.array_size);
                v.extend(x.init);
                if let Some(p) = x.placement {
                    v.extend(lists.iter::<ExprId>(p));
                }
            }
            // `CXXUnresolvedConstructExpr` and `ParenListExpr` are template
            // expressions (spec/sema-templates.md §2.4).
            // The operand, the awaiter and the three calls; the
            // `OpaqueValueExpr` (sixth) is shared.
            ExprKind::CoawaitExpr | ExprKind::CoyieldExpr => {
                v.extend(lists.iter::<ExprId>(ListId::new(n.a)).take(5));
            }
            ExprKind::PseudoObjectExpr => self.pseudo_children(n, lists, v),
            ExprKind::GenericSelectionExpr => Self::generic_children(n, lists, v),
            ExprKind::BinaryConditionalOperator
            | ExprKind::OffsetOfExpr
            | ExprKind::AtomicExpr
            | ExprKind::ShuffleVectorExpr
            | ExprKind::CXXUnresolvedConstructExpr
            | ExprKind::ParenListExpr => {
                v.extend(lists.iter::<ExprId>(ListId::new(n.a)));
            }
            _ => {}
        }
    }

    pub(crate) fn live_bytes(&self) -> u64 {
        self.vec_rows().iter().map(|r| r.live_bytes()).sum()
    }

    fn vec_rows(&self) -> [VecDims; 6] {
        [
            VecDims::of("nodes", &self.nodes),
            VecDims::of("cond_arms", &self.cond_arms),
            VecDims::of("declref_extras", &self.declref_extras),
            VecDims::of("compound_assigns", &self.compound_assigns),
            VecDims::of("init_lists", &self.init_lists),
            VecDims::of("news", &self.news),
        ]
    }
}

/// The [`Extend`] sink of [`ExprArena::first_child`]: keeps the first
/// element it is given and pulls no more.
struct FirstChild(Option<ExprId>);

impl Extend<ExprId> for FirstChild {
    fn extend<I: IntoIterator<Item = ExprId>>(&mut self, it: I) {
        if self.0.is_none() {
            self.0 = it.into_iter().next();
        }
    }
}

impl HeapSize for ExprArena {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        for row in self.vec_rows() {
            r.push(row.row());
        }
        r.push(self.lambdas.mem_row());
        r.push(self.cold.end_loc.mem_row().prefixed("cold"));
        r.push(self.cold.member_extra.mem_row().prefixed("cold"));
        r.push(self.cold.designators.mem_row().prefixed("cold"));
        r.push(self.cold.syntactic_list.mem_row().prefixed("cold"));
        r.push(self.cold.array_trait.mem_row().prefixed("cold"));
        r.push(self.cold.trait_rhs_loc.mem_row().prefixed("cold"));
        r.push(self.cold.source_loc.mem_row().prefixed("cold"));
        r.push(self.cold.union_field.mem_row().prefixed("cold"));
        r.push(self.cold.default_imm.mem_row().prefixed("cold"));
        r.push(self.cold.cast_written.mem_row().prefixed("cold"));
        r.push(self.cold.offsetof_pattern.mem_row().prefixed("cold"));
        r.push(self.cold.splice_args.mem_row().prefixed("cold"));
        r.push(self.cold.default_arg_rewritten.mem_row().prefixed("cold"));
        r
    }
}

/// The resolved parts of a `CoawaitExpr`/`CoyieldExpr` (clang's
/// `CoroutineSuspendExpr`): `common` is the awaiter, bound by `opaque`
/// (an lvalue `OpaqueValueExpr`) in the three calls.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CoawaitParts {
    /// The operand as written (for `co_yield`, the yielded expression).
    pub operand: ExprId,
    /// The awaiter (a glvalue).
    pub common: ExprId,
    /// `awaiter.await_ready()`, contextually converted to `bool`.
    pub ready: ExprId,
    /// The suspend expression (see [`SuspendKind`]).
    pub suspend: ExprId,
    /// `awaiter.await_resume()`: the value of the expression.
    pub resume: ExprId,
    /// The `OpaqueValueExpr` standing for `common`.
    pub opaque: ExprId,
}

/// The form of a suspend expression ([expr.await]/5.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SuspendKind {
    /// `await_suspend(h)` returns `void`: always suspends.
    Void,
    /// It returns `bool`: suspends unless it is `false`.
    Bool,
    /// It returns a `coroutine_handle`: the suspend expression is
    /// `__builtin_coro_resume(await_suspend(h).address())`, resuming the
    /// returned coroutine after suspending (symmetric transfer).
    Handle,
}

/// Accessors for the WP-064 expression forms (spec/ast.md §5.1).
impl ExprArena {
    /// The operand of a `CXXThrowExpr` (`None` for a rethrow).
    pub fn throw_operand(&self, e: ExprId) -> Option<ExprId> {
        ExprId::from_raw(self.expect_kind(e, ExprKind::CXXThrowExpr).a)
    }

    /// The operands of a `BinaryConditionalOperator`: the common
    /// expression, the condition (over an `OpaqueValueExpr` of the
    /// common expression), the true arm (the `OpaqueValueExpr`, possibly
    /// converted) and the false arm.
    pub fn binary_conditional_operands(&self, e: ExprId, lists: &IdLists) -> [ExprId; 4] {
        let n = self.expect_kind(e, ExprKind::BinaryConditionalOperator);
        let mut it = lists.iter::<ExprId>(ListId::new(n.a));
        let mut next =
            || it.next().expect("invariant: binary_conditional pushes exactly four operands");
        [next(), next(), next(), next()]
    }

    /// The syntactic form of a `PseudoObjectExpr` (the expression as
    /// written; not evaluated).
    pub fn pseudo_syntactic(&self, e: ExprId, lists: &IdLists) -> ExprId {
        let n = self.expect_kind(e, ExprKind::PseudoObjectExpr);
        lists
            .iter::<ExprId>(ListId::new(n.a))
            .next()
            .expect("invariant: pseudo_object pushes the syntactic form first")
    }

    /// The semantic expressions of a `PseudoObjectExpr`, in evaluation
    /// order: the bound `OpaqueValueExpr`s, then the result (and none
    /// after it for the synthesized three-way comparison).
    pub fn pseudo_semantics(&self, e: ExprId, lists: &IdLists) -> Vec<ExprId> {
        let n = self.expect_kind(e, ExprKind::PseudoObjectExpr);
        lists.iter::<ExprId>(ListId::new(n.a)).skip(1).collect()
    }

    /// The result expression of a `PseudoObjectExpr`: its value is the
    /// value of the whole expression.
    pub fn pseudo_result(&self, e: ExprId, lists: &IdLists) -> ExprId {
        let n = self.expect_kind(e, ExprKind::PseudoObjectExpr);
        lists
            .at::<ExprId>(ListId::new(n.a), n.b as usize + 1)
            .expect("invariant: pseudo_object checks its result index is in range")
    }

    /// The operand as written of a `CoawaitExpr`, `CoyieldExpr` or
    /// `DependentCoawaitExpr` ([expr.await], [expr.yield]).
    pub fn coawait_operand(&self, e: ExprId, lists: &IdLists) -> ExprId {
        let n = self.get(e);
        match n.kind {
            ExprKind::DependentCoawaitExpr => ExprId::new(n.a),
            ExprKind::CoawaitExpr | ExprKind::CoyieldExpr => lists
                .iter::<ExprId>(ListId::new(n.a))
                .next()
                .expect("invariant: coawait_expr / dependent_coawait push the operand first"),
            k => {
                panic!("invariant: coawait_operand is called on a co_await / co_yield, found {k:?}")
            }
        }
    }

    /// The resolved parts of a `CoawaitExpr` or `CoyieldExpr`; `None`
    /// for a dependent `CoyieldExpr` (a template pattern).
    pub fn coawait_parts(&self, e: ExprId, lists: &IdLists) -> Option<CoawaitParts> {
        let n = self.get(e);
        assert!(
            matches!(n.kind, ExprKind::CoawaitExpr | ExprKind::CoyieldExpr),
            "expected CoawaitExpr or CoyieldExpr, found {:?}",
            n.kind
        );
        let v: Vec<ExprId> = lists.iter::<ExprId>(ListId::new(n.a)).collect();
        let [operand, common, ready, suspend, resume, opaque] = v[..] else { return None };
        Some(CoawaitParts { operand, common, ready, suspend, resume, opaque })
    }

    /// Whether a `CoawaitExpr` is implicit (an initial or final suspend
    /// point, or the `co_await` of a `co_yield`).
    pub fn coawait_is_implicit(&self, e: ExprId) -> bool {
        self.get(e).aux & 1 != 0
    }

    /// How a `CoawaitExpr`/`CoyieldExpr` suspends (the result of its
    /// `await_suspend`).
    pub fn coawait_suspend_kind(&self, e: ExprId) -> SuspendKind {
        match (self.get(e).aux >> 1) & 3 {
            1 => SuspendKind::Bool,
            2 => SuspendKind::Handle,
            _ => SuspendKind::Void,
        }
    }

    /// The side record of a `LambdaExpr`.
    pub fn lambda_id(&self, e: ExprId) -> LambdaId {
        LambdaId::new(self.expect_kind(e, ExprKind::LambdaExpr).a)
    }

    /// The side record of a `LambdaExpr`.
    pub fn lambda(&self, e: ExprId) -> &crate::lambda::LambdaData {
        self.lambdas.get(self.lambda_id(e))
    }

    /// The compound statement of a `StmtExpr`.
    pub fn stmt_expr_body(&self, e: ExprId) -> StmtId {
        StmtId::new(self.expect_kind(e, ExprKind::StmtExpr).a)
    }

    /// The label declaration a GNU `&&label` (`AddrLabelExpr`) names.
    pub fn addr_label(&self, e: ExprId) -> DeclId {
        DeclId::new(self.expect_kind(e, ExprKind::AddrLabelExpr).a)
    }

    /// The operand type and designator as written of a value-dependent
    /// `OffsetOfExpr` ([`ColdExprData::offsetof_pattern`]).
    pub fn offsetof_pattern(&self, e: ExprId) -> Option<(QualType, &[OffsetofComponent])> {
        self.cold.offsetof_pattern.get(e).map(|(t, c)| (*t, &**c))
    }

    /// The accessor of an `ExtVectorElementExpr` (`xy` in `v.xy`).
    pub fn ext_vector_accessor(&self, e: ExprId) -> Symbol {
        Symbol::from_index(self.expect_kind(e, ExprKind::ExtVectorElementExpr).b)
    }

    /// The byte offset an `OffsetOfExpr` evaluates to.
    pub fn offsetof_value(&self, e: ExprId) -> u64 {
        u64::from(self.expect_kind(e, ExprKind::OffsetOfExpr).b)
    }

    /// Whether an `OffsetOfExpr`'s designator passes through an invalid
    /// class: its value is then not a constant expression.
    pub fn offsetof_invalid_record(&self, e: ExprId) -> bool {
        self.expect_kind(e, ExprKind::OffsetOfExpr).aux() != 0
    }

    /// The `InitListExpr` of a `CompoundLiteralExpr`.
    pub fn compound_literal_init(&self, e: ExprId) -> ExprId {
        ExprId::new(self.expect_kind(e, ExprKind::CompoundLiteralExpr).a)
    }

    /// The implicit static-storage variable of a namespace-scope array
    /// `CompoundLiteralExpr` (an lvalue, clang's `.compoundliteral`
    /// object; `AstBuilder::file_scope_compound_literal`), or `None` for
    /// a prvalue literal.
    pub fn compound_literal_var(&self, e: ExprId) -> Option<DeclId> {
        let n = self.expect_kind(e, ExprKind::CompoundLiteralExpr);
        (n.aux() != 0).then(|| DeclId::new(n.b))
    }
}
