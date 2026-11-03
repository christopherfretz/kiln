// WP-410: [c.math] builtins clang declares for double, float and long
// double (clang 19 and 22, -O0, AArch64 and x86_64 alike): roundeven and
// canonicalize lower to the overloaded llvm.roundeven / llvm.canonicalize
// intrinsic of the operand type (f64, f32, and f128 or f80 for long
// double); exp10 is a library call of exp10 / exp10f / exp10l (clang's
// default -fmath-errno). clang folds none of them, constant operands
// included.
double d(double x) {
  return __builtin_roundeven(x) + __builtin_canonicalize(x) + __builtin_exp10(x);
}
float f(float x) {
  return __builtin_roundevenf(x) + __builtin_canonicalizef(x) + __builtin_exp10f(x);
}
long double l(long double x) {
  return __builtin_roundevenl(x) + __builtin_canonicalizel(x) + __builtin_exp10l(x);
}
double k() {
  return __builtin_roundeven(2.5) + __builtin_canonicalize(1.0) + __builtin_exp10(1.0);
}
