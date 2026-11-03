//@ expect: ast
//@ skip(clang): clang's JSON dump omits nrvo_candidate; the golden was checked against the clang 22 text dump
//@ ast-filter: Use
//@ requires: clang>=22
// (clang 19 prints the category type as 'const weak_ordering':'const std::weak_ordering'.)
// [class.spaceship]/1: the synthesized three-way comparison for a member
// with only `==` and `<` is a PseudoObjectExpr whose syntactic form is the
// `<=>` and whose semantic form is the conditional chain over two opaque
// values: `l == r ? equivalent : l < r ? less : greater`.
#include "../../cmp.categories/Inputs/compare.h"
struct L { int n; bool operator==(const L&) const; bool operator<(const L&) const; };
struct Use {
  L l;
  std::weak_ordering operator<=>(const Use&) const = default;
};
bool use(Use a, Use b) { return a < b; }
