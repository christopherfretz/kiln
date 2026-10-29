// [dcl.init.general]/16.6.2.2, [dcl.init.aggr]: parenthesized aggregate initialization
// (P0960). Too many expressions is ill-formed; a missing element is
// initialized from its default member initializer or value-initialized.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:5 error "excess elements in array initializer"
//@ diag: 12:3 error "excess elements in struct initializer"
struct C { C(); };
struct B { int b; };
struct P : B { int x = 5; C c; int z; };
int b[2](1, 2, 3);
P e(B{1}, 2, C(), 4, 5);
P ok(B{1}, 2);
int fine[](1, 2, 3);
