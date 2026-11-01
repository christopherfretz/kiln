// [conv.fpint]/1, [expr.const]/5.8: a floating value out of the
// destination's range is undefined behaviour in a constant expression;
// the note spells the source value as clang's OptionalDiagnostic does
// (`APFloat::toString` with ceil(precision * log10 2) significant digits,
// rounded half up from the exact value; spec/oven.md "Diagnostics").
//@ expect: compile-fail
//@ args: --target=aarch64-linux-gnu
//@ diag: 18:23 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 18:27 note "value 1.0E+300 is outside the range of representable values of type 'const signed char'"
//@ diag: 19:23 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 19:27 note "value 9.999999999999999E+22 is outside"
//@ diag: 20:23 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 20:27 note "value 128.10001 is outside"
//@ diag: 21:23 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 21:27 note "value 333.33333333333333333333333333333332 is outside"
//@ diag: 22:23 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 22:27 note "value 1000000000000001 is outside"
constexpr signed char a = 1e300;
constexpr signed char b = 1e23;
constexpr signed char c = 128.1f;
constexpr signed char d = 1000.0L / 3;
constexpr signed char e = 1e15 + 0.5;
