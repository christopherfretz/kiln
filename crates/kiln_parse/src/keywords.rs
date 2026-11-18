//! The keyword table ([lex.key], plus the GNU spellings kiln accepts).
//!
//! The lexer produces every keyword as an identifier; the parser maps the
//! identifier's symbol to a [`Kw`] through one indexed load: the table is
//! dense in [`Symbol::index`] (the keywords are interned when the table is
//! built, so it spans the symbols interned up to then; spec/parse.md §3).

use kiln_ast::expr::TypeTrait;
use kiln_ast::types::UnaryTransformKind;
use kiln_source::symbol::{Interner, Symbol};

macro_rules! keywords {
    ($($variant:ident = $($spelling:literal)|+;)*) => {
        /// A keyword ([lex.key]) or GNU keyword spelling.
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        pub enum Kw {
            $(#[doc = concat!($("`", $spelling, "` "),+)] $variant,)*
        }

        impl Kw {
            /// Every keyword with its spellings.
            const TABLE: &'static [(Kw, &'static [&'static str])] =
                &[$((Kw::$variant, &[$($spelling),+]),)*];

            /// The primary spelling.
            pub fn spelling(self) -> &'static str {
                Self::TABLE.iter().find(|(k, _)| *k == self).map_or("", |(_, s)| s[0])
            }
        }
    };
}

keywords! {
    Alignas = "alignas" | "_Alignas";
    Alignof = "alignof" | "_Alignof";
    GnuAlignof = "__alignof" | "__alignof__";
    DataSizeof = "__datasizeof";
    Asm = "asm" | "__asm" | "__asm__";
    Atomic = "_Atomic";
    Auto = "auto";
    GnuAutoType = "__auto_type";
    BitInt = "_BitInt";
    Bool = "bool";
    Break = "break";
    Case = "case";
    Catch = "catch";
    Char = "char";
    Char8 = "char8_t";
    Char16 = "char16_t";
    Char32 = "char32_t";
    Class = "class";
    Concept = "concept";
    Const = "const" | "__const" | "__const__";
    Consteval = "consteval";
    Constexpr = "constexpr";
    Constinit = "constinit";
    ConstCast = "const_cast";
    Continue = "continue";
    ContractAssert = "contract_assert";
    CoAwait = "co_await";
    CoReturn = "co_return";
    CoYield = "co_yield";
    Decltype = "decltype" | "__decltype";
    Default = "default";
    Delete = "delete";
    Do = "do";
    Double = "double";
    DynamicCast = "dynamic_cast";
    Else = "else";
    Enum = "enum";
    Explicit = "explicit";
    Export = "export";
    Extern = "extern";
    False = "false";
    Float = "float";
    Float128 = "__float128";
    Float16 = "_Float16";
    Float32 = "_Float32";
    Float64 = "_Float64";
    StdFloat128 = "_Float128";
    Float32x = "_Float32x";
    Float64x = "_Float64x";
    Bf16 = "__bf16";
    ExtInt = "_ExtInt";
    Fp16 = "__fp16";
    For = "for";
    Friend = "friend";
    Goto = "goto";
    If = "if";
    Inline = "inline" | "__inline" | "__inline__";
    Int = "int";
    Int128 = "__int128";
    Long = "long";
    Mutable = "mutable";
    Namespace = "namespace";
    New = "new";
    Noexcept = "noexcept";
    Noreturn = "_Noreturn";
    Nullptr = "nullptr";
    Nonnull = "_Nonnull";
    Nullable = "_Nullable";
    NullUnspecified = "_Null_unspecified";
    NullableResult = "_Nullable_result";
    GnuNull = "__null";
    Operator = "operator";
    Private = "private";
    Protected = "protected";
    Public = "public";
    Register = "register";
    ReinterpretCast = "reinterpret_cast";
    Requires = "requires";
    Restrict = "__restrict" | "__restrict__";
    Return = "return";
    Short = "short";
    Signed = "signed" | "__signed" | "__signed__";
    Sizeof = "sizeof";
    Static = "static";
    StaticAssert = "static_assert" | "_Static_assert";
    StaticCast = "static_cast";
    Struct = "struct";
    Switch = "switch";
    Template = "template";
    This = "this";
    ThreadLocal = "thread_local" | "_Thread_local" | "__thread";
    Throw = "throw";
    True = "true";
    Try = "try";
    Typedef = "typedef";
    Typeid = "typeid";
    Typename = "typename";
    Typeof = "__typeof" | "__typeof__" | "typeof";
    TypeofUnqual = "__typeof_unqual" | "__typeof_unqual__";
    Union = "union";
    Unsigned = "unsigned";
    Using = "using";
    Virtual = "virtual";
    Void = "void";
    Volatile = "volatile" | "__volatile" | "__volatile__";
    WChar = "wchar_t";
    While = "while";
    Attribute = "__attribute__" | "__attribute";
    Declspec = "__declspec";
    Extension = "__extension__";
    Label = "__label__";
    Complex = "_Complex" | "__complex__" | "__complex";
    Imaginary = "_Imaginary";
    Decimal32 = "_Decimal32";
    Decimal64 = "_Decimal64";
    Decimal128 = "_Decimal128";
    Real = "__real__" | "__real";
    Imag = "__imag__" | "__imag";
    BuiltinOffsetof = "__builtin_offsetof";
    BuiltinChooseExpr = "__builtin_choose_expr";
    Generic = "_Generic";
    BuiltinBitCast = "__builtin_bit_cast";
    BuiltinVaArg = "__builtin_va_arg";
    BuiltinConvertVector = "__builtin_convertvector";
    BuiltinHasAttribute = "__builtin_has_attribute";
    BuiltinFile = "__builtin_FILE";
    BuiltinFileName = "__builtin_FILE_NAME";
    BuiltinFunction = "__builtin_FUNCTION";
    Func = "__func__";
    FunctionName = "__FUNCTION__";
    PrettyFunction = "__PRETTY_FUNCTION__";
    BuiltinLine = "__builtin_LINE";
    BuiltinColumn = "__builtin_COLUMN";
    BuiltinSourceLocation = "__builtin_source_location";
    ArmStreaming = "__arm_streaming";
    ArmStreamingCompatible = "__arm_streaming_compatible";
    ArmIn = "__arm_in";
    ArmOut = "__arm_out";
    ArmInout = "__arm_inout";
    ArmPreserves = "__arm_preserves";
    ArmAgnostic = "__arm_agnostic";
    ArmLocallyStreaming = "__arm_locally_streaming";
    ArmNew = "__arm_new";
}

impl Kw {
    /// The SME keyword attribute this keyword is (clang
    /// `KEYWORD_ATTRIBUTE`): its index in
    /// [`kiln_ast::attr::SME_KEYWORDS`].
    pub fn sme_index(self) -> Option<usize> {
        Some(match self {
            Kw::ArmStreaming => 0,
            Kw::ArmStreamingCompatible => 1,
            Kw::ArmIn => 2,
            Kw::ArmOut => 3,
            Kw::ArmInout => 4,
            Kw::ArmPreserves => 5,
            Kw::ArmAgnostic => 6,
            Kw::ArmLocallyStreaming => 7,
            Kw::ArmNew => 8,
            _ => return None,
        })
    }
}

/// Symbol-to-keyword map for one interner.
#[derive(Debug)]
pub struct Keywords {
    /// Per [`Symbol::index`]: the keyword and builtin name the symbol
    /// spells. Symbols interned after the table was built index past its
    /// end and are neither.
    table: Vec<Entry>,
    /// The contextual keyword `final` ([lex.name]/2).
    pub final_: Symbol,
    /// The contextual keyword `override`.
    pub override_: Symbol,
    /// The contextual keyword `pre` ([dcl.contract.func]).
    pub pre_: Symbol,
    /// The contextual keyword `post` ([dcl.contract.func]).
    pub post_: Symbol,
}

/// One [`Keywords`] table slot.
#[derive(Copy, Clone, Debug, Default)]
struct Entry {
    kw: Option<Kw>,
    /// The clang type-trait and type-transform names
    /// (spec/sema-templates.md §9.1, §9.2, §9.4). They are identifiers
    /// unless followed by `(` (clang's revertible type-trait keywords), so
    /// libc++'s fallbacks such as `__make_signed<T>` still parse.
    builtin: Option<BuiltinKw>,
    /// For a builtin trait name in C++: whether it is a keyword when not
    /// followed by `(` ([`TraitKw`]).
    trait_kw: TraitKw,
}

/// How a builtin trait name behaves when it is not followed by `(` (C++
/// only; spec/parse.md §1, "Trait keywords").
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
enum TraitKw {
    /// An ordinary identifier: the type transforms, clang 22's
    /// non-keyword traits ([`NON_KEYWORD_TRAITS`]), C, and a
    /// [`TraitKw::Revertible`] name once reverted.
    #[default]
    Ident,
    /// A keyword: `int __has_trivial_copy;` is an error (clang 22).
    Keyword,
    /// A keyword until `struct name` (or `static const bool __is_signed`)
    /// turns it into an identifier for the rest of the translation unit
    /// (clang's `TryKeywordIdentFallback`; [`REVERTIBLE_TRAITS`]).
    Revertible,
}

/// A builtin type-trait or type-transform name.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BuiltinKw {
    /// A `TypeTraitExpr`, `ArrayTypeTraitExpr` or `ExpressionTraitExpr`.
    Trait(TypeTrait),
    /// A `UnaryTransform` type-specifier.
    Transform(UnaryTransformKind),
    /// GCC's `__integer_pack(N)`, a primary expression (WP-1667).
    IntegerPack,
}

