// CWG2496 / [over.load]/2.3 (C++20): a member function with the
// parameter-type-list of a virtual function of a base class, one of the two
// having a ref-qualifier and the other none, can neither override nor be
// overloaded with it, whatever their cv-qualifiers; each such base function
// is diagnosed, also through an intermediate class.
//@ std: c++20
//@ expect: compile-fail
//@ diag: 25:8 error "cannot overload a member function with ref-qualifier '&' with a member function without a ref-qualifier"
//@ diag: 19:16 note "previous declaration is here"
//@ diag: 26:16 error "cannot overload a member function without a ref-qualifier with a member function with ref-qualifier '&'"
//@ diag: 20:16 note "previous declaration is here"
//@ diag: 27:8 error "cannot overload a member function with ref-qualifier '&&' with a member function without a ref-qualifier"
//@ diag: 21:16 note "previous declaration is here"
//@ diag: 28:8 error "cannot overload a member function with ref-qualifier '&' with a member function without a ref-qualifier"
//@ diag: 22:16 note "previous declaration is here"
//@ diag: 31:8 error "cannot overload a member function with ref-qualifier '&' with a member function without a ref-qualifier"
//@ diag: 19:16 note "previous declaration is here"
struct B {
  virtual void f();
  virtual void g() &;
  virtual void h(int);
  virtual void n() const;
};
struct D : B {
  void f() &;
  virtual void g();
  void h(int) &&;
  void n() &;
};
struct M : D {
  void f() &;
};
