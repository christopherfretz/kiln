// Warning groups named by `drv_*` ids (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

// ----- WP-228 (driver flags a target does not support)
EagerLoadCxxNamedModules = "eager-load-cxx-named-modules" [];
// WP-1477: `drv_hod_failed` (hod failed; the module goes to llc).
HodFallback = "hod-fallback" [];
// WP-1037: a `<h>.gch` the driver's `-include` probe ignores.
IgnoredGch = "ignored-gch" [];
OptionIgnored = "option-ignored" [];
// WP-1795: clang 22 `-ffp-model=` / complex-range flag overrides.
OverridingComplexRange = "overriding-complex-range" [];
OverridingOption = "overriding-option" [];
// WP-1403: `drv_pch_snapshot_unused`; WP-1404: `drv_pch_snapshot_not_written`.
PchSnapshotUnused = "pch-snapshot-unused" [];
UnusedCommandLineArgument = "unused-command-line-argument" [];
