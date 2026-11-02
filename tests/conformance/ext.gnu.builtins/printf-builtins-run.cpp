// The printf family of library builtins (__builtin_snprintf,
// __builtin_sprintf, __builtin_vsnprintf; [library.c]) is a call of the C
// library function of the same name. Verified against clang 19.
//@ expect: run
//@ stdout: 6 [42 2.5] 3 [abc] 5 [x=-7y]
extern "C" int printf(const char *, ...);
int vfmt(char *buf, __SIZE_TYPE__ n, const char *f, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, f);
  int r = __builtin_vsnprintf(buf, n, f, ap);
  __builtin_va_end(ap);
  return r;
}
int main() {
  char a[32], b[4], c[32];
  int n1 = __builtin_snprintf(a, sizeof a, "%d %.1f", 42, 2.5);
  int n2 = vfmt(b, sizeof b, "%s", "abc");
  int n3 = __builtin_sprintf(c, "x=%dy", -7);
  printf("%d [%s] %d [%s] %d [%s]\n", n1, a, n2, b, n3, c);
}
