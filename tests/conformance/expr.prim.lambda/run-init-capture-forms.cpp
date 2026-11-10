// [expr.prim.lambda.capture]/6: an init-capture behaves as if it declares
// `auto name initializer` -- `= e`, `(e)`, `{e}` or `= {e...}` -- and its
// closure member is initialized from that initializer. `[x{1}]` deduces
// int, `[w = {4, 5}]` std::initializer_list<int>, `&r(a)` is a reference;
// direct-initialization calls an explicit copy constructor, which
// copy-initialization (`[q = ox]`) could not.
//@ expect: run
//@ xfail(gcc): GCC 16.1 rejects a direct-initialized init-capture `[q(x)]`/`[q{x}]` of a class with an explicit copy constructor ([expr.prim.lambda.capture]/6)
//@ stdout: 15 107 107 321 2 7
#include <cstdio>
#include <initializer_list>
struct X {
  int v;
  X(int a) : v(a) {}
  explicit X(const X &o) : v(o.v + 100) {}
};
struct P {
  int a, b;
  P(int x, int y) : a(x), b(y) {}
};
int main() {
  int a = 3;
  auto l = [x{1}, y(2), z = a, &r(a), w = {4, 5}] { return x + y + z + r + (int)w.size(); };
  X ox(7);
  auto m = [q(ox)] { return q.v; };
  auto mb = [q{ox}] { return q.v; };
  auto n = [p(P(1, 2)), s{P{3, 4}}] { return p.a + p.b * 10 + s.a * 100; };
  auto mut = [c{0}]() mutable { return ++c; };
  mut();
  auto ref = [&r(a)] { r = 7; };
  ref();
  std::printf("%d %d %d %d %d %d\n", l(), m(), mb(), n(), mut(), a);
}
