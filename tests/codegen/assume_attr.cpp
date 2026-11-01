// [dcl.attr.assume] (WP-160): an assumption without possible side effects
// becomes `llvm.assume` of the converted bool, even at -O0; one with
// possible side effects (a call, a volatile read, an increment) is
// ignored. Assumptions in templates are built on instantiation.
int g(int);
volatile int vv;
struct P { int a; int b; };

int f(int x, int* p, P s) {
  [[assume(x > 0)]];
  [[assume(x)]];
  [[assume(p)]];
  [[assume(x > 0 && x < 100), assume(s.a == s.b)]];
  [[assume(g(x))]];
  [[assume(vv == 0)]];
  if (x > 50) {
    [[assume(x != 51)]];
    return 1;
  }
  return x;
}

template <class T> T t(T x) {
  [[assume(x > T(0))]];
  [[assume(sizeof(T) == 4)]];
  return x;
}
int u(int y) { return t(y); }

constexpr int k(int x) {
  [[assume(x >= 0)]];
  return x * 2;
}
int w(int y) { return k(y) + k(4); }