impl Keywords {
    /// Interns every keyword spelling in `interner`.
    /// The table for a translation unit; `gnu` (`-std=gnu++NN`) adds the
    /// GNU keywords that are not reserved identifiers (`typeof`, as clang
    /// in C++; spec/sema.md §5.8). At `compat` 22 `__is_referenceable` is
    /// an ordinary identifier (clang 22 removed the trait; spec/sema.md
    /// §1.1). Before C++11 `nullptr` is an identifier (clang's
    /// `KEYCXX11`; spec/parse.md §1). `contract_assert` is a keyword from
    /// C++26 on, at every `compat` level (spec/contracts.md §2).
    pub fn new(interner: &Interner, gnu: bool, cxx11: bool, compat: u32) -> Self {
        Self::with_std(interner, gnu, cxx11, false, compat)
    }

    /// [`Keywords::new`], with `cxx26` enabling the C++26 keywords.
    pub fn with_std(interner: &Interner, gnu: bool, cxx11: bool, cxx26: bool, compat: u32) -> Self {
        fn slot(table: &mut Vec<Entry>, sym: Symbol) -> &mut Entry {
            let i = sym.index() as usize;
            if table.len() <= i {
                table.resize(i + 1, Entry::default());
            }
            &mut table[i]
        }
        let mut table: Vec<Entry> = Vec::new();
        for (k, spellings) in Kw::TABLE {
            for s in *spellings {
                if (!gnu && *s == "typeof")
                    || (!cxx11 && *s == "nullptr")
                    || (!cxx26 && *s == "contract_assert")
                    // clang 20 added `__arm_agnostic`.
                    || (compat < 20 && *s == "__arm_agnostic")
                {
                    continue;
                }
                slot(&mut table, interner.intern(s)).kw = Some(*k);
            }
        }
        for &t in TypeTrait::all() {
            // `__builtin_types_compatible_p` is C only (clang `KEYNOCXX`):
            // `Keywords::for_c` adds it.
            if (compat >= 22 && t == TypeTrait::IsReferenceable) || t == TypeTrait::TypesCompatible
            {
                continue;
            }
            let e = slot(&mut table, interner.intern(t.spelling()));
            e.builtin = Some(BuiltinKw::Trait(t));
            e.trait_kw = if REVERTIBLE_TRAITS.binary_search(&t.spelling()).is_ok() {
                TraitKw::Revertible
            } else if NON_KEYWORD_TRAITS.contains(&t.spelling()) {
                TraitKw::Ident
            } else {
                TraitKw::Keyword
            };
        }
        for v in 0..=u8::MAX {
            let Some(k) = UnaryTransformKind::from_u8(v) else { break };
            slot(&mut table, interner.intern(k.spelling())).builtin = Some(BuiltinKw::Transform(k));
        }
        // GCC's spelling of `__remove_reference_t` (WP-1611). Like every
        // transform it is one only before `(`, so libstdc++'s and
        // libc++'s `__has_builtin(__remove_reference)` (0, as in clang 22)
        // and their own names are unaffected.
        slot(&mut table, interner.intern("__remove_reference")).builtin =
            Some(BuiltinKw::Transform(UnaryTransformKind::RemoveReference));
        // GCC's `__integer_pack(N)` (WP-1667): a builtin only before `(`,
        // as the transforms; `__has_builtin(__integer_pack)` stays 0 (as
        // in clang 22), so libstdc++ keeps `__make_integer_seq`.
        slot(&mut table, interner.intern("__integer_pack")).builtin = Some(BuiltinKw::IntegerPack);
        table.shrink_to_fit();
        Keywords {
            table,
            final_: interner.intern("final"),
            override_: interner.intern("override"),
            pre_: interner.intern("pre"),
            post_: interner.intern("post"),
        }
    }

