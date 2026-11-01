// [class.temporary]/4,6, [except.ctor]/3: an array temporary of class type
// is destroyed (elements in reverse order) at the end of its
// full-expression, or with the reference it is bound to; if an element's
// constructor throws, the elements already constructed are destroyed.
//@ expect: run
//@ stdout: f d1 d0 | r d3 d2 | d5 d4 caught | g d9 d8 end
extern "C" int printf(const char *, ...);
struct T {
  int v;
  T(int x) : v(x) {
    if (x == 6) throw 1;
  }
  ~T() { printf("d%d ", v); }
};
using TA = T[2];
using T3 = T[3];
int f(const T (&a)[2]) { return printf("f "), a[0].v; }
struct End { ~End() { printf("end\n"); } } end;
const T (&g)[2] = {8, 9};
int main() {
  f({0, 1});
  printf("| ");
  {
    const TA &r = TA{2, 3};
    printf("r ");
  }
  printf("| ");
  try { (void)T3{4, 5, 6}; } catch (int) { printf("caught | "); }
  printf("g ");
}
