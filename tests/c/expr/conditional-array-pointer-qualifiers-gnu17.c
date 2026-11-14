// C23 6.5.15p6 with 6.7.3p9 (N2607): `int (*)[3]` and `const int (*)[3]`
// point to differently qualified versions of compatible types, so `?:`
// gives a pointer to the qualified composite type, `const int (*)[3]`,
// with no diagnostic, as assignment, comparison and argument passing
// accept the same pair (array-pointer-add-qualifier-*.c); kiln applies
// this in every C mode, as clang's assignment check does. clang 19 and 22
// still warn "pointer type mismatch" and give `void *` here (their
// checkConditionalObjectPointersCompatibility reads the qualifiers of the
// non-canonical pointee). WP-1427.
//@ args: -std=gnu17 -pedantic-errors -Wall
//@ expect: compile-ok
//@ strict-diags
//@ xfail(clang): clang 19 and 22 warn -Wpointer-type-mismatch for pointers to differently qualified arrays in ?: (C23 6.5.15p6, 6.7.3p9 make them compatible)
typedef int Row[3];
int f(int c, int (*a)[3], const int (*ca)[3], volatile Row *vr, int (*m)[2][3],
      const int (*cm)[2][3]) {
  _Static_assert(_Generic(c ? a : ca, const int (*)[3]: 1, default: 0), "");
  _Static_assert(_Generic(c ? ca : a, const int (*)[3]: 1, default: 0), "");
  _Static_assert(_Generic(c ? vr : ca, const volatile int (*)[3]: 1, default: 0), "");
  _Static_assert(_Generic(c ? m : cm, const int (*)[2][3]: 1, default: 0), "");
  const int (*p)[3] = c ? a : ca;
  return (*p)[0] + (*(c ? m : cm))[1][2];
}
