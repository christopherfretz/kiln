// [expr.pre]/6: under -ffast-math a '#pragma clang fp reassociate(off)'
// or 'float_control(precise, on)' scope drops the flags it covers and
// makes unsafe-fp-math (and no-signed-zeros etc.) "false" on the
// function (clang mergeFnAttrValue; spec/codegen.md "Floating-point
// semantics", WP-806).
//@ args: -ffast-math
extern "C" double sqrt(double);
double noreassoc(double a, double b) {
#pragma clang fp reassociate(off)
  return a + b + a * b;
}
double precise(double a, double b) {
#pragma float_control(precise, on)
  return sqrt(a) + a * b + b;
}
double fast(double a, double b) { return sqrt(a) + a * b + b; }