    /// Makes `spelling` an ordinary identifier: `char8_t` without
    /// `-fchar8_t` semantics (clang `KEYCHAR8`: C++20 and later unless
    /// `-fno-char8_t`, or `-fchar8_t`; WP-808), and the
    /// [`CXX20_KEYWORDS`] before C++20.
    pub fn remove(&mut self, interner: &Interner, spelling: &str) {
        let i = interner.intern(spelling).index() as usize;
        if let Some(e) = self.table.get_mut(i) {
            e.kw = None;
        }
    }

    /// Makes `spelling` the keyword `kw`.
    fn add(&mut self, interner: &Interner, spelling: &str, kw: Kw) {
        let i = interner.intern(spelling).index() as usize;
        if self.table.len() <= i {
            self.table.resize(i + 1, Entry::default());
        }
        self.table[i].kw = Some(kw);
    }

    /// Turns the C++ table into the keyword set of C standard `c` (gnu
    /// modes when `gnu`), as clang `-x c` has it ([C17 6.4.1], [C23
    /// 6.4.1]; spec/parse.md §1): the [`CXX_ONLY_KEYWORDS`] are
    /// identifiers; `_Bool` is a keyword, `restrict` from C99, `inline`
    /// from C99 (gnu89 too); `asm` only in gnu modes; the
    /// [`C23_KEYWORDS`] and `typeof`/`typeof_unqual` only from C23
    /// (`typeof` in every gnu mode). Of the builtin traits only the
    /// [`C_TYPE_TRAITS`] and `__builtin_types_compatible_p` remain; the
    /// transforms are identifiers.
    pub fn for_c(&mut self, interner: &Interner, c: kiln_lex::CStd, gnu: bool) {
        use kiln_lex::CStd;
        for k in CXX_ONLY_KEYWORDS {
            self.remove(interner, k);
        }
        for e in &mut self.table {
            e.trait_kw = TraitKw::Ident;
            if matches!(e.builtin, Some(BuiltinKw::Trait(t)) if !C_TYPE_TRAITS.contains(&t))
                || matches!(e.builtin, Some(BuiltinKw::Transform(_) | BuiltinKw::IntegerPack))
            {
                e.builtin = None;
            }
        }
        let i = interner.intern(TypeTrait::TypesCompatible.spelling()).index() as usize;
        if self.table.len() <= i {
            self.table.resize(i + 1, Entry::default());
        }
        self.table[i].builtin = Some(BuiltinKw::Trait(TypeTrait::TypesCompatible));
        self.add(interner, "_Bool", Kw::Bool);
        if c >= CStd::C99 {
            self.add(interner, "restrict", Kw::Restrict);
        }
        if c == CStd::C89 && !gnu {
            self.remove(interner, "inline");
        }
        if !gnu {
            self.remove(interner, "asm");
        }
        if c >= CStd::C23 {
            self.add(interner, "typeof", Kw::Typeof);
            self.add(interner, "typeof_unqual", Kw::TypeofUnqual);
            self.add(interner, "nullptr", Kw::Nullptr);
        } else {
            for k in C23_KEYWORDS {
                self.remove(interner, k);
            }
        }
    }

