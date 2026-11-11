//! Preprocessor options.

use kiln_lex::{CStd, LexOptions, Std};

/// Which clang release's answers kiln reproduces (`-fclang-compat=<major>`,
/// spec/pp.md "clang compatibility level"): the version predefines
/// (`__clang_major__`, `__VERSION__`, ...), the other non-`__cpp_*`
/// predefined macros that differ between the releases, and the
/// `__has_builtin` / `__has_feature` / `__has_extension` /
/// `__has_attribute` / `__has_cpp_attribute` / `__is_identifier`
/// answers. The `__cpp_*` feature-test macros follow kiln's own feature
/// status, not this level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClangCompat {
    /// clang 19.1.7.
    Clang19,
    /// clang 22.1.8 (the default, as the driver's `-fclang-compat`).
    #[default]
    Clang22,
}

impl ClangCompat {
    /// Every level, oldest first.
    pub const ALL: [ClangCompat; 2] = [ClangCompat::Clang19, ClangCompat::Clang22];

    /// The level for clang major version `major`, if kiln models it.
    pub fn from_major(major: u32) -> Option<ClangCompat> {
        ClangCompat::ALL.into_iter().find(|c| c.major() == major)
    }

    /// The clang major version (`19`, `22`).
    pub fn major(self) -> u32 {
        match self {
            ClangCompat::Clang19 => 19,
            ClangCompat::Clang22 => 22,
        }
    }

    /// The full clang version (`19.1.7`, `22.1.8`): `__clang_major__`,
    /// `__clang_minor__` and `__clang_patchlevel__` joined, and what the
    /// driver's `-dumpversion` prints (WP-920).
    pub fn version(self) -> &'static str {
        match self {
            ClangCompat::Clang19 => "19.1.7",
            ClangCompat::Clang22 => "22.1.8",
        }
    }
}

/// The standard library the driver selected, for the note after a
/// missing `#include <meta>` at C++26 (WP-1210, spec/pp.md "Missing
/// `<meta>`"): `<meta>` (P2996) ships only with GCC 16.1's libstdc++.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetaHintLib {
    /// `-stdlib=libc++`, which has no `<meta>`.
    LibCxx,
    /// libstdc++ from the GCC installation at `dir`, version `version`.
    Gcc {
        /// The installation directory (`<prefix>/lib/gcc/<triple>/<v>`).
        dir: String,
        /// Its version directory name (`14`).
        version: String,
    },
    /// libstdc++, but no GCC installation was found.
    NoGcc,
}

/// What the note after a missing `#include <meta>` says (WP-1210).
#[derive(Clone, Debug)]
pub struct MetaHint {
    /// The selected library.
    pub lib: MetaHintLib,
    /// Looks for a GCC 16 installation on `PATH`: its `--gcc-toolchain`
    /// prefix and the executable that reported it (`g++-16`). Called
    /// only when the note is emitted, never on a successful compile.
    pub probe: fn() -> Option<(String, String)>,
}

