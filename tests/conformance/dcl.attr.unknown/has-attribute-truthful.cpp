// [cpp.cond] `__has_attribute`, `__has_cpp_attribute` and `__has_builtin`
// are truthful (spec/sema.md §4.5): 0 for an attribute kiln rejects as
// class A (clang answers 1), nonzero for an implemented or class B/C one,
// 0 for a builtin kiln declares but cannot lower and 1 for one it lowers.
// WP-805; `ms_struct` and `musttail` are implemented since WP-809.
//@ skip(clang): clang answers 1 for the attributes kiln rejects
//@ expect: compile-ok
//@ strict-diags
static_assert(!__has_attribute(weak_import));
static_assert(!__has_attribute(__address_space__));
static_assert(!__has_cpp_attribute(clang::matrix_type));
static_assert(__has_attribute(__ms_struct__) != 0 && __has_cpp_attribute(gnu::ms_struct) != 0);
static_assert(__has_cpp_attribute(clang::musttail) != 0);
static_assert(__has_attribute(used) != 0 && __has_attribute(retain) != 0);
static_assert(__has_cpp_attribute(clang::no_destroy) != 0);
static_assert(__has_attribute(no_sanitize) != 0 && __has_attribute(transparent_union) != 0);
static_assert(__has_cpp_attribute(nodiscard) != 0 && __has_cpp_attribute(assume) != 0);
static_assert(!__has_builtin(__builtin_arm_rsr) && !__has_builtin(__builtin_arm_trap));
static_assert(__has_builtin(__builtin_assume) != 0 && __has_builtin(__builtin_expect) != 0);
