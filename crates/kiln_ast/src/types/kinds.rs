//! Small enumerations and flag sets of the type layer (spec/types.md §2,
//! §5, §6): qualifiers, type flags, node kinds, builtin ids and the
//! keyword/attribute enums stored in `aux`.

use std::fmt;
use std::ops::{BitAnd, BitOr, BitOrAssign, Sub};

use crate::ids::{QualType, TypeId};

/// The "fast" cv-qualifiers carried in a [`QualType`]'s low bits
/// (spec/types.md §2). `RESTRICT` is GNU `__restrict`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Quals(u8);

impl Quals {
    /// No qualifiers.
    pub const NONE: Quals = Quals(0);
    /// `const`.
    pub const CONST: Quals = Quals(1);
    /// `volatile`.
    pub const VOLATILE: Quals = Quals(2);
    /// `__restrict`.
    pub const RESTRICT: Quals = Quals(4);
    /// `const volatile`.
    pub const CV: Quals = Quals(3);
    /// All three qualifiers.
    pub const ALL: Quals = Quals(7);

    /// The qualifiers with bits `bits` (bits above 2 are dropped).
    #[inline]
    pub const fn from_bits(bits: u8) -> Quals {
        Quals(bits & 7)
    }

    /// The raw bits.
    #[inline]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// True if no qualifier is set.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// True if every qualifier of `other` is set.
    #[inline]
    pub const fn contains(self, other: Quals) -> bool {
        self.0 & other.0 == other.0
    }

    /// True if `const` is set.
    #[inline]
    pub const fn is_const(self) -> bool {
        self.0 & 1 != 0
    }

    /// True if `volatile` is set.
    #[inline]
    pub const fn is_volatile(self) -> bool {
        self.0 & 2 != 0
    }

    /// True if `__restrict` is set.
    #[inline]
    pub const fn is_restrict(self) -> bool {
        self.0 & 4 != 0
    }

    /// Writes the qualifiers as clang does: `const volatile __restrict`,
    /// separated by single spaces, nothing for an empty set.
    pub fn write(self, out: &mut String) {
        let mut first = true;
        for (bit, name) in [(1u8, "const"), (2, "volatile"), (4, "__restrict")] {
            if self.0 & bit != 0 {
                if !first {
                    out.push(' ');
                }
                out.push_str(name);
                first = false;
            }
        }
    }
}

impl BitOr for Quals {
    type Output = Quals;
    #[inline]
    fn bitor(self, rhs: Quals) -> Quals {
        Quals(self.0 | rhs.0)
    }
}

impl BitOrAssign for Quals {
    #[inline]
    fn bitor_assign(&mut self, rhs: Quals) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for Quals {
    type Output = Quals;
    #[inline]
    fn bitand(self, rhs: Quals) -> Quals {
        Quals(self.0 & rhs.0)
    }
}

impl Sub for Quals {
    type Output = Quals;
    /// Set difference.
    #[inline]
    fn sub(self, rhs: Quals) -> Quals {
        Quals(self.0 & !rhs.0)
    }
}

impl fmt::Debug for Quals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();
        self.write(&mut s);
        write!(f, "Quals({s})")
    }
}

impl fmt::Display for Quals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();
        self.write(&mut s);
        f.write_str(&s)
    }
}

impl QualType {
    /// The qualifiers carried by this reference (not looking through sugar
    /// or array elements; see `TypeContext::effective_quals`).
    #[inline]
    pub const fn quals(self) -> Quals {
        Quals::from_bits(self.quals_bits())
    }
}

/// Per-node flags, computed at construction as the union over operands
/// plus the node's own contribution (spec/types.md §6).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TypeFlags(u8);

