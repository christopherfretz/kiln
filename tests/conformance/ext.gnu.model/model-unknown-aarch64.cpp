// GNU `model` is x86-64 (and LoongArch) only in clang 22 and unknown to
// clang 19: on AArch64 it is the unknown-attribute warning, but clang's
// parser still requires a string literal argument (an error that drops
// the attribute). spec/sema.md §4.5, WP-1745.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
//@ diag: 12:22 warning "unknown attribute 'model' ignored"
//@ diag: 13:25 warning "unknown attribute 'model' ignored"
//@ diag: 14:28 error "expected string literal as argument of 'model' attribute"
//@ diag: 15:22 warning "unknown attribute 'model' ignored"
int a __attribute__((model("large"))) = 1;
void f() __attribute__((model("large")));
int h __attribute__((model(1)));
int i __attribute__((model));
