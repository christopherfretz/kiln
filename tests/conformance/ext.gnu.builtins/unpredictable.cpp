//@ expect: run
//@ stdout: 4 1 7
// WP-726: `long __builtin_unpredictable(long)` yields its operand (clang
// `BuiltinUnpredictable`, `NoThrow, Const`); like clang it is not usable in
// a constant expression.
#include <cstdio>

static_assert(__has_builtin(__builtin_unpredictable));

long pick(long a, long b) { return __builtin_unpredictable(a < b) ? a : b; }

int count(const int* p, int n, int t) {
  int c = 0;
  for (int i = 0; i < n; ++i)
    if (__builtin_unpredictable(p[i] > t)) ++c;
  return c;
}

int main() {
  int v[] = {1, 8, 2, 9, 4, 6};
  volatile long x = 7;
  std::printf("%d %ld %ld\n", count(v, 6, 3), pick(1, 2),
              __builtin_unpredictable(x));
}
