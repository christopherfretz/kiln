// GCC's manual on __builtin_has_attribute: arguments that are given are
// "validated and matched up to the provided number", and the result is true
// if all provided arguments match. So for `format(printf, 2, 3)` a probe
// naming another format-string or first-argument index answers 0. GCC 16.1
// compares only the archetype (it answers 1 for these, and 0 for
// `__printf__` against a stored `printf`); kiln follows the manual, and
// treats `__x__` as `x` as the attribute itself does. Found running GCC's
// C++ testsuite (c-c++-common/builtin-has-attribute-3). WP-1675.
//@ std: c++20
//@ expect: compile-ok
//@ skip(clang): clang 19/22 have no __builtin_has_attribute
//@ strict-diags
#define HAS(x, ...) __builtin_has_attribute(x, __VA_ARGS__)

__attribute__((format(printf, 2, 3))) void ff(int, const char *, ...);
__attribute__((format(__printf__, 2, 0))) void fv(int, const char *, __builtin_va_list);
static_assert(HAS(ff, format(printf, 2)) && !HAS(ff, format(printf, 3)));
static_assert(HAS(ff, format(printf, 2, 3)) && !HAS(ff, format(printf, 2, 4)));
static_assert(!HAS(ff, format(printf, 1, 3)) && HAS(ff, format(__printf__)));
static_assert(HAS(fv, format(printf, 2, 0)) && !HAS(fv, format(printf, 2, 3)));
