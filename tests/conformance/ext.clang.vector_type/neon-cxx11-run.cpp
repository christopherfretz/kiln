// [[clang::neon_vector_type(N)]] / [[clang::neon_polyvector_type(N)]] in type
// position make the GNU spelling's NEON vectors (WP-809): typedefs, aliases, a
// lambda parameter and a local variable, passed by value and subscripted.
//@ expect: run
//@ requires: target-aarch64
//@ strict-diags
//@ stdout: 16 2 4 6 8
//@ stdout: 8 1 8
//@ stdout: 8 4 16 42
//@ stdout: 42
#include <cstdio>

typedef int [[clang::neon_vector_type(4)]] i4;
using u8x8 = unsigned char [[clang::neon_polyvector_type(8)]];
typedef float [[clang::neon_vector_type(2)]] f2;
using l2 = long long [[clang::neon_vector_type(2)]];

static i4 twice(i4 v) { return v + v; }

int main() {
  i4 a = {1, 2, 3, 4};
  i4 b = twice(a);
  std::printf("%zu %d %d %d %d\n", sizeof(i4), b[0], b[1], b[2], b[3]);
  u8x8 p = {1, 2, 3, 4, 5, 6, 7, 8};
  std::printf("%zu %d %d\n", sizeof(p), p[0], p[7]);
  f2 f = {1.5f, 2.5f};
  l2 l = {40, 2};
  std::printf("%zu %g %zu %lld\n", sizeof(f), f[0] + f[1], sizeof(l), l[0] + l[1]);
  auto g = [](int [[clang::neon_vector_type(2)]] x) { return x[0] * x[1]; };
  int [[clang::neon_vector_type(2)]] x = {6, 7};
  std::printf("%d\n", g(x));
}
