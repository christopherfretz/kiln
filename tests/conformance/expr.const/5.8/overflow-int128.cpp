// [expr.pre]/4: signed __int128 overflow.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_overflow_int128`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:45 note "value 170141183460469231731687303715884105728 is outside the range of representable values of type '__int128'"
//@ diag: 9:15 note "in call to 'f(85070591730234615865843651857942052864)'"
constexpr __int128 f(__int128 a) { return a + a; }
static_assert(f((__int128)1 << 126) == 0);
