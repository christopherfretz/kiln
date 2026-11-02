// [temp.dep.type]/1, [class.member.lookup]: a name not found in the
// current instantiation (no dependent base) is diagnosed at definition
// time, the class named as its injected-class-name type, as clang prints
// it: the pattern with its template parameters (a pack with `...`),
// unqualified; a partial specialization with its canonical arguments; a
// non-template member class of a template by its qualified name.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:45 error "no member named 'zz' in 'V<T>'"
//@ diag: 19:33 error "no member named 'yy' in 'V<T>'"
//@ diag: 20:49 error "no member named 'zz' in 'V<type-parameter-0-0 *>'"
//@ diag: 22:48 error "no member named 'zz' in 'P<T...>'"
//@ diag: 23:71 error "no member named 'zz' in 'Q<TT, M>'"
//@ diag: 24:75 error "no member named 'zz' in 'O<U>'"
//@ diag: 25:56 error "no member named 'q' in 'W::I'"
//@ diag: 25:64 error "no member named 'r' in 'W<T>'"
namespace N {
template <class T> struct V { void f() { V::zz = 1; }
  static int g() { return V<T>::yy; } };
template <class T> struct V<T*> { void f() { V::zz = 1; } };
}
template <class... T> struct P { void f() { P::zz = 1; } };
template <template <class> class TT, auto M> struct Q { void f() { Q::zz = 1; } };
template <class T> struct H { template <class U> struct O { void f() { O::zz = 1; } }; };
template <class T> struct W { struct I { void f() { I::q(); W::r(); } }; };
