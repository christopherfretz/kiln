// [expr.const]/5.9: L2R of a volatile glvalue.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_9_volatile`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:10 note "read of volatile-qualified type 'volatile int' is not allowed in a constant expression"
//@ diag: 12:15 note "in call to 'f()'"
constexpr int f() {
  volatile int v = 1;
  return v;
}
static_assert(f() == 1);
