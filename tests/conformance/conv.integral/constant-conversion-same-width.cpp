// [conv.integral]/3: -Wconstant-conversion for a non-negative signed
// constant of exactly the target's width that turns negative (clang
// `isSameWidthConstantConversion`) is a plain diagnostic, not a
// runtime-behavior one: also in a constant-expression (a case value, a
// template argument, an enumerator, the initializer of a namespace-scope
// constexpr variable, an immediate invocation's argument) and in
// unreachable code. A value wider than the target stays runtime-only. In a
// converted constant expression the warning comes before the narrowing
// error, both at the value's first token. clang is the oracle.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:27 warning "implicit conversion from 'int' to 'const signed char' changes value from 128 to -128"
//@ diag: 26:3 warning "implicit conversion from 'int' to 'signed char' changes value from 128 to -128"
//@ diag: 26:3 error "non-type template argument evaluates to 128, which cannot be narrowed to type 'signed char'"
//@ diag: 27:26 warning "implicit conversion from 'int' to 'signed char' changes value from 128 to -128"
//@ diag: 27:26 error "enumerator value evaluates to 128, which cannot be narrowed to type 'signed char'"
//@ diag: 28:27 warning "implicit conversion from 'int' to 'const _BitInt(7)' changes value from 64 to -64"
//@ diag: 30:11 warning "implicit conversion from 'int' to 'signed char' changes value from 128 to -128"
//@ diag: 33:8 warning "implicit conversion from 'int' to '_BitInt(7)' changes value from 64 to -64"
//@ diag: 33:8 error "case value evaluates to 64, which cannot be narrowed to type '_BitInt(7)'"
//@ diag: 34:8 error "case value evaluates to 65, which cannot be narrowed to type '_BitInt(7)'"
//@ diag: 41:19 warning "implicit conversion from 'int' to 'signed char' changes value from 128 to -128"
constexpr signed char c = 128;
constexpr signed char wide = 300;
template <signed char> struct X {};
X<128> x;
enum : signed char { Q = 128 };
constexpr _BitInt(7) b7 = 64;
consteval int g(signed char v) { return v; }
int h = g(128);
int f(_BitInt(7) s7) {
  switch (s7) {
  case 64: return 1;
  case 0x41: return 2;
  case 3: return 3;
  }
  return 0;
}
void u() {
  return;
  signed char e = 128;
}
