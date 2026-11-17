// [stmt.expand]/5.3, [expr.const] (P2686R5): a `constexpr` destructuring
// expansion statement over a prvalue in a function template binds
// `constexpr auto&& [u...]`, whose elements are constant in each
// instantiation as they are outside a template. Found running GCC's C++
// testsuite.
//@ xfail(kiln): WP-1613: in an instantiation the constexpr elements read the range's temporary as one of another evaluation
//@ skip(clang): clang 22 does not implement expansion statements
//@ skip(gcc): GCC 16.1 has no P2686
//@ expect: compile-ok
#include <utility>
template <int N> constexpr int g() {
  int s = 0;
  template for (constexpr auto i : std::make_index_sequence<N>{}) { s += i; }
  return s;
}
static_assert(g<3>() == 3);
