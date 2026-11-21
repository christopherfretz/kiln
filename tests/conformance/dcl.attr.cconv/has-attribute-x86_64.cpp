// [cpp.cond] `__has_attribute` / `__has_cpp_attribute` are truthful
// (spec/sema.md §4.5): 1 for the calling conventions, `regparm`,
// `no_caller_saved_registers`, `no_builtin`, `pass_object_size` and
// `swiftcall` / `swiftasynccall` (WP-1745), as clang. WP-808.
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-ok
//@ strict-diags
static_assert(__has_attribute(ms_abi) && __has_attribute(sysv_abi) && __has_attribute(regcall));
static_assert(__has_attribute(vectorcall) && __has_attribute(preserve_most));
static_assert(__has_attribute(preserve_all) && __has_attribute(preserve_none));
static_assert(__has_attribute(intel_ocl_bicc) && __has_attribute(aarch64_vector_pcs));
static_assert(__has_attribute(regparm) && __has_attribute(no_caller_saved_registers));
static_assert(__has_attribute(no_builtin) && __has_attribute(pass_object_size));
static_assert(__has_attribute(pass_dynamic_object_size));
static_assert(__has_cpp_attribute(gnu::ms_abi) && __has_cpp_attribute(clang::preserve_most));
static_assert(__has_cpp_attribute(clang::no_builtin) && __has_cpp_attribute(gnu::regparm));
static_assert(__has_attribute(swiftcall) && __has_attribute(swiftasynccall));
static_assert(__has_cpp_attribute(clang::swiftcall));
