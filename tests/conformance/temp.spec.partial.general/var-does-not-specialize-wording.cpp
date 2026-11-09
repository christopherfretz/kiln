// [temp.spec.partial.general]/9.3 for variable templates: clang says "to
// define the primary template" for a declaration that is a definition (no
// `extern`, not in a class; an initializer is not needed) and "to declare"
// otherwise. Found in the clang negative-test run (temp.class.spec/p8-1y.cpp),
// fixed by WP-956.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:30 error "variable template partial specialization does not specialize any template argument; to define the primary template, remove the template argument list"
//@ diag: 15:37 error "variable template partial specialization does not specialize any template argument; to declare the primary template, remove the template argument list"
//@ diag: 18:39 error "variable template partial specialization does not specialize any template argument; to declare the primary template, remove the template argument list"
template <typename... T> int v1;
template <typename... T> int v1<T...>;
template <typename... T> extern int v2;
template <typename... T> extern int v2<T...>;
struct S {
  template <typename... T> static int v3;
  template <typename... T> static int v3<T...>;
};
template <typename... T> int v4;
template <typename T, typename... U> int v4<T, U...>;
