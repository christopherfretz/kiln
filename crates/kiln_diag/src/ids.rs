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
    note_forward_declaration = Note (1) "forward declaration of {0}";
    note_macro_here = Note (1) "macro '{0}' defined here";

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

    // ----- parser
    parse_expected = Error (1) "expected {0}";
    parse_expected_after = Error (2) "expected {1} after {0}";
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
    note_matching = Note (1) "to match this {0}";

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
    sema_redefinition_different_typedef = Error (3) "{0:typedef|type alias} redefinition with different types ({1} vs {2})";
    sema_ovl_diff_return_type = Error (0) "functions that differ only in their return type cannot be overloaded";
    sema_overload_unsupported = Error (1) "overloading '{0}' is not supported yet";
    sema_static_non_static = Error (1) "static declaration of '{0}' follows non-static declaration";
    sema_param_redefinition = Error (1) "redefinition of parameter '{0}'";
    sema_duplicate_member = Error (1) "duplicate member '{0}'";
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
