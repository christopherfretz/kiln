// `__rdtsc` is declared only for an x86-64 target: on AArch64 it is an
// undeclared identifier, as in clang. WP-961.
//@ args: --target=aarch64-unknown-linux-gnu
//@ expect: compile-fail
// Not strict-diags: on AArch64 clang 22 adds a warning implicitly declaring `__dsb`.
//@ diag: 7:12 error "use of undeclared identifier '__rdtsc'"
void f() { __rdtsc(); }