impl TypeFlags {
    /// No flag.
    pub const NONE: TypeFlags = TypeFlags(0);
    /// Dependent type [temp.dep.type].
    pub const DEPENDENT: TypeFlags = TypeFlags(1);
    /// Mentions a template parameter somewhere (including inside a
    /// non-dependent `decltype`).
    pub const INSTANTIATION_DEPENDENT: TypeFlags = TypeFlags(2);
    /// Contains a variable-length array.
    pub const VARIABLY_MODIFIED: TypeFlags = TypeFlags(4);
    /// Contains an unexpanded parameter pack.
    pub const UNEXPANDED_PACK: TypeFlags = TypeFlags(8);
    /// Built from an `Error` type or a recovery expression.
    pub const CONTAINS_ERROR: TypeFlags = TypeFlags(16);
    /// Contains an undeduced placeholder (`auto`, CTAD).
    pub const UNDEDUCED: TypeFlags = TypeFlags(32);
    /// The node is sugar: `canon[t] != t`.
    pub const SUGAR: TypeFlags = TypeFlags(64);
    /// `DEPENDENT | INSTANTIATION_DEPENDENT`: what a template parameter
    /// contributes.
    pub const DEPENDENT_INSTANTIATION: TypeFlags = TypeFlags(3);
    /// The flags that propagate from an operand to the node built on it
    /// (everything but `SUGAR`).
    pub const PROPAGATE: TypeFlags = TypeFlags(63);

    /// The flags with bits `bits`.
    #[inline]
    pub const fn from_bits(bits: u8) -> TypeFlags {
        TypeFlags(bits)
    }

    /// The raw bits.
    #[inline]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// True if every bit of `other` is set.
    #[inline]
    pub const fn contains(self, other: TypeFlags) -> bool {
        self.0 & other.0 == other.0
    }

    /// True if any bit of `other` is set.
    #[inline]
    pub const fn intersects(self, other: TypeFlags) -> bool {
        self.0 & other.0 != 0
    }

    /// True if no flag is set.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The union.
    #[inline]
    #[must_use]
    pub const fn union(self, other: TypeFlags) -> TypeFlags {
        TypeFlags(self.0 | other.0)
    }

    /// `self` without the bits of `other`.
    #[inline]
    #[must_use]
    pub const fn without(self, other: TypeFlags) -> TypeFlags {
        TypeFlags(self.0 & !other.0)
    }

    /// True if [`Self::DEPENDENT`] is set.
    #[inline]
    pub const fn is_dependent(self) -> bool {
        self.0 & 1 != 0
    }

    /// True if [`Self::INSTANTIATION_DEPENDENT`] is set.
    #[inline]
    pub const fn is_instantiation_dependent(self) -> bool {
        self.0 & 2 != 0
    }
}

impl BitOr for TypeFlags {
    type Output = TypeFlags;
    #[inline]
    fn bitor(self, rhs: TypeFlags) -> TypeFlags {
        TypeFlags(self.0 | rhs.0)
    }
}

impl BitAnd for TypeFlags {
    type Output = TypeFlags;
    #[inline]
    fn bitand(self, rhs: TypeFlags) -> TypeFlags {
        TypeFlags(self.0 & rhs.0)
    }
}

impl BitOrAssign for TypeFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: TypeFlags) {
        self.0 |= rhs.0;
    }
}

impl fmt::Debug for TypeFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TypeFlags(")?;
        let names = [
            "dependent",
            "instantiation-dependent",
            "variably-modified",
            "unexpanded-pack",
            "contains-error",
            "undeduced",
            "sugar",
            "bit7",
        ];
        let mut first = true;
        for (i, name) in names.iter().enumerate() {
            if self.0 & (1 << i) != 0 {
                if !first {
                    f.write_str(" ")?;
                }
                f.write_str(name)?;
                first = false;
            }
        }
        f.write_str(")")
    }
}

