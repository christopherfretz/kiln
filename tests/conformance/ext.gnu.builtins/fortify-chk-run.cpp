// The `_FORTIFY_SOURCE` checking builtins `__builtin___*_chk` (WP-803):
// each calls the C library's `__*_chk` function, except a `memcpy` /
// `memmove` / `memset` one with constant size <= constant object size,
// which is the unchecked operation. The printf ones carry clang's
// implicit format attribute. Verified against clang 19.
//@ expect: run
//@ stdout: abcd|cdef|xxxxy|hello|3 n=7|w 3 2.5|v 9|1 1|ok
extern "C" int printf(const char *, ...);
typedef __SIZE_TYPE__ size_t;
static int vs(char *d, size_t n, const char *f, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, f);
  int r = __builtin___vsnprintf_chk(d, n, 1, __builtin_object_size(d, 1), f, ap);
  __builtin_va_end(ap);
  return r;
}
int main(int argc, char **) {
  char a[16] = "abcdef", b[16];
  __builtin___memcpy_chk(b, a, 4, sizeof b);  // folded: constant 4 <= 16
  b[4] = 0;
  printf("%s|", b);
  size_t n = 4 + argc - 1;  // not a constant: __memmove_chk
  __builtin___memmove_chk(b, a + 2, n, sizeof b);
  printf("%s|", b);
  __builtin___memset_chk(b, 'x', 4, sizeof b);
  __builtin___memset_chk(b + 4, 'y', 1, __builtin_object_size(b + 4, 0));
  b[5] = 0;
  printf("%s|", b);
  __builtin___strcpy_chk(b, "hel", sizeof b);
  __builtin___strcat_chk(b, "lo", sizeof b);
  printf("%s|", b);
  int k = __builtin___sprintf_chk(b, 1, sizeof b, "n=%d", 7);
  printf("%d %s|", k, b);
  __builtin___snprintf_chk(b, sizeof b, 1, sizeof b, "w %d %.1f", 3, 2.5);
  printf("%s|", b);
  vs(b, sizeof b, "v %d", 9);
  printf("%s|", b);
  char *e = __builtin___stpcpy_chk(b, "ab", sizeof b);
  printf("%d %d|", int(e - b) == 2, __builtin___strncpy_chk(b, "z", 2, sizeof b) == b);
  __builtin___printf_chk(1, "%s\n", "ok");
}
