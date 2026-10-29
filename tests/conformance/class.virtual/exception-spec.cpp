// [except.spec]/13: an overrider of a function with a non-throwing exception
// specification shall itself be non-throwing; tightening is allowed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 8:16 note "overridden virtual function is here"
struct A {
  virtual void f() noexcept;
  virtual void h();
};
struct B : A {
  void f() override;
  void h() noexcept override;
};
