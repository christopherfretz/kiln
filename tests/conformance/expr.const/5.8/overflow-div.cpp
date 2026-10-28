// [expr.mul]/4: `INT_MIN` / -1 is not representable.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_overflow_div`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:42 note "value 2147483648 is outside the range of representable values of type 'int'"
//@ diag: 9:15 note "in call to 'f(-2147483648, -1)'"
constexpr int f(int a, int b) { return a / b; }
static_assert(f(-2147483647 - 1, -1) == 0);
