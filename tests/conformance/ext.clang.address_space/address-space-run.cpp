//@ expect: run
//@ strict-diags
//@ stdout: 4 1 2 0 7 9 3
// clang address_space(N) (WP-1803): a qualified pointee type, overloadable
// on, convertible from the generic address space only by a cast, and
// a global or static local in the space reads and writes as any other.
#include <cstdio>

using as1_int = int __attribute__((address_space(1)));

int f(int *) { return 0; }
int f(as1_int *) { return 1; }
int f(__attribute__((opencl_generic)) int *) { return 2; }

as1_int g = 5;
struct S { int a, b; };
__attribute__((address_space(1))) S gs = {1, 2};

int bump() {
  static as1_int n = 0;
  n += 1;
  return n;
}
int sum(const as1_int *p, int k) {
  int s = 0;
  for (int i = 0; i < k; ++i) s += p[i];
  return s;
}
as1_int &ref(as1_int &r) { return r; }

int main() {
  int x = 4;
  as1_int *p = (as1_int *)&x;
  __attribute__((opencl_global)) int *q = (__attribute__((opencl_global)) int *)&x;
  ref(g) += 2;
  gs.b = gs.b + 7;
  bump();
  bump();
  int b = bump();
  int s = sum(&gs.a, 2) - 1;
  std::printf("%d %d %d %d %d %d %d\n", (int)sizeof(*p), f(p), f(q), f(&x), g, s, b);
}
