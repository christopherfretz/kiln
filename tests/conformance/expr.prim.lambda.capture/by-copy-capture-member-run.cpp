// [expr.prim.lambda.capture]/11: an odr-use of an entity captured by copy
// names the closure's member, so a `mutable` lambda modifies its copy and
// not the variable; an inner lambda capturing by reference the outer
// lambda's copy modifies that copy (/12); a by-reference capture modifies
// the variable. The same function is checked in constant evaluation.
//@ expect: run
//@ std: c++20
//@ stdout: 1 2 3 7 6 6 223 223
#include <cstdio>

struct C {
  int v;
  constexpr int bump() { return ++v; }
};

constexpr int run() {
  int n = 1;
  C c{5};
  auto l = [=]() mutable { int a = ++n; int b = c.bump(); return a * 10 + b; };
  int first = l();   // n copy 2, c copy 6
  int second = l();  // n copy 3, c copy 7
  auto outer = [=]() mutable {
    [&] { ++*&n; }();
    return n;
  };
  int o = outer();   // 2: the outer copy, not n
  auto byref = [&] { ++n; };
  byref();           // n == 2
  return n * 100 + (first == 26 && second == 37 ? 20 : 0) + (o == 2 ? 3 : 0);
}

int main() {
  int n = 1;
  auto l = [=]() mutable { return ++n; };
  int a = l();
  int b = l();
  C c{5};
  auto m = [c]() mutable { return c.bump() + c.bump(); };  // 6 + 7
  auto r = [&c] { return c.bump(); };
  int mv = m();
  int rv = r();
  constexpr int k = run();
  std::printf("%d %d %d %d %d %d %d %d\n", n, a, b, mv - 6, rv, c.v, run(), k);
}
