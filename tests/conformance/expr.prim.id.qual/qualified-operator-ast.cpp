//@ expect: ast
//@ ast-filter: Use
// [expr.prim.id.qual]/1, [expr.unary.op]/3, [except.spec]/13: `&A::operator==`
// of a defaulted comparison names the member with its resolved exception
// specification (part of the type, [dcl.fct]/14), and `a.A::operator==` is a
// qualified member access through an operator-function-id.
#include "../cmp.categories/Inputs/compare.h"

struct A {
  int n;
  bool operator==(const A &) const = default;
  auto operator<=>(const A &) const = default;
  int operator()(int) const { return 1; }
  operator int() const { return 3; }
};

namespace Use {
auto p1 = &A::operator==;
auto p2 = &A::operator<=>;
auto p3 = &A::operator();
auto p5 = &A::operator int;
bool f(A a) { return a.A::operator==(a); }
}  // namespace Use
