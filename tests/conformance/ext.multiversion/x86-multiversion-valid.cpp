// [clang ext] The valid counterparts of x86-multiversion-errors.cpp: the
// versions of one function on x86-64 with distinct CPUs or features, one
// return type, constexpr and inline specification, linkage and exception
// specification, redeclarations with identical attributes, a body-less
// `cpu_dispatch`, a `target` on `main` that is not `default`. spec/sema.md
// "Function multiversioning" (WP-1745).
//@ expect: compile-ok
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
__attribute__((cpu_specific(atom))) int c1() { return 1; }
__attribute__((cpu_specific(ivybridge))) int c1();
__attribute__((cpu_dispatch(atom, ivybridge))) int c2();
__attribute__((cpu_dispatch(atom, ivybridge))) int c2();
__attribute__((cpu_specific(atom, ivybridge))) int c3();
__attribute__((cpu_dispatch(atom, ivybridge))) int c3();
__attribute__((cpu_dispatch(atom))) int c4() {}
__attribute__((target_clones("avx2", "default"))) int t2();
__attribute__((target_clones("avx2", "default"))) int t2();
__attribute__((target("default"))) long m1();
__attribute__((target("avx2"))) long m1();
__attribute__((target("default"))) int m2();
__attribute__((target("avx2"), used)) int m2() { return 0; }
__attribute__((target("default"))) constexpr int m3() { return 1; }
__attribute__((target("avx2"))) constexpr int m3() { return 2; }
__attribute__((target("default"))) int m4();
__attribute__((target("avx2,sse4.2"))) int m4();
__attribute__((target("avx2,sse4.2"))) int m4();
__attribute__((target("default"))) int m6();
__attribute__((target("arch=haswell"))) int m6();
__attribute__((cpu_specific(atom))) int m8();
__attribute__((cpu_dispatch(atom))) int m8();
__attribute__((target("default"))) static int m14() { return 0; }
__attribute__((target("avx2"))) static int m14() { return 1; }
__attribute__((target("default"))) inline int m15() { return 0; }
__attribute__((target("avx2"))) inline int m15() { return 1; }
__attribute__((target("default"))) int m17() noexcept;
__attribute__((target("avx2"))) int m17() noexcept;
struct S {
  __attribute__((target("default"))) int m() { return 0; }
  __attribute__((target("avx2"))) int m() { return 1; }
  __attribute__((target("default"))) static int s() { return 0; }
  __attribute__((target("avx2"))) static int s() { return 1; }
};
int use(S &x) {
  long (*p)() = m1;
  return x.m() + S::s() + c1() + c2() + c3() + t2() + int(p()) + m14() + m15() + m17() + m3();
}
__attribute__((target("sse4.2"))) int main() { return 0; }
