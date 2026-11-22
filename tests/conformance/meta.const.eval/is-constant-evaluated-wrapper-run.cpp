// [meta.const.eval], [expr.const]/22: std::is_constant_evaluated() (and
// the library's own wrapper, reached through char_traits::length) is true
// in a manifestly constant-evaluated context and false outside one. The
// Oven reads the context for a library wrapper without a call
// (spec/oven.md "Library fast paths"): a static_assert, a constexpr
// variable, the constant initialization of a namespace-scope variable and
// of a const integral variable are constant; a local's initializer is not.
//@ expect: run
//@ stdout: 1 1 3 2 5
#include <cstdio>
#include <string>
#include <type_traits>

constexpr int ctx() { return std::is_constant_evaluated() ? 1 : 2; }

constexpr int len(const char *s) {
  return static_cast<int>(std::char_traits<char>::length(s)) + ctx();
}

static_assert(ctx() == 1);
static_assert(len("abcd") == 5);
constexpr int k = ctx();
int g = ctx();
const int c = len("ab");

int main(int argc, char **) {
  int r = ctx();
  int v = len(argc > 5 ? "x" : "abc");
  std::printf("%d %d %d %d %d\n", k, g, c, r, v);
}
