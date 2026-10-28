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
    /// C++26: `^^`, `[:`, `:]` (P2996).
    Cxx26,
}

/// Options fixed for the lifetime of a [`crate::Lexer`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
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
}

impl LexOptions {
    /// Options for `std` with every other field at its default.
    pub const fn new(std: Std) -> Self {
        LexOptions { std, trigraphs: false, dollar_in_identifiers: true, keep_comments: false }
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
