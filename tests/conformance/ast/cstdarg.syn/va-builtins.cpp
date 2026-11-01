// [cstdarg.syn]: calls of va_start / va_copy / va_end: the va_list operand
// binds directly to the `__builtin_va_list &` parameter; va_start's second
// operand is kept as written; a va_list argument is copied.
//@ expect: ast
//@ args: --target=aarch64-unknown-linux-gnu
//@ ast-filter: vafmt
extern "C" int vsnprintf(char *, unsigned long, const char *, __builtin_va_list);
int vafmt(char *buf, const char *f, ...) {
  __builtin_va_list ap, aq;
  __builtin_va_start(ap, f);
  __builtin_va_copy(aq, ap);
  int r = vsnprintf(buf, 64, f, aq);
  __builtin_va_end(aq);
  __builtin_va_end(ap);
  return r;
}
