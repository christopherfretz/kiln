// [conv.fpint]/1: a truncated value that does not fit is undefined.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_float_to_int_range`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:41 note "value 1.0E+10 is outside the range of representable values of type 'int'"
//@ diag: 9:15 note "in call to 'f(1.000000e+10)'"
constexpr int f(double d) { return (int)d; }
static_assert(f(1e10) == 0);
