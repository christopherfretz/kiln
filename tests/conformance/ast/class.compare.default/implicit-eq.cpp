//@ expect: ast
//@ oracle: clang
//@ ast-implicit
//@ ast-filter: S::operator
// [class.compare.default]/5: a defaulted `<=>` with no `==` declared
// implicitly declares a defaulted `==` (defined on use).
#include "../../cmp.categories/Inputs/compare.h"
struct S {
  int v;
  std::strong_ordering operator<=>(const S&) const = default;
};
bool f(const S& s) { return s == s; }
