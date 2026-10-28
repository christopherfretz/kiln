//! The diagnostic table: every [`DiagId`] with its kind, warning group,
//! argument count and message template (`spec/diag.md` section "Ids").
//!
//! Adding a diagnostic is a one-line change to the `diag_ids!` invocation
//! below. Each line reads
//!
//! ```text
//! name = Kind [Group] (nargs) "template";
//! ```
//!
//! where `[Group]` is optional and names a [`Group`] variant, `nargs` is the
//! number of arguments the emitter passes with [`Diag::arg`](crate::Diag::arg),
//! and the template uses `{0}`, `{1}`, ... (see [`crate::template`]). A
//! compile-time assertion checks every template against its `nargs`.
//! Wording follows clang's wherever clang has the diagnostic.

use crate::groups::Group;
use crate::policy::Severity;
use crate::template;

/// The kind of a diagnostic: its class (error, warning, extension, remark,
/// note) and its default severity, mirroring clang's `TableGen` classes
/// (`Error`, `Warning`, `DefaultIgnore`, `DefaultError`, `ExtWarn`, ...).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiagKind {
    /// A hard error (clang `Error`). Not controllable by `-W` options.
    Error,
    /// A fatal error (clang `DefaultFatal`): stops all later diagnostics.
    Fatal,
    /// A warning, on by default.
    Warning,
    /// A warning, off by default (clang `DefaultIgnore`).
    WarningOff,
    /// A warning that is an error by default (clang `DefaultError`); `-w`
    /// does not silence it.
    WarningError,
    /// A warning that `-Werror` does not promote (clang
    /// `DefaultWarnNoWerror`), e.g. `#pragma message`.
    WarningNoWerror,
    /// An extension diagnostic, off unless `-pedantic` (clang `Extension`).
    Extension,
    /// An extension diagnostic, on by default (clang `ExtWarn`).
    ExtWarn,
    /// A remark, off unless `-R<group>` (clang `Remark`).
    Remark,
    /// A note; only ever attached to another diagnostic.
    Note,
}

/// Diagnostic class, as clang's `CLASS_*`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiagClass {
    /// Hard and fatal errors.
    Error,
    /// Warnings of every default severity.
    Warning,
    /// Extension diagnostics (`-pedantic` affects them).
    Extension,
    /// Remarks.
    Remark,
    /// Notes.
    Note,
}

impl DiagKind {
    /// The class of diagnostics of this kind.
    pub const fn class(self) -> DiagClass {
        match self {
            DiagKind::Error | DiagKind::Fatal => DiagClass::Error,
            DiagKind::Warning
            | DiagKind::WarningOff
            | DiagKind::WarningError
            | DiagKind::WarningNoWerror => DiagClass::Warning,
            DiagKind::Extension | DiagKind::ExtWarn => DiagClass::Extension,
            DiagKind::Remark => DiagClass::Remark,
            DiagKind::Note => DiagClass::Note,
        }
    }

    /// Severity before any option is applied. Notes report `Ignored`
    /// (they follow their parent).
    pub const fn default_severity(self) -> Severity {
        match self {
            DiagKind::Error | DiagKind::WarningError => Severity::Error,
            DiagKind::Fatal => Severity::Fatal,
            DiagKind::Warning | DiagKind::WarningNoWerror | DiagKind::ExtWarn => Severity::Warning,
            DiagKind::WarningOff | DiagKind::Extension | DiagKind::Remark | DiagKind::Note => {
                Severity::Ignored
            }
        }
    }
}

macro_rules! diag_ids {
    ($( $name:ident = $kind:ident $([$group:ident])? ($n:literal) $tmpl:literal; )*) => {
        /// A stable diagnostic identifier. See the table in `ids.rs`.
        ///
        /// Variants are `snake_case` so that the name printed in JSON output
        /// and used in test expectations is the variant name verbatim.
        #[allow(non_camel_case_types)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u16)]
        pub enum DiagId {
            $(
                #[doc = concat!("`", $tmpl, "`")]
                $name,
            )*
        }

        impl DiagId {
            /// Every id, in table order (`ALL[id as usize] == id`).
            pub const ALL: &'static [DiagId] = &[$(DiagId::$name),*];

            /// The id's name (the variant name).
            pub const fn name(self) -> &'static str {
                match self { $(DiagId::$name => stringify!($name),)* }
            }

            /// The id's kind (class and default severity).
            pub const fn kind(self) -> DiagKind {
                match self { $(DiagId::$name => DiagKind::$kind,)* }
            }

            /// The warning group the id belongs to, if any.
            pub const fn group(self) -> Option<Group> {
                match self { $(DiagId::$name => diag_ids!(@group $($group)?),)* }
            }

            /// Number of arguments the message template takes.
            pub const fn arg_count(self) -> usize {
                match self { $(DiagId::$name => $n,)* }
            }

            /// The message template (`{0}` placeholders, `{0:a|b}` selects).
            pub const fn template(self) -> &'static str {
                match self { $(DiagId::$name => $tmpl,)* }
            }
        }

        // Every template uses exactly placeholders 0..nargs (spec/diag.md
        // "Ids": the static arg-count check).
        $(
            const _: () = assert!(
                template::check($tmpl, $n),
                concat!("template of `", stringify!($name), "` does not match its argument count"),
            );
        )*
    };
    (@group $g:ident) => { Some(Group::$g) };
    (@group) => { None };
}

