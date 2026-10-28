// [expr.const]/5.2: control flow through the definition of a block variable with static storage duration that is not usable in constant expressions.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_2_static_local`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 9:14 note "control flows through the definition of a static variable"
//@ diag: 12:15 note "in call to 'f()'"
constexpr int f() {
  static int x = 1;
  return x;
}
static_assert(f() == 1);