    /// The builtin trait or transform `sym` names, if any.
    pub fn builtin(&self, sym: Symbol) -> Option<BuiltinKw> {
        self.table.get(sym.index() as usize).and_then(|e| e.builtin)
    }

    /// The keyword `sym` spells, if any.
    pub fn get(&self, sym: Symbol) -> Option<Kw> {
        self.table.get(sym.index() as usize).and_then(|e| e.kw)
    }

    /// Whether `sym` is an identifier: neither a keyword nor a trait
    /// keyword ([`TraitKw`]) that has not been reverted.
    pub fn is_ident(&self, sym: Symbol) -> bool {
        self.table
            .get(sym.index() as usize)
            .is_none_or(|e| e.kw.is_none() && e.trait_kw == TraitKw::Ident)
    }

    /// Whether `sym` is a revertible trait keyword not yet reverted.
    pub fn is_revertible_trait(&self, sym: Symbol) -> bool {
        self.table.get(sym.index() as usize).is_some_and(|e| e.trait_kw == TraitKw::Revertible)
    }

    /// Turns the revertible trait keyword `sym` into an identifier for
    /// the rest of the translation unit; it is still the trait when
    /// followed by `(`.
    pub fn revert_trait(&mut self, sym: Symbol) {
        if let Some(e) = self.table.get_mut(sym.index() as usize)
            && e.trait_kw == TraitKw::Revertible
        {
            e.trait_kw = TraitKw::Ident;
        }
    }
}

