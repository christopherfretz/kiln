// [dcl.pre]/10, [stmt.if]/2 (C++23, P1401R5): the condition of
// `static_assert` and `if constexpr` is contextually converted to bool, so a
// narrowing integer-to-bool conversion is allowed. WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
static_assert(2);
template <unsigned N> constexpr int f() {
  if constexpr (N) return 1;
  else return 0;
}
static_assert(f<4>() == 1 && f<0>() == 0);
