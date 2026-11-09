// [dcl.constexpr]/6, [expr.const]: a consteval function calling a consteval
// operator template can produce a constant expression; clang does not warn.
// From clang's SemaCXX/consteval-operators.cpp. Found by the clang negative-
// test run (WP-912).
//@ std: c++20
//@ expect: compile-ok
template <typename DataT> struct B { DataT D; };
template <typename DataT> consteval B<DataT> operator+(B<DataT> lhs, B<DataT> rhs) {
  return B<DataT>{lhs.D + rhs.D};
}
consteval B<int> non_template_add(B<int> a, B<int> b) { return a + b; }
static_assert(non_template_add(B<int>{7}, B<int>{3}).D == 10);
