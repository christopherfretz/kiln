//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:25 error "function template partial specialization is not allowed"
//@ diag: 9:34 error "function template partial specialization is not allowed"
// [temp.spec.partial.general]/1: only class and variable templates have
// partial specializations.
template <class T> void f(T);
template <class T> void f<T *>(T *);
template <class T, class U> void f<T (U)>(T (*)(U));
