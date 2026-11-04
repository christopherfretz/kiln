// GNU `gnu_inline` (GNU extension; [dcl.inline]/6, [dcl.asm] asm labels;
// spec/codegen.md "GNU inline functions"): at -O0 the body of a
// `gnu_inline` function is emitted (`available_externally`) only when it
// is `always_inline` and not trivially recursive; every other one, the
// C library's asm-labelled `strchr` overload included, is only declared.
extern "C++" const char *find_byte(const char *s, int c) __asm("strchr");
extern __inline __attribute__((__always_inline__, __gnu_inline__)) const char *
find_byte(const char *s, int c) {
  return __builtin_strchr(s, c);
}
extern inline __attribute__((gnu_inline)) int twice(int x) { return x * 2; }
extern inline __attribute__((gnu_inline, always_inline)) int thrice(int x) { return x * 3; }
template <class T> extern inline __attribute__((gnu_inline)) T scale(T x) { return x * 5; }
int use(const char *p) {
  return int(find_byte(p, ' ') - p) + twice(1) + thrice(2) + scale(4);
}
