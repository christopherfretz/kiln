// [expr.eq]/7, [expr.reflect] (P2996R13): `^^M` for a namespace alias M
// represents the alias, not the namespace it names, so it compares
// unequal to `^^N` and equal only to another reflection of the same alias
// (WP-505); two aliases of one namespace, or an alias of an alias,
// compare unequal too. The comparisons are namespace-scope constexpr
// bools that are emitted and read at runtime. Compared with GCC 16.1.
//@ expect: run
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ stdout: 0110110
#include <cstdio>
namespace N { namespace Inner {} }
namespace M = N;
namespace M2 = N;
namespace MM = M;
namespace I = N::Inner;

constexpr bool eq[] = {
  ^^M == ^^N, ^^M == ^^M, ^^N == ^^N, ^^M2 == ^^M,
  ^^M2 != ^^N, ^^MM != ^^M, ^^I == ^^N::Inner,
};
int main() {
  for (bool b : eq) std::printf("%d", b);
  std::printf("\n");
}
