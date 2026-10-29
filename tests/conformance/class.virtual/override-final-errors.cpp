// [class.virtual]/4-5, [dcl.fct.spec]/2, [class.mem.general]: 'override' on a
// function that overrides nothing, overriding a 'final' function (directly or
// through an intermediate class), 'virtual' outside a class or on a non-function,
// 'static virtual', pure-specifier on a non-virtual, 'virtual friend', and
// 'virtual' on an out-of-line definition.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:15 error "non-virtual member function marked 'override' hides virtual member function"
//@ diag: 24:16 note "hidden overloaded virtual function 'A::f' declared here: different number of parameters (0 vs 1)"
//@ diag: 31:8 error "declaration of 'g' overrides a 'final' function"
//@ diag: 25:16 note "overridden virtual function is here"
//@ diag: 32:12 error "only virtual member functions can be marked 'override'"
//@ diag: 31:8 warning "'g' overrides a member function but is not marked 'override'"
//@ diag: 25:16 note "overridden virtual function is here"
//@ diag: 36:8 error "declaration of 'k' overrides a 'final' function"
//@ diag: 33:8 note "overridden virtual function is here"
//@ diag: 38:1 error "'virtual' can only appear on non-static member functions"
//@ diag: 40:3 error "'virtual' can only appear on non-static member functions"
//@ diag: 41:10 error "'virtual' can only appear on non-static member functions"
//@ diag: 42:8 error "'p' is not virtual and cannot be declared pure"
//@ diag: 43:3 error "'virtual' is invalid in friend declarations"
//@ diag: 45:1 error "'virtual' can only be specified inside the class definition"
struct A {
  virtual void f();
  virtual void g() final;
  void h();
  virtual void k(int);
};
struct B : A {
  void f(int) override;
  void g();
  void h() override;
  void k(int) final;
};
struct C : B {
  void k(int);
};
virtual void free_fn();
struct D {
  virtual int x;
  static virtual void s();
  void p() = 0;
  virtual friend void fr();
};
virtual void A::f() {}
