// [expr.const]/5.9: an object whose lifetime began within the evaluation may be read.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_9_lifetime_within_ok`.
constexpr int f() {
  int x = 1;
  int *p = &x;
  return *p;
}
static_assert(f() == 1);