/// The kind of a type node (spec/types.md §5). `u8`-sized.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum TypeKind {
    /// Index 0 of the node arena; never handed out.
    Invalid = 0,
    /// A builtin type; `data` is its id (§5.1).
    Builtin,
    /// `_BitInt(N)`.
    BitInt,
    /// `_BitInt(expr)` with a value-dependent width.
    DependentBitInt,
    /// `T *`.
    Pointer,
    /// `T &`.
    LValueRef,
    /// `T &&`.
    RValueRef,
    /// `T C::*`.
    MemberPointer,
    /// `T[N]`.
    ConstantArray,
    /// `T[]`.
    IncompleteArray,
    /// GNU VLA `T[n]`.
    VariableArray,
    /// `T[expr]` with a value-dependent bound.
    DependentSizedArray,
    /// A function type (§5.4).
    Function,
    /// A class, struct or union.
    Record,
    /// An enumeration.
    Enum,
    /// GNU `_Complex T`.
    Complex,
    /// `_Atomic(T)`.
    Atomic,
    /// `vector_size` / `ext_vector_type` vectors.
    Vector,
    /// A vector with a value-dependent size.
    DependentVector,
    /// Extended qualifiers (address spaces; reserved, M7+).
    ExtQual,
    /// A typedef or alias-declaration name.
    Typedef,
    /// A type named through a using-declaration.
    Using,
    /// A type written with a keyword and/or qualifier.
    Elaborated,
    /// cv-qualifiers applied to a sugared array type (§2.1).
    QualifiedArray,
    /// A template type parameter as named in its template.
    TemplateTypeParm,
    /// The canonical template type parameter `(depth, index, pack)`.
    CanonTemplateTypeParm,
    /// "This type was template parameter T" sugar.
    SubstTemplateTypeParm,
    /// An argument pack substituted before its expansion.
    SubstTemplateTypeParmPack,
    /// `name<args>` as written.
    TemplateSpecialization,
    /// The injected-class-name of a class template inside its definition.
    InjectedClassName,
    /// `typename T::type`.
    DependentName,
    /// `typename T::template X<U>`.
    DependentTemplateSpecialization,
    /// `T...`.
    PackExpansion,
    /// C++26 `Ts...[I]`.
    PackIndexing,
    /// `decltype(expr)`.
    Decltype,
    /// The canonical form of an instantiation-dependent `decltype`.
    DependentDecltype,
    /// `typeof(expr)`, `typeof(type)`, `typeof_unqual`.
    TypeOf,
    /// `__underlying_type(T)` and the other unary transform builtins.
    UnaryTransform,
    /// `auto`, `decltype(auto)`, `__auto_type`, possibly constrained.
    Auto,
    /// A class template name used as a placeholder for CTAD.
    DeducedTemplateSpecialization,
    /// A type with a type attribute.
    Attributed,
    /// P2996 `typename [: r :]` (M6).
    Splice,
}

impl TypeKind {
    /// Number of kinds.
    pub const COUNT: usize = TypeKind::Splice as usize + 1;

    const ALL: [TypeKind; TypeKind::COUNT] = [
        TypeKind::Invalid,
        TypeKind::Builtin,
        TypeKind::BitInt,
        TypeKind::DependentBitInt,
        TypeKind::Pointer,
        TypeKind::LValueRef,
        TypeKind::RValueRef,
        TypeKind::MemberPointer,
        TypeKind::ConstantArray,
        TypeKind::IncompleteArray,
        TypeKind::VariableArray,
        TypeKind::DependentSizedArray,
        TypeKind::Function,
        TypeKind::Record,
        TypeKind::Enum,
        TypeKind::Complex,
        TypeKind::Atomic,
        TypeKind::Vector,
        TypeKind::DependentVector,
        TypeKind::ExtQual,
        TypeKind::Typedef,
        TypeKind::Using,
        TypeKind::Elaborated,
        TypeKind::QualifiedArray,
        TypeKind::TemplateTypeParm,
        TypeKind::CanonTemplateTypeParm,
        TypeKind::SubstTemplateTypeParm,
        TypeKind::SubstTemplateTypeParmPack,
        TypeKind::TemplateSpecialization,
        TypeKind::InjectedClassName,
        TypeKind::DependentName,
        TypeKind::DependentTemplateSpecialization,
        TypeKind::PackExpansion,
        TypeKind::PackIndexing,
        TypeKind::Decltype,
        TypeKind::DependentDecltype,
        TypeKind::TypeOf,
        TypeKind::UnaryTransform,
        TypeKind::Auto,
        TypeKind::DeducedTemplateSpecialization,
        TypeKind::Attributed,
        TypeKind::Splice,
    ];

