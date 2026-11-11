// C17 6.5.2.2p7: an argument is converted as if by assignment to the
// parameter's type; a structure argument is a plain copy of the value
// (no reference binding, no constructor). At -O2 glibc's <stdio.h>
// defines vprintf inline, passing a va_list (a structure on AArch64) by
// value to vfprintf; va_start/va_end take the va_list by reference.
//@ std: c17
//@ expect: run
//@ args: -O2
//@ stdout: 12 10 10
#include <stdarg.h>
#include <stdio.h>
struct a { int x; double d; char buf[40]; };
static int f(struct a v) { v.x += 1; return v.x + (int)v.d + v.buf[39]; }
static struct a mk(int x) {
  struct a r = {x, 2.5, {0}};
  r.buf[39] = 3;
  return r;
}
static int g(const char *fmt, ...) {
  va_list ap;
  va_start(ap, fmt);
  int r = vprintf(fmt, ap);
  va_end(ap);
  return r;
}
int main(void) {
  struct a v = {10, 1.5, {0}};
  int r = f(v);
  int r2 = f(mk(4));
  g("%d %d %d\n", r, r2, v.x);
  return 0;
}
