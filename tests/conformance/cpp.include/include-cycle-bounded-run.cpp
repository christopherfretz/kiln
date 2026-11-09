// [cpp.include]: the valid twin of include-cycle-two-back-edges.cpp. A
// file that includes itself twice, bounded by `__INCLUDE_LEVEL__` below the
// nesting limit, enters every path: 2 + 4 + 8 inclusions.
//@ expect: run
//@ strict-diags
//@ stdout: 14
#if __INCLUDE_LEVEL__ == 0
extern "C" int printf(const char *, ...);
static int n = 0;
int main() {
#else
  ++n;
#endif
#if __INCLUDE_LEVEL__ < 3
#include "include-cycle-bounded-run.cpp"
#include "include-cycle-bounded-run.cpp"
#endif
#if __INCLUDE_LEVEL__ == 0
  printf("%d\n", n);
}
#endif
