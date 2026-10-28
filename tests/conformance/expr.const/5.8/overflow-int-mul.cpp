// [expr.pre]/4: signed int overflow in multiplication.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_overflow_int_mul`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:35 note "value 4294967296 is outside the range of representable values of type 'int'"
//@ diag: 9:15 note "in call to 'f(65536)'"
constexpr int f(int a) { return a * a; }
static_assert(f(65536) == 0);
