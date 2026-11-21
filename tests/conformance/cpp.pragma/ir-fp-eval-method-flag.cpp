// clang `-ffp-eval-method=extended` (C23 5.2.4.2.2 FLT_EVAL_METHOD 2): the
// unit's evaluation method, as `#pragma clang fp eval_method(extended)`
// in every scope. An operand ranked below `long double` (x87 here) is
// promoted and the result converted back, `__FLT_EVAL_METHOD__` is 2,
// and no function is `strictfp`. A pragma in a scope overrides it.
// Compared with clang 22's IR exactly (spec/driver.md "Floating-point
// flags", WP-1798).
//@ expect: ir
//@ requires: clang>=22
//@ args: --target=x86_64-unknown-linux-gnu -std=c++26 -ffp-eval-method=extended
float fma3(float a, float b, float c) { return a * b + c; }
double sub(double a, double b) { return a - b; }
bool less(float a, float b) { return a < b; }
float src(float a, float b) {
#pragma clang fp eval_method(source)
  return a * b;
}
constexpr float big = 0x1p100f;
float k1 = big * big / big;
int m = __FLT_EVAL_METHOD__;
static_assert(__FLT_EVAL_METHOD__ == 2);
