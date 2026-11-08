// [clang ext] Function multiversioning on AArch64 without the `fmv`
// target feature: every call reaches the default version, whatever the
// CPU supports, because the other versions are never declared.
// spec/landed/codegen/WP-810.md (WP-810).
//@ requires: target-aarch64
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 3 4 5
#include <cstdio>
__attribute__((target_version("sve"))) int f() { return 10; }
__attribute__((target_version("default"))) int f() { return 1; }
__attribute__((target_version("simd+fp"))) int f();
int g();
__attribute__((target_version("default"))) int g() { return 2; }
__attribute__((target_version("rng"))) int g() { return 20; }
[[gnu::target_clones("sve", "default")]] int h() { return 3; }
template <class T> __attribute__((target_version("default"))) T t(T x) { return x; }
template <class T> __attribute__((target_version("sve2"))) T t(T x) { return x + 40; }
struct S { int m(); };
__attribute__((target_version("lse"))) int S::m() { return 50; }
int S::m() { return 5; }
int main() {
  S s;
  std::printf("%d %d %d %d %d\n", f(), g(), h(), t(4), s.m());
  return f() + g() + h() + t(4) + s.m() == 15 ? 0 : 1;
}
