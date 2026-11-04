//@ args: -O1 -Xclang -disable-llvm-passes
// GNU `gnu_inline` (GNU extension; [dcl.inline]/6, [dcl.asm] asm labels;
// spec/codegen.md "GNU inline functions"): in C++ the definition of an
// `inline` function with `gnu_inline` is never externally visible (clang
// `isInlineDefinitionExternallyVisible`), so with optimization it is
// emitted `available_externally` -- function templates, members and
// `constexpr` functions included -- and not at all when it is `noinline`
// or trivially recursive: an asm-labelled overload whose body calls the
// `__builtin_` form of the labelled library function (the C library's
// optimized-only C++ `strchr` overload) is only declared, as is an
// unlabelled `extern "C"` one (named by its plain name). A function with
// internal linkage is defined as usual.
extern "C++" const char *find_byte(const char *s, int c) __asm("strchr");
extern __inline __attribute__((__always_inline__, __gnu_inline__)) const char *
find_byte(const char *s, int c) {
  return __builtin_strchr(s, c);
}
extern "C++" unsigned long text_length(const char *s) __asm("strlen");
extern inline __attribute__((gnu_inline)) unsigned long text_length(const char *s) {
  return __builtin_strlen(s);
}
extern "C" {
extern inline __attribute__((gnu_inline)) char *strrchr(const char *s, int c) {
  return __builtin_strrchr(s, c);
}
}
extern inline __attribute__((gnu_inline)) int twice(int x) { return x * 2; }
extern inline __attribute__((gnu_inline, always_inline)) int thrice(int x) { return x * 3; }
extern inline __attribute__((gnu_inline, noinline)) int quad(int x) { return x * 4; }
template <class T> extern inline __attribute__((gnu_inline)) T scale(T x) { return x * 5; }
extern inline __attribute__((gnu_inline)) constexpr int six(int x) { return x * 6; }
struct Box {
  int v;
  static int make(int x);
  inline __attribute__((gnu_inline)) int get() const;
};
inline __attribute__((gnu_inline)) int Box::make(int x) { return x + 7; }
inline int Box::get() const { return v; }
static inline __attribute__((gnu_inline)) int local(int x) { return x * 8; }
namespace {
extern inline __attribute__((gnu_inline)) int hidden(int x) { return x * 9; }
}
int use(const char *p, Box b) {
  return int(find_byte(p, ' ') - p) + int(text_length(p)) + int(strrchr(p, 'x') - p) + twice(1) +
         thrice(2) + quad(3) + scale(4) + int(scale(4.5)) + six(5) +
         Box::make(b.get()) + local(6) + hidden(7);
}
