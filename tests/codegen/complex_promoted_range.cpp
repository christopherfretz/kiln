// WP-1795: -fcomplex-arithmetic=promoted (clang 22; clang 19 at
// -fclang-compat=19 promotes every quotient, never a compound
// assignment, and warns -Whigher-precision-fp instead). A quotient by
// a complex divisor runs algebraically in the next wider type (`float`
// in `double`, `double` in `long double`) and is narrowed once; `long
// double` has no wider type and keeps Smith's algorithm (with Sema's
// -Whigher-precision-for-complex-division); a real divisor, `*`, `+`
// and `-` are not promoted and `*` skips the NaN check (spec/codegen.md
// "Promoted complex arithmetic"). An operand of a promoted quotient is
// emitted in a new emitter whose own quotients take Smith's algorithm in
// the wider type (clang's `FPHasBeenPromoted`).
//@ args: -fcomplex-arithmetic=promoted -Wno-higher-precision-for-complex-division
_Complex float divf(_Complex float a, _Complex float b) { return a / b; }
_Complex double divd(_Complex double a, _Complex double b) { return a / b; }
_Complex long double divl(_Complex long double a, _Complex long double b) {
  return a / b;
}
_Complex float mulf(_Complex float a, _Complex float b) { return a * b; }
_Complex double muld(_Complex double a, _Complex double b) { return a * b; }
_Complex float addf(_Complex float a, _Complex float b) { return a + b - a; }
_Complex float div_real_lhs(float a, _Complex float b) { return a / b; }
_Complex float div_real_rhs(_Complex float a, float b) { return a / b; }
_Complex double div_real_sum(_Complex double a, double b, double c) {
  return a / (b + c);
}
void compound(_Complex float &a, _Complex float b, float c) {
  a /= b;
  a *= b;
  a /= c;
}
void compound_mixed(_Complex double &a, _Complex float b) { a /= b; }
void compound_ld(_Complex long double &a, _Complex long double b) { a /= b; }
_Complex float chain(_Complex float a, _Complex float b, _Complex float c) {
  return a / b / c;
}
_Complex float nested(_Complex float a, _Complex float b, _Complex float c) {
  return (a * b) / (b + c) / -(a - c);
}
_Complex double ld_then_d(_Complex long double a, _Complex long double b,
                          _Complex double c, _Complex double d) {
  return (_Complex double)(a / b) + c / d;
}
_Complex float paren(_Complex float a, _Complex float b) { return (a) / (+b); }
_Complex double pragma_on(_Complex double a, _Complex double b) {
#pragma STDC CX_LIMITED_RANGE ON
  return a / b;
}
_Complex double pragma_off(_Complex double a, _Complex double b) {
#pragma STDC CX_LIMITED_RANGE OFF
  return a / b;
}
_Complex double pragma_default(_Complex double a, _Complex double b) {
#pragma STDC CX_LIMITED_RANGE DEFAULT
  return a / b;
}
_Complex int idiv(_Complex int a, _Complex int b) { return a / b; }
