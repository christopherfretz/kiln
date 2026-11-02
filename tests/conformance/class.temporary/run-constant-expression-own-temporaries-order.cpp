// [intro.execution]/5, [class.temporary]/4, [expr.const]/22: a template
// argument that has temporaries of its own (`C<D(1).v>`) is a
// full-expression of its own, built while the enclosing full-expression
// still has pending cleanups. It gets its own cleanups and leaves the
// enclosing ones in place: `D(3)` is still destroyed at the end of its
// full-expression, before the next statement. Explicit arguments, the same
// in an instantiated function template, and a default template argument
// with a temporary substituted by overload resolution. WP-301.
//@ expect: run
//@ stdout: use3 ~D3 | f4 ~D4 | use6 ~D6 | use7 ~D7 | use9 ~D9 | 4 2 11 3 6
#include <cstdio>

struct D {
  int v;
  constexpr D(int v) : v(v) {}
  constexpr ~D() {
    if !consteval {
      std::printf("~D%d ", v);
    }
  }
};
template <int N> struct C { static constexpr int v = N; };
int use(const D& d) { std::printf("use%d ", d.v); return d.v; }
template <int N> int f(const D& d) { std::printf("f%d ", d.v); return N; }
template <class T, int N = D(5).v> int g(T t) { return N + use(t); }
template <int N> int h() { return use(D(N + 4)) + C<D(N).v>::v; }
template <class T, int N = D(sizeof(T)).v> int k(T t) { return N + use(t) - 7; }

int main() {
  int r = use(D(3)) + C<D(1).v>::v;
  std::printf("| ");
  int s = f<D(2).v>(D(4));
  std::printf("| ");
  int t = g(D(6));
  std::printf("| ");
  int u = h<3>() - 7;
  std::printf("| ");
  int w = k(D(9)) + sizeof(D) - 4;
  std::printf("| %d %d %d %d %d\n", r, s, t, u, w);
}
