export module imglf;
export namespace lf {
inline int plain(int x) { auto f = [x](int y) { return x + y; }; return f(1); }
inline int byref(int x) {
  int s = 0;
  auto f = [&s, x](int y) mutable { s += x + y; x = 0; return s; };
  f(2);
  return f(3);
}
inline int noncap() { auto f = [](int y) { return y * 2; }; int (*p)(int) = f; return p(4) + f(1); }
inline int initcap(int x) { auto f = [z = x + 1, &r = x]() { r = 9; return z; }; return f() + x; }
struct S {
  int v = 3;
  int get() { return [this] { return v; }() + [*this] { return v; }(); }
};
inline int generic() { auto g = [](auto a, auto b) { return a + b; }; return g(1, 2) + int(g(1.5, 2.5)); }
template <class T> T tl(T t) { auto f = [t](T u) { return t * u; }; auto g = [] { return 1; }; return f(t) + g(); }
inline int use_tl() { return tl(2); }
inline constexpr auto nsl = [](int a) { return a + 100; };
inline constexpr auto nsg = []<class T>(const T& a) { return a + 1; };
inline auto v1 = [] { return 10; };
using V1 = decltype(v1);
inline auto withcap = [n = 4] { return n; };
struct M { int (*fp)(int) = [](int a) { return a - 1; }; };
struct K { static constexpr auto sm = [](int x) { return x + 1; }; int m = [] { return 5; }(); };
template <class T> inline auto vt = [](T x) { return x + T(1); };
inline int defarg(int (*f)(int) = [](int a) { return a * 3; }) { return f(2); }
inline int (*fp())(int) { return [](int a) { return a * 7; }; }
template <class T> struct TC {
  int k() { return 1; }
  int f() { return [this] { return int(sizeof(T)) + k(); }(); }
};
inline int use_tc() { return TC<int>{}.f(); }
template <class... Ts> int sum_each(Ts&... xs) { int s = 0; ([&] { s += int(xs); }(), ...); return s; }
template <class... Ts> int count_each() { int n = 0; ([&] { n += int(sizeof(Ts)); }(), ...); return n; }
template <class... Ts> auto caps(Ts... xs) { return [xs...] { return (xs + ... + 0); }; }
template <class... Ts> auto initpack(Ts... xs) { return [... ys = xs] { return (ys * ... * 1); }; }

// A local class's member functions are not emitted for an importer yet
// (local-class-members-link-run.cpp, xfail): data members and nested
// types only.
inline int lc1(int x) { struct P { int a, b; }; P p{x, 2}; return p.a + p.b; }
inline int le1(int x) { enum E { A = 1, B = 4 }; enum class F : char { X = 2 }; return x + B + int(F::X); }
inline int lc2() {
  struct Q { enum { K = 7 }; struct In { int z = 5; }; };
  return Q::K + Q::In{}.z;
}
template <class T> T lct(T t) { struct W { T v; T twice() const { return v + v; } }; return W{t}.twice(); }
inline int lboth(int x) { struct C { int m; }; auto f = [](C c) { return c.m * 2; }; return f(C{x}); }
} // namespace lf
