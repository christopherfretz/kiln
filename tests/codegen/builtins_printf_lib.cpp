// The printf family of library builtins ([library.c]; clang
// getBuiltinLibFunction): a call of the C library function.
typedef __builtin_va_list va_list;
int f1(char *b, __SIZE_TYPE__ n, const char *f, va_list ap) {
  return __builtin_vsnprintf(b, n, f, ap);
}
int f2(char *b, __SIZE_TYPE__ n, int x) { return __builtin_snprintf(b, n, "%d", x); }
int f3(char *b, double d) { return __builtin_sprintf(b, "%g", d); }
