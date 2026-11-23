// [support.srcloc.obs], [lex.string]/9, [intro.object]/9: kiln makes one
// source-location string object per (content, role, declaration the use
// is evaluated in) (spec/ast.md §5.8.1), and codegen still emits one
// global per content (`private unnamed_addr`, as clang 19 and 22 do), so
// at run time the "f" of f(int) and of f(long), and two callers' names
// made through a default argument, have one address.
//@ expect: run
//@ stdout: f f 1
//@ stdout: cf cf 1
//@ stdout: h h 1
//@ stdout: std::source_location loc(int) std::source_location loc(long) 0
#include <cstdio>
#include <source_location>

const char *f(int) { return __builtin_FUNCTION(); }
const char *f(long) { return __builtin_FUNCTION(); }
const char *cf(int) {
  constexpr const char *p = __builtin_FUNCTION();
  return p;
}
const char *cf(long) {
  constexpr const char *p = __builtin_FUNCTION();
  return p;
}
const char *g(const char *s = __builtin_FUNCTION()) { return s; }
const char *h() { return g(); }
namespace x {
const char *h() { return g(); }
}
std::source_location loc(int) { return std::source_location::current(); }
std::source_location loc(long) { return std::source_location::current(); }

int main() {
  const char *a = f(1);
  const char *b = f(1L);
  std::printf("%s %s %d\n", a, b, a == b);
  const char *c = cf(1);
  const char *d = cf(1L);
  std::printf("%s %s %d\n", c, d, c == d);
  const char *e = h();
  const char *k = x::h();
  std::printf("%s %s %d\n", e, k, e == k);
  std::source_location l1 = loc(1);
  std::source_location l2 = loc(1L);
  std::printf("%s %s %d\n", l1.function_name(), l2.function_name(),
              l1.function_name() == l2.function_name());
}
