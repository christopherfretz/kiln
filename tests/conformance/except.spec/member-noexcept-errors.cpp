// [except.spec]/2,5,13: member noexcept-specifiers are complete-class
// contexts, converted to bool; a redeclaration must match; an overrider
// must not be potentially-throwing when the overridden function is not
// ([except.spec]/7); [conv.fctptr] drops but never adds noexcept.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:30 error "exception specification is not available until end of class definition"
//@ diag: 24:21 error "noexcept specifier argument evaluates to 2, which cannot be narrowed to type 'bool'"
//@ diag: 25:21 error "noexcept specifier argument is not a constant expression"
//@ diag: 25:21 note "implicit use of 'this' pointer is only allowed within the evaluation of a call to a 'constexpr' member function"
//@ diag: 30:9 error "'f' is missing exception specification 'noexcept(true)'"
//@ diag: 20:8 note "previous declaration is here"
//@ diag: 31:9 error "exception specification in declaration does not match previous declaration"
//@ diag: 21:8 note "previous declaration is here"
//@ diag: 33:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 27:16 note "overridden virtual function is here"
//@ diag: 35:11 error "cannot initialize a variable of type 'void (A::*)() noexcept' with an rvalue of type 'void (A::*)() noexcept(false)': different exception specifications"
//@ diag: 36:8 error "cannot initialize a variable of type 'void (*)() noexcept' with an rvalue of type 'void (*)()': different exception specifications"
struct A {
  void f() noexcept(true);
  void g() noexcept(false);
  void h() noexcept(noexcept(k()));
  void k() noexcept(true);
  void m() noexcept(2);
  void n() noexcept(x);
  int x;
  virtual void v() noexcept;
  static void s();
};
void A::f() {}
void A::g() noexcept {}
struct D : A {
  void v() noexcept(false) override;
};
void (A::*pm)() noexcept = &A::g;
void (*fp)() noexcept = &A::s;
void (A::*ok)() = &A::f;
