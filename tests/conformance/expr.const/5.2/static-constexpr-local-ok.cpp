// [expr.const]/5.2: a static variable usable in constant expressions may be defined (C++23).
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_2_static_constexpr_local_ok`.
constexpr int f() {
  static constexpr int x = 4;
  return x;
}
static_assert(f() == 4);
