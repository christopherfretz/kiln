// C23 6.7.10p17-19 (designated initializers), plus the GNU old-style
// field designator `{ name: value }` (GCC "Designated Inits"; clang
// `ext_gnu_old_style_field_designator`, on by default as -Wgnu-designator).
// In C, designators may name members in any order and mix with positional
// initializers, so neither -Wreorder-init-list nor -Wc99-designator fires
// (clang checks both only in C++).
//@ expect: run
//@ strict-diags
//@ args: -Wall -Wextra
//@ stdout: 1 2 3 4 6 5 7 2 3 2.5
//@ diag: 17:16 warning "use of GNU old-style field designator extension"
//@ diag: 17:22 warning "use of GNU old-style field designator extension"
//@ diag: 22:45 warning "use of GNU old-style field designator extension"
#include <stdio.h>
struct P { int x, y; };
struct Q { struct P p; int z[3]; };
struct P a = { y: 2, x: 1 };
struct P b = { .y = 4, .x = 3 };
struct Q c = { .z = { [2] = 7, [0] = 5 }, .p.y = 6 };
int a3[4] = { [3] = 1, [1] = 2, 3 };
static double finite_hi(double v) {
  return ((union { double d; int i[2]; }) { d: v }).d;
}
int main(void) {
  printf("%d %d %d %d %d %d %d %d %d %g\n", a.x, a.y, b.x, b.y, c.p.y, c.z[0], c.z[2], a3[1], a3[2], finite_hi(2.5));
  return 0;
}
