//! The keyword table ([lex.key], plus the GNU spellings kiln accepts).
//!
//! The lexer produces every keyword as an identifier; the parser maps the
//! identifier's symbol to a [`Kw`] through one hash lookup.

use std::collections::HashMap;

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
    Asm = "asm" | "__asm" | "__asm__";
    Auto = "auto";
    Bool = "bool" | "_Bool";
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
    Nullptr = "nullptr";
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
}

/// Symbol-to-keyword map for one interner.
#[derive(Debug)]
pub struct Keywords {
    map: HashMap<Symbol, Kw>,
}

impl Keywords {
    /// Interns every keyword spelling in `interner`.
    pub fn new(interner: &Interner) -> Self {
        let mut map = HashMap::new();
        for (k, spellings) in Kw::TABLE {
            for s in *spellings {
                map.insert(interner.intern(s), *k);
            }
        }
        Keywords { map }
    }

    /// The keyword `sym` spells, if any.
    pub fn get(&self, sym: Symbol) -> Option<Kw> {
        self.map.get(&sym).copied()
    }
}

/// The punctuator an alternative token ([lex.digraph]) stands for.
pub fn alternative_token(s: &str) -> Option<kiln_lex::Punct> {
    use kiln_lex::Punct as P;
    Some(match s {
        "and" => P::AmpAmp,
        "or" => P::PipePipe,
        "not" => P::Exclaim,
        "bitand" => P::Amp,
        "bitor" => P::Pipe,
        "xor" => P::Caret,
        "compl" => P::Tilde,
        "and_eq" => P::AmpEqual,
        "or_eq" => P::PipeEqual,
        "xor_eq" => P::CaretEqual,
        "not_eq" => P::ExclaimEqual,
        _ => return None,
    })
}
