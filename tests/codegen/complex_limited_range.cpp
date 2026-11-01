// -fcx-limited-range: complex `*` and `/` use the algebraic formulas
// (clang `EmitBinMul` without the NaN check, `EmitAlgebraicDiv`);
// spec/abi.md §8.17, C23 G.5.1 / [complex.numbers].
//@ args: -fcx-limited-range
_Complex float mulf(_Complex float a, _Complex float b) { return a * b; }
_Complex float divf(_Complex float a, _Complex float b) { return a / b; }
_Complex double muld(_Complex double a, _Complex double b) { return a * b; }
_Complex double divd(_Complex double a, _Complex double b) { return a / b; }
_Complex long double divl(_Complex long double a, _Complex long double b) {
  return a / b;
}
_Complex double div_real_lhs(double a, _Complex double b) { return a / b; }
_Complex double div_real_rhs(_Complex double a, double b) { return a / b; }
_Complex float mul_real(_Complex float a, float b) { return a * b; }
void compound(_Complex double &a, _Complex double b) {
  a *= b;
  a /= b;
}
_Complex int idiv(_Complex int a, _Complex int b) { return a / b; }
