// Warning groups named by `drv_*` ids (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

// ----- WP-228 (driver flags a target does not support)
EagerLoadCxxNamedModules = "eager-load-cxx-named-modules" [];
// WP-1037: a `<h>.gch` the driver's `-include` probe ignores.
IgnoredGch = "ignored-gch" [];
OptionIgnored = "option-ignored" [];
UnusedCommandLineArgument = "unused-command-line-argument" [];
