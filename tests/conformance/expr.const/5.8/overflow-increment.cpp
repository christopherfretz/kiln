// [expr.pre.incr]/1: ++ on `INT_MAX` overflows.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_overflow_increment`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:3 note "value 2147483648 is outside the range of representable values of type 'int'"
//@ diag: 13:15 note "in call to 'f()'"
constexpr int f() {
  int x = 2147483647;
  ++x;
  return x;
}
static_assert(f() == 0);
