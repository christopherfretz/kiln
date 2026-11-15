// C17 6.7.3p10 (array-pointer-qualifiers-pedantic-gnu17.c): under
// -pedantic-errors the pre-C23 diagnostic for pointers to differently
// qualified arrays is an error. A conversion that also drops a qualifier
// keeps its discards-qualifiers warning (GCC 14 gives both); the group
// -Wc23-array-pointer-qualifiers controls it (here through a pragma).
// clang 19 and 22 give only the discards-qualifiers warning, a deliberate
// difference (spec/diag.md D17). WP-1435.
//@ args: -std=gnu17 -pedantic-errors
//@ expect: compile-fail
//@ strict-diags
//@ xfail(clang): clang 19 and 22 do not diagnose pointers to differently qualified arrays before C23 (C17 6.7.3p10; kiln errors under -pedantic-errors as GCC does)
//@ diag: 17:22 error "invalid use of pointers to arrays with different qualifiers in ISO C before C23"
//@ diag: 18:32 error "invalid use of pointers to arrays with different qualifiers in ISO C before C23"
//@ diag: 18:24 error "initializing 'int (*)[3]' with an expression of type 'const int (*)[3]' discards qualifiers"
//@ diag: 19:28 error "invalid use of pointers to arrays with different qualifiers in ISO C before C23"
int a[3];
const int (*ca)[3] = &a;
int drop(void) { int (*b)[3] = ca; return (*b)[0]; }
int same(void) { return &a == ca; }
#pragma GCC diagnostic ignored "-Wc23-array-pointer-qualifiers"
const int (*quiet)[3] = &a;
