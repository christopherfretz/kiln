// [conv.fpint]/1: an implicit floating-integral conversion out of range.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_float_to_int_implicit`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 9:11 note "value 10000000001 is outside the range of representable values of type 'int'"
//@ diag: 12:15 note "in call to 'f(1.000000e+10)'"
constexpr int f(double d) {
  int x = d + 1;
  return x;
}
static_assert(f(1e10) == 0);
