// [temp.deduct.type]/18, [temp.spec.partial.match]/2: 256 does not survive
// the conversion to the deduced 'unsigned char' parameter, so the partial
// specialization does not match. From clang's
// CXX/temp/temp.decls/temp.class.spec/p9.cpp. Found by the clang negative-
// test run (WP-912).
//@ std: c++20
//@ expect: compile-ok
template <int, int, int> struct int_values {
  static const unsigned value = 0;
};
template <unsigned char C1, unsigned char C3>
struct int_values<C1, 12, C3> {
  static const unsigned value = 1;
};
static_assert(int_values<256, 12, 3>::value == 0, "");
static_assert(int_values<255, 12, 3>::value == 1, "");
