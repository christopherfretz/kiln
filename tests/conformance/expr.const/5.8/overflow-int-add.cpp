// [expr.pre]/4: signed int overflow.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_overflow_int_add`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:35 note "value 2147483648 is outside the range of representable values of type 'int'"
//@ diag: 9:15 note "in call to 'f(2147483647)'"
constexpr int f(int a) { return a + 1; }
static_assert(f(2147483647) == 0);
