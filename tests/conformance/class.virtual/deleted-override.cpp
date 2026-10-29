// [class.virtual]/18: a deleted function shall not override a non-deleted one
// and vice versa.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(kiln): deleted functions are WP-067a (act_on_deleted is a stub); the override check is in place
//@ diag: 14:8 error "deleted function 'f' cannot override a non-deleted function"
//@ diag: 11:16 note "overridden virtual function is here"
//@ diag: 20:8 error "non-deleted function 'e' cannot override a deleted function"
//@ diag: 17:16 note "overridden virtual function is here"
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
