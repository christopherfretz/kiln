// [expr.pre]/6, C23 7.3.4: '#pragma clang fp reassociate/reciprocal',
// '#pragma float_control(precise, on|off)' (with push/pop at file scope)
// and '#pragma STDC CX_LIMITED_RANGE' change the fast-math flags of the
// compound statement they open; a scope that turns a flag off makes the
// function's *-fp-math attribute "false" (clang CGFPOptionsRAII;
// spec/codegen.md "Floating-point semantics", WP-806).
extern "C" {
float sqrtf(float);
float atan2f(float, float);
double pow(double, double);
int printf(const char *, ...);
}
double reassoc(double x) {
#pragma clang fp reassociate(on)
  { double y = pow(x, 2.0) + x + x; printf("%f\n", y); return y; }
}
float recip(float a, float b) {
  float r = a / b;
  {
#pragma clang fp reciprocal(on)
    r += a / b;
  }
  return r;
}
float block(bool c, float a, float b) {
  float t;
  {
#pragma clang fp reassociate(on)
    t = c ? a : b;
  }
  return t + a;
}
_Complex double cmul(_Complex double a, _Complex double b) { return a * b; }
_Complex double cdiv(_Complex double a, _Complex double b) {
#pragma STDC CX_LIMITED_RANGE ON
  return a / b;
}
_Complex float cmulf(_Complex float a, _Complex float b) {
#pragma STDC CX_LIMITED_RANGE OFF
  return a * b;
}
float precise_off(float a, float b) {
#pragma float_control(precise, off)
  return a * b + a / b;
}
#pragma float_control(push)
#pragma float_control(precise, off)
float file_fast(float x) { return sqrtf(x) * 2.0f + atan2f(x, x); }
#pragma float_control(pop)
float file_strict(float x) { return sqrtf(x) * 2.0f; }
