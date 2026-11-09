// [except.spec]/5 (C++14): if a virtual function's return or parameter
// type is a pointer (or reference, or pointer to member) to a function,
// the overrider's matching type must have an equivalent exception
// specification. No specification and noexcept(false) are not
// equivalent (clang CheckEquivalentExceptionSpec). Clang reports "exception
// specifications of argument types differ" or "... of return types differ"
// at the overrider, with no note. The check runs only when the
// overrider's own specification passes the subset check, so j reports
// only "more lax". Like clang 19 and 22, kiln compares each of the base's
// parameter types with the overrider's FIRST one (q). Found by the clang
// negative-test run (WP-957), fixed by WP-1078 (WP-1079). Valid twin:
// override-nested-exception-spec-ok.cpp; C++17:
// override-nested-exception-spec-cxx17.cpp.
//@ std: c++14
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-c++17-compat-mangling
//@ diag: 48:8 error "exception specifications of argument types differ"
//@ diag: 49:8 error "exception specifications of argument types differ"
//@ diag: 50:10 error "exception specifications of return types differ"
//@ diag: 51:10 error "exception specifications of return types differ"
//@ diag: 52:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 38:16 note "overridden virtual function is here"
//@ diag: 53:8 error "exception specifications of argument types differ"
//@ diag: 54:8 error "exception specifications of argument types differ"
//@ diag: 55:8 error "exception specifications of argument types differ"
//@ diag: 56:8 error "exception specifications of argument types differ"
//@ diag: 57:8 error "exception specifications of argument types differ"
//@ diag: 58:10 error "exception specifications of argument types differ"
//@ diag: 59:8 error "exception specifications of argument types differ"

struct S;
struct A {
  virtual void f(void (*)() throw());
  virtual void g(void (*)());
  virtual void (*h())() throw();
  virtual void (*i())();
  virtual void j(void (*)() throw(int)) throw();
  virtual void k(void (*)() noexcept);
  virtual void l(void (&)() noexcept);
  virtual void m(void (S::*)() throw(), S *);
  virtual void n(void (*)(), void (*)() noexcept(false));
  virtual void o(void (*)() noexcept) noexcept;
  virtual void (*p(void (*)() noexcept))() noexcept;
  virtual void q(void (*)(), void (*)() noexcept);
};
struct B : A {
  void f(void (*)());
  void g(void (*)() throw());
  void (*h())();
  void (*i())() throw();
  void j(void (*)() throw(long));
  void k(void (*)());
  void l(void (&)() throw(int));
  void m(void (S::*)(), S *);
  void n(void (*)(), void (*)());
  void o(void (*)()) noexcept;
  void (*p(void (*)()))() noexcept;
  void q(void (*)(), void (*)());
};
