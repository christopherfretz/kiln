// [temp.res.general]/6, [temp.inst], [dcl.init.aggr]/4.2,
// [dcl.init.list]/3.7, [over.ics.list], [over.sub]: a braced-init-list
// that semantic analysis turned into a list-initializing construction
// (an aggregate or array element `{x, 1}`, a braced argument of a
// constructor, of a subscript operator or of a call) is a braced list
// again at instantiation when its elements are instantiation-dependent,
// and the initialization is redone from it. WP-301.
//@ expect: compile-ok
#include <initializer_list>

struct S {
  int v;
  constexpr S(int a, int b = 0) : v(a * 10 + b) {}
  constexpr S() : v(5) {}
};
struct A { S s; int k; };
struct P { S s; constexpr P(S x, int) : s(x) {} };
struct L {
  int n;
  constexpr L(std::initializer_list<S> l) : n(int(l.size())) {}
};
struct M {
  constexpr int m(S s) const { return s.v; }
  constexpr int operator()(S s, S t) const { return s.v + t.v; }
  constexpr int operator[](S s) const { return s.v; }
};
constexpr int take(S s) { return s.v; }

template <int K> constexpr int f() {
  int a = K;
  S arr[2] = {{a, 1}, {a}};     // 11 10
  A ag = {{a, 2}, a};           // 12 1
  A ag2{{a, 3}, a};             // 13 1
  S d[3] = {{a}, {}};           // d[2] value-initialized: 5
  L ll{{a}, {a, 1}, {}};        // 3
  P p({a, 4}, a);               // 14
  P q{{a, 5}, a};               // 15
  M mm;
  int calls = take({a, 6}) + mm.m({a, 7}) + mm({a, 8}, {}) + mm[{a, 9}];
  return arr[0].v + arr[1].v + ag.s.v + ag.k + ag2.s.v + ag2.k + d[2].v + ll.n + p.s.v
       + q.s.v + calls;
}
static_assert(f<1>() == 11 + 10 + 12 + 1 + 13 + 1 + 5 + 3 + 14 + 15 + 16 + 17 + 18 + 5 + 19);

template <class T> struct TT {
  S s{T(1), 2};
  S t = {T(2)};
  constexpr int v() const { return s.v + t.v; }
};
static_assert(TT<int>().v() == 32);

int main() {}
