// [dcl.meaning.general]/2: a qualified declarator-id that matches no
// member of the nominated class. clang 22 names a current instantiation
// fully qualified and notes the class searched; clang 19's wording is
// qualified-no-match-class-note-clang19.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 32:33 error "out-of-line definition of 'g' does not match any declaration in 'N::A<T>'"
//@ diag: 31:40 note "A defined here"
//@ diag: 33:53 error "out-of-line definition of 'Q' does not match any declaration in 'N::A<T>'"
//@ diag: 34:8 error "forward declaration of struct cannot have a nested name specifier"
//@ diag: 34:14 error "no struct named 'Z' in 'N::S'"
//@ diag: 35:32 error "out-of-line definition of 'Y' does not match any declaration in 'N::S'"
//@ diag: 36:7 error "out-of-line definition of 'S' does not match any declaration in 'N::S'"
//@ diag: 31:92 note "S defined here"
//@ diag: 37:7 error "definition of implicitly declared destructor"
//@ diag: 40:34 error "out-of-line definition of 'h' does not match any declaration in 'B::In'"
//@ diag: 39:32 note "In defined here"
//@ diag: 42:25 error "out-of-line definition of 'E<T>' does not match any declaration in 'E<T>'"
//@ diag: 41:26 note "E defined here"
//@ diag: 44:29 error "out-of-line definition of 'F<T>' does not match any declaration in 'NE::F<T>'"
//@ diag: 43:41 note "F defined here"
//@ diag: 46:10 error "out-of-line definition of 'f' does not match any declaration in 'C1'"
//@ diag: 45:8 note "C1 defined here"
//@ diag: 48:32 error "out-of-line definition of 'f' does not match any declaration in 'D1<T>'"
//@ diag: 47:27 note "D1 defined here"
//@ diag: 50:15 error "out-of-line definition of 'f' does not match any declaration in 'D1<int>'"
//@ diag: 49:20 note "D1 defined here"
//@ diag: 52:33 error "out-of-line definition of 'f' does not match any declaration in 'D1<T *>'"
//@ diag: 51:27 note "D1 defined here"
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
