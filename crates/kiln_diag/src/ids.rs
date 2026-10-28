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
    pp_defined_requires_identifier = Error (0) "operator 'defined' requires an identifier";
    pp_expansion_to_defined = Warning [ExpansionToDefined] (0) "macro expansion producing 'defined' has undefined behavior";
    pp_expansion_to_defined_fn = Extension [ExpansionToDefined] (0) "macro expansion producing 'defined' has undefined behavior";
    pp_expr_bad_token_binop = Error (0) "token is not a valid binary operator in a preprocessor subexpression";
    pp_expected_rparen_in_expr = Error (0) "expected ')' in preprocessor expression";
    pp_nested_paren = Error (1) "nested parentheses not permitted in '{0}'";
    pp_has_warning_invalid_option = ExtWarn [MalformedWarningCheck] (0) "__has_warning expected option name (e.g. \"-Wundef\")";
    pp_expected_string_literal = Error (1) "expected string literal in {0}";
    pp_date_time = WarningOff [DateTime] (0) "expansion of date or time macro is not reproducible";
    pp_malformed_ident = Error (0) "invalid #ident directive";
    pp_expr_unsupported = Error (0) "kiln: this #if expression needs the full evaluator (WP-015)";
    pp_include_unsupported = Fatal (0) "kiln: no include resolver is installed";
    pp_expected = Error (1) "expected {0}";
    pp_pragma_diagnostic_unknown_warning = Warning [UnknownWarningOption] (1) "unknown warning group '{0}', ignored";
    pp_pragma_diagnostic_invalid = Warning [UnknownPragmas] (0) "pragma diagnostic expected 'error', 'warning', 'ignored', 'fatal', 'push', or 'pop'";
    pp_pragma_diagnostic_invalid_token = Warning [UnknownPragmas] (0) "unexpected token in pragma diagnostic";
    pp_pragma_diagnostic_invalid_option = Warning [UnknownPragmas] (0) "pragma diagnostic expected option name (e.g. \"-Wundef\")";
    pp_pragma_diagnostic_cannot_pop = Warning [UnknownPragmas] (0) "pragma diagnostic pop could not pop, no matching push";

    // ----- shared notes ------------------------------------------------
    note_previous_definition = Note (0) "previous definition is here";
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
