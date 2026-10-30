//! Expressions: the 20-byte [`ExprNode`] header and side records
//! (spec/ast.md §5).
//!
//! Nodes are created by the `AstBuilder`, which computes dependence and
//! value category; this module owns storage, the operator/cast enums and
//! typed accessors.
#![warn(missing_docs)]

use kiln_source::Loc;

use crate::ids::{
    ArgListId, ConstValueId, DeclId, ExprId, ListId, NnsId, QualType, RawId, StmtId, StringLitId,
};
use crate::lists::IdLists;
use crate::mem::{HeapSize, MemReport, MemRow};
use crate::slot::{AlreadySet, ColdMap};

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
    NoInitExpr,
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
    /// `CXXNewExpr` / `CXXDeleteExpr`: `::new` / `::delete` (dump `global`).
    pub const GLOBAL: ExprFlags = ExprFlags(1 << 4);
    /// `CXXNewExpr`: an array new; `CXXDeleteExpr`: deletes an array
    /// (`delete[]`, or `delete` of a pointer to array; dump `array`).
    pub const ARRAY_FORM: ExprFlags = ExprFlags(1 << 5);
    /// `CXXNewExpr`: the allocation function takes `std::align_val_t`.
    pub const PASS_ALIGN: ExprFlags = ExprFlags(1 << 6);
    /// `CXXDeleteExpr`: written `delete[]`.
    pub const ARRAY_AS_WRITTEN: ExprFlags = ExprFlags(1 << 6);

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
    /// `ExpressionTraitExpr`. Spellings are the keywords; aliases
    /// (`__is_same_as`, `__is_convertible_to`, `__is_literal`) are their
    /// own rows because clang dumps the spelling's trait. Arity, status and
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
        /// `__is_literal` (an alias of `__is_literal_type`).
        IsLiteral = "__is_literal",
        /// `__is_literal_type`.
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
        /// `__is_base_of`.
        IsBaseOf = "__is_base_of",
        /// `__is_same`.
        IsSame = "__is_same",
        /// `__is_same_as` (an alias of `__is_same`).
        IsSameAs = "__is_same_as",
        /// `__is_assignable`.
        IsAssignable = "__is_assignable",
        /// `__is_convertible`.
        IsConvertible = "__is_convertible",
        /// `__is_convertible_to` (an alias of `__is_convertible`).
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
            kind: ConstructionKind::from_u8(aux & 3).expect("2-bit construction kind"),
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
    /// Designators as written for `InitListExpr` (§5.6).
    pub designators: ColdMap<ExprId, ListId>,
    /// `ArrayTypeTraitExpr`: the dimension operand of `__array_extent`
    /// (unevaluated by codegen; kept for substitution) and the value.
    pub array_trait: ColdMap<ExprId, ArrayTraitExtra>,
    /// `TypeTraitExpr`: the location of the second type argument of a
    /// binary trait (clang reports `__is_base_of`'s incomplete derived
    /// type there).
    pub trait_rhs_loc: ColdMap<ExprId, Loc>,
    /// The values of the source-location builtins at a use (§5.8): for a
    /// `SourceLocExpr` its own location and context, for a
    /// `CXXDefaultArgExpr` / `CXXDefaultInitExpr` whose initializer holds
    /// one, the location and context of the use. Computed by sema (which
    /// owns the source manager and the function-name printer).
    pub source_loc: ColdMap<ExprId, SourceLocValues>,
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
            array_trait: ColdMap::new("array_trait"),
            trait_rhs_loc: ColdMap::new("trait_rhs_loc"),
            source_loc: ColdMap::new("source_loc"),
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

    pub(crate) fn push(&mut self, n: ExprNode) -> ExprId {
        let id = ExprId::new(self.nodes.len() as u32);
        self.nodes.push(n);
        id
    }

    pub(crate) fn push_cond_arms(&mut self, then: ExprId, els: ExprId) -> u32 {
        push_side(&mut self.cond_arms, [then, els])
    }
    pub(crate) fn push_declref_extra(&mut self, x: DeclRefExtra) -> u32 {
        push_side(&mut self.declref_extras, x)
    }
    pub(crate) fn push_compound_assign(&mut self, x: CompoundAssignExtra) -> u32 {
        push_side(&mut self.compound_assigns, x)
    }
    pub(crate) fn push_init_list(&mut self, x: InitListExtra) -> u32 {
        push_side(&mut self.init_lists, x)
    }
    pub(crate) fn push_new_extra(&mut self, x: NewExtra) -> u32 {
        push_side(&mut self.news, x)
    }

    fn expect_kind(&self, e: ExprId, k: ExprKind) -> &ExprNode {
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
            k => panic!("{k:?} references no declaration"),
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
        UnaryOp::from_u8(self.expect_kind(e, ExprKind::UnaryOperator).aux).expect("valid opcode")
    }

    /// The operator of a `BinaryOperator` / `CompoundAssignOperator`.
    pub fn binary_op(&self, e: ExprId) -> BinaryOp {
        let n = self.get(e);
        assert!(
            matches!(n.kind, ExprKind::BinaryOperator | ExprKind::CompoundAssignOperator),
            "expected a binary operator, found {:?}",
            n.kind
        );
        BinaryOp::from_u8(n.aux).expect("valid opcode")
    }

    /// The side record of a `CompoundAssignOperator`.
    pub fn compound_assign(&self, e: ExprId) -> &CompoundAssignExtra {
        &self.compound_assigns[self.expect_kind(e, ExprKind::CompoundAssignOperator).b as usize]
    }

    /// The cast kind of a cast expression.
    pub fn cast_kind(&self, e: ExprId) -> CastKind {
        let n = self.get(e);
        assert!(n.kind.is_cast(), "expected a cast, found {:?}", n.kind);
        CastKind::from_u8(n.aux).expect("valid cast kind")
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

    /// The side record of an `InitListExpr`.
    pub fn init_list(&self, e: ExprId) -> &InitListExtra {
        &self.init_lists[self.expect_kind(e, ExprKind::InitListExpr).b as usize]
    }

    /// The kind of a `CharacterLiteral`.
    pub fn char_kind(&self, e: ExprId) -> CharKind {
        CharKind::from_u8(self.expect_kind(e, ExprKind::CharacterLiteral).aux)
            .expect("valid char kind")
    }

    /// The code point of a `CharacterLiteral`.
    pub fn char_value(&self, e: ExprId) -> u32 {
        self.expect_kind(e, ExprKind::CharacterLiteral).a
    }

    /// The kind of a `StringLiteral`.
    pub fn string_kind(&self, e: ExprId) -> StringKind {
        StringKind::from_u8(self.expect_kind(e, ExprKind::StringLiteral).aux)
            .expect("valid string kind")
    }

    /// The value of a `CXXBoolLiteralExpr`.
    pub fn bool_value(&self, e: ExprId) -> bool {
        self.expect_kind(e, ExprKind::CXXBoolLiteralExpr).a != 0
    }

    /// The radix of an `IntegerLiteral` as written.
    pub fn radix(&self, e: ExprId) -> Radix {
        Radix::from_u8(self.expect_kind(e, ExprKind::IntegerLiteral).aux).expect("valid radix")
    }

    /// Why a `DeclRefExpr` / `MemberExpr` is not an odr-use.
    pub fn non_odr_use(&self, e: ExprId) -> NonOdrUse {
        let n = self.get(e);
        assert!(
            matches!(n.kind, ExprKind::DeclRefExpr | ExprKind::MemberExpr),
            "expected DeclRefExpr or MemberExpr, found {:?}",
            n.kind
        );
        NonOdrUse::from_u8(n.aux).expect("valid non-odr-use kind")
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

    /// Retypes the string literal `e` that initializes a character array
    /// (and the parentheses around it) to the array type `ty`, as a
    /// prvalue ([dcl.init.string]; clang's `updateStringLiteralType`).
    ///
    /// # Panics
    /// If `e` is not a `StringLiteral` inside zero or more `ParenExpr`s.
    pub fn update_string_init_type(&mut self, e: ExprId, ty: QualType) {
        let mut e = e;
        loop {
            let n = &mut self.nodes[e.index() as usize];
            n.ty = ty;
            n.bits &= !3;
            match n.kind {
                ExprKind::StringLiteral => return,
                ExprKind::ParenExpr => e = ExprId::new(n.a),
                k => panic!("update_string_init_type on {k:?}"),
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
        crate::names::OverloadedOperator::from_u8(n.aux).expect("operator kind")
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
        (BinaryOp::from_u8(n.aux).expect("valid opcode"), ListId::new(n.b))
    }

    /// The implied object argument of a `CXXMemberCallExpr`: the base of
    /// its `MemberExpr` callee, or the left operand of a `.*`/`->*`
    /// callee (sema-class.md §5.1; for `->` a pointer).
    pub fn member_call_object(&self, e: ExprId) -> ExprId {
        let callee = self.get(ExprId::new(self.expect_kind(e, ExprKind::CXXMemberCallExpr).a));
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
        (ConstructionKind::from_u8(aux & 3).expect("2-bit kind"), aux & 4 != 0)
    }

    /// The storage duration of a `MaterializeTemporaryExpr`.
    pub fn temporary_storage(&self, e: ExprId) -> StorageDuration {
        let aux = self.expect_kind(e, ExprKind::MaterializeTemporaryExpr).aux;
        StorageDuration::from_u8(aux & 3).expect("2-bit storage duration")
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

    /// The explicit initializers of a `CXXParenListInitExpr`.
    pub fn paren_list_inits(&self, e: ExprId) -> ListId {
        ListId::new(self.expect_kind(e, ExprKind::CXXParenListInitExpr).a)
    }

    /// The array filler of a `CXXParenListInitExpr`.
    pub fn paren_list_filler(&self, e: ExprId) -> Option<ExprId> {
        ExprId::from_raw(self.expect_kind(e, ExprKind::CXXParenListInitExpr).b)
    }

    /// The common expression (an `OpaqueValueExpr`) and element
    /// initializer of an `ArrayInitLoopExpr`.
    pub fn array_init_loop(&self, e: ExprId) -> [ExprId; 2] {
        let n = self.expect_kind(e, ExprKind::ArrayInitLoopExpr);
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
            TypeidOperand::Type(QualType::from_raw(n.a).expect("typeid type operand"))
        } else {
            TypeidOperand::Expr(ExprId::new(n.a))
        }
    }

    /// True if the operand of a `CXXTypeidExpr` is potentially evaluated
    /// (a glvalue of polymorphic class type, [expr.typeid]/4).
    pub fn typeid_potentially_evaluated(&self, e: ExprId) -> bool {
        self.expect_kind(e, ExprKind::CXXTypeidExpr).aux & 1 != 0
    }

    /// The explicit initializers of an `InitListExpr` (a `ListId` of
    /// `ExprId`s; the filler is in [`ExprArena::init_list`]).
    pub fn init_list_inits(&self, e: ExprId) -> ListId {
        ListId::new(self.expect_kind(e, ExprKind::InitListExpr).a)
    }

    /// The kind of a `UnaryExprOrTypeTraitExpr`.
    pub fn trait_kind(&self, e: ExprId) -> TraitKind {
        TraitKind::from_u8(self.expect_kind(e, ExprKind::UnaryExprOrTypeTraitExpr).aux)
            .expect("valid trait kind")
    }

    /// The type operand of a `UnaryExprOrTypeTraitExpr` (`None` for an
    /// expression operand, which is its only child).
    pub fn trait_type_arg(&self, e: ExprId) -> Option<QualType> {
        let n = self.expect_kind(e, ExprKind::UnaryExprOrTypeTraitExpr);
        n.has(ExprFlags::ARG_IS_TYPE).then(|| QualType::from_raw(n.a)).flatten()
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
        TypeTrait::from_u8(n.aux).expect("valid type trait")
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

    /// The queried type of an `ArrayTypeTraitExpr`.
    pub fn array_trait_type(&self, e: ExprId) -> QualType {
        QualType::from_raw(self.expect_kind(e, ExprKind::ArrayTypeTraitExpr).a)
            .expect("array trait type")
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
            .expect("valid new init style")
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
    /// `CXXBindTemporaryExpr`, `ExprWithCleanups` or `PackExpansionExpr`
    /// (its pattern).
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
                        | ExprKind::ExprWithCleanups
                        | ExprKind::PackExpansionExpr
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

    /// The closing-token location of `e`, if stored.
    pub fn end_loc(&self, e: ExprId) -> Option<Loc> {
        let n = self.get(e);
        match n.kind {
            ExprKind::ParenExpr => Some(Loc::from_raw(n.b)),
            ExprKind::InitListExpr => Some(self.init_lists[n.b as usize].rbrace),
            _ => self.cold.end_loc.get(e).copied(),
        }
    }

    /// The child expressions of `e` in dump order (§11): the expressions
    /// `e` owns, as clang's `Stmt::children`. Shared subexpressions are
    /// not children (the dump prints them, §11): the source of an
    /// `OpaqueValueExpr`, the default member initializer of a
    /// `CXXDefaultInitExpr` and the default argument of a
    /// `CXXDefaultArgExpr`.
    pub fn children(&self, e: ExprId, lists: &IdLists) -> Vec<ExprId> {
        let n = self.get(e);
        let id = |w: u32| ExprId::from_raw(w);
        let mut v = Vec::new();
        match n.kind {
            ExprKind::MemberExpr
            | ExprKind::UnaryOperator
            | ExprKind::ParenExpr
            | ExprKind::ConstantExpr
            | ExprKind::MaterializeTemporaryExpr
            | ExprKind::CXXBindTemporaryExpr
            | ExprKind::ExprWithCleanups => v.extend(id(n.a)),
            ExprKind::CXXTypeidExpr if !n.has(ExprFlags::ARG_IS_TYPE) => v.extend(id(n.a)),
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                v.extend(lists.iter::<ExprId>(ListId::new(n.b)));
            }
            ExprKind::CXXParenListInitExpr => {
                v.extend(lists.iter::<ExprId>(ListId::new(n.a)));
                v.extend(id(n.b));
            }
            ExprKind::ArrayInitLoopExpr => {
                v.extend(id(n.a));
                v.extend(id(n.b));
            }
            ExprKind::SubstNonTypeTemplateParmExpr => v.extend(id(n.b)),
            k if k.is_cast() => v.extend(id(n.a)),
            ExprKind::UnaryExprOrTypeTraitExpr if !n.has(ExprFlags::ARG_IS_TYPE) => {
                v.extend(id(n.a));
            }
            // `CXXDependentScopeMemberExpr` and `PackExpansionExpr` are
            // template expressions (spec/sema-templates.md §2.4).
            ExprKind::CXXNoexceptExpr
            | ExprKind::ExpressionTraitExpr
            | ExprKind::CXXThrowExpr
            | ExprKind::CompoundLiteralExpr
            | ExprKind::CXXDeleteExpr
            | ExprKind::CXXDependentScopeMemberExpr
            | ExprKind::PackExpansionExpr => v.extend(id(n.a)),
            ExprKind::BinaryOperator | ExprKind::ArraySubscriptExpr | ExprKind::CXXFoldExpr => {
                v.extend(id(n.a));
                v.extend(id(n.b));
            }
            ExprKind::CompoundAssignOperator => {
                v.extend(id(n.a));
                v.push(self.compound_assigns[n.b as usize].rhs);
            }
            ExprKind::ConditionalOperator => {
                v.extend(id(n.a));
                v.extend(self.cond_arms[n.b as usize]);
            }
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr => {
                v.extend(id(n.a));
                v.extend(lists.iter::<ExprId>(ListId::new(n.b)));
            }
            ExprKind::CXXRewrittenBinaryOperator => v.extend(id(n.a)),
            ExprKind::InitListExpr => {
                v.extend(lists.iter::<ExprId>(ListId::new(n.a)));
                v.extend(self.init_lists[n.b as usize].array_filler);
            }
            ExprKind::RecoveryExpr => v.extend(lists.iter::<ExprId>(ListId::new(n.a))),
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
            ExprKind::BinaryConditionalOperator
            | ExprKind::OffsetOfExpr
            | ExprKind::CXXUnresolvedConstructExpr
            | ExprKind::ParenListExpr => {
                v.extend(lists.iter::<ExprId>(ListId::new(n.a)));
            }
            _ => {}
        }
        v
    }

    pub(crate) fn live_bytes(&self) -> u64 {
        self.vec_rows().iter().map(|r| r.live_bytes).sum()
    }

    fn vec_rows(&self) -> [MemRow; 6] {
        [
            MemRow::of_vec("nodes", &self.nodes),
            MemRow::of_vec("cond_arms", &self.cond_arms),
            MemRow::of_vec("declref_extras", &self.declref_extras),
            MemRow::of_vec("compound_assigns", &self.compound_assigns),
            MemRow::of_vec("init_lists", &self.init_lists),
            MemRow::of_vec("news", &self.news),
        ]
    }
}

impl HeapSize for ExprArena {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        for row in self.vec_rows() {
            r.push(row);
        }
        r.push(self.cold.end_loc.mem_row().prefixed("cold"));
        r.push(self.cold.member_extra.mem_row().prefixed("cold"));
        r.push(self.cold.designators.mem_row().prefixed("cold"));
        r.push(self.cold.array_trait.mem_row().prefixed("cold"));
        r.push(self.cold.trait_rhs_loc.mem_row().prefixed("cold"));
        r.push(self.cold.source_loc.mem_row().prefixed("cold"));
        r
    }
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
        let mut next = || it.next().expect("four operands");
        [next(), next(), next(), next()]
    }

    /// The compound statement of a `StmtExpr`.
    pub fn stmt_expr_body(&self, e: ExprId) -> StmtId {
        StmtId::new(self.expect_kind(e, ExprKind::StmtExpr).a)
    }

    /// The byte offset an `OffsetOfExpr` evaluates to.
    pub fn offsetof_value(&self, e: ExprId) -> u64 {
        u64::from(self.expect_kind(e, ExprKind::OffsetOfExpr).b)
    }

    /// The `InitListExpr` of a `CompoundLiteralExpr`.
    pub fn compound_literal_init(&self, e: ExprId) -> ExprId {
        ExprId::new(self.expect_kind(e, ExprKind::CompoundLiteralExpr).a)
    }
}
