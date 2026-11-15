// offsetof with an array index that is not a constant (GNU extension;
// clang and GCC accept it; found building an open-source RPC library,
// whose C runtime sizes structs with a flexible array member this
// way, C17 7.19p3, 6.7.2.1p18): member offsets plus each index times its
// element size, in size_t. Nested and multi-dimensional designators, an
// index with side effects (evaluated once), narrow, wide, signed and
// negative indices; the values equal hand-computed offsets.
//@ expect: run
//@ args: -std=gnu23
//@ stdout: 40 48 124 8 12804 256 272 -60
//@ stdout: 8 1 72 8 -16 1
#include <stddef.h>
#include <stdio.h>

struct in { char c; int x[3][5]; };               /* 64 bytes */
struct out { int n; struct in a[4]; long data[]; }; /* a at 4, data at 264 */
struct flex { int n; long data[]; };              /* data at 8 */

size_t bytes(size_t count) { return offsetof(struct flex, data[count]); }
size_t nested(int i, int j, int k) { return offsetof(struct out, a[i].x[j][k]); }
size_t narrow(unsigned char c) { return offsetof(struct out, a[c]); }
size_t wide(__int128 w) { return offsetof(struct out, data[w]); }
size_t sgn(short s) { return offsetof(struct out, a[s]); }

int main(void) {
  printf("%zu %zu %zu %zu %zu %zu %zu %lld\n", bytes(4), bytes(5), nested(1, 2, 3),
         nested(0, 0, 0), narrow(200), wide(-1), wide((__int128)1 << 64 | 1), (long long)sgn(-1));
  int n = 0;
  size_t once = offsetof(struct flex, data[n++]);
  size_t twice = offsetof(struct flex, data[(n += 7)]);
  int vla[offsetof(struct flex, data[n]) / 8];
  printf("%zu %d %zu %d %lld %d\n", once, n == 8, twice, n,
         (long long)offsetof(struct flex, data[-3]), (int)(sizeof vla == 9 * sizeof(int)));
  return 0;
}
