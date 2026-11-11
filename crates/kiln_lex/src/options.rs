//! Lexer configuration: [`LexOptions`], [`Std`], [`CStd`], [`LexMode`].

/// The C++ language standard, as far as lexing depends on it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Std {
    /// C++98.
    Cxx98,
    /// C++03.
    Cxx03,
    /// C++11: raw strings, `u`/`U`/`u8` string prefixes, ud-suffixes.
    Cxx11,
    /// C++14: digit separators.
    Cxx14,
    /// C++17: `u8` character literals; trigraphs removed.
    Cxx17,
    /// C++20: `<=>`.
    Cxx20,
    /// C++23: `\u{...}` and `\N{...}`.
    Cxx23,
    /// C++26 (with `reflection`: `^^`, `[:`, `:]`, P2996).
    Cxx26,
}

/// The C language standard (`-std=c89` .. `-std=c23`, and the `gnu`
/// dialects), when the translation unit is C rather than C++
/// ([`LexOptions::c`]; spec/driver.md "Language mode").
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CStd {
    /// C89 / C90 (ISO/IEC 9899:1990).
    C89,
    /// C99: `inline`, `restrict`, `_Bool`, `_Complex`, `_Imaginary`.
    C99,
    /// C11: `_Alignas`, `_Alignof`, `_Atomic`, `_Generic`, `_Noreturn`,
    /// `_Static_assert`, `_Thread_local`.
    C11,
    /// C17 (C18): no new keywords.
    C17,
    /// C23: `bool`, `true`, `false`, `nullptr`, `constexpr`, ..., digit
    /// separators and `u8` character literals.
    C23,
}

/// Options fixed for the lifetime of a [`crate::Lexer`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "independent command-line switches")]
pub struct LexOptions {
    /// Language standard. Default: [`Std::Cxx26`]. In C mode
    /// ([`Self::c`]) the C++ standard sharing C's lexical rules
    /// (spec/lex.md "C mode").
    pub std: Std,
    /// The C standard when the input is C; `None` (the default) is C++.
    pub c: Option<CStd>,
    /// A GNU dialect (`-std=gnuNN`). Only C reads it: raw string literals
    /// are a GNU extension there from gnu99 on (clang 19).
    pub gnu: bool,
    /// Translate trigraphs in phase 1 ([lex.trigraph], removed in C++17).
    /// Default: `false` (clang's default for C++17 and later); when off,
    /// trigraphs outside comments are diagnosed with
    /// [`crate::LexDiagKind::TrigraphIgnored`].
    pub trigraphs: bool,
    /// Accept `$` as an identifier character (a common extension, diagnosed
    /// with [`crate::LexDiagKind::DollarInIdentifier`]). Default: `true`.
    pub dollar_in_identifiers: bool,
    /// Return comments as [`crate::TokKind::Comment`] tokens (`-C`/`-CC`)
    /// instead of treating them as whitespace. Default: `false`. Can be
    /// changed later with [`crate::Lexer::set_keep_comments`].
    pub keep_comments: bool,
    /// `-freflection` (P2996): lex `^^`, `[:` and `:]` as single tokens
    /// (C++26 only). Default: `false`; like clang and GCC 16, the tokens
    /// exist only in reflection mode, so `^^` is otherwise `^` `^`.
    pub reflection: bool,
}

impl LexOptions {
    /// Options for `std` with every other field at its default.
    pub const fn new(std: Std) -> Self {
        LexOptions {
            std,
            c: None,
            gnu: false,
            trigraphs: false,
            dollar_in_identifiers: true,
            keep_comments: false,
            reflection: false,
        }
    }
}

impl Default for LexOptions {
    fn default() -> Self {
        LexOptions::new(Std::Cxx26)
    }
}

/// How the next token is lexed. Set by the preprocessor with
/// [`crate::Lexer::set_mode`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum LexMode {
    /// Ordinary pp-token lexing.
    #[default]
    Normal,
    /// The next token may be a header-name ([lex.header]): used after
    /// `#include`, `#embed`, `__has_include(`, `import` and the like. The
    /// mode applies to exactly one token (comments returned with
    /// `keep_comments` do not count) and then resets to `Normal`.
    HeaderName,
}
