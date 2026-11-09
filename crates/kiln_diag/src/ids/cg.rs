// Diagnostic ids `cg_*` (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

// clang `err_alias_to_undefined` (`checkAliases`): {0} is 0 for an
// alias, 1 for an ifunc; WP-763.
cg_alias_to_undefined = Error (1) "{0:alias|ifunc} must point to a defined {0:variable or function|function}";
// clang `warn_alias_to_weak_alias` (`checkAliases`): {0} the alias's
// symbol after retargeting, {1} the weak alias's, {2} 1 for an ifunc;
// WP-763.
cg_alias_to_weak_alias = Warning [IgnoredAttributes] (3) "{2:alias|ifunc} will always resolve to {0} even if weak definition of {1} is overridden";
// clang `warn_alias_with_section` (`checkAliases`): {0} the alias's
// section, {1} 1 for an ifunc; WP-763.
cg_alias_with_section = Warning [IgnoredAttributes] (2) "{1:alias|ifunc} will not be in section '{0}' but in the same section as the {1:aliasee|resolver}";
cg_atomic_misaligned = Warning [AtomicAlignment] (2) "misaligned atomic operation may incur significant performance penalty; the expected alignment ({0} bytes) exceeds the actual alignment ({1} bytes)";
cg_atomic_oversized = Warning [AtomicAlignment] (2) "large atomic operation may incur significant performance penalty; the access size ({0} bytes) exceeds the max lock-free size ({1} bytes)";
// clang 19's text (a doubled space), kept for -fclang-compat=19.
cg_atomic_oversized_19 = Warning [AtomicAlignment] (2) "large atomic operation may incur significant performance penalty; the access size ({0} bytes) exceeds the max lock-free size ({1}  bytes)";
// WP-448 (spec/codegen.md §7r): kiln-only, no clang counterpart.
cg_auto_var_init_too_large = Remark [AutoVarInit] (3) "erroneous value of variable {0} not written: its size ({1} bytes) exceeds -ftrivial-auto-var-init-max-size={2}";
// clang `warn_avx_calling_convention`: {0} is 0 for a return, 1 for an
// argument; {2} the target feature (`avx`, `avx512f`).
cg_avx_calling_convention = Warning [Psabi] (3) "AVX vector {0:return|argument} of type {1} without '{2}' enabled changes the ABI";
// clang `err_avx_calling_convention`: as `cg_avx_calling_convention`,
// when only one of the caller and the callee has the feature; WP-1000.
cg_avx_calling_convention_mismatch = Error (3) "AVX vector {0:return|argument} of type {1} without '{2}' enabled changes the ABI";
// clang `err_builtin_needs_feature` (`checkTargetFeatures`): {0} the
// builtin, {1} its required-feature expression; WP-558.
cg_builtin_needs_feature = Error (2) "'{0}' needs target feature {1}";
// clang's custom "cannot yet mangle expression type %0" error
// (`CXXNameMangler::mangleExpression`): {0} the expression's class; a
// requires-clause the mangler cannot encode (spec/abi.md §8.15; WP-818).
cg_cannot_mangle_expression = Error (1) "cannot yet mangle expression type {0}";
// clang `err_cyclic_alias`: {0} is 0 for an alias, 1 for an ifunc;
// WP-763.
cg_cyclic_alias = Error (1) "{0:alias|ifunc} definition is part of a cycle";
// clang `err_duplicate_mangled_name` (`GetOrCreateLLVMFunction`); WP-492.
cg_duplicate_mangled_name = Error (1) "definition with same mangled name '{0}' as another definition";
cg_error = Error (1) "{0}";
// clang `err_function_needs_feature` (`checkTargetFeatures`): {0} the
// caller, {1} the `always_inline` callee, {2} the missing feature; WP-558.
cg_function_needs_feature = Error (3) "always_inline function '{1}' requires target feature '{2}', but would be inlined into function '{0}' that is compiled without support for '{2}'";
// clang `err_ifunc_resolver_return` (`checkAliases`); WP-763.
cg_ifunc_resolver_return = Error (0) "ifunc resolver function must return a pointer";
cg_sync_op_misaligned = Warning [SyncAlignment] (0) "__sync builtin operation must have natural alignment (consider using __atomic)";
