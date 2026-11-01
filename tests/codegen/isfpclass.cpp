// __builtin_isfpclass lowers to llvm.is.fpclass with the folded mask;
// __builtin_expect_with_probability at -O0 is its first operand (clang
// EmitBuiltinExpr).
int f(double d, float x, long v) {
  int r = __builtin_isfpclass(d, 0x3) + __builtin_isfpclass(x, 0x264);
  if (__builtin_expect_with_probability(v, 1, 0.9)) r++;
  return r;
}
