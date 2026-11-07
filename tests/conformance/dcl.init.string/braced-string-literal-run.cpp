// [dcl.init.string]/1, [dcl.init.list]/3.3, [dcl.init.aggr]/16: an array
// of character type may be initialized by a string literal enclosed in
// braces, at the top level (`char s[] {"..."}`, `= {"..."}`, an unknown
// bound completed from the literal) and as a braced member or element
// initializer (`S{{"ab"}}`, `{{"ab"}, {"cd"}}`). kiln initialized the
// first element from the literal ("cannot initialize an array element").
// nlohmann/json unit-constructor1.cpp `const char s[] {"Hello world"}`
// (WP-668). Verified against clang 19.
//@ expect: run
//@ stdout: Hello world|ab|cd|xyz
#include <cstdio>
#include <cstring>

struct S { char m[4]; int n; };
const char g[] {"xyz"};
static_assert(sizeof(g) == 4);
constexpr char k[] = {"abc"};
static_assert(sizeof(k) == 4 && k[2] == 'c' && k[3] == 0);

int main() {
  const char s[] {"Hello world"};
  char t[] = {"ab"};
  char u[6] {"cd"};
  wchar_t w[] {L"wide"};
  char16_t c16[] = {u"x"};
  S a{{"ab"}, 1};
  char grid[2][3] = {{"ab"}, {"cd"}};
  static_assert(sizeof(s) == 12 && sizeof(t) == 3 && sizeof(w) == 5 * sizeof(wchar_t));
  static_assert(sizeof(c16) == 2 * sizeof(char16_t));
  if (u[2] != 0 || u[5] != 0 || a.n != 1 || std::strcmp(a.m, "ab") != 0) return 1;
  std::printf("%s|%s|%s|%s\n", s, grid[0], grid[1], g);
  return std::strcmp(t, "ab") == 0 && w[3] == L'e' && c16[0] == u'x' ? 0 : 2;
}
