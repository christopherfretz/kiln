// [temp.spec.partial.order], [temp.constr.order]: partial
// specializations with requires-clauses and type-constraints are
// selected by subsumption. With `--pch-diff`, the header's precompiled
// snapshot carries their constraints (WP-1342).
//@ expect: run
#include "Inputs/constrained_partial_specs.h"

struct HT { using type = int; };
struct Big { using type = int; long a, b; };

int main() {
  static_assert(cps::pick<int, HT>::value == 0);
  static_assert(cps::pick<int &, HT>::value == 2);
  static_assert(cps::pick<int &, Big>::value == 1);
  static_assert(cps::pick<int *, Big>::value == 3);
  static_assert(cps::pick<Big *, Big>::value == 0);
  static_assert(cps::kind<char> == 1 && cps::kind<Big> == 0);
  return cps::pick<int &, HT>::value + cps::pick<long &, Big>::value - 3;
}
