// [class.base.init]/7, [temp.inst]: a braced-init-list argument of a
// mem-initializer in a class template's constructor is rebuilt per
// instantiation, both for a plain argument (`m(v, {v + 1, v + 2})`) and for one
// containing a pack expansion (`m(a..., {a..., 7})`); so are braced members.
//@ expect: run
#include <cstdio>
struct P { int a, b; };
struct M { int x; P p; constexpr M(int x, P p) : x(x), p(p) {} };
struct Q { int v[3]; };
template<class T> struct S {
  M m; Q q; P p2; T t;
  constexpr S(T v) : m(v, {v + 1, v + 2}), q{{v, v * 2, v * 3}}, p2{v, {}}, t{v} {}
};
template<class T> struct U {
  M m;
  template<class... A> constexpr U(A... a) : m(a..., {a..., 7}) {}
};
static_assert(S<int>(3).m.p.b == 5);
static_assert(S<int>(3).q.v[2] == 9);
static_assert(U<int>(4).m.p.a == 4 && U<int>(4).m.p.b == 7);
static_assert(S<short>(2).m.p.a == 3 && S<short>(2).p2.b == 0);
int main() {
  S<int> s(10);
  U<int> u(5);
  std::printf("%d %d %d %d %d\n", s.m.x, s.m.p.a, s.m.p.b, s.q.v[1], u.m.p.b);
  return s.m.p.b == 12 && u.m.p.b == 7 ? 0 : 1;
}
