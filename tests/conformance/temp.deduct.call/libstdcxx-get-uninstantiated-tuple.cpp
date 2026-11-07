// [temp.deduct.call]/4.3, [temp.inst]/2 (WP-682): libstdc++ 14 `std::get`
// on a `std::tuple&` whose specialization nothing instantiated earlier in
// the TU: `std::__get_helper<__i>(__t)` (tuple:2446) deduces
// `_Tuple_impl<__i, _Head, _Tail...>&` from the tuple's base, so the tuple
// is instantiated first (googletest gmock-actions_test.cc).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 7 8 9
#include <stdio.h>
#include <tuple>

int one(std::tuple<int> &a) { return std::get<0>(a); }
int two(std::tuple<int, int> &a) { return std::get<1>(a); }
int three(const std::tuple<int, int, int> &a) { return std::get<2>(a); }

int main() {
  std::tuple<int> a{7};
  std::tuple<int, int> b{0, 8};
  std::tuple<int, int, int> c{0, 0, 9};
  printf("%d %d %d\n", one(a), two(b), three(c));
}
