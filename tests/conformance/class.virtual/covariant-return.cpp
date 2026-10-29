// [class.virtual]/8-9: an overrider's return type is the same or covariant: both
// pointers or both references (same kind) to classes, the overrider's class is
// complete (or being defined), derived via an unambiguous accessible base, and
// no more cv-qualified; otherwise the program is ill-formed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:12 warning "direct base 'X' is inaccessible due to ambiguity:"
//@ diag: 56:8 error "return type of virtual function 'f2' is not covariant with the return type of the function it overrides ('int *' is not derived from 'X *')"
//@ diag: 40:14 note "overridden virtual function is here"
//@ diag: 57:6 error "invalid covariant return for virtual function: 'X' is a private base class of 'Z'"
//@ diag: 32:12 note "declared private here"
//@ diag: 58:6 error "return type of virtual function 'f4' is not covariant with the return type of the function it overrides (ambiguous conversion from derived class 'Q' to base class 'X':"
//@ diag: 42:14 note "overridden virtual function is here"
//@ diag: 60:5 error "virtual function 'f6' has a different return type ('Y') than the function it overrides (which has return type 'X *')"
//@ diag: 44:14 note "overridden virtual function is here"
//@ diag: 61:8 error "virtual function 'f7' has a different return type ('long') than the function it overrides (which has return type 'int')"
//@ diag: 45:15 note "overridden virtual function is here"
//@ diag: 63:6 error "return type of virtual function 'f9' is not covariant with the return type of the function it overrides ('W *' is not derived from 'X *')"
//@ diag: 47:14 note "overridden virtual function is here"
//@ diag: 64:6 error "return type of virtual function 'f10' is not covariant with the return type of the function it overrides ('X *' is not derived from 'U *')"
//@ diag: 48:14 note "overridden virtual function is here"
//@ diag: 65:14 error "return type of virtual function 'f11' is not covariant with the return type of the function it overrides (class type 'const int *' is more qualified than class type 'int *'"
//@ diag: 49:16 note "overridden virtual function is here"
//@ diag: 66:6 error "virtual function 'f12' has a different return type ('Y &') than the function it overrides (which has return type 'X &&')"
//@ diag: 50:15 note "overridden virtual function is here"
//@ diag: 67:6 error "return type of virtual function 'f13' is not covariant with the return type of the function it overrides ('I' is incomplete)"
//@ diag: 37:8 note "forward declaration of 'I'"
//@ diag: 68:6 error "return type of virtual function 'f14' is not covariant with the return type of the function it overrides ('B *' is not derived from 'X *')"
//@ diag: 52:14 note "overridden virtual function is here"
struct X {};
struct Y : X {};
struct Z : private X {};
struct P : X {};
struct Q : X, P {};
struct W {};
union U {};
struct I;
struct A {
  virtual X *f1();
  virtual X *f2();
  virtual X *f3();
  virtual X *f4();
  virtual X &f5();
  virtual X *f6();
  virtual int f7();
  virtual const X *f8();
  virtual X *f9();
  virtual U *f10();
  virtual int *f11();
  virtual X &&f12();
  virtual X *f13();
  virtual X *f14();
};
struct B : A {
  Y *f1() override;
  int *f2() override;
  Z *f3() override;
  Q *f4() override;
  Y &f5() override;
  Y f6() override;
  long f7() override;
  Y *f8() override;
  W *f9() override;
  X *f10() override;
  const int *f11() override;
  Y &f12() override;
  I *f13() override;
  B *f14() override;
};
