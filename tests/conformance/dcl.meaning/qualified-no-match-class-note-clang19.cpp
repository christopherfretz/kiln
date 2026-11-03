// [dcl.meaning.general]/2: clang 19 names a current instantiation
// unqualified and adds no note (clang 22: qualified-no-match-class-note.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 21:33 error "out-of-line definition of 'g' does not match any declaration in 'A<T>'"
//@ diag: 22:53 error "out-of-line definition of 'Q' does not match any declaration in 'A<T>'"
//@ diag: 23:8 error "forward declaration of struct cannot have a nested name specifier"
//@ diag: 23:14 error "no struct named 'Z' in 'N::S'"
//@ diag: 24:32 error "out-of-line definition of 'Y' does not match any declaration in 'N::S'"
//@ diag: 25:7 error "out-of-line definition of 'S' does not match any declaration in 'N::S'"
//@ diag: 26:7 error "definition of implicitly declared destructor"
//@ diag: 29:34 error "out-of-line definition of 'h' does not match any declaration in 'B::In'"
//@ diag: 31:25 error "out-of-line definition of 'E<T>' does not match any declaration in 'E<T>'"
//@ diag: 33:29 error "out-of-line definition of 'F<T>' does not match any declaration in 'F<T>'"
//@ diag: 35:10 error "out-of-line definition of 'f' does not match any declaration in 'C1'"
//@ diag: 37:32 error "out-of-line definition of 'f' does not match any declaration in 'D1<T>'"
//@ diag: 39:15 error "out-of-line definition of 'f' does not match any declaration in 'D1<int>'"
//@ diag: 41:33 error "out-of-line definition of 'f' does not match any declaration in 'D1<type-parameter-0-0 *>'"
namespace N { template<class T> struct A { void f(); template<class U> struct M; }; struct S { }; }
template<class T> void N::A<T>::g() {}
template<class T> template<class U> struct N::A<T>::Q {};
struct N::S::Z;
template<class U> struct N::S::Y {};
N::S::S(int) {}
N::S::~S() {}
template<class T> struct B { struct In; };
template<class T> struct B<T>::In { void h(); };
template<class T> void B<T>::In::h(int) {}
template<class T> struct E { E(); ~E(); };
template<class T> E<T>::E(int) {}
namespace NE { template<class T> struct F { }; }
template<class T> NE::F<T>::F(int) {}
struct C1 { void f(); };
void C1::f(int) {}
template <class T> struct D1 { void f(); };
template <class T> void D1<T>::f(int) {}
template <> struct D1<int> { void f(); };
void D1<int>::f(int) {}
template <class T> struct D1<T*> { void f(); };
template <class T> void D1<T*>::f(int) {}