/// clang 22's revertible type-trait keywords (sorted): the names clang's
/// `ParseClassSpecifier` turns into identifiers after `struct`, so that
/// libstdc++ and libc++ may name class templates after them.
const REVERTIBLE_TRAITS: [&str; 56] = [
    "__is_abstract",
    "__is_aggregate",
    "__is_arithmetic",
    "__is_array",
    "__is_assignable",
    "__is_base_of",
    "__is_bounded_array",
    "__is_class",
    "__is_complete_type",
    "__is_compound",
    "__is_const",
    "__is_constructible",
    "__is_convertible",
    "__is_convertible_to",
    "__is_destructible",
    "__is_empty",
    "__is_enum",
    "__is_final",
    "__is_floating_point",
    "__is_function",
    "__is_fundamental",
    "__is_integral",
    "__is_literal",
    "__is_literal_type",
    "__is_lvalue_expr",
    "__is_lvalue_reference",
    "__is_member_function_pointer",
    "__is_member_object_pointer",
    "__is_member_pointer",
    "__is_nothrow_assignable",
    "__is_nothrow_constructible",
    "__is_nothrow_convertible",
    "__is_nothrow_destructible",
    "__is_object",
    "__is_pod",
    "__is_pointer",
    "__is_polymorphic",
    "__is_reference",
    "__is_rvalue_expr",
    "__is_rvalue_reference",
    "__is_same",
    "__is_same_as",
    "__is_scalar",
    "__is_scoped_enum",
    "__is_signed",
    "__is_standard_layout",
    "__is_trivial",
    "__is_trivially_assignable",
    "__is_trivially_constructible",
    "__is_trivially_copyable",
    "__is_trivially_equality_comparable",
    "__is_unbounded_array",
    "__is_union",
    "__is_unsigned",
    "__is_void",
    "__is_volatile",
];

/// The builtin traits that clang 22 does not make keywords in C++: plain
/// identifiers unless followed by `(`. (`__is_referenceable` at `compat`
/// 22 and `__builtin_types_compatible_p` are not C++ traits at all.)
const NON_KEYWORD_TRAITS: [&str; 4] =
    ["__builtin_is_structural", "__is_invocable", "__is_nothrow_invocable", "__is_nullptr"];

