// [dcl.constexpr]/3 (C++23, P2647R1): a constexpr function may declare a
// static or thread_local variable that is usable in constant expressions.
// WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
constexpr char digit(int n) {
  static constexpr char digits[] = "0123456789";
  return digits[n];
}
static_assert(digit(7) == '7');
