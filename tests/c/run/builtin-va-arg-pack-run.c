// GCC's __builtin_va_arg_pack() / __builtin_va_arg_pack_len() in an
// always-inlined `gnu_inline` variadic function: the call's anonymous
// arguments are forwarded, and their count is a constant of the inlined
// body. Not implemented (builtin-va-arg-pack-unknown.c); GCC 16.1 prints
// the expected output. WP-1550.
//@ std: c17
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 19/22 do not implement __builtin_va_arg_pack
//@ xfail(kiln): WP-1587 remainder: __builtin_va_arg_pack and __builtin_va_arg_pack_len (expansion at the always-inline call site)
//@ stdout: 7 x 2
//@ stdout: 0
int printf(const char *, ...);
extern inline __attribute__((always_inline, gnu_inline)) int fwd(const char *f, ...) {
  return printf(f, __builtin_va_arg_pack());
}
extern inline __attribute__((always_inline, gnu_inline)) int count(int a, ...) {
  return __builtin_va_arg_pack_len();
}
int main(void) {
  fwd("%d %s %d\n", 7, "x", count(1, 2.0, "y"));
  printf("%d\n", count(1));
  return 0;
}
