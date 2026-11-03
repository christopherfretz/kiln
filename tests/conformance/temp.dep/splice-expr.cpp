// [temp.dep.expr], [temp.dep.constexpr] P2996R13: a splice-expression
// whose operand is value-dependent is type- and value-dependent; it is
// resolved when the template is instantiated: a variable, a function and
// an enumerator through a template parameter of type info, and a
// pointer to member through `&`. Run through codegen.
//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 3 9 5 2
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
namespace N {
int x = 3;
int sq(int a) { return a * a; }
enum E { Five = 5 };
}  // namespace N
struct S { int m; };

template <std::meta::info R> int value() { return [: R :]; }
template <std::meta::info R, class... A> int call(A... a) { return [: R :](a...); }
template <std::meta::info R> constexpr int k = [: R :];
template <std::meta::info R> auto member_pointer() { return &[: R :]; }

int main() {
  S s{2};
  printf("%d %d %d %d\n", value<^^N::x>(), call<^^N::sq>(3), k<^^N::Five>,
         s.*member_pointer<^^S::m>());
  return 0;
}
