// GNU old-style field designators in C (an extension; C23 6.7.10):
// `name: value` in a braced initializer means `.name = value`, and may be
// mixed with positional and `.name =` initializers, nested, given a
// braced value, and used for a union member or in a compound literal.
// Unlike C++ ([dcl.init.aggr]/3.1), C allows designators in any order,
// so an out-of-order list draws no -Wreorder-init-list.
// gcc.c-torture: compile/20000224-1.c, compile/init-2.c, compile/init-3.c,
// execute/20030408-1.c, execute/991228-1.c, execute/compndlit-1.c,
// execute/struct-ini-4.c.
//@ std: c17
//@ args: -Wgnu-designator
//@ expect: run
//@ strict-diags
//@ diag: 23:16 warning "use of GNU old-style field designator extension"
//@ diag: 23:22 warning "use of GNU old-style field designator extension"
//@ diag: 24:16 warning "use of GNU old-style field designator extension"
//@ diag: 25:19 warning "use of GNU old-style field designator extension"
//@ diag: 26:36 warning "use of GNU old-style field designator extension"
//@ diag: 32:28 warning "use of GNU old-style field designator extension"
//@ stdout: 2 1 0 | 3 0 7 8 | 4 | 5 6 | 9 2
#include <stdio.h>
struct S { int a, b, c; int d[2]; };
struct S x = { b: 1, a: 2 };
struct S y = { a: 3, .d = { 7, 8 } };
struct S z = { 9, d: { 1, 2 } };
union U { double f; int i; } u = { i: 4 };
struct T { struct S s; int n; };
struct T t = { { .b = 6 }, 5 };

int main(void) {
  t.s.a = 5;
  struct S w = (struct S){ c: 2, 9 };
  printf("%d %d %d | %d %d %d %d | %d | %d %d | %d %d\n", x.a, x.b, x.c,
         y.a, y.b, y.d[0], y.d[1], u.i, t.s.a, t.s.b, z.a, w.c);
  return z.d[1] - 2 + w.d[0] - 9;
}
