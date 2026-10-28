// [expr.const]/5.15: a `reinterpret_cast`.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_15_reinterpret_cast`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:11 note "reinterpret_cast is not allowed in a constant expression"
//@ diag: 12:15 note "in call to 'f()'"
constexpr int f() {
  int x = 1;
  return *reinterpret_cast<int *>(&x);
}
static_assert(f() == 1);
