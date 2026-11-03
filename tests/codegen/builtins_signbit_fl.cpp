// [c.math.fpclass], clang EmitBuiltinExpr / EmitSignBit (WP-419):
// __builtin_signbitf / __builtin_signbitl take float / long double (the
// argument converts) and lower as __builtin_signbit: the operand's bits as
// an integer of its width (i32, i128 for AArch64's fp128, i80 for x86_64's
// x86_fp80), compared less than zero, widened to int. clang 19 and 22
// fold a constant argument (__builtin_signbit included) to 0 or 1.
int a(float x) { return __builtin_signbitf(x); }
int b(long double x) { return __builtin_signbitl(x); }
int c(double x) { return __builtin_signbitf(x) + __builtin_signbitl(x); }
int d() { return __builtin_signbitf(-1.0f); }
int e() { return __builtin_signbitl(2.0L); }
int f() { return __builtin_signbit(-0.0); }
int g() { return __builtin_signbitf(-0.0) + __builtin_signbitl(-1); }
