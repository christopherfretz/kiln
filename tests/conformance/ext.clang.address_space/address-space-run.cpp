//@ expect: run
//@ strict-diags
//@ requires: target-aarch64
//@ xfail(kiln): WP-809 follow-up, address_space(N) is not implemented (a hard error)
//@ stdout: 4 1
// clang address_space(N): a qualified pointee type, overloadable on, and
// convertible from the generic address space only by a cast.
#include <cstdio>

using as1_int = int __attribute__((address_space(1)));

int f(int *) { return 0; }
int f(as1_int *) { return 1; }

int main() {
  int x = 4;
  as1_int *p = (as1_int *)&x;
  std::printf("%d %d\n", (int)sizeof(*p), f(p));
}
