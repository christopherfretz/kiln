// [expr.eq]/3, [expr.rel]/3, [expr.type]/4.6, [conv.qual]/3: pointers
// that are similar at any depth are compared after conversion to their
// cv-combined composite pointer type, and [over.ics.rank]/3.2.5 prefers
// the qualification conversion that adds fewer qualifiers.
//@ expect: run
//@ strict-diags
struct A { int m; };
int f(const int *const *) { return 1; }
int f(int *const *) { return 2; }
int main() {
  int x = 1;
  int *p = &x;
  int **pp = &p;
  const int **cpp = const_cast<const int **>(pp);
  int *volatile *pvp = pp;
  int ***ppp = &pp;
  const int ***cppp = const_cast<const int ***>(ppp);
  if (!(pp == cpp) || pp != cpp || cpp != pp || pp < cpp || !(pp <= cpp)) return 1;
  if (ppp != cppp || pvp != cpp) return 2;
  int A::*m = &A::m;
  int A::**mpp = &m;
  const int A::**cmpp = const_cast<const int A::**>(mpp);
  if (mpp != cmpp) return 3;
  if (f(pp) != 2) return 4;
  const int *const *r = true ? pp : cpp;
  if (**r != 1) return 5;
  return 0;
}
