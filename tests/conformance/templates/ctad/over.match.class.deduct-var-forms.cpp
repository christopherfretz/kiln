// [dcl.type.class.deduct]/1, [over.match.class.deduct]/1: a variable
// declared with a class template placeholder deduces its arguments from
// the implicit guide of each constructor, whatever the initializer form.
//@ strict-diags
template<class T, class U> struct P {
  T a;
  U b;
  constexpr P(T x, U y) : a(x), b(y) {}
};
P p1(1, 2.0);
static_assert(__is_same(decltype(p1), P<int, double>));
P p2{1, 'c'};
static_assert(__is_same(decltype(p2), P<int, char>));
P p3 = {1L, 2u};
static_assert(__is_same(decltype(p3), P<long, unsigned>));
const P p4(true, 1.0f);
static_assert(__is_same(decltype(p4), const P<bool, float>));
constexpr P p5(3, 4);
static_assert(p5.a + p5.b == 7);
// [over.match.class.deduct]/1.2: a template with no constructors gets a
// guide from a hypothetical `C()`; defaulted parameters complete it.
template<class T = int> struct D {};
D d;
static_assert(__is_same(decltype(d), D<int>));
namespace N {
template<class T> struct Q { Q(T *); };
}
int i;
N::Q q(&i);
static_assert(__is_same(decltype(q), N::Q<int>));
constexpr int local() {
  P r(5, 6);
  return r.a * r.b;
}
static_assert(local() == 30);
