// [basic.stc.thread], [temp.res.general]/6: a `__thread` initializer that
// is instantiation-dependent is checked when instantiated (`T()` of a
// non-constexpr constructor, the non-constant `V::v`); the template
// definitions are valid. Clang 19 rejects each of them in the template
// definition (static-tls-dependent-19.cpp).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ requires: clang>=22
//@ diag: 21:54 error "initializer for thread-local variable must be a constant expression"
//@ diag: 29:25 note "in instantiation of function template specialization 'h1<N>' requested here"
//@ diag: 21:50 note "use 'thread_local' to allow this"
//@ diag: 22:56 error "initializer for thread-local variable must be a constant expression"
//@ diag: 29:34 note "in instantiation of function template specialization 'h2<V>' requested here"
//@ diag: 22:52 note "use 'thread_local' to allow this"
//@ diag: 25:43 error "initializer for thread-local variable must be a constant expression"
//@ diag: 30:15 note "in instantiation of static data member 'S<V>::m' requested here"
//@ diag: 25:39 note "use 'thread_local' to allow this"
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
