// Variadic calls and definitions (va_list on AArch64).
int printf(const char *, ...);
int total(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  int t = __builtin_va_arg(ap, int);
  __builtin_va_end(ap);
  return t;
}
void hello(void) { printf("%d %f\n", total(1, 2), 1.5); }
