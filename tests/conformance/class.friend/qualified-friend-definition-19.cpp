// [class.friend]/6: the clang 19 shape of qualified-friend-definition.cpp
// (clang 19 prints the nested-name-specifier with its trailing '::').
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:15 error "friend function definition cannot be qualified with 'Bq<sizeof(T)>::'"
//@ diag: 29:15 error "friend function definition cannot be qualified with 'N::'"
//@ diag: 30:10 error "friend function definition cannot be qualified with 'A<T>::'"
//@ diag: 33:15 error "friend function definition cannot be qualified with '::'"
//@ diag: 34:15 error "friend function definition cannot be qualified with 'N::'"
//@ diag: 35:34 error "friend function definition cannot be qualified with 'N::'"
//@ diag: 36:15 error "friend function definition cannot be qualified with 'C::'"
//@ diag: 37:10 error "friend function definition cannot be qualified with 'C::'"
//@ diag: 38:10 error "friend function definition cannot be qualified with 'C::'"
//@ diag: 39:18 error "friend declaration of 'nope' does not match any declaration in namespace 'N'"
//@ diag: 40:15 error "friend function definition cannot be qualified with 'decltype(b)::'"
//@ diag: 43:34 error "friend function definition cannot be qualified with 'A<T>::'"
//@ diag: 43:40 warning "dependent nested name specifier 'A<T>::' for friend class declaration is not supported; turning off access control for 'X'"
namespace N { void f(); void h(); template <class U> void k(U); }
void gf();
template <int I> struct Bq { void g(); };
template <class T> struct A { void m(); A(); };
struct C { C(); ~C(); void m(); };
int v(...);
struct B { void g(); };
B b;
template <class T> struct S {
  friend void Bq<sizeof(T)>::g() { }
  friend void N::f() { }
  friend A<T>::A() { }
};
struct P {
  friend void ::gf() { }
  friend void N::h() { (void)v(); }
  template <class U> friend void N::k(U) { }
  friend void C::m() { }
  friend C::C() { }
  friend C::~C() { }
  friend void N::nope() { }
  friend void decltype(b)::g() { }
};
struct X {
  template <class T> friend void A<T>::m() { }
};
