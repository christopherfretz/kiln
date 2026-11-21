// [clang ext] `target_version` on x86-64: clang 22 does not know the
// attribute there and ignores it with a warning, so the declarations are
// those of ordinary functions (x86-target-version-19.cpp: clang 19).
// spec/sema.md "Function multiversioning" (WP-1745).
//@ expect: compile-ok
//@ requires: clang>=22
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
//@ diag: 14:16 warning "unknown attribute 'target_version' ignored"
//@ diag: 15:16 warning "unknown attribute 'target_version' ignored"
//@ diag: 16:16 warning "unknown attribute 'target_version' ignored"
//@ diag: 17:16 warning "unknown attribute 'target_version' ignored"
//@ diag: 18:3 warning "unknown attribute 'gnu::target_version' ignored"
__attribute__((target_version("default"))) int f() { return 0; }
__attribute__((target_version("avx2"))) int f();
__attribute__((target_version("avx2"))) int f2() { return 1; }
__attribute__((target_version("bogus"))) int g();
[[gnu::target_version("sse4.2")]] int h();
int h() { return 2; }
int use() { return f() + h(); }
