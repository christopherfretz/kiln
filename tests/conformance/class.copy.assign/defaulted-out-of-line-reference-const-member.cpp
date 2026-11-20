// [class.copy.assign]/7, /12: a defaulted assignment operator of a class
// with a reference or const member is deleted when defaulted on its
// first declaration; defaulted after it, its definition is ill-formed:
// "cannot define the implicit copy assignment operator ... because
// non-static reference member 'r' cannot use copy assignment operator"
// (clang's wording, also for the move assignment operator).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:7 error "defaulting this copy assignment operator would delete it after its first declaration"
//@ diag: 25:22 note "copy assignment operator of 'C' is implicitly deleted because field 'c' is of const-qualified type 'const int'"
//@ diag: 19:26 error "cannot define the implicit copy assignment operator for 'R<int>', because non-static reference member 'r' cannot use copy assignment operator"
//@ diag: 20:31 note "in defaulted copy assignment operator for 'R<int>' first required here"
//@ diag: 21:23 note "in instantiation of member function 'R<int>::operator=' requested here"
//@ diag: 19:33 note "declared here"
//@ diag: 22:26 error "cannot define the implicit copy assignment operator for 'K<int>', because non-static const member 'c' cannot use copy assignment operator"
//@ diag: 23:31 note "in defaulted move assignment operator for 'K<int>' first required here"
//@ diag: 24:23 note "in instantiation of member function 'K<int>::operator=' requested here"
//@ diag: 22:38 note "declared here"
template<class T> struct R { T& r; R& operator=(const R&); };
template<class T> R<T>& R<T>::operator=(const R&) = default;
void f(R<int>& a) { a = a; }
template<class T> struct K { const T c[2]; K& operator=(K&&); };
template<class T> K<T>& K<T>::operator=(K&&) = default;
void g(K<int>& a) { a = static_cast<K<int>&&>(a); }
struct C { const int c; C& operator=(const C&); };
C& C::operator=(const C&) = default;
