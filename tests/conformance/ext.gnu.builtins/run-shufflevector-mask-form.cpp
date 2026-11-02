// __builtin_shufflevector mask form (clang 19 CGBuiltin EmitShuffleVector):
// result[i] = x[mask[i] & (NextPowerOf2(n - 1) - 1)].
//@ expect: run
//@ stdout: 4 3 2 1 | 1 3 1 2
#include <cstdio>
typedef int v4 __attribute__((vector_size(16)));
typedef unsigned u4 __attribute__((vector_size(16)));
v4 shuf(v4 x, u4 m) { return __builtin_shufflevector(x, m); }
int main() {
  v4 x = {1, 2, 3, 4};
  v4 r = shuf(x, (u4){3, 2, 1, 0});
  v4 s = shuf(x, (u4){4, 6, 8, 13}); // masked to 0, 2, 0, 1
  std::printf("%d %d %d %d | %d %d %d %d\n", r[0], r[1], r[2], r[3], s[0], s[1], s[2], s[3]);
}
