// [expr.const]/5.16: compound assignment to a global.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_16_compound_assign_global`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:5 note "a constant expression cannot modify an object that is visible outside that expression"
//@ diag: 13:15 note "in call to 'f()'"
int g = 0;
constexpr int f() {
  g += 1;
  return 0;
}
static_assert(f() == 0);
