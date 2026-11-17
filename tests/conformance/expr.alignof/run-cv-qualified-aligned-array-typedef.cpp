// [expr.alignof] with GNU `__attribute__((aligned))` on a typedef of an
// array type: `alignof(const T)` is the typedef's alignment, as
// `alignof(T)` is (cv-qualifiers do not change alignment; the
// qualifiers of an array type are its elements', [basic.type.qualifier]/3,
// and the qualified type keeps the typedef as sugar). kiln's constant
// evaluator reads the typedef's alignment only when a typedef is in the
// sugar chain it walks, and the qualified array node desugars straight to
// its canonical type, skipping the typedef: `alignof(const T)` is the
// element alignment (8). The layout of a class or variable of type
// `const T` is right. Found running GCC's C++ testsuite (at C++98 the
// value is read at run time, later the same value fails a
// static_assert).
//@ std: c++20
//@ expect: run
//@ stdout: 32 32 32 32 32 32 32
//@ xfail(kiln): WP-1567 follow-up, alignof of a cv-qualified aligned array typedef drops the typedef (kiln_oven leaf.rs typedef_attributed, compile.rs typedef_align)
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
