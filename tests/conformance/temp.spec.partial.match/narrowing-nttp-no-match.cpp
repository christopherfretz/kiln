// [temp.deduct.type]/20, [temp.spec.partial.match]/2: the type of `C1`
// (`unsigned char`) differs from the type of the corresponding parameter
// of `int_values` (`int`), so deduction fails and the partial
// specialization matches no argument list, 255 included (and 256 also
// does not survive the conversion, [temp.deduct.type]/18). Adapted from
// clang's CXX/temp/temp.decls/temp.class.spec/p9.cpp, which expects 255 to
// match; clang 19/22 do not check the type rule for class templates, GCC
// 16.1 does. Found by the clang negative-test run (WP-912); strict
// matching WP-1405.
//@ std: c++20
//@ oracle: gcc
//@ skip(clang): clang 19/22 match a partial specialization whose non-type parameter type differs from the primary's
//@ expect: compile-ok
template <int, int, int> struct int_values {
  static const unsigned value = 0;
};
template <unsigned char C1, unsigned char C3>
struct int_values<C1, 12, C3> {
  static const unsigned value = 1;
};
static_assert(int_values<256, 12, 3>::value == 0, "");
static_assert(int_values<255, 12, 3>::value == 0, "");
