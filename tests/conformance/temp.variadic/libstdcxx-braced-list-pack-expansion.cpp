// [temp.variadic]/5.4: the pattern of a pack expansion in an
// initializer-list is an initializer-clause, so a braced-init-list
// `{t, 1}...` expands to one braced-init-list per element: in an
// aggregate's list, a class's list (copy-list-initialization through a
// constructor), a mem-initializer, a designated clause and a function
// call's arguments. libstdc++ mutex:591 `unique_lock<_L0> __locks[_Np] =
// {{__l0, defer_lock}, {__lockables, defer_lock}...};` (`<mutex>`,
// `<future>`; docs/status/libstdcxx.md cluster Z, WP-332).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 63 5 5 7 6 3 9 6 3
extern "C" int printf(const char *, ...);
struct P { int a; int b; };
struct Q { P p; char c; };
struct D {};
struct U {
  int *p;
  U(int &x, D) : p(&x) {}
};
template <class... T> int sum(T... t) {
  P arr[sizeof...(T)] = {{t, 1}...};
  int s = 0;
  for (auto &p : arr) s += p.a * 10 + p.b;
  return s;
}
template <class... T> int last(T... t) {
  P arr[1 + sizeof...(T)] = {{0, 0}, {t, t}...};
  return arr[sizeof...(T)].a;
}
template <class... T> struct S {
  static int f(T... t) {
    Q q[sizeof...(T)] = {{{t, 2}, 'x'}...};
    return q[0].p.b + q[0].p.a;
  }
  template <class... V> static int g(V... v) {
    P p[sizeof...(T)] = {{T(0), v}...};
    return p[0].b;
  }
};
template <class L0, class... L> int locks(L0 &l0, L &...l) {
  U us[1 + sizeof...(L)] = {{l0, D{}}, {l, D{}}...};
  return *us[sizeof...(L)].p;
}
int take(P a, P b) { return a.a + b.b; }
template <class... T> int call(T... t) { return take({t, 1}...); }
template <class... T> struct M {
  P ps[sizeof...(T)];
  M(T... t) : ps{{t, 4}...} {}
};
template <class... T> struct Agg { P ps[sizeof...(T)]; };
template <class... T> Agg<T...> des(T... t) { return {.ps = {{t, 3}...}}; }
struct H {
  P ps[2];
  template <class... T> H(T... t) : ps{{t, 9}...} {}
};
int main() {
  int a = 1, b = 2, c = 3;
  printf("%d %d %d %d %d %d %d %d %d\n", sum(1, 2, 3), last(4, 5), S<int>::f(3),
         S<int, long>::g(7, 8), call(5, 6), locks(a, b, c), H(1, 2).ps[1].b,
         M<int, int>(5, 6).ps[1].a, des(1, 2).ps[0].b);
}
