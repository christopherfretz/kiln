// Diagnostic ids `cg_*` (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

cg_atomic_misaligned = Warning [AtomicAlignment] (2) "misaligned atomic operation may incur significant performance penalty; the expected alignment ({0} bytes) exceeds the actual alignment ({1} bytes)";
cg_atomic_oversized = Warning [AtomicAlignment] (2) "large atomic operation may incur significant performance penalty; the access size ({0} bytes) exceeds the max lock-free size ({1} bytes)";
// clang 19's text (a doubled space), kept for -fclang-compat=19.
cg_atomic_oversized_19 = Warning [AtomicAlignment] (2) "large atomic operation may incur significant performance penalty; the access size ({0} bytes) exceeds the max lock-free size ({1}  bytes)";
cg_error = Error (1) "{0}";
