// GNU `model` checks on x86-64 (clang 22 `handleCodeModelAttr`): the
// argument must be a string literal (clang's parser), then the subject
// must be a non-TLS global variable (an error), there must be exactly
// one argument, and x86-64 accepts only `small` and `large`.
// spec/sema.md §4.5, WP-1745.
//@ expect: compile-fail
//@ args: --target=x86_64-unknown-linux-gnu -ferror-limit=0
//@ requires: clang>=22
//@ strict-diags
//@ diag: 21:28 error "code model 'extreme' is not supported on this target"
//@ diag: 22:28 error "code model 'medium' is not supported on this target"
//@ diag: 23:28 error "code model 'Large' is not supported on this target"
//@ diag: 24:25 error "'model' attribute only applies to non-TLS global variables"
//@ diag: 25:79 error "'model' attribute only applies to non-TLS global variables"
//@ diag: 26:28 error "expected string literal as argument of 'model' attribute"
//@ diag: 27:20 error "expected string literal as argument of 'model' attribute"
//@ diag: 28:22 error "'model' attribute takes one argument"
//@ diag: 29:22 error "'model' attribute takes one argument"
//@ diag: 30:35 error "'model' attribute only applies to non-TLS global variables"
//@ diag: 31:33 error "'model' attribute only applies to non-TLS global variables"
int c __attribute__((model("extreme")));
int d __attribute__((model("medium")));
int e __attribute__((model("Large")));
void f() __attribute__((model("large")));
void g() { static int s __attribute__((model("large"))); int l __attribute__((model("large"))); }
int h __attribute__((model(1)));
int i [[gnu::model(1)]];
int j __attribute__((model));
int k __attribute__((model("small", "large")));
thread_local int t __attribute__((model("large")));
struct S { int x __attribute__((model("large"))); };
int ok __attribute__((model("large")));
