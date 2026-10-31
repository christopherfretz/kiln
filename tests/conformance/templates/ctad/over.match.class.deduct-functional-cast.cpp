// [expr.type.conv]/1, [dcl.type.class.deduct]/2: a functional cast
// naming a class template deduces its arguments.
//@ strict-diags
template<class T, class U> struct P {
  T a;
  U b;
  constexpr P(T x, U y) : a(x), b(y) {}
};
template<class T> struct One {
  T v;
  constexpr One(T x) : v(x) {}
};
template<class T = char> struct Def {};
static_assert(__is_same(decltype(P(1, 2u)), P<int, unsigned>));
static_assert(__is_same(decltype(P{1.0, 'c'}), P<double, char>));
static_assert(__is_same(decltype(One(3L)), One<long>));
static_assert(__is_same(decltype(Def()), Def<char>));
static_assert(__is_same(decltype(Def{}), Def<char>));
P p = P(1, 2);
static_assert(__is_same(decltype(p), P<int, int>));
static_assert(P(20, 22).a + P(20, 22).b == 42);
static_assert(One(7).v == 7);
