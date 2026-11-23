// [cstdarg.syn]/1 with `-Wno-non-pod-varargs`: `va_arg` of a class that
// is not trivially copyable is valid (conditionally-supported, clang
// accepts it), and kiln compiles it as an ordinary `VAArgExpr`, without
// the error bit it carries while the diagnostic is an error. Pair of
// va-arg-non-pod-erroneous.cpp (WP-2362).
//@ expect: compile-ok
//@ args: -Wno-non-pod-varargs
//@ strict-diags
struct N { N(const N &); int m; };
int take(N);
void f(int n, ...) {
  __builtin_va_list l;
  __builtin_va_start(l, n);
  int a = __builtin_va_arg(l, N).m;
  N b = __builtin_va_arg(l, N);
  take(__builtin_va_arg(l, N));
  auto c = __builtin_va_arg(l, N);
}
template <class T> int g(__builtin_va_list l) { return __builtin_va_arg(l, T).m; }
int gi(__builtin_va_list l) { return g<N>(l); }
