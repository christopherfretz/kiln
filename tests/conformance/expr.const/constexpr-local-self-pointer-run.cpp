//@ expect: run
//@ skip(clang): neither clang 22 nor GCC 16.1 implements P2686R5 constexpr references to automatic objects
//@ stdout: 127 1 1
// [expr.const.init]/2-4 (C++26): a constexpr local whose value points into
// itself designates the object of the current invocation at run time: each
// recursive call's object points to itself, not to a shared constant, and a
// closure capturing a local by reference sees later writes to it. Reduced
// from a find running GCC's C++ testsuite (WP-1680).
extern "C" int printf(const char *, ...);
struct S { const S *self = this; int v = 1; };
struct R { const R &self = *this; int v = 2; };
struct U { int v = 3; const int *p = &v; };
struct N { const N *next; int v; };
template <class T> constexpr bool tm() { constexpr T t{}; return t.self == &t; }
int run(int n) {
  constexpr S s{};
  constexpr R r{};
  constexpr U u{};
  constexpr N a[2] = {{&a[1], 4}, {&a[0], 5}};
  int k = n;
  constexpr auto l = [&k] { return k; };
  k += 1;
  constexpr auto [x, y] = S{};
  return (s.self == &s) + 2 * (&r.self == &r) + 4 * (u.p == &u.v) +
         8 * (a[0].next == &a[1]) + 16 * (l() == n + 1) + 32 * (y == 1 && x->v == 1) +
         64 * tm<S>();
}
int depth(int n) {
  constexpr S s{};
  const S *mine = &s;
  int deeper = n ? depth(n - 1) : 1;
  return deeper && mine == s.self;
}
constexpr bool rec(int n) { constexpr S s{}; return n ? rec(n - 1) && s.self == &s : s.self == &s; }
int main(int argc, char **) { printf("%d %d %d\n", run(argc + 4), depth(3), rec(argc + 1)); }
