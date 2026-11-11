//! Lexer configuration: [`LexOptions`], [`Std`], [`LexMode`].

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

/// TEMPORARY WP-1276 stub of WP-1275's C standard (reverted at merge).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CStd {
    /// C89.
    C89,
    /// C99.
    C99,
    /// C11.
    C11,
    /// C17.
    C17,
    /// C23.
    C23,
}

/// Options fixed for the lifetime of a [`crate::Lexer`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "independent command-line switches")]
pub struct LexOptions {
    /// Language standard. Default: [`Std::Cxx26`].
    pub std: Std,
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
