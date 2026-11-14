// [temp.variadic]/5, [temp.alias]/2: pack expansions of a known length
// in alias templates' underlying types and a return type. With
// `--pch-diff`, the header's snapshot carries each expansion (WP-1300).
//@ expect: run
#include "Inputs/expr_pack.h"

static_assert(pk_twice<int, char>::n == 4 && pk_ptrs<int, char, long>::n == 3);

int main() {
  auto l = pk_append(1, 'c');
  return decltype(l)::n == 3 && pk_twice<>::n == 0 ? 0 : 1;
}
