// Diagnostic ids `cg_*` (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

cg_atomic_misaligned = Warning [AtomicAlignment] (2) "misaligned atomic operation may incur significant performance penalty; the expected alignment ({0} bytes) exceeds the actual alignment ({1} bytes)";
cg_atomic_oversized = Warning [AtomicAlignment] (2) "large atomic operation may incur significant performance penalty; the access size ({0} bytes) exceeds the max lock-free size ({1} bytes)";
// clang 19's text (a doubled space), kept for -fclang-compat=19.
cg_atomic_oversized_19 = Warning [AtomicAlignment] (2) "large atomic operation may incur significant performance penalty; the access size ({0} bytes) exceeds the max lock-free size ({1}  bytes)";
// WP-448 (spec/codegen.md §7r): kiln-only, no clang counterpart.
cg_auto_var_init_too_large = Remark [AutoVarInit] (3) "erroneous value of variable {0} not written: its size ({1} bytes) exceeds -ftrivial-auto-var-init-max-size={2}";
// clang `warn_avx_calling_convention`: {0} is 0 for a return, 1 for an
// argument; {2} the target feature (`avx`, `avx512f`).
cg_avx_calling_convention = Warning [Psabi] (3) "AVX vector {0:return|argument} of type {1} without '{2}' enabled changes the ABI";
cg_error = Error (1) "{0}";
