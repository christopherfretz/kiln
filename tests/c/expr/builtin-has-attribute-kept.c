// __builtin_has_attribute in C answers 1 for `nonstring` and `nocommon`,
// which kiln accepts silently, where GCC keeps them, and for `nonnull` on a
// function typedef; `format` may give only its leading arguments. Found
// running GCC's C++ testsuite (its c-c++-common cases). WP-1675.
//@ std: c17
//@ expect: compile-ok
//@ requires: clang>=22
//@ oracle: gcc
//@ skip(clang): clang 19/22 have no __builtin_has_attribute
//@ strict-diags
__attribute__((nonstring)) char ns[4];
char plain[4];
__attribute__((nocommon)) int nc;
int pc;
typedef __attribute__((nonnull(2))) void F2(void *, void *);
F2 fd;
__attribute__((format(printf, 1, 2))) int pf(const char *, ...);
union __attribute__((transparent_union)) TU { int *a; long *b; };
_Static_assert(__builtin_has_attribute(ns, nonstring), "ns");
_Static_assert(!__builtin_has_attribute(plain, nonstring), "plain");
_Static_assert(__builtin_has_attribute(nc, nocommon), "nc");
_Static_assert(!__builtin_has_attribute(pc, nocommon), "pc");
_Static_assert(__builtin_has_attribute(F2, nonnull(2)), "F2");
_Static_assert(__builtin_has_attribute(fd, nonnull(2)), "fd");
_Static_assert(__builtin_has_attribute(pf, format(printf, 1)), "pf");
_Static_assert(__builtin_has_attribute(union TU, transparent_union), "TU");