/// Options of one preprocessor run.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug)]
pub struct PpOptions {
    /// Language standard (affects the lexer, `#elifdef`, `#warning`,
    /// `__VA_OPT__`, module directives).
    pub std: Std,
    /// The C standard when the input is C (`None`: C++); passed to the
    /// lexer ([`LexOptions::c`]; spec/driver.md "Language mode").
    pub c: Option<CStd>,
    /// A GNU dialect (`-std=gnuNN`); passed to the lexer
    /// ([`LexOptions::gnu`]).
    pub gnu: bool,
    /// `-trigraphs`.
    pub trigraphs: bool,
    /// Accept `$` in identifiers (clang default: yes).
    pub dollar_in_identifiers: bool,
    /// `-C`: keep comments in the output (they reach [`crate::Preprocessor::next`]
    /// as `TokKind::Comment` tokens, except inside directives).
    pub keep_comments: bool,
    /// The output is preprocessed text (`-E`, `-M`): strict C89 lexes `//`
    /// as two `/` tokens there ([`LexOptions::preprocessed_output`]).
    pub preprocessed_output: bool,
    /// `-CC`: implies `-C`, and also keeps comments inside macro
    /// arguments. (Comments in `#define` bodies are not retained yet:
    /// clang keeps them, rewriting `//` comments as `/* */`.)
    pub keep_macro_comments: bool,
    /// Text of the predefines buffer: `#define` lines for the target and
    /// language (normally `resource/predefines/<triple>.<std>.txt`), read
    /// before the main file as clang's `<built-in>` buffer. `__cplusplus`
    /// comes from here.
    pub predefines: String,
    /// `-D name` / `-D name=value`, in command-line order (interleaved with
    /// `undefines` by the driver if order matters; here defines go first).
    pub defines: Vec<(String, Option<String>)>,
    /// `-U name`.
    pub undefines: Vec<String>,
    /// `#include` nesting limit (clang: 200).
    pub max_include_depth: u32,
    /// Fixed `__DATE__` and `__TIME__` spellings (without quotes), for
    /// reproducible output. `None` uses the current time.
    pub reproducible_time: Option<(String, String)>,
    /// Fixed `__TIMESTAMP__` spelling (without quotes). `None` uses the
    /// modification time of the file if known, else `??? ??? ?? ??:??:?? ????`.
    pub timestamp: Option<String>,
    /// `-freflection`: lex the P2996 tokens `^^`, `[:`, `:]` (C++26).
    pub reflection: bool,
    /// Modules enabled (C++20 and later, not with `-E`): module and import
    /// directives are consumed and imports performed (spec/modules.md).
    pub modules: Option<crate::ModuleOptions>,
    /// The parser runs on this stream: the loop-hint pragmas (`unroll`,
    /// `nounroll`, `unroll_and_jam`, `nounroll_and_jam`, `GCC unroll`,
    /// `GCC nounroll`, `clang loop`) macro-expand their tokens, as the
    /// parser's `PragmaUnrollHintHandler` / `PragmaLoopHintHandler` do in
    /// clang; `-E` keeps them unexpanded (spec/parse.md "Loop hints").
    pub parser_pragmas: bool,
    /// `-f[no-]char8_t`: a `u8` character literal in `#if` is unsigned
    /// as `char8_t` (`None`: from C++20 on).
    pub char8: Option<bool>,
    /// `-MG`: an `#include` whose file is not found is skipped without
    /// an error (clang `SetSuppressIncludeNotFoundError`); the dependency
    /// callbacks still see it.
    pub suppress_include_not_found: bool,
    /// `-dD`: queue a [`crate::Marker::Define`] / [`crate::Marker::Undef`]
    /// for every `#define` / `#undef` (the `-E` printer prints them).
    pub dump_defines: bool,
    /// `-dI`: queue a [`crate::Marker::Include`] for every `#include`,
    /// `#include_next` and `#import`.
    pub dump_includes: bool,
    /// The main file is a header (clang `LangOptions::IsHeaderFile`: a
    /// `-x c++-header` input being precompiled, WP-1023): `#pragma once`
    /// in it is not `-Wpragma-once-outside-header`.
    pub main_is_header: bool,
    /// Set by the driver at C++26: an angled `#include <meta>` that is
    /// not found gets a note naming the selected library (WP-1210).
    pub meta_hint: Option<MetaHint>,
    /// `-fclang-compat=19`: decode `#if` character literals with clang
    /// 19's escape rules (`LiteralOptions::clang19_escapes`; an empty
    /// `\x{}` leaves its `}`, WP-1268).
    pub clang19_escapes: bool,
}

impl PpOptions {
    /// Defaults for `std`: no predefines, clang's limits.
    pub fn new(std: Std) -> PpOptions {
        PpOptions {
            std,
            c: None,
            gnu: false,
            trigraphs: false,
            dollar_in_identifiers: true,
            keep_comments: false,
            preprocessed_output: false,
            keep_macro_comments: false,
            predefines: String::new(),
            defines: Vec::new(),
            undefines: Vec::new(),
            max_include_depth: 200,
            reproducible_time: None,
            timestamp: None,
            reflection: false,
            modules: None,
            parser_pragmas: false,
            char8: None,
            suppress_include_not_found: false,
            dump_defines: false,
            dump_includes: false,
            main_is_header: false,
            meta_hint: None,
            clang19_escapes: false,
        }
    }

    /// The lexer options these options imply.
    pub fn lex_options(&self) -> LexOptions {
        let mut o = LexOptions::new(self.std);
        o.c = self.c;
        o.gnu = self.gnu;
        o.trigraphs = self.trigraphs;
        o.dollar_in_identifiers = self.dollar_in_identifiers;
        o.keep_comments = self.keep_comments || self.keep_macro_comments;
        o.reflection = self.reflection;
        o.preprocessed_output = self.preprocessed_output;
        o.clang19 = self.clang19_escapes;
        o
    }

    /// True for C++20 and later.
    pub fn cxx20(&self) -> bool {
        self.std >= Std::Cxx20
    }
}

impl Default for PpOptions {
    fn default() -> Self {
        PpOptions::new(Std::Cxx26)
    }
}