    /// The kind with discriminant `v` (`Invalid` for out-of-range values).
    #[inline]
    pub fn from_u8(v: u8) -> TypeKind {
        TypeKind::ALL.get(usize::from(v)).copied().unwrap_or(TypeKind::Invalid)
    }

    /// True for the four array kinds (not `QualifiedArray`).
    #[inline]
    pub const fn is_array(self) -> bool {
        matches!(
            self,
            TypeKind::ConstantArray
                | TypeKind::IncompleteArray
                | TypeKind::VariableArray
                | TypeKind::DependentSizedArray
        )
    }

    /// True for lvalue and rvalue references.
    #[inline]
    pub const fn is_reference(self) -> bool {
        matches!(self, TypeKind::LValueRef | TypeKind::RValueRef)
    }
}

macro_rules! builtins {
    ($($(#[$doc:meta])* $konst:ident = $id:literal, $variant:ident, $name:literal;)*) => {
        /// The builtin types, pre-interned at fixed ids 1..=33 in this
        /// order (spec/types.md §5.1). The discriminant is the `TypeId`
        /// index.
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
        #[repr(u8)]
        pub enum BuiltinType {
            $($(#[$doc])* $variant = $id,)*
        }

        impl BuiltinType {
            /// All builtins in id order.
            pub const ALL: [BuiltinType; 33] = [$(BuiltinType::$variant,)*];

            /// The builtin with type id index `id`, if it is one.
            pub fn from_index(id: u32) -> Option<BuiltinType> {
                match id {
                    $($id => Some(BuiltinType::$variant),)*
                    _ => None,
                }
            }

            /// How clang 19 prints the type in C++.
            pub const fn name(self) -> &'static str {
                match self {
                    $(BuiltinType::$variant => $name,)*
                }
            }

            /// The fixed type id.
            #[inline]
            pub const fn type_id(self) -> TypeId {
                TypeId::new(self as u32)
            }

            /// The unqualified type.
            #[inline]
            pub const fn qual_type(self) -> QualType {
                QualType::unqual(self.type_id())
            }
        }

        impl TypeId {
            $(
                #[doc = concat!("The builtin `", $name, "` (fixed id ", stringify!($id), ").")]
                pub const $konst: TypeId = TypeId::new($id);
            )*
        }

        impl QualType {
            $(
                #[doc = concat!("The unqualified builtin `", $name, "`.")]
                pub const $konst: QualType = QualType::unqual(TypeId::new($id));
            )*
        }
    };
}

builtins! {
    /// `void`.
    VOID = 1, Void, "void";
    /// `bool`.
    BOOL = 2, Bool, "bool";
    /// `char`.
    CHAR = 3, Char, "char";
    /// `signed char`.
    SCHAR = 4, SChar, "signed char";
    /// `unsigned char`.
    UCHAR = 5, UChar, "unsigned char";
    /// `wchar_t`.
    WCHAR = 6, WChar, "wchar_t";
    /// `char8_t`.
    CHAR8 = 7, Char8, "char8_t";
    /// `char16_t`.
    CHAR16 = 8, Char16, "char16_t";
    /// `char32_t`.
    CHAR32 = 9, Char32, "char32_t";
    /// `short`.
    SHORT = 10, Short, "short";
    /// `unsigned short`.
    USHORT = 11, UShort, "unsigned short";
    /// `int`.
    INT = 12, Int, "int";
    /// `unsigned int`.
    UINT = 13, UInt, "unsigned int";
    /// `long`.
    LONG = 14, Long, "long";
    /// `unsigned long`.
    ULONG = 15, ULong, "unsigned long";
    /// `long long`.
    LONGLONG = 16, LongLong, "long long";
    /// `unsigned long long`.
    ULONGLONG = 17, ULongLong, "unsigned long long";
    /// `__int128`.
    INT128 = 18, Int128, "__int128";
    /// `unsigned __int128`.
    UINT128 = 19, UInt128, "unsigned __int128";
    /// `_Float16` (`std::float16_t`).
    FLOAT16 = 20, Float16, "_Float16";
    /// `__bf16` (`std::bfloat16_t`).
    BFLOAT16 = 21, BFloat16, "__bf16";
    /// `float`.
    FLOAT = 22, Float, "float";
    /// `double`.
    DOUBLE = 23, Double, "double";
    /// `long double`.
    LONGDOUBLE = 24, LongDouble, "long double";
    /// `__float128` (target-gated).
    FLOAT128 = 25, Float128, "__float128";
    /// `__fp16` (storage-only).
    HALF = 26, Half, "__fp16";
    /// `std::nullptr_t`.
    NULLPTR = 27, NullPtr, "std::nullptr_t";
    /// `std::meta::info` (M6).
    META_INFO = 28, MetaInfo, "std::meta::info";
    /// The type of a type-dependent expression.
    DEPENDENT = 29, Dependent, "<dependent type>";
    /// The type of an overload set.
    OVERLOAD = 30, Overload, "<overloaded function type>";
    /// The type of a bound member function.
    BOUND_MEMBER = 31, BoundMember, "<bound member function type>";
    /// The type of a builtin function name.
    BUILTIN_FN = 32, BuiltinFn, "<builtin fn type>";
    /// Recovery type; suppresses cascading diagnostics.
    ERROR = 33, Error, "<error type>";
}

impl BuiltinType {
    /// True for the integer types (including `bool` and the character
    /// types) [basic.fundamental].
    pub const fn is_integral(self) -> bool {
        (self as u8) >= 2 && (self as u8) <= 19
    }

    /// True for the signed integer types (plain `char` is not counted:
    /// its signedness is the target's).
    pub const fn is_signed_integer(self) -> bool {
        matches!(
            self,
            BuiltinType::SChar
                | BuiltinType::Short
                | BuiltinType::Int
                | BuiltinType::Long
                | BuiltinType::LongLong
                | BuiltinType::Int128
        )
    }

    /// True for the unsigned integer types (excluding `bool`, `wchar_t`
    /// and the `charN_t` types, whose underlying types decide).
    pub const fn is_unsigned_integer(self) -> bool {
        matches!(
            self,
            BuiltinType::UChar
                | BuiltinType::UShort
                | BuiltinType::UInt
                | BuiltinType::ULong
                | BuiltinType::ULongLong
                | BuiltinType::UInt128
        )
    }

    /// True for the floating-point types.
    pub const fn is_floating(self) -> bool {
        (self as u8) >= 20 && (self as u8) <= 26
    }

    /// True for the placeholder builtins (`Dependent`, `Overload`,
    /// `BoundMember`, `BuiltinFn`, `Error`).
    pub const fn is_placeholder(self) -> bool {
        (self as u8) >= 29
    }
}

/// The keyword of an elaborated or dependent type name.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[repr(u8)]
pub enum TagKeyword {
    /// No keyword written.
    #[default]
    None = 0,
    /// `struct`.
    Struct,
    /// `class`.
    Class,
    /// `union`.
    Union,
    /// `enum`.
    Enum,
    /// `typename`.
    Typename,
    /// `__interface` (MS; never produced in C++ mode, reserved).
    Interface,
}

impl TagKeyword {
    /// The keyword as written, empty for `None`.
    pub const fn spelling(self) -> &'static str {
        match self {
            TagKeyword::None => "",
            TagKeyword::Struct => "struct",
            TagKeyword::Class => "class",
            TagKeyword::Union => "union",
            TagKeyword::Enum => "enum",
            TagKeyword::Typename => "typename",
            TagKeyword::Interface => "__interface",
        }
    }

    /// The keyword with discriminant `v` (`None` if out of range).
    pub fn from_u8(v: u8) -> TagKeyword {
        match v {
            1 => TagKeyword::Struct,
            2 => TagKeyword::Class,
            3 => TagKeyword::Union,
            4 => TagKeyword::Enum,
            5 => TagKeyword::Typename,
            6 => TagKeyword::Interface,
            _ => TagKeyword::None,
        }
    }
}

/// The flavor of a vector type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum VectorKind {
    /// GNU `vector_size`.
    Generic = 0,
    /// ARM NEON `neon_vector_type`.
    Neon,
    /// ARM NEON `neon_polyvector_type`.
    NeonPoly,
    /// Clang `ext_vector_type`.
    Ext,
}

impl VectorKind {
    /// The kind with discriminant `v` (`Generic` if out of range).
    pub fn from_u8(v: u8) -> VectorKind {
        match v {
            1 => VectorKind::Neon,
            2 => VectorKind::NeonPoly,
            3 => VectorKind::Ext,
            _ => VectorKind::Generic,
        }
    }
}

/// The keyword of a placeholder type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum AutoKeyword {
    /// `auto`.
    Auto = 0,
    /// `decltype(auto)`.
    DecltypeAuto,
    /// GNU `__auto_type`.
    GnuAutoType,
}

