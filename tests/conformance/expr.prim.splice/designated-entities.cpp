// [expr.prim.splice]/2 P2996R13: a splice-expression designating a
// variable or a static data member is an lvalue of it; a function, a
// static member function, an enumerator and a structured binding name
// themselves as an id-expression would. Run through codegen.
//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 3 5 8 13 13 21 34 55 89 10
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
namespace N {
int x = 3;
int add(int a, int b) { return a + b; }
}  // namespace N
struct S {
  static int s;
  static int twice(int a) { return 2 * a; }
};
int S::s = 4;
enum E { Thirteen = 13 };
enum class C { TwentyOne = 21 };
constexpr std::meta::info rx = ^^N::x;
constexpr int k = 34;

int main() {
  int x = [: rx :];
  [: rx :] = 5;  // an lvalue
  int five = N::x;
  int eight = [: ^^N::add :](x, five);
  int s = [: ^^S::s :] * 2 + 5;
  int thirteen = [: ^^Thirteen :];
  int tw = static_cast<int>([: ^^C::TwentyOne :]);
  constexpr int ck = [: ^^k :];  // a constant splice is a constant
  struct P { int a, b; };
  auto [a, b] = P{55, 89};
  int (*fp)(int) = &[: ^^S::twice :];
  printf("%d %d %d %d %d %d %d %d %d %d\n", x, five, eight, s, thirteen, tw, ck, [: ^^a :],
         [: ^^b :], fp(five));
  return 0;
}
