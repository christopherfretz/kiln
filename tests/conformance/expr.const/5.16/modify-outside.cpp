// [expr.const]/5.16: modification of an object whose lifetime began outside the evaluation.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_16_modify_outside`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:3 note "a constant expression cannot modify an object that is visible outside that expression"
//@ diag: 13:15 note "in call to 'f()'"
int g = 0;
constexpr int f() {
  ++g;
  return g;
}
static_assert(f() == 1);