impl AutoKeyword {
    /// The keyword with discriminant `v` (`Auto` if out of range).
    pub fn from_u8(v: u8) -> AutoKeyword {
        match v {
            1 => AutoKeyword::DecltypeAuto,
            2 => AutoKeyword::GnuAutoType,
            _ => AutoKeyword::Auto,
        }
    }

    /// The keyword as written.
    pub const fn spelling(self) -> &'static str {
        match self {
            AutoKeyword::Auto => "auto",
            AutoKeyword::DecltypeAuto => "decltype(auto)",
            AutoKeyword::GnuAutoType => "__auto_type",
        }
    }
}

macro_rules! unary_transforms {
    ($($variant:ident = $v:literal, $name:literal;)*) => {
        /// The clang unary type-transform builtins (`__underlying_type`,
        /// `__remove_cvref`, ...), which libc++ 19 uses when available.
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        #[repr(u8)]
        pub enum UnaryTransformKind {
            $(#[doc = concat!("`", $name, "`.")] $variant = $v,)*
        }

        impl UnaryTransformKind {
            /// The builtin's spelling.
            pub const fn spelling(self) -> &'static str {
                match self { $(UnaryTransformKind::$variant => $name,)* }
            }

            /// The transform with discriminant `v`.
            pub fn from_u8(v: u8) -> Option<UnaryTransformKind> {
                match v { $($v => Some(UnaryTransformKind::$variant),)* _ => None }
            }
        }
    };
}

