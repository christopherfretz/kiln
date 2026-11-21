// [clang ext] Function multiversioning on x86-64 at run time: the ifunc
// resolvers call `__cpu_indicator_init` and pick a version from
// `__cpu_model` / `__cpu_features2`. Every x86-64 CPU has SSE2, so the
// `sse2` version of `f` outranks its default (also through a pointer),
// both clones of `g` return the same value, and the `pentium_4` version
// of `c` (whose features are a subset of x86-64's) outranks `generic`.
// spec/codegen.md "Function multiversioning" (WP-1745).
//@ requires: target-x86_64
//@ expect: run
//@ strict-diags
//@ stdout: 2 2 3 5 7
#include <cstdio>
__attribute__((target("default"))) int f() { return 1; }
__attribute__((target("sse2"))) int f() { return 2; }
__attribute__((target_clones("sse2", "default"))) int g() { return 3; }
__attribute__((cpu_specific(generic))) int c() { return 4; }
__attribute__((cpu_specific(pentium_4))) int c() { return 5; }
__attribute__((cpu_dispatch(generic, pentium_4))) int c();
struct S {
  __attribute__((target("default"))) int m() const { return 6; }
  __attribute__((target("sse2"))) int m() const { return 7; }
};
int main() {
  int (*p)() = f;
  const int a = f();
  const int b = p();
  const int x = g();
  const int y = c();
  const int z = S{}.m();
  std::printf("%d %d %d %d %d\n", a, b, x, y, z);
  return a == 2 && b == 2 && x == 3 && y == 5 && z == 7 ? 0 : 1;
}
