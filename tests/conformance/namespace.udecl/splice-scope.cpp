// [namespace.udecl]/1, [expr.prim.id.qual]/2 P2996R13: the
// nested-name-specifier of a using-declarator may start with a
// splice-scope-specifier, at block scope (a namespace member) and as a
// member-declaration (a base class member). Run through codegen.
//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 3 4 5
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
namespace N {
int x = 3;
namespace M { int y = 4; }
struct C { static constexpr int z = 5; };
}  // namespace N

struct D : N::C {
  using [: ^^N::C :]::z;
};

int main() {
  using [: ^^N :]::x;
  using [: ^^N :]::M::y;
  printf("%d %d %d\n", x, y, D::z);
  return 0;
}
