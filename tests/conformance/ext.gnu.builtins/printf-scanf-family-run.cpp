// The rest of clang 19's printf/scanf family of library builtins
// (__builtin_printf, __builtin_vprintf, __builtin_vsprintf,
// __builtin_sscanf, __builtin_vsscanf; [library.c]) is a call of the C
// library function of the same name, declared or not. Verified against
// clang 19.
//@ expect: run
//@ stdout: [x=-7y] 5|n=3 4 2.5 ab|r=2 9 cd|v=9
int vout(const char *f, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, f);
  int r = __builtin_vprintf(f, ap);
  __builtin_va_end(ap);
  return r;
}
int vfmt(char *buf, const char *f, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, f);
  int r = __builtin_vsprintf(buf, f, ap);
  __builtin_va_end(ap);
  return r;
}
int vscan(const char *s, const char *f, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, f);
  int r = __builtin_vsscanf(s, f, ap);
  __builtin_va_end(ap);
  return r;
}
int main() {
  char c[32], w[8], w2[8];
  int n = vfmt(c, "x=%dy", -7);
  __builtin_printf("[%s] %d|", c, n);
  int i = 0, j = 0;
  double d = 0;
  int r = __builtin_sscanf("4 2.5 ab", "%d %lf %7s", &i, &d, w);
  __builtin_printf("n=%d %d %.1f %s|", r, i, d, w);
  int r2 = vscan("9 cd", "%d %7s", &j, w2);
  int v = vout("r=%d %d %s|", r2, j, w2);
  __builtin_printf("v=%d\n", v);
}
