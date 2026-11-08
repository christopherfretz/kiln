// [cpp.cond]/5 [intro.compliance.general]/7 `__has_builtin` of a library
// builtin is 0 under `-ffreestanding` and `-fno-builtin` (clang
// `Builtin::Context::initializeBuiltins` skips the library ones), and a
// later `-fbuiltin` does not cancel `-ffreestanding`; a `__builtin_` form
// stays 1. spec/pp.md "feature queries", WP-808.
//@ expect: compile-ok
//@ strict-diags
//@ args: -ffreestanding -fbuiltin
static_assert(!__has_builtin(memcpy));
static_assert(!__has_builtin(strlen));
static_assert(!__has_builtin(ceil));
static_assert(__has_builtin(__builtin_memcpy));
static_assert(__has_builtin(__builtin_strlen));
static_assert(__STDC_HOSTED__ == 0);
