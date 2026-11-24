// [expr.prim.id.qual]/1, [class.mfct]/1: a constructor defined out of line
// through a nested-name-specifier naming a templated class without a
// definition (a member class template of a specialization, a member of
// the current instantiation, a primary or partial specialization never
// defined) is "incomplete type 'C' named in nested name specifier",
// printing the class's own name, at the begin of the specifier (clang 22;
// the clang 19 location is incomplete-qualifier-constructor-clang19.cpp),
// with no "does not match" after it. Reduced from real code.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 22:30 error "incomplete type 'C' named in nested name specifier"
//@ diag: 23:37 error "incomplete type 'C' named in nested name specifier"
//@ diag: 25:19 error "incomplete type 'I' named in nested name specifier"
//@ diag: 27:19 error "incomplete type 'F' named in nested name specifier"
//@ diag: 30:19 error "incomplete type 'H' named in nested name specifier"
//@ diag: 32:19 error "incomplete type 'L' named in nested name specifier"
//@ diag: 36:45 error "out-of-line definition of 'C<U>' does not match any declaration in 'A<long>::C<U>'"
//@ diag: 35:46 note "C defined here"
template<class T> struct A { template<class U> struct C; };
template<> template<class U> A<int>::C<U>::C() : m(1) {}
template<class T> template<class U> A<T>::C<U>::C() {}
template<class T> struct G { struct H { struct I; }; };
template<class T> G<T>::H::I::I() {}
template<class T> struct F;
template<class T> F<T>::F() {}
template<class T> struct X;
template<class T> struct X<T*> { struct H; };
template<class T> X<T*>::H::H(int) {}
namespace q { template<class T> struct K { struct L; }; }
template<class T> q::K<T>::L::L() {}
// A complete member template whose constructor does not match keeps the
// mismatch error.
template<> template<class U> struct A<long>::C { C(); int m; };
template<> template<class U> A<long>::C<U>::C(int) : m(1) {}
// Valid: defined first.
template<> template<class U> A<long>::C<U>::C() : m(2) {}
template<class T> struct G<T>::H::I { I(); };
template<class T> G<T>::H::I::I() {}
