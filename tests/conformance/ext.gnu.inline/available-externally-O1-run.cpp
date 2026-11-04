// GNU `gnu_inline` (GNU extension; [dcl.inline]/6, [dcl.asm] asm labels;
// spec/codegen.md "GNU inline functions"): in C++ the definition of an
// `extern inline __attribute__((gnu_inline))` function is not externally
// visible, so it is emitted `available_externally` or not at all, never
// as a definition of the labelled symbol. The shape is the C library's
// optimized-only C++ overload of a string function: an overload labelled
// with the library name whose body calls the `__builtin_` form. Emitting
// it as a definition replaces the library's own function with one that
// calls itself (an infinite loop once LLVM optimizes the tail call).
//@ expect: run
//@ args: -O1
//@ stdout-file: available-externally.stdout
#include <cstdio>

extern "C++" const char *find_byte(const char *s, int c) __asm("strchr");
extern __inline __attribute__((__always_inline__, __gnu_inline__)) const char *
find_byte(const char *s, int c) {
  return __builtin_strchr(s, c);
}

extern "C++" unsigned long text_length(const char *s) __asm("strlen");
extern inline __attribute__((gnu_inline)) unsigned long text_length(const char *s) {
  return __builtin_strlen(s);
}

int main(int argc, char **) {
  char line[] = "key hello world";
  line[0] = char('k' + argc - 1);
  const char *sp = find_byte(line, ' ');
  std::printf("key=%d rest=[%s] len=%lu\n", int(sp - line), sp + 1,
              text_length(line));
}
