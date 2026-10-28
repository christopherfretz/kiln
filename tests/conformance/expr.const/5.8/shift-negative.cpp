// [expr.shift]/1: negative shift count.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_shift_negative`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:35 note "negative shift count -1"
//@ diag: 9:15 note "in call to 'f(-1)'"
constexpr int f(int n) { return 1 >> n; }
static_assert(f(-1) == 0);
