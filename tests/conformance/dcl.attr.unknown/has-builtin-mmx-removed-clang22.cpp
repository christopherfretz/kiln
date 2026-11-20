// [cpp.cond] `__has_builtin` at clang compatibility 22: the MMX builtins
// clang 20 removed (`__builtin_ia32_maskmovq`, `packsswb`, `movntq`) answer
// 0, as in clang 22, even though kiln still lowers them for clang 19
// compatibility; target builtins clang 22 keeps answer 1 (WP-1739).
//@ requires: clang>=22
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-ok
//@ strict-diags
static_assert(!__has_builtin(__builtin_ia32_maskmovq));
static_assert(!__has_builtin(__builtin_ia32_packsswb));
static_assert(!__has_builtin(__builtin_ia32_movntq));
static_assert(__has_builtin(__builtin_ia32_rdtsc));
static_assert(__has_builtin(__builtin_ia32_pause));
