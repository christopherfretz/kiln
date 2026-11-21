// clang's `#pragma clang fp eval_method(double|extended|source)` (C23
// 5.2.4.2.2 FLT_EVAL_METHOD 1 and 2): in its scope every operand of a
// floating type ranked below the method's type is promoted by the usual
// unary conversions (arithmetic, unary `-`/`+`, comparisons, a compound
// assignment's right side), and the result is converted back where it is
// stored or returned; `++` and a conditional of one type are not
// promoted. `extended` is `long double` (here fp128). Constant evaluation
// follows the promoted types, so `a * a / a` that overflows float does not
// under `double`. Compared with clang 22's IR exactly (WP-1744).
//@ expect: ir
//@ requires: clang>=22
//@ args: --target=aarch64-unknown-linux-gnu -std=c++26
float f1(float a, float b, float c) {
#pragma clang fp eval_method(double)
  return a * b + c;
}
float f2(float a, float b) {
#pragma clang fp eval_method(extended)
  return a / b;
}
double f3(double a, double b) {
#pragma clang fp eval_method(extended)
  return a - b;
}
bool f4(float a, float b) {
#pragma clang fp eval_method(double)
  return a < b;
}
float f5(float a, float b) {
#pragma clang fp eval_method(double)
  a += b; return -a;
}
_Float16 f6(_Float16 a, _Float16 b) {
#pragma clang fp eval_method(double)
  return a + b;
}
_Complex float f7(_Complex float a, _Complex float b) {
#pragma clang fp eval_method(double)
  return a * b;
}
float f8(float a) {
#pragma clang fp eval_method(double)
  return a++;
}
float f9(float a, float b, bool c) {
#pragma clang fp eval_method(double)
  return c ? a : b;
}
float f10(float a, int b) {
#pragma clang fp eval_method(double)
  return a * b;
}
float f11(float a, float b) {
#pragma clang fp eval_method(source)
  return a * b;
}
#pragma clang fp eval_method(double)
float f12(float a, float b) { return a * b; }
extern const float k1 = [] { float a = 0x1p100f; return a * a / a; }();
extern const float k2 = [] { float x = 0x1.000002p0f; return x * x - 1.0f; }();
extern const float k3 = 16777216.0f + 1.0f;
template <class T> T tm(T a, T b) { return a * b; }
#pragma clang fp eval_method(source)
float f13(float a, float b) { return tm(a, b); }
extern const float k4 = 16777216.0f + 1.0f;
__bf16 f14(__bf16 a, __bf16 b) {
#pragma clang fp eval_method(extended)
  return a * b;
}
extern const float k5 = [] { float a = 0x1p100f; return a * a / a; }();
