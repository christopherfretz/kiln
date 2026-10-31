// [class.spaceship]/2-4: the return type of a defaulted `auto <=>` is the
// common comparison category of its subobject comparisons
// (strong_ordering for none); the body compares subobjects in order and
// returns the first nonzero result. Constant evaluated in the Oven.
//@ expect: compile-ok
//@ strict-diags
#include "../cmp.categories/Inputs/compare.h"
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
struct Empty { auto operator<=>(const Empty&) const = default; };
static_assert(same<decltype(Empty{} <=> Empty{}), std::strong_ordering>::value);
struct I { int i; constexpr auto operator<=>(const I&) const = default; };
static_assert(same<decltype(I{} <=> I{}), std::strong_ordering>::value);
struct W {
  int x;
  constexpr std::weak_ordering operator<=>(const W& o) const { return x <=> o.x; }
};
struct D : I {
  W w;
  double d;
  constexpr auto operator<=>(const D&) const = default;
};
static_assert(same<decltype(D{} <=> D{}), std::partial_ordering>::value);
struct IW { I i; W w; constexpr auto operator<=>(const IW&) const = default; };
static_assert(same<decltype(IW{} <=> IW{}), std::weak_ordering>::value);
// The specified return type need not be the deduced one.
struct Sp {
  int a;
  constexpr std::partial_ordering operator<=>(const Sp&) const = default;
};
static_assert(same<decltype(Sp{} <=> Sp{}), std::partial_ordering>::value);
static_assert((Sp{1} <=> Sp{2}) == std::partial_ordering::less);
constexpr D d1{{1}, {2}, 3.0}, d2{{1}, {2}, 4.0}, d3{{0}, {9}, 9.0};
static_assert((d1 <=> d1) == 0);
static_assert((d1 <=> d2) < 0);
static_assert((d2 <=> d1) > 0);
static_assert((d3 <=> d1) < 0);
static_assert(d1 < d2 && d2 > d1 && d1 <= d1 && d3 >= d3);
constexpr D dn{{1}, {2}, __builtin_nan("")};
static_assert((dn <=> d1) == std::partial_ordering::unordered);
static_assert(!(dn < d1) && !(dn > d1) && !(dn <= d1));
// Arrays compare element-wise.
struct A {
  int v[3];
  friend constexpr auto operator<=>(const A&, const A&) = default;
};
static_assert(A{{1, 2, 3}} < A{{1, 3, 0}});
static_assert((A{{1, 2, 3}} <=> A{{1, 2, 3}}) == std::strong_ordering::equal);
