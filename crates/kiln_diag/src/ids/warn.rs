// Diagnostic ids `warn_*` (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

warn_for_range_const_ref_binds_temp_built_from_ref = WarningOff [RangeLoopConstruct] (3) "loop variable '{0}' of type {1} binds to a temporary constructed from type {2}";
warn_for_range_copy = WarningOff [RangeLoopConstruct] (2) "loop variable '{0}' creates a copy from type {1}";
warn_for_range_ref_binds_ret_temp = WarningOff [RangeLoopBindReference] (2) "loop variable '{0}' binds to a temporary value produced by a range of type {1}";
