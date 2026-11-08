// GNU `tls_model` checks (WP-807; clang `handleTLSModelAttr`): the first
// argument must be a string literal (clang's parser, before the subject),
// then the subject must be a thread-local variable (an error), there must
// be exactly one argument, and it must name one of the four models.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 33:41 error "tls_model must be \"global-dynamic\", \"local-dynamic\", \"initial-exec\" or \"local-exec\""
//@ diag: 34:42 error "tls_model must be \"global-dynamic\", \"local-dynamic\", \"initial-exec\" or \"local-exec\""
//@ diag: 35:41 error "expected string literal as argument of 'tls_model' attribute"
//@ diag: 36:31 error "'tls_model' attribute takes one argument"
//@ diag: 37:31 error "'tls_model' attribute takes one argument"
//@ diag: 38:22 error "'tls_model' attribute only applies to thread-local variables"
//@ diag: 39:33 error "expected string literal as argument of 'tls_model' attribute"
//@ diag: 40:23 error "'tls_model' attribute only applies to thread-local variables"
//@ diag: 41:23 error "'tls_model' attribute only applies to thread-local variables"
//@ diag: 42:32 error "'tls_model' attribute takes one argument"
//@ diag: 43:42 error "expected string literal as argument of 'tls_model' attribute"
//@ diag: 44:23 error "'tls_model' attribute only applies to thread-local variables"
//@ diag: 45:26 error "'tls_model' attribute only applies to thread-local variables"
//@ diag: 46:29 error "'tls_model' attribute only applies to thread-local variables"
//@ diag: 49:24 error "'tls_model' attribute only applies to thread-local variables"
//@ diag: 50:32 error "'tls_model' attribute only applies to thread-local variables"
//@ diag: 55:25 error "'tls_model' attribute only applies to thread-local variables"
//@ diag: 58:41 error "'tls_model' attribute only applies to thread-local variables"
//@ diag: 59:41 error "an unevaluated string literal cannot have an encoding prefix"
//@ diag: 61:31 error "'tls_model' attribute only applies to thread-local variables"
//@ diag: 62:23 error "'tls_model' attribute only applies to thread-local variables"
__thread int a __attribute__((tls_model("initial-exec")));
thread_local int b __attribute__((tls_model("local-exec"))) = 1;
static thread_local int c [[gnu::tls_model("local-dynamic")]];
__thread int d __attribute__((__tls_model__("global-dynamic")));
__thread int e __attribute__((tls_model("bogus")));
__thread int e2 __attribute__((tls_model("Initial-exec")));
__thread int f __attribute__((tls_model(1)));
__thread int g __attribute__((tls_model));
__thread int h __attribute__((tls_model("initial-exec", "x")));
int i __attribute__((tls_model("initial-exec")));
int i2 __attribute__((tls_model(1)));
int i3 __attribute__((tls_model("bogus")));
int i4 __attribute__((tls_model("x", 1)));
__thread int i5 __attribute__((tls_model("x", 1)));
__thread int i6 __attribute__((tls_model(1, "x")));
int i7 __attribute__((tls_model()));
void fn() __attribute__((tls_model("initial-exec")));
void p(int q __attribute__((tls_model("initial-exec"))));
void loc() {
  static __thread int s __attribute__((tls_model("initial-exec")));
  int l __attribute__((tls_model("initial-exec")));
  static int l2 __attribute__((tls_model("initial-exec")));
  (void)l; (void)l2;
}
struct S {
  static thread_local int m __attribute__((tls_model("local-exec")));
  int nm __attribute__((tls_model("local-exec")));
};
template <class T> thread_local T vt __attribute__((tls_model("initial-exec"))) = T();
template <class T> T nvt __attribute__((tls_model("initial-exec"))) = T();
__thread int j __attribute__((tls_model(u8"initial-exec")));
__thread int k __attribute__((tls_model("initial" "-exec")));
typedef int TD __attribute__((tls_model("initial-exec")));
struct __attribute__((tls_model("initial-exec"))) R {};
