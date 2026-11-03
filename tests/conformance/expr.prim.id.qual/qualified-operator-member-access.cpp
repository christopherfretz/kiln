//@ expect: run
//@ strict-diags
//@ stdout: 1 1 1 2 3 1 1 2 3 1
// [expr.prim.id.qual]/1, [expr.unary.op]/3, [expr.ref]/1, [over.oper.general]/1,
// [class.conv.fct]/1: `&A::operator==` (and `<=>`, `()`, `[]`, a
// conversion-function-id) forms a pointer to member, and `a.A::operator==`
// names the member through a qualified operator-function-id after `.`/`->`;
// the defaulted comparisons are usable both ways.
#include "../cmp.categories/Inputs/compare.h"
#include <stdio.h>

struct A {
  int n;
  bool operator==(const A &) const = default;
  auto operator<=>(const A &) const = default;
  constexpr int operator()(int) const { return 1; }
  constexpr int operator[](int) const { return 2; }
  constexpr operator int() const { return 3; }
};

constexpr auto p1 = &A::operator==;
constexpr auto p2 = &A::operator<=>;
constexpr auto p3 = &A::operator();
constexpr auto p4 = &A::operator[];
constexpr auto p5 = &A::operator int;
bool (A::*q1)(const A &) const = &A::operator==;

constexpr bool f(A a) {
  return a.A::operator==(a) && a.A::operator()(1) == 1 &&
         a.A::operator[](0) == 2 && a.A::operator int() == 3 &&
         (a.A::operator<=>(a) == 0);
}
static_assert(f(A{4}));
static_assert((A{1}.*p1)(A{1}) && (A{1}.*p2)(A{2}) < 0);

int main() {
  A a{5};
  A *pa = &a;
  printf("%d %d %d %d %d %d %d %d %d %d\n", (int)(a.*p1)(a),
         (int)((a.*p2)(a) == 0), (a.*p3)(0), (a.*p4)(0), (a.*p5)(),
         (int)pa->A::operator==(a), pa->A::operator()(7), pa->A::operator[](7),
         pa->A::operator int(), (int)(a.*q1)(a));
}
