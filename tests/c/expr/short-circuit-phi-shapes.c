// C23 6.5.14, 6.5.15, 6.5.16, 6.8.5.1: codegen shapes that build phis over
// short-circuit operands (WP-1288 audit after the GNU `?:` ICE): `&&`/`||`
// as a condition and as a value, a `?:` with void operands, GNU statement
// expressions inside conditions, a `?:` whose condition is a `?:`, and
// stored logical results reused as conditions. The -O2 build is
// short-circuit-phi-shapes-O2.c.
//@ std: c17
//@ expect: run
//@ stdout: 1 0 1 0 | 1 1 | 4 1 2 3 | 1 0 5 | 6 0 1 | 2 3
#include <stdio.h>
static int cnt;
static void inc(void) { ++cnt; }
static int lor(int a, int b, int c) { return a || (b && c); }
static int land(int a, int b) {
  if (a && (b || a > 2))
    return 1;
  return 0;
}
static void voids(int a, int b) {
  (a || b) ? inc() : (void)0;
  (a && b) ? (void)0 : inc();
}
static int se(int a, int b) { return ({ int t = a || b; t; }) ?: 4; }
static int se2(int a, int b) {
  if (({ a && b; }))
    return 1;
  return ({ int q = a; q || b; }) ? 2 : 3;
}
static int se3(int a, int b) { return (a && ({ int t = b * 2; t; })) || (({ b; }) && a); }
static int cc(int a, int b, int c) { return ((a || b) ? (b && c) : (a || c)) ? 5 : 0; }
static int lnot(int a, int b) { return !(a || b) ? 6 : 0; }
static int reuse(int a, int b) {
  int x;
  int y = (x = a && b) ? 1 : 2;
  return y + (x ?: 1);
}
int main(void) {
  printf("%d %d %d %d | ", lor(1, 0, 0), lor(0, 1, 0), land(3, 0), land(0, 1));
  voids(0, 0);
  int c0 = cnt;
  voids(1, 1);
  printf("%d %d | ", c0, cnt - c0);
  printf("%d %d %d %d | ", se(0, 0), se(2, 0), se2(0, 1), se2(0, 0));
  printf("%d %d %d | ", se3(1, 1), se3(0, 0), cc(1, 1, 1));
  printf("%d %d %d | ", lnot(0, 0), lnot(1, 0), se2(1, 1));
  printf("%d %d\n", reuse(1, 1), reuse(0, 1));
  return 0;
}
