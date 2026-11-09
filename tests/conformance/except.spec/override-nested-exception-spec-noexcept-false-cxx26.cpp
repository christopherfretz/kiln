// [except.spec]/5 (C++14), kept by clang in later modes: from C++17
// noexcept(false) and no specification give the same function type, so the
// overriders below override, yet clang still compares the WRITTEN nested
// specifications (CheckParamExceptionSpec reads the type sugar):
// noexcept(false) is potentially throwing, the base's missing one is too,
// but a written noexcept(false) against none (and the reverse) is
// "exception specifications of argument types differ" (return types: r).
// An overrider whose parameter adds noexcept has another type and hides
// B::h. Each base parameter is compared with the overrider's first one (j
// is not diagnosed, k is). kiln compared canonical types, which drop the
// written specification from C++17. WP-1102.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:8 error "exception specifications of argument types differ"
//@ diag: 32:8 error "exception specifications of argument types differ"
//@ diag: 33:31 error "non-virtual member function marked 'override' hides virtual member function"
//@ diag: 25:16 note "hidden overloaded virtual function 'B::h' declared here: type mismatch at 1st parameter ('void (*)()' vs 'void (*)() noexcept')"
//@ diag: 34:10 error "exception specifications of return types differ"
//@ diag: 36:8 error "exception specifications of argument types differ"

struct B {
  virtual void f(void (*)());
  virtual void g(void (*)() noexcept(false));
  virtual void h(void (*)());
  virtual void (*r())();
  virtual void j(int, void (*)() noexcept(false));
  virtual void k(void (*)(), int);
};
struct D : B {
  void f(void (*)() noexcept(false)) override;
  void g(void (*)()) override;
  void h(void (*)() noexcept) override;
  void (*r())() noexcept(false) override;
  void j(int, void (*)()) override;
  void k(void (*)() noexcept(false), int) override;
};
