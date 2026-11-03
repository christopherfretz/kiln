// [dcl.type.splice]/1, [temp.res.general]/4 P2996R13: in a type-only
// context a splice-specifier is a splice-type-specifier without
// `typename`: an alias-declaration, a namespace-scope decl-specifier, a
// member-declaration and a member function's parameter, a trailing return
// type, a template parameter's default, the type-id of a cast and of a
// new-expression. Run through codegen.
//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 1 2 3 4 5 6 7
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
constexpr std::meta::info ri = ^^int;
using T = [: ri :];
[: ri :] gx = 1;
struct S {
  [: ^^int :] m;
  [: ^^int :] f([: ^^int :] a) const { return m + a; }
};
auto h() -> [: ri :] { return 4; }
template <typename U = [: ri :]> struct D { U v = 5; };

int main() {
  S s{1};
  T t = 2;
  int c = static_cast<[: ri :]>(6.5);
  int* np = new [: ri :](7);
  printf("%d %d %d %d %d %d %d\n", gx, t, s.f(2), h(), D<>{}.v, c, *np);
  delete np;
  return 0;
}
