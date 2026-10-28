// [expr.const]/5.16: modifying an object whose lifetime began outside the evaluation.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_modify_constexpr_global`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:14 note "a constant expression cannot modify an object that is visible outside that expression"
//@ diag: 13:15 note "in call to 'f()'"
constexpr int g = 1;
constexpr int f() {
  *(int *)&g = 2;
  return g;
}
static_assert(f() == 2);