unary_transforms! {
    AddLvalueReference = 0, "__add_lvalue_reference";
    AddPointer = 1, "__add_pointer";
    AddRvalueReference = 2, "__add_rvalue_reference";
    Decay = 3, "__decay";
    MakeSigned = 4, "__make_signed";
    MakeUnsigned = 5, "__make_unsigned";
    RemoveAllExtents = 6, "__remove_all_extents";
    RemoveConst = 7, "__remove_const";
    RemoveCv = 8, "__remove_cv";
    RemoveCvref = 9, "__remove_cvref";
    RemoveExtent = 10, "__remove_extent";
    RemovePointer = 11, "__remove_pointer";
    RemoveReference = 12, "__remove_reference_t";
    RemoveRestrict = 13, "__remove_restrict";
    RemoveVolatile = 14, "__remove_volatile";
    UnderlyingType = 15, "__underlying_type";
}

/// Calling conventions (4 bits in `FuncInfo`). Only non-default
/// conventions written through attributes produce anything but `Default`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[repr(u8)]
pub enum CallConv {
    /// The target's default convention.
    #[default]
    Default = 0,
    /// `cdecl` written explicitly.
    C,
    /// x86 `stdcall`.
    StdCall,
    /// x86 `fastcall`.
    FastCall,
    /// x86 `thiscall`.
    ThisCall,
    /// `vectorcall`.
    VectorCall,
    /// `regcall`.
    RegCall,
    /// `preserve_most`.
    PreserveMost,
    /// `preserve_all`.
    PreserveAll,
    /// x86-64 `sysv_abi`.
    SysV,
    /// x86-64 `ms_abi`.
    Win64,
    /// ARM `pcs("aapcs")`.
    Aapcs,
    /// ARM `pcs("aapcs-vfp")`.
    AapcsVfp,
    /// `AArch64` `aarch64_vector_pcs`.
    AArch64VectorCall,
    /// `AArch64` `aarch64_sve_pcs`.
    AArch64SveCall,
    /// `preserve_none`.
    PreserveNone,
}

