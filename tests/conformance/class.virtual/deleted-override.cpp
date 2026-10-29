// [class.virtual]/18: a deleted function shall not override a non-deleted one
// and vice versa.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:8 error "deleted function 'f' cannot override a non-deleted function"
//@ diag: 10:16 note "overridden virtual function is here"
//@ diag: 19:8 error "non-deleted function 'e' cannot override a deleted function"
//@ diag: 16:16 note "overridden virtual function is here"
struct A {
  virtual void f();
};
struct B : A {
  void f() override = delete;
};
struct E {
  virtual void e() = delete;
};
struct F : E {
  void e() override;
};
