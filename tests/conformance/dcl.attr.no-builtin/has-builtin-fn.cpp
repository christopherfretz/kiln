// [cpp.cond]/5 `__has_builtin` of a library builtin named by
// `-fno-builtin-<name>` is 0; the other library builtins and every
// `__builtin_` form stay 1. spec/pp.md "feature queries", WP-808.
//@ expect: compile-ok
//@ strict-diags
//@ args: -fno-builtin-memcpy -fno-builtin-ceil
static_assert(!__has_builtin(memcpy));
static_assert(!__has_builtin(ceil));
static_assert(__has_builtin(memset));
static_assert(__has_builtin(strlen));
static_assert(__has_builtin(__builtin_memcpy));
static_assert(__STDC_HOSTED__ == 1);
