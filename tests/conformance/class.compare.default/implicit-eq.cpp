// [class.compare.default]/5: a defaulted `<=>` with no `==` declared
// implicitly declares a defaulted `==` with the same parameters, access,
// constexpr and noexcept; `!=` is rewritten to it.
//@ expect: compile-ok
//@ strict-diags
#include "../cmp.categories/Inputs/compare.h"
struct A {
  int x;
  constexpr auto operator<=>(const A&) const = default;
};
static_assert(A{1} == A{1});
static_assert(A{1} != A{2});
static_assert(noexcept(A{1} == A{1}));
struct B {
  int y;
  friend constexpr auto operator<=>(const B&, const B&) = default;
};
static_assert(B{3} == B{3});
static_assert(!(B{3} != B{3}));
struct C {
  int z;
  auto operator<=>(const C&) const = default;
  // A user-declared `==` suppresses the implicit one.
  constexpr bool operator==(const C&) const { return true; }
};
static_assert(C{1} == C{2});
struct D : A {
  float f;
  constexpr auto operator<=>(const D&) const = default;
};
static_assert(D{{1}, 2.0f} == D{{1}, 2.0f});
static_assert(D{{1}, 2.0f} != D{{2}, 2.0f});
