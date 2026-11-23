// [class.friend]/6: a function may be defined in a friend declaration only if
// the function name is unqualified. Each qualified form is diagnosed at the
// start of its qualifier: a dependent qualifier at the declaration, a
// non-dependent one once the friend matches its target (a friend that matches
// nothing reports the no-match error instead). Clang 22 prints the specifier
// without its trailing '::' (the -19 twin has the clang 19 shape). Reduced
// from real code.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:15 error "friend function definition cannot be qualified with 'Bq<sizeof(T)>'"
//@ diag: 34:15 error "friend function definition cannot be qualified with 'N'"
//@ diag: 35:10 error "friend function definition cannot be qualified with 'A<T>'"
//@ diag: 38:15 error "friend function definition cannot be qualified with '::'"
//@ diag: 39:15 error "friend function definition cannot be qualified with 'N'"
//@ diag: 40:34 error "friend function definition cannot be qualified with 'N'"
//@ diag: 41:15 error "friend function definition cannot be qualified with 'C'"
//@ diag: 42:10 error "friend function definition cannot be qualified with 'C'"
//@ diag: 43:10 error "friend function definition cannot be qualified with 'C'"
//@ diag: 44:18 error "friend declaration of 'nope' does not match any declaration in namespace 'N'"
//@ diag: 45:15 error "friend function definition cannot be qualified with 'decltype(b)'"
//@ diag: 48:34 error "friend function definition cannot be qualified with 'A<T>'"
//@ diag: 48:40 warning "dependent nested name specifier 'A<T>' for friend class declaration is not supported; turning off access control for 'X'"
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
