// [class.base.init]/1-2 (C++11): a mem-initializer-id may be a
// decltype-specifier (class-or-decltype) naming a direct base, a virtual
// base or the class itself (a delegating constructor); in a template a
// dependent one is resolved when the constructor is instantiated.
//@ expect: run
//@ std: c++23
extern "C" int printf(const char*, ...);
struct B {
  int v;
  constexpr B(int x) : v(x) {}
};
extern B gb;
struct D : B {
  constexpr D(int x) : decltype(gb)(x + 1) {}
};
static_assert(D(3).v == 4);

template <class T> struct E : T {
  constexpr E(T t) : decltype(t)(t) {}
};
static_assert(E<B>(B(5)).v == 5);

struct S {
  static S s0;
  int k;
  constexpr S(int x) : k(x * 2) {}
  constexpr S() : decltype(s0)(21) {}
};
static_assert(S().k == 42);

struct V : virtual B {
  V() : B(1) {}
};
struct W : V {
  W(int x) : decltype(gb)(x), V() {}
};

int main() {
  D d(9);
  E<B> e(B(7));
  S s;
  W w(11);
  printf("%d %d %d %d\n", d.v, e.v, s.k, w.v);
  return d.v == 10 && e.v == 7 && s.k == 42 && w.v == 11 ? 0 : 1;
}
