// `__rdtsc` is declared only for an x86-64 target: on AArch64 it is an
// undeclared identifier, as in clang. WP-961.
//@ args: --target=aarch64-unknown-linux-gnu
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:12 error "use of undeclared identifier '__rdtsc'"
void f() { __rdtsc(); }
