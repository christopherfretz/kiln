// [expr.shift]/1: shift count >= width.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_shift_too_wide`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:35 note "shift count 32 >= width of type 'int' (32 bits)"
//@ diag: 9:15 note "in call to 'f(32)'"
constexpr int f(int n) { return 1 << n; }
static_assert(f(32) == 0);
