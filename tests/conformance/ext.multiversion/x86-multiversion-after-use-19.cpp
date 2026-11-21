// [clang ext] A function used before a declaration makes it multiversioned
// on x86-64 (clang `CheckMultiVersionFirstFunction` and
// `CheckTargetCausesMultiVersioning`): an error, without the note at the
// earlier declaration that clang 22 adds (x86-multiversion-after-use.cpp).
// spec/sema.md "Function multiversioning" (WP-1745).
//@ expect: compile-fail
//@ requires: clang<22
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
//@ diag: 14:40 error "function declaration cannot become a multiversioned function after first usage"
//@ diag: 18:55 error "function declaration cannot become a multiversioned function after first usage"
int m11();
int u11 = m11();
__attribute__((target("default"))) int m11();
__attribute__((target("avx2"))) int m11();
int m12();
int u12 = m12();
__attribute__((target_clones("avx2", "default"))) int m12();
// Unused, the earlier declaration becomes the default version.
int m14();
__attribute__((target("default"))) int m14();
__attribute__((target("avx2"))) int m14();
int u14 = m14();
