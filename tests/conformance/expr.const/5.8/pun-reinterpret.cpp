// [expr.const]/5.15: a `reinterpret_cast` (type punning through a pointer).
// Mirrors crates/kiln_oven/tests/ub.rs `ub_pun_reinterpret`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:11 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 12:15 note "in call to 'f()'"
constexpr long f() {
  double d = 1.0;
  return *(long *)&d;
}
static_assert(f() == 0);
