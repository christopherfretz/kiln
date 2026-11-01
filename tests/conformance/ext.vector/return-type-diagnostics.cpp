// A vector attribute after a declarator applies to the whole declared
// type, so on a function, pointer or array declarator it names an
// invalid element type; clang then leaves the type unchanged (WP-156).
//@ expect: compile-fail
//@ diag: 12:24 error "invalid vector element type 'int ()'"
//@ diag: 13:23 error "invalid vector element type 'int *'"
//@ diag: 14:25 error "invalid vector element type 'int[2]'"
//@ diag: 15:33 error "invalid vector element type 'int *'"
//@ diag: 19:15 error "static assertion failed due to requirement '__is_same(int, __attribute__((__vector_size__(4 * sizeof(int)))) int)'"
//@ diag: 20:16 error "cannot initialize a variable of type 'v4i' (vector of 4 'int' values) with an rvalue of type 'int'"
typedef int v4i __attribute__((vector_size(16)));
int f() __attribute__((vector_size(16)));
int *q __attribute__((vector_size(16)));
int b[2] __attribute__((vector_size(16)));
typedef int *bad __attribute__((vector_size(16)));

static_assert(__is_same(bad, int*));
static_assert(__is_same(decltype(q), int*));
static_assert(__is_same(decltype(f()), v4i));
void h() { v4i v = f(); }
