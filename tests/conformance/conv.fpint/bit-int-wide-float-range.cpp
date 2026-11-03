// [conv.fpint]/1, [expr.const]/5.8: a floating value whose truncation is
// outside the range of a `_BitInt(N)` with N > 128 (a negative value for an
// unsigned type, a NaN) is undefined, so not a core constant expression;
// the note names the destination type as written, with its `aka` (also for
// a narrow typedef), at the operand's begin. clang is the oracle; x86-64
// only (aarch64's `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:13 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 30:20 note "value 3.402823669209385E+38 is outside the range of representable values of type 'X' (aka '_BitInt(129)')"
//@ diag: 31:13 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 31:20 note "value -1 is outside the range of representable values of type 'U' (aka 'unsigned _BitInt(200)')"
//@ diag: 32:13 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 32:20 note "value 1.60693804425899E+60 is outside the range of representable values of type 'U' (aka 'unsigned _BitInt(200)')"
//@ diag: 34:13 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 34:20 note "value NaN is outside the range of representable values of type 'X' (aka '_BitInt(129)')"
//@ diag: 40:13 error "constexpr variable 'g' must be initialized by a constant expression"
//@ diag: 37:5 note "value 2.0E+300 is outside the range of representable values of type 'X' (aka '_BitInt(129)')"
//@ diag: 40:17 note "in call to 'f()'"
//@ diag: 42:13 error "constexpr variable 'h' must be initialized by a constant expression"
//@ diag: 42:19 note "value 1.0E+20 is outside the range of representable values of type 'I' (aka 'int')"
//@ diag: 43:24 error "constexpr variable 'k' must be initialized by a constant expression"
//@ diag: 43:42 note "value NaN is outside the range of representable values of type '_BitInt(129)'"
using X = _BitInt(129);
using U = unsigned _BitInt(200);
using I = int;
constexpr X ok = (X)-0x1p128; // the minimum
constexpr U ok2 = (U)-0.5;    // truncates to 0
constexpr X a = (X)0x1p128;
constexpr U b = (U)-1.0;
constexpr U c = (U)0x1p200;
// clang: a NaN, at the operand's begin
constexpr X e = (X)__builtin_nan("");
constexpr X f() {
  X w = 2;
  w *= 1e300;
  return w;
}
constexpr X g = f();
// A narrow typedef also keeps its `aka`.
constexpr I h = I(1e20);
constexpr _BitInt(129) k = (_BitInt(129))__builtin_nan("");
