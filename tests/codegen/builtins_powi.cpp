// GNU __builtin_powi{,f,l} lower to `llvm.powi.fT.i32` (clang
// EmitBuiltinExpr); __builtin_copysignf16 to `llvm.copysign.f16` with the
// float operands truncated to half and the result extended back;
// __builtin_verbose_trap to `llvm.trap` (the category and message only
// feed debug info in clang).
double pw(double x, int n) { return __builtin_powi(x, n) + __builtin_powif(2.0f, n) + (double)__builtin_powil(x, 3); }
float cs(float a, float b) { return __builtin_copysignf16(a, b); }
void vt(bool c) {
  if (c)
    __builtin_verbose_trap("cat", "msg");
}