diag_ids! {
    // ----- lexer ([lex]) -----------------------------------------------
    lex_invalid_utf8 = Error (0) "source file is not valid UTF-8";
    lex_invalid_char = Error (1) "unexpected character <U+{0}>";
    lex_backslash_newline_space = Warning [BackslashNewlineEscape] (0) "backslash and newline separated by space";
    lex_unterminated_comment = Error (0) "unterminated /* comment";
    lex_unterminated_string = ExtWarn [InvalidPpToken] (0) "missing terminating '\"' character";
    lex_unterminated_char = ExtWarn [InvalidPpToken] (0) "missing terminating ' character";
    lex_unterminated_raw_string = Error (1) "raw string missing terminating delimiter ){0}\"";
    lex_raw_delim_too_long = Error (0) "raw string delimiter longer than 16 characters; use PREFIX( )PREFIX to delimit raw string";

    // ----- preprocessor ([cpp]) ----------------------------------------
    pp_invalid_directive = Error (0) "invalid preprocessing directive";
    pp_macro_name_missing = Error (0) "macro name missing";
    pp_macro_name_not_identifier = Error (0) "macro name must be an identifier";
    pp_macro_redefined = ExtWarn [MacroRedefined] (1) "'{0}' macro redefined";
    pp_macro_unused = WarningOff [UnusedMacros] (0) "macro is not used";
    pp_unterminated_conditional = Error (0) "unterminated conditional directive";
    pp_else_without_if = Error (0) "#else without #if";
    pp_elif_without_if = Error (0) "#elif without #if";
    pp_endif_without_if = Error (0) "#endif without #if";
    pp_file_not_found = Fatal (1) "'{0}' file not found";
    pp_include_nested_too_deeply = Error (0) "#include nested too deeply";
    pp_error_directive = Error (1) "{0}";
    pp_warning_directive = Warning [PoundWarning] (1) "{0}";
    pp_pragma_message = WarningNoWerror [PoundPragmaMessage] (1) "{0}";
    pp_expected_eol = ExtWarn [ExtraTokens] (1) "extra tokens at end of #{0} directive";
    pp_expected_value_in_expr = Error (0) "expected value in expression";
    pp_invalid_token_in_if = Error (0) "invalid token at start of a preprocessor expression";
    pp_division_by_zero = Error (1) "{0:division|remainder} by zero in preprocessor expression";
    pp_line_invalid_number = Error (0) "#line directive requires a positive integer argument";
    pp_macro_args_mismatch = Error (1) "too {0:few|many} arguments provided to function-like macro invocation";
    pp_unterminated_macro_args = Error (0) "unterminated function-like macro invocation";
    pp_hash_not_followed_by_param = Error (0) "'#' is not followed by a macro parameter";
    pp_paste_at_edge = Error (1) "'##' cannot appear at {0:start|end} of macro expansion";
    pp_paste_invalid_token = Error (1) "pasting formed '{0}', an invalid preprocessing token";
    pp_unknown_pragma = WarningOff [UnknownPragmas] (0) "unknown pragma ignored";

    // ----- shared notes ------------------------------------------------
    note_previous_definition = Note (0) "previous definition is here";
    note_macro_here = Note (1) "macro '{0}' defined here";

    // ----- driver and option parsing -----------------------------------
    drv_no_input_files = Error (0) "no input files";
    drv_unknown_argument = Error (1) "unknown argument: '{0}'";
    drv_unknown_argument_suggest = Error (2) "unknown argument '{0}'; did you mean '{1}'?";
    drv_invalid_int_value = Error (2) "invalid integral value '{1}' in '{0}'";
    drv_invalid_value = Error (2) "invalid value '{1}' in '{0}'";
    drv_ignoring_tabstop = Warning (2) "ignoring invalid -ftabstop value '{0}', using default value {1}";

    // ----- produced by the diagnostics engine itself --------------------
    diag_unknown_warning_option = Warning [UnknownWarningOption] (4) "unknown {0:warning|remark} option '{1}'{2:|; did you mean '{3}'?}";
    diag_unknown_warning_specifier = Warning [UnknownWarningOption] (2) "unknown {0} warning specifier: '{1}'";
    diag_too_many_errors = Fatal (0) "too many errors emitted, stopping now";
    diag_expanded_from_macro = Note (1) "expanded from macro '{0}'";
    diag_expanded_from_here = Note (0) "expanded from here";
    diag_backtrace_skipped = Note (1) "(skipping {0} expansions in backtrace; use -fmacro-backtrace-limit=0 to see all)";
}

impl DiagId {
    /// Number of ids in the table.
    pub const COUNT: usize = DiagId::ALL.len();

    /// The id's class.
    pub const fn class(self) -> DiagClass {
        self.kind().class()
    }

    /// Severity before options are applied.
    pub const fn default_severity(self) -> Severity {
        self.kind().default_severity()
    }

    /// True for warnings and extensions: the ids `-W` options control
    /// (clang's `isBuiltinWarningOrExtension`).
    pub const fn is_warning_or_extension(self) -> bool {
        matches!(self.class(), DiagClass::Warning | DiagClass::Extension)
    }

    /// True if the id is an error unless options say otherwise (clang's
    /// `isDefaultMappingAsError`).
    pub const fn is_default_error(self) -> bool {
        matches!(self.default_severity(), Severity::Error | Severity::Fatal)
    }

    /// The group whose name is shown in `[-W...]` and in JSON output:
    /// [`group`](Self::group), or `pedantic` for an extension diagnostic
    /// without a group (clang puts those in `-Wpedantic`).
    pub fn display_group(self) -> Option<Group> {
        self.group().or((self.class() == DiagClass::Extension).then_some(Group::Pedantic))
    }

    /// Looks an id up by name.
    pub fn from_name(name: &str) -> Option<DiagId> {
        DiagId::ALL.iter().copied().find(|id| id.name() == name)
    }
}
