// [expr.const]/16 (WP-137): the immediate invocations of default arguments
// and default member initializers that are constant expressions where they
// are used keep their value at each use: calls, constructions, aggregate
// initializations, defaulted constructors, and template instantiations.
//@ expect: run
//@ stdout: 25 7 9
//@ stdout: 9 3
//@ stdout: 16 2
//@ stdout: 4 25
extern "C" int printf(const char *, ...);
consteval int sq(int x) { return x * x; }
constexpr int seven = 7;
int f(int p = sq(5)) { return p; }
int g(int p = seven) { return p; }
struct X { int v; X(int a = sq(3)) : v(a) {} };
struct S { int m = sq(3); int n = 3; S() = default; };
struct A { int a = sq(4); int b; };
template <class T> int tf(T t) { return f() / t; }
int main() {
  X x;
  printf("%d %d %d\n", f(), g(), x.v);
  S s;
  printf("%d %d\n", s.m, s.n);
  A y{.b = 2};
  printf("%d %d\n", y.a, y.b);
  printf("%d %d\n", tf(6), tf(1));
}
