// [basic.stc.thread], [temp.res.general]/6: the parenthesized
// instantiation-dependent `__thread` initializers of
// static-tls-paren-19.cpp are checked only when instantiated, at the
// first part that is not a constant initializer (the `T` of `T(1)`, the
// `V::v` of `(T::v)`).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ requires: clang>=22
//@ diag: 26:69 error "initializer for thread-local variable must be a constant expression"
//@ diag: 26:68 note "use 'thread_local' to allow this"
//@ diag: 20:52 error "initializer for thread-local variable must be a constant expression"
//@ diag: 29:14 note "in instantiation of function template specialization 'h1<N>' requested here"
//@ diag: 20:50 note "use 'thread_local' to allow this"
//@ diag: 24:54 error "initializer for thread-local variable must be a constant expression"
//@ diag: 29:23 note "in instantiation of function template specialization 'h5<V>' requested here"
//@ diag: 24:52 note "use 'thread_local' to allow this"
struct N { N(int) {} int v; };
struct V { static int v; };
template <class T> void h1() { static __thread T w(T(1)); (void)w; }
template <class T> void h2() { static __thread T w((T(1))); (void)w; }
template <class T> void h3() { static __thread int w(sizeof(T)); (void)w; }
template <class T> void h4() { static __thread int w(T::v, 1); (void)w; }
template <class T> void h5() { static __thread int w(T::v); (void)w; }
template <class T> void h6() { static __thread int w(T::v + 1); (void)w; }
template <class T> struct X { X(int); void f() { static __thread X w(1); (void)w; } };
template <class T> void h7() { static __thread T *w(&T::v); (void)w; }
template <class T> __thread T vt(T(1));
void use() { h1<N>(); h5<V>(); }
