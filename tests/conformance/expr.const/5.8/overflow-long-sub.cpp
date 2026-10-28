// [expr.pre]/4: signed long overflow.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_overflow_long_sub`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:37 note "value -9223372036854775809 is outside the range of representable values of type 'long'"
//@ diag: 9:15 note "in call to 'f(-9223372036854775808)'"
constexpr long f(long a) { return a - 1; }
static_assert(f(-9223372036854775807L - 1) == 0);
