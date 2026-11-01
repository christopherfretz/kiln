//@ expect: run
//@ strict-diags
//@ args: -std=gnu++26
//@ stdout: 45 40 10
//@ stdout: 6 24 288
//@ stdout: 12345
// WP-183: variable length arrays at run time. In a GNU dialect the
// -Wvla-cxx-extension warning is off by default (clang's
// `ext_vla_cxx_in_gnu_mode`). The stack is saved before a scope's first
// variable length array and restored when the scope is left, so a loop
// allocating one per iteration does not grow the stack.
extern "C" int printf(const char*, ...);

int sum(int n) {
  int a[n];
  for (int i = 0; i < n; ++i) a[i] = i;
  int s = 0;
  for (int i = 0; i < n; ++i) s += a[i];
  return s;
}

int main(int argc, char**) {
  int n = 9 + argc;
  printf("%d %d %d\n", sum(n), (int)sizeof(int[n]), n);
  int m[argc + 1][3];
  int k[2][argc + 2];
  printf("%d %d %d\n", (int)(sizeof(m) / sizeof(int)), (int)sizeof(m),
         (int)sizeof(k[1]) * (int)sizeof(k));
  long total = 0;
  for (int i = 0; i < 1000000; ++i) {
    char buf[4096 + (i & 1)];
    buf[i % 4096] = (char)i;
    total += buf[i % 4096] & 1;
  }
  int r = 0;
  for (int d = 1; d <= 5; ++d) {
    int digits[d];
    digits[d - 1] = d;
    r = r * 10 + digits[d - 1];
  }
  printf("%d\n", r + (int)(total - 500000));
  return 0;
}
