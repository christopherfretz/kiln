// [dcl.decl.general]/3: each init-declarator is analyzed as if it were alone,
// so `__restrict` in a ptr-operator qualifies that declarator only, and a
// `__restrict` decl-specifier on a non-pointer type ("restrict requires a
// pointer or reference", C99 6.7.3p2) is reported once per declarator.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:1 error "restrict requires a pointer or reference ('int' is invalid)"
//@ diag: 20:1 error "restrict requires a pointer or reference ('int' is invalid)"
//@ diag: 21:1 error "restrict requires a pointer or reference ('int' is invalid)"
//@ diag: 21:1 error "restrict requires a pointer or reference ('int' is invalid)"
//@ diag: 22:12 error "restrict requires a pointer or reference ('int' is invalid)"
//@ diag: 22:12 error "restrict requires a pointer or reference ('int' is invalid)"
//@ diag: 23:12 error "restrict requires a pointer or reference ('int' is invalid)"
//@ diag: 23:12 error "restrict requires a pointer or reference ('int' is invalid)"
//@ diag: 24:20 error "cannot initialize a variable of type 'int' with an rvalue of type 'int *__restrict *'"
int *__restrict a, *b;
int *__restrict *c, d;
static_assert(__is_same(decltype(a), int *__restrict) && __is_same(decltype(b), int *));
static_assert(__is_same(decltype(c), int *__restrict *) && __is_same(decltype(d), int));
__restrict int e, f;
__restrict int *g, h;
struct S { __restrict int m, n; };
void k() { __restrict int x, y; }
int *__restrict p, q = &a;
