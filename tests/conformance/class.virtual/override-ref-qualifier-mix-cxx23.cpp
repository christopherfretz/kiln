// CWG2496 in C++23: clang 22 diagnoses a ref-qualifier mix between a
// member function and a virtual function of a base with the same
// parameter-type-list in every language mode (clang 19 only before C++23;
// see the -19 sibling). WP-1041.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 21:8 error "cannot overload a member function with ref-qualifier '&' with a member function without a ref-qualifier"
//@ diag: 16:16 note "previous declaration is here"
//@ diag: 22:8 error "cannot overload a member function without a ref-qualifier with a member function with ref-qualifier '&'"
//@ diag: 17:16 note "previous declaration is here"
//@ diag: 23:8 error "cannot overload a member function with ref-qualifier '&&' with a member function without a ref-qualifier"
//@ diag: 18:16 note "previous declaration is here"
struct B {
  virtual void f();
  virtual void g() &;
  virtual void h() const;
};
struct D : B {
  void f() &;
  void g();
  void h() &&;
};
