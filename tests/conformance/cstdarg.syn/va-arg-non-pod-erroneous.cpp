// [cstdarg.syn]/1 (clang `BuildVAArgExpr`): `va_arg` of a class that is not
// trivially copyable is `-Wnon-pod-varargs`, an error by default. The
// expression stays a `VAArgExpr` of its type (the program is valid with
// the warning disabled) and carries the error bit while the diagnostic is
// an error, so a use of it reports nothing more: a member access, an
// initializer, a call argument, a deduced type, a template's
// instantiation (WP-2362). Its pair with the warning disabled:
// va-arg-non-pod-warning-disabled.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:31 error "second argument to 'va_arg' is of non-POD type 'N'"
//@ diag: 23:29 error "second argument to 'va_arg' is of non-POD type 'N'"
//@ diag: 24:28 error "second argument to 'va_arg' is of non-POD type 'N'"
//@ diag: 25:32 error "second argument to 'va_arg' is of non-POD type 'N'"
//@ diag: 27:76 error "second argument to 'va_arg' is of non-POD type 'N'"
//@ diag: 28:38 note "in instantiation of function template specialization 'g<N>' requested here"
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
