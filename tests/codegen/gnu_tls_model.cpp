// GNU `tls_model` (WP-807): the variable's global gets the model, from
// any of its declarations and through template instantiation; a
// lifetime-extended temporary takes its variable's model; a dynamic
// `thread_local` keeps its wrapper. (A template's constant-initialized
// `thread_local` lacks clang's `_ZTW` wrapper: WP-807 follow-up.)
extern __thread int r1;
__thread int r1 __attribute__((tls_model("initial-exec")));
extern __thread int r2 __attribute__((tls_model("local-exec")));
__thread int r2;
__thread int r3 __attribute__((tls_model("global-dynamic")));
static __thread int r4 __attribute__((tls_model("local-dynamic")));
int use() { return r1 + r2 + r3 + r4; }
template <class T> struct S { static __thread T m __attribute__((tls_model("initial-exec"))); };
template <class T> __thread T S<T>::m = 1;
template <class T> __thread T vt __attribute__((tls_model("local-dynamic"))) = 0;
int f();
thread_local int dyn __attribute__((tls_model("initial-exec"))) = f();
thread_local const int &ref __attribute__((tls_model("local-exec"))) = f();
int u2() {
  static thread_local int z __attribute__((tls_model("local-exec"))) = 3;
  return S<int>::m + vt<int> + z + dyn + ref;
}
