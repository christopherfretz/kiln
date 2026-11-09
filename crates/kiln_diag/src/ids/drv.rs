// Diagnostic ids `drv_*` (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

drv_argument_not_allowed_with = Error (2) "invalid argument '{0}' not allowed with '{1}'";
// WP-941: `-fcoverage-mapping` without `-fprofile-instr-generate`.
drv_argument_only_allowed_with = Error (2) "invalid argument '{0}' only allowed with '{1}'";
drv_argument_unused = Warning [UnusedCommandLineArgument] (1) "argument unused during compilation: '{0}'";
drv_cannot_execute = Error (2) "unable to execute command: {0}: {1}";
drv_cannot_open_output = Error (2) "unable to open output file '{0}': '{1}'";
drv_cannot_read_input = Fatal (2) "cannot read '{0}': {1}";
drv_cannot_read_predefines = Fatal (2) "cannot read the predefined macros '{0}': {1}";
drv_command_failed = Error (2) "{0} command failed with exit code {1} (use -v to see invocation)";
drv_command_signalled = Error (1) "{0} command failed due to signal (use -v to see invocation)";
// WP-920: `-MJ <file>` (clang's JSON compilation-database fragment).
drv_compilation_database = Error (2) "compilation database '{0}' could not be opened: {1}";
drv_emit_llvm_link = Error (0) "-emit-llvm cannot be used when linking";
drv_framework_dir_ignored = Warning (1) "ignoring framework directory '{0}' (kiln does not search frameworks)";
drv_ignoring_tabstop = Warning (2) "ignoring invalid -ftabstop value '{0}', using default value {1}";
// WP-811: a driver option spelled like a warning (`-Wframe-larger-than=N`);
// WP-813: `-fpatchable-function-entry=` with a malformed value.
drv_invalid_argument_to_option = Error (2) "invalid argument '{0}' to -{1}";
drv_invalid_gcc_install_dir = Error (1) "'{0}' does not contain a GCC installation";
drv_invalid_int_value = Error (2) "invalid integral value '{1}' in '{0}'";
drv_invalid_stdlib_name = Error (1) "invalid library name in argument '{0}'";
drv_invalid_value = Error (2) "invalid value '{1}' in '{0}'";
drv_language_not_recognized = Error (1) "language not recognized: '{0}'";
drv_linker_input_unused = Warning [UnusedCommandLineArgument] (1) "{0}: 'linker' input unused";
// WP-1016: LLVM discovery (spec/driver.md "LLVM discovery").
drv_llvm_dir_invalid = Error (3) "'{0}' (from {1}) is not a usable LLVM installation: {2}";
drv_llvm_install_hint = Note (1) "install LLVM {0} with clang (docs/COMPATIBILITY.md \"Runtime dependencies\"), or name an installation with --llvm-dir=<dir> or KILN_LLVM_DIR";
drv_llvm_not_found = Error (1) "no supported LLVM installation found (kiln supports LLVM {0})";
drv_llvm_not_on_path = Note (1) "not found on PATH: {0}";
drv_llvm_tried = Note (2) "skipped '{0}': {1}";
drv_llvm_unsupported = Error (4) "unsupported LLVM major {0} at '{1}' (from {2}); kiln supports LLVM {3}";
// WP-920: `-MG` lists missing headers only for `-M`/`-MM`.
drv_mg_requires_m_or_mm = Error (0) "option '-MG' requires '-M' or '-MM'";
drv_missing_argument = Error (2) "argument to '{0}' is missing (expected {1} value)";
drv_module_file_nameless = Warning [EagerLoadCxxNamedModules] (0) "the form '-fmodule-file=<BMI-path>' is deprecated for standard C++ named modules;consider to use '-fmodule-file=<module-name>=<BMI-path>' instead";
drv_no_input_files = Error (0) "no input files";
drv_no_such_file = Error (1) "no such file or directory: '{0}'";
drv_not_implemented = Error (1) "{0} is not implemented yet";
drv_opt_not_valid_on_target = Error (1) "option '{0}' cannot be specified on this target";
// ----- WP-228 (driver flags a target does not support)
drv_option_ignored = Warning [OptionIgnored] (2) "'{0}' does not support '{1}'; flag ignored";
drv_out_file_multiple = Error (0) "cannot specify -o when generating multiple output files";
drv_precompile_not_module = Error (0) "--precompile requires a module interface or partition unit";
// WP-982: kiln-scan-deps (spec/modules.md §11.9).
drv_scan_one_job = Error (1) "unable to handle compilation, expected exactly one compiler job in '{0}'";
// WP-792: clang's `-ftime-trace` family, added by build systems for clang.
drv_time_trace_ignored = Warning [UnusedCommandLineArgument] (1) "'{0}' ignored: kiln writes no time trace (see -fkiln-instantiation-report)";
drv_treating_input_as_cxx = Warning [Deprecated] (2) "treating '{0}' input as '{1}' when in C++ mode, this behavior is deprecated";
drv_unable_to_execute = Error (1) "unable to execute command: Executable \"{0}\" doesn't exist!";
drv_unknown_argument = Error (1) "unknown argument: '{0}'";
drv_unknown_argument_suggest = Error (2) "unknown argument '{0}'; did you mean '{1}'?";
// WP-605: a recognised GCC/clang flag kiln deliberately does not implement.
drv_unsupported_flag = Error (1) "'{0}' is not supported by kiln";
drv_unsupported_fpatchable_function_entry_argument = Error (0) "the second argument of '-fpatchable-function-entry' must be smaller than the first argument";
// WP-469: `-msve-vector-bits=` off AArch64.
drv_unsupported_opt_for_target = Error (2) "unsupported option '{0}' for target '{1}'";
drv_unsupported_option_argument = Error (2) "unsupported argument '{1}' to option '{0}'";
drv_unsupported_target = Error (1) "unsupported target '{0}' (kiln supports aarch64 and x86_64 Linux)";
drv_verify_prefix_spelling = Note (0) "-verify prefixes must start with a letter and contain only alphanumeric characters, hyphens, and underscores";
