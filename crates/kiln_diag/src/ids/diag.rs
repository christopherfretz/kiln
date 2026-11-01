// Diagnostic ids `diag_*` (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

diag_backtrace_skipped = Note (1) "(skipping {0} expansions in backtrace; use -fmacro-backtrace-limit=0 to see all)";
diag_expanded_from_here = Note (0) "expanded from here";
diag_expanded_from_macro = Note (1) "expanded from macro '{0}'";
diag_too_many_errors = Fatal (0) "too many errors emitted, stopping now";
diag_unknown_warning_option = Warning [UnknownWarningOption] (4) "unknown {0:warning|remark} option '{1}'{2:|; did you mean '{3}'?}";
diag_unknown_warning_specifier = Warning [UnknownWarningOption] (2) "unknown {0} warning specifier: '{1}'";
