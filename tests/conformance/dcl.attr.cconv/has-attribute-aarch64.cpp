// [cpp.cond] `__has_attribute` on AArch64 (spec/sema.md §4.5): 1 for the
// conventions kiln implements, including `aarch64_vector_pcs` and
// `aarch64_sve_pcs`, and `swiftcall` (WP-1745); 0 for x86
// `no_caller_saved_registers`, as clang. WP-808.
//@ args: --target=aarch64-unknown-linux-gnu
//@ expect: compile-ok
//@ strict-diags
static_assert(__has_attribute(aarch64_vector_pcs) && __has_attribute(aarch64_sve_pcs));
static_assert(__has_attribute(ms_abi) && __has_attribute(preserve_none));
static_assert(__has_cpp_attribute(clang::aarch64_vector_pcs) && __has_attribute(no_builtin));
static_assert(__has_attribute(swiftcall) && !__has_attribute(no_caller_saved_registers));
