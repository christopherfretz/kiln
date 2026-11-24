// [class.mfct]/1, [class.static.data]/3: a member function, destructor or
// static data member defined out of line through a nested-name-specifier
// naming a templated class without a definition (a member of the current
// instantiation, a primary template never defined) is
// "out-of-line definition of 'f' from class 'G::H' without definition" at
// the declarator's name, with no "does not match" or "no member named"
// after it.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 16:33 error "out-of-line definition of 'f' from class 'G::H' without definition"
//@ diag: 17:32 error "out-of-line definition of 'x' from class 'G::H' without definition"
//@ diag: 18:28 error "out-of-line definition of '~J' from class 'G::J' without definition"
//@ diag: 20:30 error "out-of-line definition of 'f' from class 'F<T>' without definition"
template<class T> struct G { struct H; struct J; };
template<class T> void G<T>::H::f() {}
template<class T> int G<T>::H::x = 1;
template<class T> G<T>::J::~J() {}
template<class T> struct F;
template<class T> void F<T>::f() {}
// Valid: defined first, then its members out of line.
template<class T> struct G<T>::J { ~J(); void f(); static int x; };
template<class T> G<T>::J::~J() {}
template<class T> void G<T>::J::f() {}
template<class T> int G<T>::J::x = 2;
