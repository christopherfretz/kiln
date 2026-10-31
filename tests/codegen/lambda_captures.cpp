// Lambda captures: this/*this/[=] in members, by-copy, by-reference,
// capture defaults, init captures, mutable, nested capture, a constant
// that is not odr-used, class-type and array captures (ArrayInitLoopExpr).
struct S {
  int v;
  int get() { auto l = [this] { return v; }; return l(); }
  int star() { auto l = [*this] { return v; }; return l(); }
  int impl() { auto l = [=] { return v + 1; }; return l(); }
};
int copy(int a) { auto l = [a] { return a * 2; }; return l(); }
int ref(int a) { auto l = [&a] { a += 1; }; l(); return a; }
int dflt(int a, int b) { auto l = [=] { return a + b; }; return l(); }
int dref(int a) { auto l = [&] { return ++a; }; l(); return a; }
int initc(int a) { auto l = [x = a + 1, &y = a] { return x + y; }; return l(); }
int mut(int a) { auto l = [a]() mutable { return ++a; }; l(); return l(); }
int nest(int a) { return [&] { return [=] { return a; }(); }(); }
int konst() { const int n = 5; return [] { return n; }(); }
struct S2 { double d; char c; };
double agg(S2 s) { return [s] { return s.d + s.c; }(); }
int arr(int (&a)[3]) { return [a] { return a[0] + a[2]; }(); }
int useS() { S s{3}; return s.get() + s.star() + s.impl(); }
