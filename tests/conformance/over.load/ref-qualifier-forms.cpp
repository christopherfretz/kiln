// [over.load]/2.3 (C++20): member function declarations with the same
// parameter-type-list cannot be overloaded when one has a ref-qualifier and
// the other has none, whatever their cv-qualifiers. Each later declaration
// is diagnosed against the first one it mixes with.
//@ std: c++20
//@ expect: compile-fail
//@ diag: 19:8 error "cannot overload a member function without a ref-qualifier with a member function with ref-qualifier '&'"
//@ diag: 18:8 note "previous declaration is here"
//@ diag: 21:8 error "cannot overload a member function without a ref-qualifier with a member function with ref-qualifier '&&'"
//@ diag: 20:8 note "previous declaration is here"
//@ diag: 23:8 error "cannot overload a member function with ref-qualifier '&' with a member function without a ref-qualifier"
//@ diag: 22:8 note "previous declaration is here"
//@ diag: 25:26 error "cannot overload a member function without a ref-qualifier with a member function with ref-qualifier '&'"
//@ diag: 24:26 note "previous declaration is here"
//@ diag: 27:8 error "cannot overload a member function without a ref-qualifier with a member function with ref-qualifier '&&'"
//@ diag: 26:7 note "previous declaration is here"
struct S {
  void f() const &;
  void f();
  void g(int) &&;
  void g(int) volatile;
  void h(int, ...);
  void h(int, ...) &;
  template<class T> void t(T) &;
  template<class T> void t(T);
  int r() &&;
  long r() const;
};
