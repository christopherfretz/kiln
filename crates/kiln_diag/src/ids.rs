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
    lex_char_not_allowed_in_identifier = Error (1) "character <U+{0}> not allowed in an identifier";
    lex_unicode_whitespace = ExtWarn [UnicodeWhitespace] (0) "treating Unicode character as whitespace";
    lex_null_character = Warning [NullCharacter] (0) "null character ignored";
    lex_backslash_newline_eof = Extension (0) "backslash-newline at end of file";
    lex_trigraph_ignored = Warning [Trigraphs] (0) "trigraph ignored";
    lex_trigraph_converted = Warning [Trigraphs] (1) "trigraph converted to '{0}' character";
    lex_no_newline_eof = WarningOff [NewlineEof] (0) "no newline at end of file";
    lex_nested_block_comment = Warning [Comment] (0) "'/*' within block comment";
    lex_escaped_newline_block_comment_end = Warning [Comment] (0) "escaped newline between */ characters at block comment end";
    lex_multi_line_line_comment = Extension [Comment] (0) "multi-line // comment";
    lex_empty_character = ExtWarn [InvalidPpToken] (0) "empty character constant";
    lex_raw_delim_invalid_char = Error (1) "invalid character '{0}' character in raw string delimiter; use PREFIX( )PREFIX to delimit raw string";
    lex_reserved_ud_suffix = WarningError [ReservedUserDefinedLiteral] (0) "invalid suffix on literal; C++11 requires a space between literal and identifier";
    lex_ucn_no_digits = Warning [Unicode] (1) "\\{0} used with no following hex digits; treating as '\\' followed by identifier";
    lex_ucn_incomplete = Warning [Unicode] (0) "incomplete universal character name; treating as '\\' followed by identifier";
    lex_ucn_delimited_empty = Warning [Unicode] (0) "empty delimited universal character name; treating as '\\' 'u' '{{' '}}'";
    lex_ucn_delimited_incomplete = Warning [Unicode] (0) "incomplete delimited universal character name; treating as '\\' 'u' '{{' identifier";
    lex_ucn_too_large = Error (0) "hex escape sequence out of range";
    lex_ucn_basic_scs = Error (1) "character '{0}' cannot be specified by a universal character name";
    lex_ucn_control_character = Error (0) "universal character name refers to a control character";
    lex_ucn_invalid = Error (0) "invalid universal character";
    lex_ucn_unknown_name = Error (1) "'{0}' is not a valid Unicode character name";
    lex_delimited_escape_ext = Extension [DelimitedEscapeSequenceExtension] (1) "{0:delimited|named} escape sequences are a C++23 extension";
    lex_identifier_not_nfc = WarningOff (0) "identifier is not in Normalization Form C";
    lex_dollar_in_identifier = Extension [DollarInIdentifierExtension] (0) "'$' in identifier";
    lex_spaceship_compat = WarningOff [Cxx20Compat] (0) "'<=>' is a single token in C++20; add a space to avoid a change in behavior";

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
    pp_defined_macro_name = Error (0) "'defined' cannot be used as a macro name";
    pp_operator_macro_name = Error (2) "C++ operator '{0}' (aka '{1}') used as a macro name";
    pp_builtin_macro_redefined = Warning [BuiltinMacroRedefined] (0) "redefining builtin macro";
    pp_builtin_macro_undefined = Warning [BuiltinMacroRedefined] (0) "undefining builtin macro";
    pp_macro_reserved_id = WarningOff [ReservedMacroIdentifier] (0) "macro name is a reserved identifier";
    pp_macro_hides_keyword = WarningOff [KeywordAsMacro] (0) "keyword is hidden by macro definition";
    pp_duplicate_param = Error (1) "duplicate macro parameter name '{0}'";
    pp_expected_comma_in_params = Error (0) "expected comma in macro parameter list";
    pp_invalid_token_in_params = Error (0) "invalid token in macro parameter list";
    pp_expected_ident_in_params = Error (0) "expected identifier in macro parameter list";
    pp_missing_rparen_in_params = Error (0) "missing ')' in macro parameter list";
    pp_missing_whitespace_after_name = ExtWarn (0) "ISO C99 requires whitespace after the macro name";
    pp_named_variadic = Extension [VariadicMacros] (0) "named variadic macros are a GNU extension";
    pp_va_args_misuse = Extension (0) "__VA_ARGS__ can only appear in the expansion of a C99 variadic macro";
    pp_va_opt_misuse = ExtWarn [VariadicMacros] (0) "__VA_OPT__ can only appear in the expansion of a variadic macro";
    pp_va_opt_nested = Error (0) "__VA_OPT__ cannot be nested within its own replacement tokens";
    pp_va_opt_missing_lparen = Error (0) "missing '(' following __VA_OPT__";
    pp_paste_at_va_opt_edge = Error (1) "'##' cannot appear at {0:start|end} of __VA_OPT__ argument";
    pp_expected_after = Error (2) "missing {1} after {0}";
    pp_paste_comma = Extension [GnuZeroVariadicMacroArguments] (0) "token pasting of ',' and __VA_ARGS__ is a GNU extension";
    pp_missing_varargs_arg = Extension [GnuZeroVariadicMacroArguments] (0) "must specify at least one argument for '...' parameter of variadic macro";
    pp_invalid_string_literal = Warning (0) "invalid string literal, ignoring final '\\'";
    pp_disabled_macro_expansion = WarningOff [DisabledMacroExpansion] (0) "disabled expansion of recursive macro";
    pp_embedded_directive = Error (1) "embedding a #{0} directive within macro arguments is not supported";
    pp_embedded_directive_ext = Extension [EmbeddedDirective] (0) "embedding a directive within macro arguments has undefined behavior";
    pp_else_after_else = Error (0) "#else after #else";
    pp_elif_after_else = Error (1) "#{0} after #else";
    pp_elifdef_without_if = Error (1) "#{0} without #if";
    pp_elifdef_ext = Extension (1) "use of a '#{0}' directive is a C++23 extension";
    pp_line_marker_invalid_number = Error (0) "line marker directive requires a positive integer argument";
    pp_line_digit_sequence = Error (1) "{0:#line|line marker} directive requires a simple digit sequence";
    pp_line_decimal = Warning (1) "{0:#line|line marker} directive interprets number as decimal, not octal";
    pp_line_zero = Extension [GnuZeroLineDirective] (0) "#line directive with zero argument is a GNU extension";
    pp_line_too_big = Extension (1) "C requires #line number to be less than {0}, allowed as extension";
    pp_line_invalid_filename = Error (1) "invalid filename for {0:#line|line marker} directive";
    pp_line_marker_flag = Error (0) "invalid flag line marker directive";
    pp_line_marker_pop = Error (0) "invalid line marker flag '2': cannot pop empty include stack";
    pp_line_marker_ext = Extension [GnuLineMarker] (0) "this style of line directive is a GNU extension";
    pp_string_udl = Error (0) "string literal with user-defined suffix cannot be used here";
    pp_warning_directive_ext = Extension (0) "#warning is a C++23 extension";
    pp_ident_ext = Extension (0) "#ident is a language extension";
    pp_import_ext = Extension (0) "#import is a language extension";
    pp_include_next_ext = Extension [GnuIncludeNext] (0) "#include_next is a language extension";
    pp_include_next_in_primary = Warning [IncludeNextOutsideHeader] (0) "#include_next in primary source file; will search from start of include path";
    pp_include_next_absolute_path = Warning [IncludeNextAbsPath] (0) "#include_next in file found relative to primary source file or found by absolute path; will search from start of include path";
    pp_include_expects_file = Error (0) "expected \"FILENAME\" or <FILENAME>";
    pp_empty_filename = Error (0) "empty filename";
    pp_embed_ext = ExtWarn [C23Extensions] (0) "#embed is a Clang extension";
    pp_embed_unknown_param = Error (1) "unknown embed preprocessor parameter '{0}'";
    pp_embed_duplicate_param = Error (1) "cannot specify parameter '{0}' twice in the same '#embed' directive";
    pp_embed_malformed_param = Error (1) "expected '(' or ')' in parameter '{0}'";
    pp_embed_defined = Error (0) "'defined' cannot appear within this context";
    pp_embed_negative_value = Error (1) "invalid value '{0}'; must be positive";
    pp_header_unit_unsupported = Error (0) "importing a header unit is not supported yet";
    pp_module_name_is_macro = Error (1) "module name component '{0}' cannot be an object-like macro";
    pp_pragma_once_in_main = Warning [PragmaOnceOutsideHeader] (0) "#pragma once in main file";
    pp_pragma_system_header_in_main = Warning [PragmaSystemHeaderOutsideHeader] (0) "#pragma system_header ignored in main file";
    pp_pragma_operator_malformed = Error (0) "_Pragma takes a parenthesized string literal";
    pp_pragma_gcc_error = Error (1) "{0}";
    pp_pragma_message_malformed = Error (1) "pragma {0:message|warning|error} requires parenthesized string";
    pp_pragma_push_pop_malformed = Error (1) "pragma {0} requires a parenthesized string";
    pp_pragma_pop_without_push = Warning [UnknownPragmas] (1) "pragma pop_macro could not pop '{0}', no matching push_macro";
    pp_pragma_poison_malformed = Error (0) "can only poison identifier tokens";
    pp_poisoning_existing_macro = Warning (0) "poisoning existing macro";
    pp_poisoned_identifier = Error (0) "attempt to use a poisoned identifier";
    pp_has_include_outside_directive = Error (1) "'{0}' must be used within a preprocessing directive";
    pp_missing_lparen_after = Error (1) "missing '(' after '{0}'";
    pp_feature_check_malformed = Error (0) "builtin feature check macro requires a parenthesized identifier";
    pp_expected_id_building_module = Error (0) "expected a module name in '__building_module' expression";
    pp_defined_requires_identifier = Error (0) "operator 'defined' requires an identifier";
    pp_expansion_to_defined = Warning [ExpansionToDefined] (0) "macro expansion producing 'defined' has undefined behavior";
    pp_expansion_to_defined_fn = Extension [ExpansionToDefined] (0) "macro expansion producing 'defined' has undefined behavior";
    pp_expr_bad_token_binop = Error (0) "token is not a valid binary operator in a preprocessor subexpression";
    pp_expected_rparen_in_expr = Error (0) "expected ')' in preprocessor expression";
    pp_nested_paren = Error (1) "nested parentheses not permitted in '{0}'";
    // #if expressions ([cpp.cond], WP-015; clang PPExpressions.cpp)
    pp_undef_identifier = WarningOff [Undef] (1) "'{0}' is not defined, evaluates to 0";
    pp_expected_eol_in_expr = Error (0) "expected end of line in preprocessor expression";
    pp_illegal_floating_literal = Error (0) "floating point literal in preprocessor expression";
    pp_invalid_udl = Error (1) "{0:character|integer} literal with user-defined suffix cannot be used in preprocessor constant expression";
    pp_expr_overflow = Warning (0) "integer overflow in preprocessor expression";
    pp_convert_to_positive = Warning (2) "{0:left|right} side of operator converted from negative value to unsigned: {1}";
    pp_comma_expr = Extension (0) "comma operator in operand of #if";
    pp_colon_without_question = Error (0) "':' without preceding '?'";
    pp_long_long_ext = Extension [Cxx11LongLong] (0) "'long long' is a C++11 extension";
    // literal values ([lex.icon], [lex.ccon]; clang LiteralSupport.cpp)
    lit_invalid_digit = Error (2) "invalid digit '{0}' in {1:binary|octal|decimal} constant";
    lit_invalid_suffix = Error (2) "invalid suffix '{0}' on {1:integer|floating} constant";
    lit_exponent_no_digits = Error (0) "exponent has no digits";
    lit_hex_float_requires = Error (1) "hexadecimal floating literal requires {0:an exponent|a significand}";
    lit_digit_separator = Error (1) "digit separator cannot appear at {0:start|end} of digit sequence";
    lit_binary_ext = Extension [Cxx14BinaryLiteral] (0) "binary integer literals are a C++14 extension";
    lit_hex_float_ext = Extension [Cxx17Extensions] (0) "hexadecimal floating literals are a C++17 feature";
    lit_size_t_ext = ExtWarn [Cxx23Extensions] (0) "'size_t' suffix for literals is a C++23 extension";
    lit_unknown_escape = ExtWarn [UnknownEscapeSequence] (1) "unknown escape sequence '\\{0}'";
    lit_nonstandard_escape = Extension (1) "use of non-standard escape character '\\{0}'";
    lit_escape_no_digits = Error (1) "\\{0} used with no following hex digits";
    lit_escape_missing_brace = Error (1) "expected '{{' after '\\{0}' escape sequence";
    lit_delimited_escape_empty = Error (0) "delimited escape sequence cannot be empty";
    lit_delimited_escape_invalid = Error (1) "invalid digit '{0}' in escape sequence";
    lit_hex_escape_too_large = Error (0) "hex escape sequence out of range";
    lit_octal_escape_too_large = Error (0) "octal escape sequence out of range";
    lit_ucn_escape_incomplete = Error (0) "incomplete universal character name";
    lit_char_too_large = Error (0) "character too large for enclosing character literal type";
    lit_multichar = Warning [Multichar] (0) "multi-character character constant";
    lit_four_char = WarningOff [FourByteMultiChar] (0) "multi-character character constant";
    lit_char_constant_too_long = Warning (0) "character constant too long for its type";
    lit_multichar_not_allowed = Error (1) "{0:wide|Unicode} character literals may not contain multiple characters";
    lit_unevaluated_prefix = Error (0) "an unevaluated string literal cannot have an encoding prefix";
    lit_unevaluated_escape = Error (1) "invalid escape sequence '{0}' in an unevaluated string literal";
    lit_unevaluated_udl = Error (0) "an unevaluated string literal cannot be a user-defined literal";
    note_ucn_name_loose_match = Note (0) "characters names in Unicode escape sequences are sensitive to case and whitespaces";
    pp_has_warning_invalid_option = ExtWarn [MalformedWarningCheck] (0) "__has_warning expected option name (e.g. \"-Wundef\")";
    pp_expected_string_literal = Error (1) "expected string literal in {0}";
    pp_date_time = WarningOff [DateTime] (0) "expansion of date or time macro is not reproducible";
    pp_malformed_ident = Error (0) "invalid #ident directive";
    pp_expr_unsupported = Error (0) "kiln: this #if expression needs the full evaluator (WP-015)";
    pp_include_unsupported = Fatal (0) "kiln: no include resolver is installed";
    pp_expected = Error (1) "expected {0}";
    pp_pragma_non_macro = Error (1) "no macro named '{0}'";
    pp_pragma_diagnostic_unknown_warning = Warning [UnknownWarningOption] (1) "unknown warning group '{0}', ignored";
    pp_pragma_diagnostic_invalid = Warning [UnknownPragmas] (0) "pragma diagnostic expected 'error', 'warning', 'ignored', 'fatal', 'push', or 'pop'";
    pp_pragma_diagnostic_invalid_token = Warning [UnknownPragmas] (0) "unexpected token in pragma diagnostic";
    pp_pragma_diagnostic_invalid_option = Warning [UnknownPragmas] (0) "pragma diagnostic expected option name (e.g. \"-Wundef\")";
    pp_pragma_diagnostic_cannot_pop = Warning [UnknownPragmas] (0) "pragma diagnostic pop could not pop, no matching push";

    // ----- shared notes ------------------------------------------------
    note_previous_definition = Note (0) "previous definition is here";
    note_forward_declaration = Note (1) "forward declaration of {0}";
    note_macro_here = Note (1) "macro '{0}' defined here";
    note_macro_expansion_here = Note (1) "expansion of macro '{0}' requested here";
    note_matching = Note (1) "to match this {0}";
    note_pp_poisoned_here = Note (0) "poisoned here";

    // ----- driver and option parsing -----------------------------------
    drv_no_input_files = Error (0) "no input files";
    drv_unknown_argument = Error (1) "unknown argument: '{0}'";
    drv_unknown_argument_suggest = Error (2) "unknown argument '{0}'; did you mean '{1}'?";
    drv_invalid_int_value = Error (2) "invalid integral value '{1}' in '{0}'";
    drv_invalid_value = Error (2) "invalid value '{1}' in '{0}'";
    drv_ignoring_tabstop = Warning (2) "ignoring invalid -ftabstop value '{0}', using default value {1}";
    drv_no_such_file = Error (1) "no such file or directory: '{0}'";
    drv_missing_argument = Error (2) "argument to '{0}' is missing (expected {1} value)";
    drv_argument_not_allowed_with = Error (2) "invalid argument '{0}' not allowed with '{1}'";
    drv_invalid_stdlib_name = Error (1) "invalid library name in argument '{0}'";
    drv_language_not_recognized = Error (1) "language not recognized: '{0}'";
    drv_framework_dir_ignored = Warning (1) "ignoring framework directory '{0}' (kiln does not search frameworks)";
    drv_not_implemented = Error (1) "{0} is not implemented yet";
    drv_unsupported_target = Error (1) "unsupported target '{0}' (kiln supports aarch64 and x86_64 Linux)";
    drv_cannot_open_output = Error (2) "unable to open output file '{0}': '{1}'";
    drv_cannot_read_input = Fatal (2) "cannot read '{0}': {1}";
    drv_cannot_read_predefines = Fatal (2) "cannot read the predefined macros '{0}': {1}";
    // driver codegen path (WP-052b), clang's wording
    drv_out_file_multiple = Error (0) "cannot specify -o when generating multiple output files";
    drv_emit_llvm_link = Error (0) "-emit-llvm cannot be used when linking";
    drv_unable_to_execute = Error (1) "unable to execute command: Executable \"{0}\" doesn't exist!";
    drv_command_failed = Error (2) "{0} command failed with exit code {1} (use -v to see invocation)";
    drv_command_signalled = Error (1) "{0} command failed due to signal (use -v to see invocation)";
    drv_cannot_execute = Error (2) "unable to execute command: {0}: {1}";
    drv_linker_input_unused = Warning [UnusedCommandLineArgument] (1) "{0}: 'linker' input unused";
    drv_argument_unused = Warning [UnusedCommandLineArgument] (1) "argument unused during compilation: '{0}'";
    cg_error = Error (1) "{0}";

    // ----- sema conversions (WP-049): [conv], [expr.arith.conv], [lex.*con] --
    // Type arguments are passed pre-quoted, with clang's `(aka '...')`
    // suffix when sugar hides the canonical type (`Sema::ty_arg`).
    sema_int_literal_too_large = Error (0) "integer literal is too large to be represented in any integer type";
    sema_int_literal_implicitly_unsigned = ExtWarn [ImplicitlyUnsignedLiteral] (0) "integer literal is too large to be represented in a signed integer type, interpreting as unsigned";
    sema_size_t_literal_too_large = Error (1) "{0:signed |}'size_t' literal is out of range of possible {0:signed |}'size_t' values";
    sema_float_literal_overflow = Warning [LiteralRange] (2) "magnitude of floating-point constant too large for type {0}; maximum is {1}";
    sema_float_literal_underflow = Warning [LiteralRange] (2) "magnitude of floating-point constant too small for type {0}; minimum is {1}";
    sema_invalid_float_suffix = Error (1) "invalid suffix '{0}' on floating constant";
    sema_init_conversion_failed = Error (4) "cannot initialize {0:a variable|a parameter|return object|a value} of type {1} with an {2:rvalue|lvalue} of type {3}";
    sema_no_viable_conversion = Error (2) "no viable conversion from {0} to {1}";
    sema_no_viable_conversion_return = Error (2) "no viable conversion from returned value of type {0} to function return type {1}";
    sema_assign_incompatible = Error (2) "assigning to {0} from incompatible type {1}";
    sema_assign_discards_qualifiers = Error (2) "assigning to {0} from {1} discards qualifiers";
    sema_assign_discards_nested_qualifiers = Error (2) "assigning to {0} from {1} discards qualifiers in nested pointer types";
    sema_assign_incompatible_pointer = Error (2) "incompatible pointer types assigning to {0} from {1}";
    sema_assign_int_to_pointer = Error (3) "incompatible integer to pointer conversion assigning to {0} from {1}{2:|; dereference with *|; take the address with &|; remove *|; remove &}";
    sema_assign_pointer_to_int = Error (3) "incompatible pointer to integer conversion assigning to {0} from {1}{2:|; dereference with *|; take the address with &|; remove *|; remove &}";
    sema_not_contextually_convertible_to_bool = Error (1) "value of type {0} is not contextually convertible to 'bool'";
    sema_invalid_operands = Error (2) "invalid operands to binary expression ({0} and {1})";
    sema_arith_enum_enum = Error (3) "invalid {0:arithmetic between|bitwise operation between|comparison of|conditional expression between|compound assignment to} different enumeration types ({1} and {2})";
    sema_arith_enum_float = Error (4) "invalid {0:arithmetic between|bitwise operation between|comparison of|conditional expression between|compound assignment to} {1:floating-point|enumeration} type {2} {0:and|and|with|and|from} {1:enumeration|floating-point} type {3}";
    sema_arith_float_unordered = Error (3) "invalid {0:arithmetic between|bitwise operation between|comparison of|conditional expression between|compound assignment to} floating-point types {1} and {2} with unordered conversion ranks";
    sema_ambiguous_derived_to_base = Error (3) "ambiguous conversion from derived class {0} to base class {1}:{2}";
    sema_ambiguous_member_pointer_conv = Error (4) "ambiguous conversion from pointer to member of {0:base|derived} class {1} to pointer to member of {0:derived|base} class {2}:{3}";
    sema_member_pointer_conv_via_virtual = Error (3) "conversion from pointer to member of class {0} to pointer to member of class {1} via virtual base {2} is not allowed";
    sema_init_list_type_narrowing = WarningError [Cxx11Narrowing] (2) "type {0} cannot be narrowed to {1} in initializer list";
    sema_init_list_variable_narrowing = WarningError [Cxx11Narrowing] (2) "non-constant-expression cannot be narrowed from type {0} to {1} in initializer list";
    sema_init_list_constant_narrowing = WarningError [Cxx11Narrowing] (2) "constant expression evaluates to {0} which cannot be narrowed to type {1}";
    note_init_list_narrowing_silence = Note (0) "insert an explicit cast to silence this issue";
    sema_lvalue_ref_bind_to_temporary = Error (3) "{0:non-const|volatile} lvalue reference to type {1} cannot bind to a temporary of type {2}";
    sema_lvalue_ref_bind_to_unrelated = Error (3) "{0:non-const|volatile} lvalue reference to type {1} cannot bind to a value of unrelated type {2}";
    sema_rvalue_ref_bind_to_lvalue = Error (2) "rvalue reference to type {0} cannot bind to lvalue of type {1}";
    sema_reference_bind_drops_quals = Error (4) "binding reference of type {0} to value of type {1} drops {2} qualifier{3:|s}";
    sema_conv_unsupported = Error (1) "{0} is not supported yet";

    // ----- produced by the diagnostics engine itself --------------------
    diag_unknown_warning_option = Warning [UnknownWarningOption] (4) "unknown {0:warning|remark} option '{1}'{2:|; did you mean '{3}'?}";
    diag_unknown_warning_specifier = Warning [UnknownWarningOption] (2) "unknown {0} warning specifier: '{1}'";
    diag_too_many_errors = Fatal (0) "too many errors emitted, stopping now";
    diag_expanded_from_macro = Note (1) "expanded from macro '{0}'";
    diag_expanded_from_here = Note (0) "expanded from here";
    diag_backtrace_skipped = Note (1) "(skipping {0} expansions in backtrace; use -fmacro-backtrace-limit=0 to see all)";

    // ----- parser
    parse_expected = Error (1) "expected {0}";
    parse_expected_after = Error (2) "expected {1} after {0}";
    parse_expected_lparen_after_type = Error (0) "expected '(' for function-style cast or type construction";
    parse_expected_semi_declaration = Error (0) "expected ';' at end of declaration";
    parse_expected_semi_decl_list = Error (0) "expected ';' at end of declaration list";
    parse_expected_semi_after_expr = Error (0) "expected ';' after expression";
    parse_expected_semi_toplevel = Error (0) "expected ';' after top level declarator";
    parse_function_definition_not_allowed = Error (0) "function definition is not allowed here";
    parse_expected_init_in_condition = Error (0) "variable declaration in condition must have an initializer";
    parse_expected_init_in_condition_lparen = Error (0) "variable declaration in condition cannot have a parenthesized initializer";
    parse_expected_semi_after_stmt = Error (1) "expected ';' after {0} statement";
    parse_expected_expression = Error (0) "expected expression";
    parse_expected_name_of_pack = Error (0) "expected the name of a parameter pack";
    parse_stmt_expr_in_default_arg = Error (0) "default argument may not use a GNU statement expression";
    parse_expected_unqualified_id = Error (0) "expected unqualified-id";
    parse_expected_type = Error (0) "expected a type";
    parse_expected_member_name = Error (0) "expected member name or ';' after declaration specifiers";
    parse_expected_fn_body = Error (0) "expected function body after function declarator";
    parse_expected_while = Error (0) "expected 'while' in do/while loop";
    parse_expected_namespace_name = Error (0) "expected namespace name";
    parse_expected_string_literal = Error (0) "expected string literal for diagnostic message in static_assert";
    parse_expected_class_name = Error (0) "expected class name";
    parse_missing_type_specifier = Error (0) "a type specifier is required for all declarations";
    parse_extraneous_closing_brace = Error (0) "extraneous closing brace ('}}')";
    parse_extraneous_rparen = Error (0) "extraneous ')' before ';'";
    parse_duplicate_decl_spec = ExtWarn [DuplicateDeclSpecifier] (1) "duplicate '{0}' declaration specifier";
    parse_invalid_decl_spec_combination = Error (1) "cannot combine with previous '{0}' declaration specifier";
    parse_long_long_long = Error (0) "'long long long' is invalid";
    parse_unknown_attribute = Warning [UnknownAttributes] (1) "unknown attribute '{0}' ignored";
    parse_attribute_ignored = Warning [IgnoredAttributes] (1) "'{0}' attribute ignored";
    parse_attribute_wrong_arg_count = Error (2) "'{0}' attribute takes {1:no arguments|one argument}";
    parse_attribute_requires_string = Error (1) "expected string literal as argument of '{0}' attribute";
    parse_unsupported = Error (1) "{0} is not supported yet";
    parse_expected_lbrace_after_base = Error (0) "expected '{{' after base class list";
    // pragmas handled by the parser (WP-055; clang ParsePragma.cpp)
    parse_pragma_expected_lparen = Warning [IgnoredPragmas] (1) "missing '(' after '#pragma {0}' - ignoring";
    parse_pragma_expected_rparen = Warning [IgnoredPragmas] (1) "missing ')' after '#pragma {0}' - ignoring";
    parse_pragma_expected_identifier = Warning [IgnoredPragmas] (1) "expected identifier in '#pragma {0}' - ignored";
    parse_pragma_extra_tokens = Warning [IgnoredPragmas] (1) "extra tokens at end of '#pragma {0}' - ignored";
    parse_pragma_stdc_unknown = ExtWarn [UnknownPragmas] (0) "unknown pragma in STDC namespace";
    sema_unknown_visibility = Warning [IgnoredAttributes] (1) "unknown visibility '{0}'";
    sema_visibility_pop_mismatch = Error (0) "#pragma visibility pop with no matching #pragma visibility push";

    // ----- using-declarations, namespace aliases (WP-062)
    parse_cxx11_attr_placement = ExtWarn [CxxAttributeExtension] (0) "ISO C++ does not allow an attribute list to appear here";
    sema_using_requires_qualname = Error (0) "using declaration requires a qualified name";
    sema_using_decl_can_not_refer_to_namespace = Error (0) "using declaration cannot refer to a namespace";
    note_namespace_using_decl = Note (0) "did you mean 'using namespace'?";
    sema_using_decl_conflict = Error (0) "target of using declaration conflicts with declaration already in scope";
    sema_using_decl_conflict_reverse = Error (0) "declaration conflicts with target of using declaration already in scope";
    note_using_decl_target = Note (0) "target of using declaration";
    note_using_decl_conflict = Note (0) "conflicting declaration";
    note_using_decl = Note (0) "using declaration";
    sema_use_of_empty_using_if_exists = Error (0) "reference to unresolved using declaration";
    note_empty_using_if_exists_here = Note (0) "using declaration annotated with 'using_if_exists' here";
    sema_using_enum_not_enum = Error (1) "'{0}' is not an enumerated type";
    sema_using_enum_unknown_typename = Error (1) "unknown type name {0}";
    sema_redefinition_different_namespace_alias = Error (1) "redefinition of '{0}' as an alias for a different namespace";
    note_previous_namespace_alias = Note (1) "previously defined as an alias for '{0}'";
    sema_using_decl_nested_name_specifier_is_not_class = Error (1) "using declaration in class refers into '{0}', which is not a class";
    sema_using_decl_refers_to_own_class = Error (0) "using declaration refers to its own class";
    sema_using_decl_not_base_class = Error (2) "using declaration refers into '{0}', which is not a base class of '{1}'";
    sema_using_decl_can_not_refer_to_class_member = Error (0) "using declaration cannot refer to class member";
    note_using_decl_class_member_alias = Note (0) "use an alias declaration instead";
    note_using_decl_class_member_reference = Note (0) "use a reference instead";
    sema_using_enum_incomplete = Error (1) "enumeration '{0}' is incomplete";
    parse_using_enum_requires_name = Error (0) "using enum requires an enum or typedef name";

    // ----- disambiguation, expression forms, declarators (WP-064)
    parse_vexing_parse = Warning [VexingParse] (0) "parentheses were disambiguated as a function declaration";
    parse_empty_parens_vexing = Warning [VexingParse] (0) "empty parentheses interpreted as a function declaration";
    note_additional_parens_for_variable_declaration = Note (0) "add a pair of parentheses to declare a variable";
    note_empty_parens_default_ctor = Note (0) "remove parentheses to declare a variable";
    note_empty_parens_zero_initialize = Note (0) "replace parentheses with an initializer to declare a variable";
    sema_braces_around_init = Warning [BracedScalarInit] (0) "braces around scalar initializer";
    sema_default_arg_redefinition = Error (0) "redefinition of default argument";
    sema_missing_default_arg = Error (1) "missing default argument on parameter '{0}'";
    sema_missing_default_arg_unnamed = Error (0) "missing default argument on parameter";
    sema_trailing_return_without_auto = Error (1) "function with trailing return type must specify return type 'auto', not {0}";
    sema_auto_fn_different_deductions = Error (3) "'{0:auto|decltype(auto)}' in return type deduced as {1} here but deduced as {2} in earlier return statement";
    sema_auto_fn_used_before_defined = Error (1) "function '{0}' with deduced return type cannot be used before it is defined";
    sema_auto_fn_init_list = Error (0) "cannot deduce return type from initializer list";
    sema_auto_fn_deduction_failure = Error (2) "cannot deduce return type {0} from returned value of type {1}";
    sema_auto_var_deduction_failure = Error (3) "variable '{0}' with type {1} has incompatible initializer of type {2}";
    sema_auto_init_list_deduction = Error (0) "cannot deduce type of initializer list because std::initializer_list was not found; include <initializer_list>";
    sema_decltype_auto_compound = Error (1) "cannot form {0:pointer to|reference to|array of} 'decltype(auto)'";
    sema_need_header_before_typeid = Error (0) "you need to include <typeinfo> before using the 'typeid' operator";
    sema_typeid_no_rtti = Error (0) "use of typeid requires -frtti";
    sema_typeid_incomplete = Error (1) "'typeid' of incomplete type {0}";
    sema_bad_offsetof_type = Error (1) "offsetof requires struct, union, or class type, {0} invalid";
    sema_no_member_offsetof = Error (2) "no member named '{0}' in {1}";
    sema_stmtexpr_file_scope = Error (0) "statement expression not allowed at file scope";
    sema_throw_disabled = Error (0) "cannot use 'throw' with exceptions disabled";
    sema_throw_incomplete = Error (1) "cannot throw object of incomplete type {0}";
    sema_throw_incomplete_ptr = Error (1) "cannot throw pointer to object of incomplete type {0}";
    note_parameter_named_here = Note (1) "passing argument to parameter '{0}' here";
    note_parameter_here = Note (0) "passing argument to parameter here";
    parse_init_list_bin_op = Error (2) "initializer list cannot be used on the {0:left|right} hand side of operator '{1}'";
    sema_return_init_list = Error (1) "void function '{0}' must not return a value";
    sema_init_list_variadic = Error (0) "cannot pass initializer list to variadic function";
    sema_many_braces_around_scalar = Warning [ManyBracesAroundScalarInit] (0) "too many braces around scalar initializer";
    // ----- sema (WP-048)
    sema_undeclared_var_use = Error (1) "use of undeclared identifier '{0}'";
    sema_unknown_typename = Error (1) "unknown type name '{0}'";
    sema_no_member = Error (2) "no member named '{0}' in {1}";
    sema_ambiguous_reference = Error (1) "reference to '{0}' is ambiguous";
    note_ambiguous_candidate = Note (1) "candidate found by name lookup is '{0}'";
    sema_not_a_type = Error (1) "'{0}' does not name a type";
    sema_redefinition = Error (1) "redefinition of '{0}'";
    sema_redefinition_of_enumerator = Error (1) "redefinition of enumerator '{0}'";
    sema_redefinition_different_kind = Error (1) "redefinition of '{0}' as different kind of symbol";
    sema_redefinition_different_type = Error (3) "redefinition of '{0}' with a different type: {1} vs {2}";
    sema_redeclaration_different_type = Error (3) "redeclaration of '{0}' with a different type: {1} vs {2}";
    sema_redefinition_different_typedef = Error (3) "{0:typedef|type alias} redefinition with different types ({1} vs {2})";
    sema_ovl_diff_return_type = Error (0) "functions that differ only in their return type cannot be overloaded";
    sema_overload_unsupported = Error (1) "overloading '{0}' is not supported yet";
    sema_static_non_static = Error (1) "static declaration of '{0}' follows non-static declaration";
    sema_param_redefinition = Error (1) "redefinition of parameter '{0}'";
    sema_duplicate_member = Error (1) "duplicate member '{0}'";
    sema_anon_struct_not_member = Error (0) "anonymous structs and classes must be class members";
    sema_anon_union_not_static = Error (0) "anonymous unions at namespace or global scope must be declared 'static'";
    sema_anon_record_member_redecl = Error (2) "member of anonymous {0} redeclares '{1}'";
    sema_use_with_wrong_tag = Error (1) "use of '{0}' with tag type that does not match previous declaration";
    sema_enum_scoped_mismatch = Error (1) "enumeration previously declared with {0:non|}scoped underlying type";
    sema_enum_redeclare_type_mismatch = Error (2) "enumeration redeclared with different underlying type {0} (was {1})";
    sema_enum_fixed_redecl_unfixed = Error (0) "enumeration previously declared with fixed underlying type";
    sema_enum_unfixed_redecl_fixed = Error (0) "enumeration previously declared with nonfixed underlying type";
    sema_enum_invalid_underlying = Error (1) "non-integral type {0} is an invalid underlying type";
    sema_cce_narrowing_value = Error (3) "{0} evaluates to {1}, which cannot be narrowed to type {2}";
    sema_enumerator_too_large = Error (2) "enumerator value {0} is not representable in the underlying type {1}";
    sema_enumerator_wrapped = Error (0) "enumerator value is not representable in any integral type";
    sema_no_declarators = ExtWarn [MissingDeclarations] (0) "declaration does not declare anything";
    sema_typecheck_decl_incomplete_type = Error (1) "variable has incomplete type {0}";
    sema_field_incomplete = Error (1) "field has incomplete type {0}";
    sema_reference_var_requires_init = Error (1) "declaration of reference variable '{0}' requires an initializer";
    sema_default_init_const = Error (1) "default initialization of an object of const type {0}";
    sema_constexpr_var_non_constant = Error (1) "constexpr variable '{0}' must be initialized by a constant expression";
    sema_constexpr_non_literal = Error (0) "constexpr can only be used in variable and function declarations";
    sema_extern_init = Warning [ExternInitializer] (0) "'extern' variable has an initializer";
    sema_block_extern_init = Error (0) "declaration of block scope identifier with linkage cannot have an initializer";
    sema_invalid_storage_class_in_func_decl = Error (0) "invalid storage class specifier in function declarator";
    sema_static_block_func = Error (0) "function declared in block scope cannot have 'static' storage class";
    sema_storage_class_on_typedef = Error (0) "'typedef' cannot be combined with a storage class";
    sema_void_only_param = Error (0) "'void' must be the first and only parameter if specified";
    sema_param_with_void_type = Error (0) "argument may not have 'void' type";
    sema_void_var = Error (1) "variable has incomplete type {0}";
    sema_func_returning_array_function = Error (2) "function cannot return {0:array|function} type {1}";
    sema_array_of_functions = Error (2) "'{0}' declared as array of functions of type {1}";
    sema_array_of_references = Error (2) "'{0}' declared as array of references of type {1}";
    sema_array_incomplete_type = Error (1) "array has incomplete element type {0}";
    sema_array_of_void = Error (1) "'{0}' declared as array of void";
    sema_pointer_to_reference = Error (2) "'{0}' declared as a pointer to a reference of type {1}";
    sema_reference_to_reference = Error (1) "'{0}' declared as a reference to a reference";
    sema_reference_to_void = Error (0) "cannot form a reference to 'void'";
    sema_array_size_non_int = Error (1) "size of array has non-integer type {0}";
    sema_decl_negative_array_size = Error (1) "'{0}' declared as an array with a negative size";
    sema_typecheck_negative_array_size = Error (0) "array size is negative";
    sema_array_too_large = Error (1) "array is too large ({0} elements)";
    sema_vla_decl_in_file_scope = Error (0) "variable length array declaration not allowed at file scope";
    sema_vla_cxx_extension = Warning [VlaCxxExtension] (0) "variable length arrays in C++ are a Clang extension";
    sema_vla_unsupported = Error (0) "variable length arrays are not supported yet";
    sema_expr_not_ice = Error (0) "expression is not an integral constant expression";
    sema_bitfield_negative_width = Error (2) "bit-field '{0}' has negative width ({1})";
    sema_anon_bitfield_negative_width = Error (1) "anonymous bit-field has negative width ({0})";
    sema_bitfield_zero_width = Error (1) "named bit-field '{0}' has zero width";
    sema_bitfield_non_integral = Error (2) "bit-field '{0}' has non-integral type {1}";
    sema_anon_bitfield_non_integral = Error (1) "anonymous bit-field has non-integral type {0}";
    sema_bitfield_width_exceeds = Warning [BitfieldWidth] (4) "width of bit-field '{0}' ({1} bits) exceeds the width of its type; value will be truncated to {2} bit{3}";
    sema_alignment_not_power_of_two = Error (0) "requested alignment is not a power of 2";
    sema_alignas_underaligned = Error (2) "requested alignment is less than minimum alignment of {0} for type {1}";
    sema_alignment_too_big = Error (1) "requested alignment must be {0} bytes or smaller";
    sema_attribute_wrong_decl_type = Warning [IgnoredAttributes] (2) "{0} attribute only applies to {1}";
    sema_static_assert_failed = Error (0) "static assertion failed";
    sema_static_assert_failed_msg = Error (1) "static assertion failed: {0}";
    sema_static_assert_requirement_failed = Error (1) "static assertion failed due to requirement '{0}'";
    sema_static_assert_requirement_failed_msg = Error (2) "static assertion failed due to requirement '{0}': {1}";
    sema_static_assert_not_ice = Error (0) "static assertion expression is not an integral constant expression";
    oven_enum_constexpr_conversion = WarningError [EnumConstexprConversion] (4) "integer value {0} is outside the valid range of values [{1}, {2}] for the enumeration type '{3}'";
    note_expr_evaluates_to = Note (3) "expression evaluates to '{0} {1} {2}'";
    note_constexpr_overflow = Note (2) "value {0} is outside the range of representable values of type {1}";
    note_expr_divide_by_zero = Note (0) "division by zero";
    note_constexpr_negative_shift = Note (1) "negative shift count {0}";
    note_constexpr_large_shift = Note (4) "shift count {0} >= width of type {1} ({2} bit{3})";
    note_constexpr_lshift_of_negative = Note (1) "left shift of negative value {0}";
    note_constexpr_invalid_function = Note (1) "non-constexpr function '{0}' cannot be used in a constant expression";
    note_constexpr_unsupported_call = Note (1) "call to '{0}' cannot be evaluated yet";
    note_constexpr_ltor_non_const_int = Note (1) "read of non-const variable '{0}' is not allowed in a constant expression";
    note_constexpr_ltor_non_constexpr = Note (1) "read of non-constexpr variable '{0}' is not allowed in a constant expression";
    note_constexpr_var_init_unknown = Note (1) "initializer of '{0}' is unknown";
    note_constexpr_read_past_end = Note (0) "read of dereferenced one-past-the-end pointer is not allowed in a constant expression";
    note_constexpr_array_index = Note (2) "cannot refer to element {0} of array of {1} elements in a constant expression";
    note_constexpr_function_param_value_unknown = Note (1) "function parameter '{0}' with unknown value cannot be used in a constant expression";
    note_declared_at = Note (0) "declared here";
    note_entity_declared_at = Note (1) "{0} declared here";
    note_previous_declaration = Note (0) "previous declaration is here";
    note_previous_use = Note (0) "previous use is here";
    note_var_declared_const = Note (1) "variable '{0}' declared const here";
    note_marked_deprecated = Note (1) "'{0}' has been explicitly marked deprecated here";
    sema_not_assignable = Error (0) "expression is not assignable";
    sema_assign_const_var = Error (2) "cannot assign to variable '{0}' with const-qualified type {1}";
    sema_read_only_not_assignable = Error (0) "read-only variable is not assignable";
    sema_indirection_requires_pointer = Error (1) "indirection requires pointer operand ({0} invalid)";
    sema_indirection_incomplete = Error (1) "indirection not permitted on operand of type {0}";
    sema_subscript_value = Error (0) "subscripted value is not an array, pointer, or vector";
    sema_subscript_not_integer = Error (0) "array subscript is not an integer";
    sema_call_not_function = Error (1) "called object type {0} is not a function or function pointer";
    sema_call_too_few_args = Error (2) "too few arguments to function call, expected {0}, have {1}";
    sema_call_too_few_args_at_least = Error (2) "too few arguments to function call, expected at least {0}, have {1}";
    sema_call_too_few_args_one = Error (1) "too few arguments to function call, single argument '{0}' was not specified";
    sema_call_too_many_args = Error (2) "too many arguments to function call, expected {0}, have {1}";
    sema_call_too_many_args_one = Error (2) "too many arguments to function call, expected single argument '{0}', have {1} arguments";
    sema_member_reference_struct_union = Error (1) "member reference base type {0} is not a structure or union";
    sema_member_reference_suggestion = Error (2) "member reference type {0} is {1:a|not a} pointer; did you mean to use '{1:->|.}'?";
    sema_member_reference_incomplete = Error (1) "incomplete definition of type {0}";
    sema_invalid_unary_operand = Error (1) "invalid argument type {0} to unary expression";
    sema_incompatible_cond_operands = Error (2) "incompatible operand types ({0} and {1})";
    sema_conditional_void_nonvoid = Error (2) "{0:left|right} operand to ? is void, but {0:right|left} operand is of type {1}";
    sema_sub_ptr_compatible = Error (2) "{0} and {1} are not pointers to compatible types";
    sema_comparison_distinct_pointers = Error (2) "comparison of distinct pointer types ({0} and {1})";
    sema_comparison_pointer_integer = Error (2) "comparison between pointer and integer ({0} and {1})";
    sema_arith_incomplete_pointee = Error (1) "arithmetic on a pointer to an incomplete type {0}";
    sema_arith_void_pointer = Extension [PointerArith] (1) "arithmetic on{0: a pointer| pointers} to void is a GNU extension";
    sema_arith_function_pointer = Extension [PointerArith] (2) "arithmetic on{0: a pointer| pointers} to the function type {1} is a GNU extension";
    sema_addrof_rvalue = Error (1) "cannot take the address of an rvalue of type {0}";
    sema_addrof_bitfield = Error (0) "address of bit-field requested";
    sema_incdec_bool = Error (1) "ISO C++17 does not allow {0:incrementing|decrementing} expression of type bool";
    sema_incdec_enum = Error (2) "cannot {0:increment|decrement} expression of enum type {1}";
    sema_ovl_no_viable_function_in_call = Error (1) "no matching function for call to '{0}'";
    note_ovl_candidate_not_viable = Note (1) "candidate function not viable: {0}";
    sema_sizeof_pack_no_pack = Error (1) "{0} does not refer to the name of a parameter pack";
    sema_udl_no_operator = Error (2) "no matching literal operator for call to {0} with argument of type {1}";
    sema_udl_no_operator_raw = Error (3) "no matching literal operator for call to {0} with argument of type {1} or {2}, and no matching literal operator template";
    sema_udl_no_operator_string = Error (3) "no matching literal operator for call to {0} with arguments of types {1} and {2}, and no matching literal operator template";
    sema_unsupported_string_concat = Error (0) "unsupported non-standard concatenation of string literals";
    sema_ref_non_value = Error (1) "{0} does not refer to a value";
    // lexer and literal diagnostics the parser reports (a stopgap until the
    // preprocessor maps every LexDiagKind, spec/parse.md §8)
    lex_invalid_digit = Error (2) "invalid digit '{0}' in {1} constant";
    lex_invalid_suffix = Error (2) "invalid suffix '{0}' on {1:integer|floating} constant";
    lex_empty_char = Error (0) "empty character constant";
    lex_literal_error = Error (1) "{0}";
    lex_unknown_escape = Warning [UnknownEscapeSequence] (1) "unknown escape sequence '\\{0}'";
    lex_multichar = Warning [Multichar] (0) "multi-character character constant";
    lex_literal_warning = Warning (1) "{0}";
    sema_incdec_invalid = Error (2) "cannot {0:increment|decrement} value of type {1}";
    sema_sizeof_incomplete = Error (2) "invalid application of '{0}' to an incomplete type {1}";
    sema_sizeof_function = Extension [PointerArith] (1) "invalid application of '{0}' to a function type";
    sema_sizeof_function_expr = Error (1) "invalid application of '{0}' to a function type";
    sema_sizeof_void = Extension [PointerArith] (1) "invalid application of '{0}' to a void type";
    sema_sizeof_bitfield = Error (1) "invalid application of '{0}' to bit-field";
    sema_bad_cxx_cast = Error (3) "{0:const_cast|static_cast|reinterpret_cast|dynamic_cast|C-style cast|functional-style cast} from {1} to {2} is not allowed";
    sema_bad_cast_qualifiers_away = Error (3) "{0:const_cast|static_cast|reinterpret_cast|dynamic_cast|C-style cast|functional-style cast} from {1} to {2} casts away qualifiers";
    sema_bad_const_cast_dest = Error (2) "{0:const_cast|static_cast|reinterpret_cast|dynamic_cast|C-style cast|functional-style cast} to {1}, which is not a reference, pointer-to-object, or pointer-to-data-member";
    sema_reinterpret_cast_small_int = Error (1) "cast from pointer to smaller type {0} loses information";
    sema_excess_initializers = Error (1) "excess elements in {0:array|vector|scalar|union|struct} initializer";
    sema_array_init_not_init_list = Error (1) "array initializer must be an initializer list{0:| or string literal| or wide string literal}";
    sema_init_string_too_long = Error (2) "initializer-string for char array is too long, array size is {0} but initializer has size {1} (including the null terminating character)";
    sema_illegal_initializer_type = Error (1) "illegal initializer type {0}";
    sema_init_list_bad_dest = Error (1) "initializer list cannot be used on the left hand side of operator '{0}'";
    sema_division_by_zero = Warning [DivisionByZero] (1) "{0} by zero is undefined";
    sema_shift_negative = Warning [ShiftCountNegative] (0) "shift count is negative";
    sema_shift_gt_typewidth = Warning [ShiftCountOverflow] (0) "shift count >= width of type";
    sema_unused_result = Warning [UnusedResult] (1) "ignoring return value of function declared with {0} attribute";
    sema_unused_result_msg = Warning [UnusedResult] (2) "ignoring return value of function declared with {0} attribute: {1}";
    sema_unused_expr = Warning [UnusedValue] (0) "expression result unused";
    sema_unused_comparison = Warning [UnusedComparison] (1) "{0:equality|inequality|relational|three-way} comparison result unused";
    sema_deprecated = Warning [DeprecatedDeclarations] (1) "'{0}' is deprecated";
    sema_deprecated_msg = Warning [DeprecatedDeclarations] (2) "'{0}' is deprecated: {1}";
    sema_break_not_in_loop_or_switch = Error (0) "'break' statement not in loop or switch statement";
    sema_continue_not_in_loop = Error (0) "'continue' statement not in loop statement";
    sema_case_not_in_switch = Error (0) "'case' statement not in switch statement";
    sema_default_not_in_switch = Error (0) "'default' statement not in switch statement";
    sema_duplicate_case = Error (1) "duplicate case value '{0}'";
    note_duplicate_case_prev = Note (0) "previous case defined here";
    sema_multiple_default_labels = Error (0) "multiple default labels in one switch";
    sema_statement_requires_integer = Error (1) "statement requires expression of integer type ({0} invalid)";
    sema_expr_not_cce = Error (1) "{0} is not a constant expression";
    sema_cce_conversion_not_allowed = Error (2) "conversion from {0} to {1} is not allowed in a converted constant expression";
    sema_cce_not_implicitly_convertible = Error (2) "value of type {0} is not implicitly convertible to {1}";
    sema_dynamic_exception_spec = WarningError [DynamicExceptionSpec] (0) "ISO C++17 does not allow dynamic exception specifications";
    note_use_noexcept_false = Note (0) "use 'noexcept(false)' instead";
    sema_exception_spec_mismatch = Error (0) "exception specification in declaration does not match previous declaration";
    sema_missing_exception_spec = Error (2) "'{0}' is missing exception specification '{1}'";
    sema_undeclared_label = Error (1) "use of undeclared label '{0}'";
    sema_redefinition_of_label = Error (1) "redefinition of label '{0}'";
    sema_return_missing_expr = Error (1) "non-void function '{0}' should return a value";
    sema_return_has_expr = Error (1) "void function '{0}' should not return a value";
    sema_return_void_expr_in_nonvoid = Error (1) "non-void function '{0}' should return a value";
    sema_falloff_nonvoid = Warning [ReturnType] (0) "non-void function does not return a value";
    sema_falloff_nonvoid_maybe = Warning [ReturnType] (0) "non-void function does not return a value in all control paths";
    sema_main_returns_nonint = Error (0) "'main' must return 'int'";
    sema_switch_into_protected_scope = Error (0) "cannot jump from switch statement to this case label";
    sema_goto_into_protected_scope = Error (0) "cannot jump from this goto statement to its label";
    note_protected_by_variable_init = Note (0) "jump bypasses variable initialization";
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
