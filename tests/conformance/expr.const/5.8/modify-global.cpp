// [expr.const]/5.16: modifying a non-const global.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_modify_global`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:5 note "a constant expression cannot modify an object that is visible outside that expression"
//@ diag: 13:15 note "in call to 'f()'"
int g = 1;
constexpr int f() {
  g = 2;
  return 2;
}
static_assert(f() == 2);
