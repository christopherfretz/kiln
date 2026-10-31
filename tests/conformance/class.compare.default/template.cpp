// [class.compare.default]/1, [temp.inst]: defaulted comparisons of class
// templates (member and friend) are instantiated with the class and
// defined on use in each specialization.
//@ expect: compile-ok
//@ strict-diags
#include "../cmp.categories/Inputs/compare.h"
template <class T> struct P {
  T a, b;
  friend constexpr auto operator<=>(const P&, const P&) = default;
};
template <class T> struct Q {
  T x;
  constexpr bool operator==(const Q&) const = default;
  constexpr auto operator<=>(const Q&) const = default;
};
template <class T> struct R {
  T v;
  friend constexpr bool operator==(const R&, const R&) = default;
};
static_assert(P<int>{1, 2} == P<int>{1, 2});
static_assert(P<int>{1, 2} < P<int>{1, 3});
static_assert((P<int>{1, 2} <=> P<int>{1, 2}) == 0);
static_assert(P<double>{1, 2} >= P<double>{1, 2});
static_assert(Q<int>{1} != Q<int>{2});
static_assert(Q<int>{1} < Q<int>{2});
static_assert(Q<float>{1} <= Q<float>{1});
static_assert(R<char>{'a'} == R<char>{'a'});
static_assert(R<long>{1} != R<long>{2});
bool f(Q<long> a, Q<long> b) { return a > b; }
