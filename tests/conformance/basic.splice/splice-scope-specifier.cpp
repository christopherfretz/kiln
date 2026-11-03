// [expr.prim.id.qual]/1-2, [basic.lookup.qual.general]/1 P2996R13: a
// splice-scope-specifier `[: r :]::` nominates the namespace, namespace
// alias, class or enumeration r designates, for qualified lookup in an
// expression, a nested name, an elaborated-type-specifier, a
// typename-specifier, a bare type-only name and a pointer-to-member
// declarator. `int v1 = [:^^TCls<1>:]::s;` is from [expr.prim.id.qual]
// Example 1. Run through codegen.
//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 1 2 3 4 5 6 7 8 9 10
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
namespace N {
int x = 1;
namespace M { constexpr int k = 2; }
}  // namespace N
namespace NA = N;
struct S {
  static int s;
  struct In { int q; static constexpr int v = 5; };
  enum E { Six = 6 };
  int m;
};
int S::s = 4;
enum class Col { Seven = 7 };
template <int> struct TCls { static constexpr int s = 8; };
int v1 = [:^^TCls<1>:]::s;

struct [: ^^S :]::In* p = nullptr;   // elaborated-type-specifier
typename [: ^^S :]::In in{9};        // typename-specifier
[: ^^S :]::In in2{10};               // type-only context
int [: ^^S :]::* pm = &S::m;         // ptr-operator

int main() {
  int a = [: ^^N :]::x;
  int b = [: ^^N :]::M::k;
  int c = [: ^^NA :]::M::k + 1;
  int d = [: ^^S :]::s;
  int e = [: ^^S::In :]::v;
  int f = [: ^^S :]::Six;
  int g = static_cast<int>([: ^^Col :]::Seven);
  S s{};
  s.*pm = 0;
  printf("%d %d %d %d %d %d %d %d %d %d\n", a, b, c, d, e, f, g, v1, in.q + s.m,
         in2.q + (p == nullptr ? 0 : 1));
  return 0;
}
