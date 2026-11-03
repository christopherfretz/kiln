// [basic.stc.thread]: clang 19 (and kiln at -fclang-compat=19) rejects an
// instantiation-dependent `__thread` initializer in the template
// definition (static-tls-dependent-19.cpp). A parenthesized initializer of
// a dependent type, or whose one expression is type-dependent, is clang
// 19's `ParenListExpr` and is reported at its `(`; a value-dependent
// one of a non-dependent type (h3) at the expression. Each is reported
// again when instantiated, at the culprit (static-tls-paren.cpp).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ requires: clang<22
//@ diag: 38:51 error "initializer for thread-local variable must be a constant expression"
//@ diag: 38:50 note "use 'thread_local' to allow this"
//@ diag: 39:51 error "initializer for thread-local variable must be a constant expression"
//@ diag: 39:50 note "use 'thread_local' to allow this"
//@ diag: 40:54 error "initializer for thread-local variable must be a constant expression"
//@ diag: 40:52 note "use 'thread_local' to allow this"
//@ diag: 41:53 error "initializer for thread-local variable must be a constant expression"
//@ diag: 41:52 note "use 'thread_local' to allow this"
//@ diag: 42:53 error "initializer for thread-local variable must be a constant expression"
//@ diag: 42:52 note "use 'thread_local' to allow this"
//@ diag: 43:53 error "initializer for thread-local variable must be a constant expression"
//@ diag: 43:52 note "use 'thread_local' to allow this"
//@ diag: 44:69 error "initializer for thread-local variable must be a constant expression"
//@ diag: 44:68 note "use 'thread_local' to allow this"
//@ diag: 45:52 error "initializer for thread-local variable must be a constant expression"
//@ diag: 45:51 note "use 'thread_local' to allow this"
//@ diag: 46:33 error "initializer for thread-local variable must be a constant expression"
//@ diag: 46:31 note "use 'thread_local' to allow this"
//@ diag: 38:52 error "initializer for thread-local variable must be a constant expression"
//@ diag: 47:14 note "in instantiation of function template specialization 'h1<N>' requested here"
//@ diag: 38:50 note "use 'thread_local' to allow this"
//@ diag: 42:54 error "initializer for thread-local variable must be a constant expression"
//@ diag: 47:23 note "in instantiation of function template specialization 'h5<V>' requested here"
//@ diag: 42:52 note "use 'thread_local' to allow this"
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
