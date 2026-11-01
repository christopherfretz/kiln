// [cstdarg.syn]: `__builtin_va_arg(ap, T)` is a VAArgExpr whose type is the
// written type (a prvalue); the `va_list` operand is an lvalue (a
// parameter of `va_list` type on AAPCS64 is the struct itself).
//@ expect: ast
//@ ast-filter: vaget
struct P { int a, b; };
int vaget(__builtin_va_list ap, int n, ...) {
  __builtin_va_list aq;
  __builtin_va_start(aq, n);
  P p = __builtin_va_arg(aq, P);
  double d = __builtin_va_arg(ap, double);
  __builtin_va_end(aq);
  return p.a + (int)d;
}
