//@ expect: ast
//@ oracle: clang
//@ ast-filter: S::operator<
//@ requires: clang>=22
// [class.compare.secondary]/2: the implicit definition of a defaulted
// secondary operator returns the rewritten comparison `x <=> y < 0`.
#include "../../cmp.categories/Inputs/compare.h"
struct S {
  int v;
  std::strong_ordering operator<=>(const S&) const;
  bool operator<(const S&) const = default;
};
bool f(const S& s) { return s < s; }
