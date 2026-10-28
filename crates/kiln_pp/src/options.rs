//! Preprocessor options.

use kiln_lex::{LexOptions, Std};

/// Options of one preprocessor run.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug)]
pub struct PpOptions {
    /// Language standard (affects the lexer, `#elifdef`, `#warning`,
    /// `__VA_OPT__`, module directives).
    pub std: Std,
    /// `-trigraphs`.
    pub trigraphs: bool,
    /// Accept `$` in identifiers (clang default: yes).
    pub dollar_in_identifiers: bool,
    /// `-C`: keep comments in the output (they reach [`crate::Preprocessor::next`]
    /// as `TokKind::Comment` tokens, except inside directives).
    pub keep_comments: bool,
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
}

impl PpOptions {
    /// Defaults for `std`: no predefines, clang's limits.
    pub fn new(std: Std) -> PpOptions {
        PpOptions {
            std,
            trigraphs: false,
            dollar_in_identifiers: true,
            keep_comments: false,
            keep_macro_comments: false,
            predefines: String::new(),
            defines: Vec::new(),
            undefines: Vec::new(),
            max_include_depth: 200,
            reproducible_time: None,
            timestamp: None,
        }
    }

    /// The lexer options these options imply.
    pub fn lex_options(&self) -> LexOptions {
        let mut o = LexOptions::new(self.std);
        o.trigraphs = self.trigraphs;
        o.dollar_in_identifiers = self.dollar_in_identifiers;
        o.keep_comments = self.keep_comments || self.keep_macro_comments;
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
