// [expr.shift]/1: an unsigned shift count >= width is also undefined.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_shift_unsigned_too_wide`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:41 note "shift count 32 >= width of type 'unsigned int' (32 bits)"
//@ diag: 9:15 note "in call to 'f(32)'"
constexpr unsigned f(int n) { return 1u >> n; }
static_assert(f(32) == 0);
