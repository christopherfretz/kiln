// [temp.dep.type]/1, [class.member.lookup]: a name not found in the
// current instantiation (no dependent base) is diagnosed at definition
// time, the class named as its injected-class-name type, as clang prints
// it: the pattern with its template parameters (a pack with `...`),
// unqualified; a partial specialization with its canonical arguments; a
// non-template member class of a template by its qualified name.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:45 error "no member named 'zz' in 'N::V<T>'"
//@ diag: 21:33 error "no member named 'yy' in 'N::V<T>'"
//@ diag: 22:49 error "no member named 'zz' in 'N::V<T *>'"
//@ diag: 24:48 error "no member named 'zz' in 'P<T...>'"
//@ diag: 25:71 error "no member named 'zz' in 'Q<TT, M>'"
//@ diag: 26:75 error "no member named 'zz' in 'H::O<U>'"
//@ diag: 27:56 error "no member named 'q' in 'W::I'"
//@ diag: 27:64 error "no member named 'r' in 'W<T>'"
//@ requires: clang>=22
// clang 22 spelling of no-member-in-current-instantiation-name.cpp (WP-394).
namespace N {
template <class T> struct V { void f() { V::zz = 1; }
  static int g() { return V<T>::yy; } };
template <class T> struct V<T*> { void f() { V::zz = 1; } };
}
template <class... T> struct P { void f() { P::zz = 1; } };
template <template <class> class TT, auto M> struct Q { void f() { Q::zz = 1; } };
template <class T> struct H { template <class U> struct O { void f() { O::zz = 1; } }; };
template <class T> struct W { struct I { void f() { I::q(); W::r(); } }; };
