// [expr.shift]/1: shift count >= width of long.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_shift_long_too_wide`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:37 note "shift count 64 >= width of type 'long' (64 bits)"
//@ diag: 9:15 note "in call to 'f(64)'"
constexpr long f(int n) { return 1L << n; }
static_assert(f(64) == 0);
