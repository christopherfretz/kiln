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
    Complex = "_Complex" | "__complex__" | "__complex";
    Real = "__real__" | "__real";
    Imag = "__imag__" | "__imag";
    BuiltinOffsetof = "__builtin_offsetof";
    BuiltinChooseExpr = "__builtin_choose_expr";
    BuiltinBitCast = "__builtin_bit_cast";
    BuiltinVaArg = "__builtin_va_arg";
    BuiltinConvertVector = "__builtin_convertvector";
    BuiltinFile = "__builtin_FILE";
    BuiltinFileName = "__builtin_FILE_NAME";
    BuiltinFunction = "__builtin_FUNCTION";
    Func = "__func__";
    FunctionName = "__FUNCTION__";
    PrettyFunction = "__PRETTY_FUNCTION__";
    BuiltinLine = "__builtin_LINE";
    BuiltinColumn = "__builtin_COLUMN";
    BuiltinSourceLocation = "__builtin_source_location";
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
}

/// A builtin type-trait or type-transform name.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BuiltinKw {
    /// A `TypeTraitExpr`, `ArrayTypeTraitExpr` or `ExpressionTraitExpr`.
    Trait(TypeTrait),
    /// A `UnaryTransform` type-specifier.
    Transform(UnaryTransformKind),
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
                {
                    continue;
                }
                slot(&mut table, interner.intern(s)).kw = Some(*k);
            }
        }
        for &t in TypeTrait::all() {
            if compat >= 22 && t == TypeTrait::IsReferenceable {
                continue;
            }
            slot(&mut table, interner.intern(t.spelling())).builtin = Some(BuiltinKw::Trait(t));
        }
        for v in 0..=u8::MAX {
            let Some(k) = UnaryTransformKind::from_u8(v) else { break };
            slot(&mut table, interner.intern(k.spelling())).builtin = Some(BuiltinKw::Transform(k));
        }
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

    /// The builtin trait or transform `sym` names, if any.
    pub fn builtin(&self, sym: Symbol) -> Option<BuiltinKw> {
        self.table.get(sym.index() as usize).and_then(|e| e.builtin)
    }

    /// The keyword `sym` spells, if any.
    pub fn get(&self, sym: Symbol) -> Option<Kw> {
        self.table.get(sym.index() as usize).and_then(|e| e.kw)
    }
}

/// The keywords new in C++20 other than `char8_t`: before C++20 they are
/// ordinary identifiers (clang's `CXX20_KEYWORD` and `COROUTINES_KEYWORD`
/// in TokenKinds.def, `KEYCXX20` / `KEYCOROUTINES`; spec/parse.md §1).
pub const CXX20_KEYWORDS: [&str; 7] =
    ["concept", "consteval", "constinit", "co_await", "co_return", "co_yield", "requires"];

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