impl CallConv {
    /// The convention with discriminant `v` (`Default` if out of range).
    pub fn from_u8(v: u8) -> CallConv {
        const ALL: [CallConv; 16] = [
            CallConv::Default,
            CallConv::C,
            CallConv::StdCall,
            CallConv::FastCall,
            CallConv::ThisCall,
            CallConv::VectorCall,
            CallConv::RegCall,
            CallConv::PreserveMost,
            CallConv::PreserveAll,
            CallConv::SysV,
            CallConv::Win64,
            CallConv::Aapcs,
            CallConv::AapcsVfp,
            CallConv::AArch64VectorCall,
            CallConv::AArch64SveCall,
            CallConv::PreserveNone,
        ];
        ALL.get(usize::from(v)).copied().unwrap_or_default()
    }

    /// The attribute clang prints after a function type with this
    /// convention, if any (clang's `printFunctionAfter`).
    pub const fn attribute(self) -> Option<&'static str> {
        match self {
            CallConv::Default | CallConv::C => None,
            CallConv::StdCall => Some("stdcall"),
            CallConv::FastCall => Some("fastcall"),
            CallConv::ThisCall => Some("thiscall"),
            CallConv::VectorCall => Some("vectorcall"),
            CallConv::RegCall => Some("regcall"),
            CallConv::PreserveMost => Some("preserve_most"),
            CallConv::PreserveAll => Some("preserve_all"),
            CallConv::SysV => Some("sysv_abi"),
            CallConv::Win64 => Some("ms_abi"),
            CallConv::Aapcs => Some("pcs(\"aapcs\")"),
            CallConv::AapcsVfp => Some("pcs(\"aapcs-vfp\")"),
            CallConv::AArch64VectorCall => Some("aarch64_vector_pcs"),
            CallConv::AArch64SveCall => Some("aarch64_sve_pcs"),
            CallConv::PreserveNone => Some("preserve_none"),
        }
    }
}

/// A method's ref-qualifier.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[repr(u8)]
pub enum RefQual {
    /// None written.
    #[default]
    None = 0,
    /// `&`.
    LValue,
    /// `&&`.
    RValue,
}

/// Type attributes kept as `Attributed` sugar.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u16)]
pub enum TypeAttrKind {
    /// `_Nonnull`.
    Nonnull = 0,
    /// `_Nullable`.
    Nullable,
    /// `_Null_unspecified`.
    NullUnspecified,
    /// `_Nullable_result`.
    NullableResult,
    /// `noderef`.
    NoDeref,
    /// A calling-convention attribute (the convention is in the
    /// equivalent function type).
    CallingConv,
    /// `noreturn` on a function type.
    NoReturn,
    /// `[[clang::lifetimebound]]` on an implicit object parameter.
    LifetimeBound,
    /// `__ptr32`/`__ptr64`/`__sptr`/`__uptr` and other attributes kiln
    /// keeps only for printing.
    Other,
}

impl TypeAttrKind {
    /// The kind with discriminant `v` (`Other` if out of range).
    pub fn from_u16(v: u16) -> TypeAttrKind {
        match v {
            0 => TypeAttrKind::Nonnull,
            1 => TypeAttrKind::Nullable,
            2 => TypeAttrKind::NullUnspecified,
            3 => TypeAttrKind::NullableResult,
            4 => TypeAttrKind::NoDeref,
            5 => TypeAttrKind::CallingConv,
            6 => TypeAttrKind::NoReturn,
            7 => TypeAttrKind::LifetimeBound,
            _ => TypeAttrKind::Other,
        }
    }
}
