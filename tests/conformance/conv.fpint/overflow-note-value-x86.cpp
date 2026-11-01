// [conv.fpint]/1, [expr.const]/5.8: the out-of-range note's value for an
// x87 `long double` (64-bit significand: 20 significant digits) and
// `-Inf` (spec/oven.md "Diagnostics").
//@ expect: compile-fail
//@ args: --target=x86_64-linux-gnu
//@ diag: 10:23 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 10:27 note "value 333.33333333333333334 is outside the range of representable values of type 'const signed char'"
//@ diag: 11:23 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 11:27 note "value -Inf is outside"
constexpr signed char a = 1000.0L / 3;
constexpr signed char b = -__builtin_inf();
