// Diagnostic ids `diag_*` (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

diag_backtrace_skipped = Note (1) "(skipping {0} expansions in backtrace; use -fmacro-backtrace-limit=0 to see all)";
diag_expanded_from_here = Note (0) "expanded from here";
diag_expanded_from_macro = Note (1) "expanded from macro '{0}'";
diag_too_many_errors = Fatal (0) "too many errors emitted, stopping now";
diag_unknown_warning_option = Warning [UnknownWarningOption] (4) "unknown {0:warning|remark} option '{1}'{2:|; did you mean '{3}'?}";
diag_unknown_warning_specifier = Warning [UnknownWarningOption] (2) "unknown {0} warning specifier: '{1}'";
diag_verify_ambiguous_marker = Error (1) "reference to marker '{0}' is ambiguous";
diag_verify_ambiguous_marker_here = Note (1) "ambiguous marker '{0}' is defined here";
diag_verify_inconsistent_diags = Error (3) "'{0}' diagnostics {1:expected|seen} but not {1:seen|expected}: {2}";
diag_verify_invalid_content = Error (2) "invalid expected {0}: {1}";
diag_verify_invalid_no_diags = Error (2) "{1:expected|'{0}-no-diagnostics'} directive cannot follow {1:'{0}-no-diagnostics' directive|other expected directives}";
diag_verify_invalid_range = Error (1) "invalid range following '-' in expected {0}";
diag_verify_missing_end = Error (2) "cannot find end ('{1}') of expected {0}";
diag_verify_missing_file = Error (2) "file '{0}' could not be located in expected {1}";
diag_verify_missing_line = Error (1) "missing or invalid line number following '@' in expected {0}";
diag_verify_missing_regex = Error (1) "cannot find start of regex ('{{{{') in {0}";
diag_verify_missing_start = Error (1) "cannot find start ('{{{{') of expected {0}";
diag_verify_no_directives = Error (1) "no expected directives found: consider use of '{0}-no-diagnostics'";
diag_verify_no_such_marker = Error (1) "use of undefined marker '{0}'";
