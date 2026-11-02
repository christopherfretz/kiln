// [temp.res.general]/4.2 (P0634): a dependent qualified name is implicitly
// a type in a parameter-declaration only in a member-declaration, in a
// declarator with a qualified declarator-id, in a lambda-declarator or a
// requires-expression (and, as clang, a deduction guide). A parameter of a
// function declared at namespace or block scope with an unqualified
// declarator-id needs `typename` ([temp.res.general]/5): clang's error at
// the name, recovered as if `typename` were written.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:28 error "missing 'typename' prior to dependent type name 'X<T>::m'"
//@ diag: 22:33 error "missing 'typename' prior to dependent type name 'X<T>::m'"
//@ diag: 23:34 error "missing 'typename' prior to dependent type name 'X<T>::m'"
//@ diag: 24:28 error "missing 'typename' prior to dependent type name 'X<T>::m'"
//@ diag: 25:28 error "missing 'typename' prior to dependent type name 'T::type'"
//@ diag: 26:42 error "missing 'typename' prior to dependent type name 'X<T>::m'"
template <class T> struct X {
  using m = int;
};
template <class T> struct S { S(int*); };

template <class T> void g2(X<T>::m x);
template <class T> void g3(int, X<T>::m* p);
template <class T> void g4(const X<T>::m& x) {}
template <class T> void g5(X<T>::m x = 0);
template <class T> void g6(T::type x, int);
namespace N { template <class T> void g7(X<T>::m x); }

// Implicit typename: no diagnostic.
template <class T> struct M {
  void m1(X<T>::m);
  friend void m2(M, X<T>::m) {}
  void m3(X<T>::m);
  template <class U> M(X<U>::m* p);
};
template <class T> void M<T>::m3(X<T>::m) {}
template <class T> template <class U> M<T>::M(X<U>::m*) {}
template <class T> void l() {
  auto f = [](X<T>::m a) { return a; };
  (void)f;
}
template <class T> concept C = requires(X<T>::m a) { a; };
template <class T> S(X<T>::m*, T) -> S<T>;
template <class T> void ok(typename X<T>::m x, X<int>::m y);
