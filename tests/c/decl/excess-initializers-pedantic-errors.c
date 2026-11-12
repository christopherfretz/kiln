// C23 6.7.11p2 (initialization) is a constraint, which clang reports as
// an extension warning (-Wexcess-initializers): an error under
// -pedantic-errors, but a recoverable one, so the declaration stays
// valid and -Wunused-variable still sees it. C++ keeps the hard error
// ([dcl.init.aggr]/13, tests/conformance).
//@ expect: compile-fail
//@ strict-diags
//@ args: -pedantic-errors -Wall
//@ diag: 21:16 error "excess elements in array initializer"
//@ diag: 22:13 error "excess elements in scalar initializer"
//@ diag: 23:21 error "excess elements in struct initializer"
//@ diag: 24:20 error "excess elements in array initializer"
//@ diag: 25:19 error "excess elements in scalar initializer"
//@ diag: 26:15 error "excess elements in vector initializer"
//@ diag: 27:14 error "initializer-string for char array is too long"
//@ diag: 27:20 error "excess elements in char array initializer"
//@ diag: 29:18 error "excess elements in array initializer"
//@ diag: 29:7 warning "unused variable 'l'"
struct S { int a, b; };
typedef int v2 __attribute__((vector_size(2 * sizeof(int))));
int a[1] = {1, 2};
int x = {1, 2};
struct S s = {1, 2, 3};
int n[1][1] = {{1, 2}};
struct S t = {{1, 2}, 3};
v2 v = {1, 2, 3};
char c[1] = {"ab", 'c'};
void f(void) {
  int l[1] = {1, 2};
}
