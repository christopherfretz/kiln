// [class.mem.general]/7, [temp.names]/3: a default member initializer or a
// member function's default argument is saved and parsed after the class,
// so a `,` inside a possible template argument list is ambiguous. As clang
// does (`ConsumeAndStoreInitializer`), the comma ends the initializer only
// if what follows is an init-declarator-list ending in `;` (or, for a
// default argument, parameters with default arguments).
//@ expect: compile-ok
template <class A, class B> struct P {
  A a; B b;
  constexpr P(A a, B b) : a(a), b(b) {}
  constexpr int f() const { return int(a) + int(b); }
};
template <int I, int J> struct Q { static constexpr int v = I + J; };
constexpr int a = 1, b = 2;
struct S {
  bool x = a < b, y = true;
  int z = P<int, long>(3, 4L).f(), w = 5;
  int u = Q<1, 2>::v;
  int t = a > b, s = a<b;
  P<int, char> pc = P<int, char>(1, 'a');
  constexpr int g(int p = a < b, int q = 2) const { return p + q; }
  constexpr int h(int p = P<int, int>(5, 6).f(), int q = 3) const { return p + q; }
  constexpr int k(int p = Q<10, 20>::v) const { return p; }
};
static_assert(S{}.x && S{}.y);
static_assert(S{}.z == 7 && S{}.w == 5 && S{}.u == 3);
static_assert(!S{}.t && S{}.s);
static_assert(S{}.pc.f() == 98);
static_assert(S{}.g() == 3 && S{}.h() == 14 && S{}.k() == 30);
template <class X> struct V {
  using alloc = long;
  P<X *, alloc> c = P<X *, alloc>(nullptr, 2L), d = c;
  template <class U> using R = P<U, U>;
  int r = R<int>(1, 2).f();
};
static_assert(V<int>{}.r == 3);
