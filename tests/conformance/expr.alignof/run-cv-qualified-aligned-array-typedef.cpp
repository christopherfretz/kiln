// [expr.alignof] with GNU `__attribute__((aligned))` on a typedef of an
// array type: `alignof(const T)` is the typedef's alignment, as
// `alignof(T)` is (cv-qualifiers do not change alignment; the
// qualifiers of an array type are its elements', [basic.type.qualifier]/3,
// and the qualified type keeps the typedef as sugar). kiln's constant
// evaluator reads the typedef's alignment only when a typedef is in the
// sugar chain it walks; the qualified array node desugars straight to its
// canonical type, so the walk continues into its base, which keeps the
// typedef (before WP-1567 `alignof(const T)` was the element alignment,
// 8). Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: run
//@ stdout: 32 32 32 32 32 32 32
#include <cstdio>
typedef double T[4][4] __attribute__((aligned(32)));
typedef const T CT;
struct S { T s; };
struct M { char c; const T m; };
const T g = {};
int main() {
  std::printf("%zu %zu %zu %zu %zu %zu %zu\n", alignof(S), alignof(T), alignof(const S),
              alignof(const T), alignof(volatile T), __alignof__(CT), alignof(M));
}
