// [expr.pre]/4: signed long long overflow.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_overflow_long_long_mul`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:47 note "value 13835058055282163712 is outside the range of representable values of type 'long long'"
//@ diag: 9:15 note "in call to 'f(4611686018427387904)'"
constexpr long long f(long long a) { return a * 3; }
static_assert(f(4611686018427387904LL) == 0);
