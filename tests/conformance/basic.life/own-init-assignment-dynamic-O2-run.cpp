// At -O2. [basic.start.static]/2, [expr.const]/10.8, [basic.life]/7: an
// assignment to a later element in an earlier element's initializer is
// not a constant expression, so the array is initialized dynamically; the
// assignment then stores to the zero-initialized storage, and the trailing
// elements are not stored again (clang `CheckAggExprForMemSetUse`: the
// array is zeroed by one `memset` first). Kiln constant-folded the
// initializer and lost both stores (found running GCC's C++ testsuite).
//@ std: c++11
//@ args: -O2
//@ expect: run
//@ stdout: 5 3 0 5 0 0 0 3 0 0 / 5 3 0 5 0 0 0 3 0 0
#include <cstdio>
static int arr[10] = {arr[3] = 5, arr[7] = 3};
struct L { int v[10]; };
static L l = {{l.v[3] = 5, l.v[7] = 3}};
int main() {
  for (int i = 0; i < 10; i++) std::printf("%d ", arr[i]);
  std::printf("/");
  for (int i = 0; i < 10; i++) std::printf(" %d", l.v[i]);
  std::printf("\n");
}
