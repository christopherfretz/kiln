// __builtin_offsetof with an array index that is not a constant (GNU
// extension, as clang and GCC; found building an open-source RPC
// library): the value is computed at run time as the member offsets plus
// each index times its element size, in size_t. Nested designators,
// multi-dimensional members, a flexible array member, an index with side
// effects (evaluated once), index types narrower and wider than size_t,
// signed and negative ones. In a constexpr function the index is known
// when the call is evaluated ([support.types.layout]/1, [expr.const]).
//@ expect: run
//@ stdout: 40 48 124 8 12804 256 272 -60
//@ stdout: 8 1 72 8 32 96 -16
extern "C" int printf(const char*, ...);
typedef __SIZE_TYPE__ size_t;

struct In { char c; int x[3][5]; };          // 64 bytes
struct Out { int n; In a[4]; long data[]; }; // a at 4, data at 264
struct Flex { int n; long data[]; };         // data at 8

size_t flex(size_t count) { return __builtin_offsetof(Flex, data[count]); }
size_t nested(int i, int j, int k) { return __builtin_offsetof(Out, a[i].x[j][k]); }
size_t narrow(unsigned char c) { return __builtin_offsetof(Out, a[c]); }
size_t wide(__int128 w) { return __builtin_offsetof(Out, data[w]); }
size_t sgn(short s) { return __builtin_offsetof(Out, a[s]); }

constexpr size_t cx(int i) { return __builtin_offsetof(Out, a[i].x[i][1]); }
static_assert(cx(2) == 4 + 2 * 64 + 4 + 2 * 20 + 4);
static_assert(__builtin_offsetof(Out, a[1].x[2][3]) == 4 + 64 + 4 + 40 + 12);

template <class T> size_t tmpl(int i) { return __builtin_offsetof(T, data[i]); }

int main() {
  printf("%zu %zu %zu %zu %zu %zu %zu %lld\n", flex(4), flex(5), nested(1, 2, 3), nested(0, 0, 0),
         narrow(200), wide(-1), wide((__int128)1 << 64 | 1), (long long)sgn(-1));
  int n = 0;
  size_t once = __builtin_offsetof(Flex, data[n++]);
  size_t twice = __builtin_offsetof(Flex, data[(n += 7)]);
  printf("%zu %d %zu %d %zu %zu %lld\n", once, n == 8 ? 1 : 0, twice, n, tmpl<Flex>(3), cx(1),
         (long long)__builtin_offsetof(Flex, data[-3]));
  return 0;
}
