// clang 22's [[clang::ext_vector_type(N)]] (WP-809; clang 19 knows only the
// GNU spelling): typedefs, aliases, a dependent alias, and after a declarator
// (applied to the declared type, which clang 22 allows for this attribute).
//@ expect: run
//@ requires: clang>=22
//@ strict-diags
//@ stdout: 16 5 5 5 5
//@ stdout: 8 12
//@ stdout: 16 3 16 4
//@ stdout: 16 0.75
#include <cstdio>

typedef float [[clang::ext_vector_type(4)]] f4;
using i2 = int [[clang::ext_vector_type(2)]];
// After a declarator clang 22 applies it to the declared type silently.
typedef float f3 [[clang::ext_vector_type(3)]];
float g4 [[clang::ext_vector_type(4)]] = {1, 2, 3, 4};

template <class T> using vec2 = T [[clang::ext_vector_type(2)]];

int main() {
  f4 a = {1, 2, 3, 4};
  f4 b = a.wzyx + a;
  std::printf("%zu %g %g %g %g\n", sizeof(f4), b.x, b.y, b.z, b.w);
  i2 c = {3, 4};
  std::printf("%zu %d\n", sizeof(c), c.x * c.y);
  f3 d = {1, 2, 3};
  std::printf("%zu %g %zu %g\n", sizeof(d), d.z, sizeof(g4), g4.w);
  vec2<double> e = {0.5, 0.25};
  std::printf("%zu %g\n", sizeof(e), e.x + e.y);
}
