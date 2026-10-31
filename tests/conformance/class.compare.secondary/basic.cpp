// [class.compare.secondary]/2: a defaulted secondary comparison operator
// `x @ y` is defined as the rewritten `<=>` (or `==` for `!=`) comparison
// selected by overload resolution. Constant evaluated in the Oven.
//@ expect: compile-ok
//@ strict-diags
#include "../cmp.categories/Inputs/compare.h"
struct S {
  int a, b;
  constexpr auto operator<=>(const S&) const = default;
  constexpr bool operator<(const S&) const = default;
  constexpr bool operator<=(const S&) const = default;
  constexpr bool operator>(const S&) const = default;
  constexpr bool operator>=(const S&) const = default;
  constexpr bool operator!=(const S&) const = default;
};
static_assert(S{1, 2} < S{1, 3});
static_assert(!(S{1, 3} < S{1, 3}));
static_assert(S{1, 3} <= S{1, 3});
static_assert(S{2, 0} > S{1, 9});
static_assert(S{2, 0} >= S{1, 9});
static_assert(S{2, 0} != S{1, 9});
static_assert(!(S{2, 0} != S{2, 0}));
// The three-way comparison need not be defaulted, and a friend form.
struct T {
  int v;
  friend constexpr std::weak_ordering operator<=>(const T& x, const T& y) { return x.v % 4 <=> y.v % 4; }
  friend constexpr bool operator==(const T& x, const T& y) { return x.v % 4 == y.v % 4; }
  friend constexpr bool operator<(const T&, const T&) = default;
  friend constexpr bool operator!=(const T&, const T&) = default;
};
static_assert(T{5} < T{2});
static_assert(!(T{1} != T{5}));
// A comparison with a parameter of another type.
struct U {
  int u;
  constexpr std::strong_ordering operator<=>(int i) const { return u <=> i; }
  constexpr bool operator>(const U&) const = default;
  constexpr auto operator<=>(const U&) const = default;
};
static_assert(U{3} > U{2});
