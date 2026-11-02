//@ expect: ast
//@ skip(clang): clang's JSON dump omits nrvo_candidate; the golden was checked against the clang 22 text dump
//@ ast-filter: A::operator
//@ requires: clang>=22
// golden checked against the clang 22 text dump (clang 19 printed scoped second strings and a NoOp over DerivedToBase)
// [class.spaceship]/1-3: the implicit definition of a defaulted `<=>`
// compares each subobject in an `if (auto cmp = x <=> y; cmp != 0)
// return cmp;` and returns `strong_ordering::equal` converted to the
// return type; the return type is deduced as the common category.
#include "../../cmp.categories/Inputs/compare.h"
struct B {
  int x;
  bool operator==(const B&) const = default;
  auto operator<=>(const B&) const = default;
};
struct A : B {
  int a;
  float f[2];
  auto operator<=>(const A&) const = default;
};
bool t(const A& x, const A& y) { return (x <=> y) < 0; }
