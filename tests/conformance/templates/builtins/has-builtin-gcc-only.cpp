// __has_builtin answers clang 22's question, not "does kiln implement it":
// the GCC builtins kiln adds for GCC's own code (__builtin_shuffle,
// __builtin_speculation_safe_value, __builtin_assoc_barrier, the generic
// __builtin_clrsbg / __builtin_ffsg / __builtin_parityg, __builtin_puts,
// __builtin_has_attribute)
// answer 0, as in clang, so a library probing for a clang builtin never
// takes a path clang would not; the library builtins clang also has
// (__builtin_index, __builtin_rindex, __builtin_strstr) answer 1
// (spec/sema.md "GCC-only builtins").
// WP-1548, WP-1552, WP-1611.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
#define GCC_ONLY(x) static_assert(!__has_builtin(x), #x);
#define BOTH(x) static_assert(__has_builtin(x), #x);
GCC_ONLY(__builtin_shuffle)
GCC_ONLY(__builtin_speculation_safe_value)
GCC_ONLY(__builtin_assoc_barrier)
GCC_ONLY(__builtin_clrsbg)
GCC_ONLY(__builtin_ffsg)
GCC_ONLY(__builtin_parityg)
GCC_ONLY(__builtin_puts)
GCC_ONLY(__builtin_has_attribute)
BOTH(__builtin_shufflevector)
BOTH(__builtin_index)
BOTH(__builtin_rindex)
BOTH(__builtin_strstr)
BOTH(__builtin_clrsb)
BOTH(__builtin_popcountg)

#if __has_builtin(__builtin_shuffle) || __has_builtin(__builtin_ffsg)
#error GCC-only builtin claimed in #if
#endif
