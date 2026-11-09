// WP-1057: `__builtin_readcyclecounter()` (clang, every target kiln
// supports) reads the processor's cycle counter, `unsigned long long`;
// `__has_builtin` reports it and sema declares it on each target. Two
// reads in one thread do not go backwards.
//@ expect: run
//@ stdout: 1 1
#include <cstdio>

static_assert(__has_builtin(__builtin_readcyclecounter));

int main() {
  unsigned long long a = __builtin_readcyclecounter();
  unsigned long long b = __builtin_readcyclecounter();
  static_assert(sizeof(a) == 8);
  std::printf("%d %d\n", b >= a, __is_same(decltype(__builtin_readcyclecounter()), unsigned long long));
}
