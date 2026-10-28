// [expr.const]/4, [expr.const]/5.9: a const integral variable with a constant initializer is usable in constant expressions.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_4_const_integral_usable_ok`.
const int k = 3;
const int a[2] = {1, 2};
constexpr int x = 5;
static_assert(k + x == 8);