/// The builtin traits that are keywords in C too (clang `KEYALL` in
/// TokenKinds.def; the others are `KEYCXX`). `__builtin_types_compatible_p`
/// is C only (`KEYNOCXX`).
pub const C_TYPE_TRAITS: [TypeTrait; 3] =
    [TypeTrait::IsDestructible, TypeTrait::IsNothrowDestructible, TypeTrait::IsBitwiseCloneable];

/// The keywords new in C++11 other than `nullptr` (which
/// [`Keywords::with_std`] drops): before C++11 they are ordinary
/// identifiers in C++ (clang's `KEYCXX11`; `struct static_assert;` is
/// valid C++98; spec/parse.md §1). C keeps its own set ([`for_c`]).
///
/// [`for_c`]: Keywords::for_c
pub const CXX11_KEYWORDS: [&str; 9] = [
    "alignas",
    "alignof",
    "char16_t",
    "char32_t",
    "constexpr",
    "decltype",
    "noexcept",
    "static_assert",
    "thread_local",
];

/// The keywords new in C++20 other than `char8_t`: before C++20 they are
/// ordinary identifiers (clang's `CXX20_KEYWORD` and `COROUTINES_KEYWORD`
/// in TokenKinds.def, `KEYCXX20` / `KEYCOROUTINES`; spec/parse.md §1).
pub const CXX20_KEYWORDS: [&str; 7] =
    ["concept", "consteval", "constinit", "co_await", "co_return", "co_yield", "requires"];

/// The C++ keywords that are identifiers in every C mode (clang
/// `KEYCXX`, `KEYCXX11`, `KEYCXX20`, `KEYCXX26`, `KEYMS|KEYBORLAND`
/// keywords; checked against clang 19 `__is_identifier` with `-x c`).
pub const CXX_ONLY_KEYWORDS: [&str; 44] = [
    "catch",
    "char8_t",
    "char16_t",
    "char32_t",
    "class",
    "co_await",
    "co_return",
    "co_yield",
    "concept",
    "const_cast",
    "consteval",
    "constinit",
    "contract_assert",
    "decltype",
    "__decltype",
    "delete",
    "dynamic_cast",
    "explicit",
    "export",
    "friend",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "operator",
    "private",
    "protected",
    "public",
    "reinterpret_cast",
    "requires",
    "static_cast",
    "template",
    "this",
    "throw",
    "try",
    "typeid",
    "typename",
    "using",
    "virtual",
    "wchar_t",
    "__null",
    "__datasizeof",
    "__declspec",
    "__builtin_source_location",
];

/// The keywords C23 adds over C17 (C23 6.4.1; before C23 they are
/// identifiers, and `<stdbool.h>`/`<stdalign.h>`/... spell them as macros).
pub const C23_KEYWORDS: [&str; 9] = [
    "alignas",
    "alignof",
    "bool",
    "constexpr",
    "false",
    "nullptr",
    "static_assert",
    "thread_local",
    "true",
];

/// The alternative tokens ([lex.digraph]/2) other than the digraphs (which
/// the lexer maps itself), with the punctuators they stand for.
pub const ALTERNATIVE_TOKENS: [(&str, kiln_lex::Punct); 11] = {
    use kiln_lex::Punct as P;
    [
        ("and", P::AmpAmp),
        ("or", P::PipePipe),
        ("not", P::Exclaim),
        ("bitand", P::Amp),
        ("bitor", P::Pipe),
        ("xor", P::Caret),
        ("compl", P::Tilde),
        ("and_eq", P::AmpEqual),
        ("or_eq", P::PipeEqual),
        ("xor_eq", P::CaretEqual),
        ("not_eq", P::ExclaimEqual),
    ]
};

/// The punctuator an alternative token ([lex.digraph]) stands for.
pub fn alternative_token(s: &str) -> Option<kiln_lex::Punct> {
    ALTERNATIVE_TOKENS.iter().find(|(a, _)| *a == s).map(|&(_, p)| p)
}
