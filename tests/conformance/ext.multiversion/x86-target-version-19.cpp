// [clang ext] `target_version` on x86-64 with clang 19, which treats it as
// on AArch64 without the `fmv` target feature: a version names features
// of `__builtin_cpu_supports`, a `default` version is the function, and a
// non-default one is never declared (`f2` below is not defined).
// x86-target-version.cpp: clang 22. spec/sema.md "Function
// multiversioning" (WP-1745).
//@ expect: compile-ok
//@ requires: clang<22
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
//@ diag: 15:31 warning "unsupported 'bogus' in the 'target_version' attribute string; 'target_version' attribute ignored"
__attribute__((target_version("default"))) int f() { return 0; }
__attribute__((target_version("avx2"))) int f();
__attribute__((target_version("avx2"))) int f2() { return 1; }
__attribute__((target_version("bogus"))) int g();
[[gnu::target_version("sse4.2")]] int h();
int h() { return 2; }
int use() { return f() + h(); }
