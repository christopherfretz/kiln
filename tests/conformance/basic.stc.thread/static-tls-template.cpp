// [basic.stc.thread], [temp.inst]: a `__thread` variable of dependent type
// is checked when instantiated (the error is followed by the
// instantiation note); an initializer that is not instantiation-dependent
// is checked in the template definition as well. Same in clang 22 and 19.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 27:53 error "initializer for thread-local variable must be a constant expression"
//@ diag: 27:49 note "use 'thread_local' to allow this"
//@ diag: 28:36 error "initializer for thread-local variable must be a constant expression"
//@ diag: 28:31 note "use 'thread_local' to allow this"
//@ diag: 22:37 error "initializer for thread-local variable must be a constant expression"
//@ diag: 24:16 note "in instantiation of static data member 'S<N>::m' requested here"
//@ diag: 22:37 note "use 'thread_local' to allow this"
//@ diag: 25:49 error "type of thread-local variable has non-trivial destruction"
//@ diag: 26:24 note "in instantiation of function template specialization 'h<D>' requested here"
//@ diag: 25:49 note "use 'thread_local' to allow this"
int f();
struct N { N() : v(0) {} int v; };
struct D { ~D() {} };
template <class T> struct S { static __thread T m; };
template <class T> __thread T S<T>::m;
int u1 = S<int>::m;
int u2 = S<N>::m.v;
template <class T> void h() { static __thread T x; (void)x; }
void use() { h<int>(); h<D>(); }
template <class T> void k() { static __thread T y = f(); }
template <class T> __thread T vt = f();
