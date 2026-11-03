// [namespace.udir]/1 P2996R13: `using namespace [: r :];` nominates the
// namespace r designates (a namespace alias designates its target), and a
// splice-scope-specifier may lead the nested-name-specifier of the
// namespace-name. Run through codegen.
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
}  // namespace N
namespace O { int z = 5; }
namespace NA = N;
constexpr std::meta::info ro = ^^O;

int f1() { using namespace [: ^^N :]; return x; }
int f2() { using namespace [: ^^NA :]::M; return y; }
using namespace [: ro :];

int main() {
  printf("%d %d %d\n", f1(), f2(), z);
  return 0;
}
