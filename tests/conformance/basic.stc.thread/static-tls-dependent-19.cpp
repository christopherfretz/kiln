// [basic.stc.thread]: clang 19 (and kiln at -fclang-compat=19) rejects a
// `__thread` initializer that is instantiation-dependent, unless it is a
// braced list, already in the template definition, where clang 22 waits
// for the instantiation (static-tls-dependent.cpp; [temp.res.general]/6
// allows either for the `T()` of h1, not for h3's `sizeof(T)`).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ requires: clang<22
//@ diag: 33:54 error "initializer for thread-local variable must be a constant expression"
//@ diag: 33:50 note "use 'thread_local' to allow this"
//@ diag: 34:56 error "initializer for thread-local variable must be a constant expression"
//@ diag: 34:52 note "use 'thread_local' to allow this"
//@ diag: 35:56 error "initializer for thread-local variable must be a constant expression"
//@ diag: 35:52 note "use 'thread_local' to allow this"
//@ diag: 37:43 error "initializer for thread-local variable must be a constant expression"
//@ diag: 37:39 note "use 'thread_local' to allow this"
//@ diag: 39:36 error "initializer for thread-local variable must be a constant expression"
//@ diag: 39:31 note "use 'thread_local' to allow this"
//@ diag: 40:37 error "initializer for thread-local variable must be a constant expression"
//@ diag: 40:31 note "use 'thread_local' to allow this"
//@ diag: 33:54 error "initializer for thread-local variable must be a constant expression"
//@ diag: 41:25 note "in instantiation of function template specialization 'h1<N>' requested here"
//@ diag: 33:50 note "use 'thread_local' to allow this"
//@ diag: 34:56 error "initializer for thread-local variable must be a constant expression"
//@ diag: 41:34 note "in instantiation of function template specialization 'h2<V>' requested here"
//@ diag: 34:52 note "use 'thread_local' to allow this"
//@ diag: 37:43 error "initializer for thread-local variable must be a constant expression"
//@ diag: 42:15 note "in instantiation of static data member 'S<V>::m' requested here"
//@ diag: 37:39 note "use 'thread_local' to allow this"
struct N { N() : v(0) {} int v; };
struct V { static int v; };
template <class T> void h1() { static __thread T x = T(); (void)x; }
template <class T> void h2() { static __thread int x = T::v; (void)x; }
template <class T> void h3() { static __thread int x = sizeof(T); (void)x; }
template <class T> struct S { static __thread int m; static __thread T n; };
template <class T> __thread int S<T>::m = T::v;
template <class T> __thread T S<T>::n{};
template <int I> __thread int vt = I;
template <class T> __thread T vt2 = T();
void use() { h1<int>(); h1<N>(); h2<V>(); h3<int>(); }
int u = S<V>::m + vt<1> + vt2<int>;
