// [temp.friend]/9: "A non-template friend declaration with a
// requires-clause shall be a definition. A friend function template with
// a constraint that depends on a template parameter from an enclosing
// template shall be a definition." clang (`ActOnFunctionDeclarator`)
// reports each at the start of the declaration after its template head;
// the friend is invalid, so its default argument is not reported too. A
// constraint naming only the friend's own parameters, a non-dependent
// one, a static data member or a member class of the enclosing template
// (clang's `FriendConstraintsDependOnEnclosingTemplate`) is fine, as is a
// definition (`= delete` included). clang 19 and 22 agree on all of
// these.
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 41:3 error "non-template friend declaration with a requires clause must be a definition"
//@ diag: 42:3 error "non-template friend declaration with a requires clause must be a definition"
//@ diag: 43:3 error "non-template friend declaration with a requires clause must be a definition"
//@ diag: 44:3 error "non-template friend declaration with a requires clause must be a definition"
//@ diag: 44:3 error "non-template friend declaration with a requires clause must be a definition"
//@ diag: 45:3 error "non-template friend declaration with a requires clause must be a definition"
//@ diag: 46:3 error "non-template friend declaration with a requires clause must be a definition"
//@ diag: 48:22 error "friend declaration with a constraint that depends on an enclosing template parameter must be a definition"
//@ diag: 49:21 error "friend declaration with a constraint that depends on an enclosing template parameter must be a definition"
//@ diag: 50:36 error "friend declaration with a constraint that depends on an enclosing template parameter must be a definition"
//@ diag: 51:3 error "friend declaration with a constraint that depends on an enclosing template parameter must be a definition"
//@ diag: 52:22 error "friend declaration with a constraint that depends on an enclosing template parameter must be a definition"
//@ diag: 53:22 error "friend declaration with a constraint that depends on an enclosing template parameter must be a definition"
//@ diag: 54:22 error "friend declaration with a constraint that depends on an enclosing template parameter must be a definition"
//@ diag: 55:20 error "friend declaration with a constraint that depends on an enclosing template parameter must be a definition"
template <class T> concept C = sizeof(T) > 0;
template <class T, class U> concept D = sizeof(T) == sizeof(U);
template <class T> struct B {
  void m() requires C<T>;
};

template <class T> struct A {
  static constexpr int N = sizeof(T);
  using V = T;
  struct Nested {};

  friend void f1() requires C<T>;
  friend void f2() requires true;
  void friend f3() requires C<T>;
  friend void f4() noexcept requires C<T>, f5() requires C<T>;
  friend void f6(int = 0) requires C<T>;
  friend void B<T>::m() requires C<T>;

  template <class U> friend void g1(U) requires C<T>;
  template <D<T> U> friend void g2(U);
  template <class U> requires C<T> friend void g3(U);
  friend void g4(auto) requires C<T>;
  template <class U> friend void g5(U) requires C<V>;
  template <class U> friend void g6(U) requires C<A>;
  template <class U> friend void g7(U) requires requires(T t) { t; };
  template <int I> friend void g8() requires(I > sizeof(T));

  template <class U> friend void ok1(U) requires C<U>;
  template <C U> friend void ok2(U);
  template <class U> friend void ok3(U) requires C<int>;
  template <class U> friend void ok4(U) requires(N > 0);
  template <class U> friend void ok5(U) requires C<Nested>;
  friend void ok6() requires C<T> {}
  friend void ok7() requires C<T> = delete;
  template <class U> friend void ok8(U) requires C<T> {}
};

A<int> a;
A<long> b;
