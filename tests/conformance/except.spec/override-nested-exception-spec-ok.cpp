// [except.spec]/5 (C++14): valid twin of
// override-nested-exception-spec-cxx14.cpp. Equivalent nested
// specifications: throw() and noexcept are both non-throwing, the same
// list in another order or with repeats, two throwing ones, and function
// types more than one parameter level deep (not compared). As in clang 19
// and 22, each base parameter type is compared with the overrider's FIRST
// one only, so a mismatch after a non-function first parameter (e) or
// after equivalent non-throwing ones (f) is not diagnosed. WP-1078.
//@ std: c++14
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-c++17-compat-mangling

struct A {
  virtual void a(void (*)() throw());
  virtual void b(void (&)() noexcept);
  virtual void c(void (*)() throw(int, long));
  virtual void d(void (*)(void (*)() throw()));
  virtual void e(int, void (*)() noexcept);
  virtual void f(void (*)() noexcept, void (*)() noexcept);
  virtual void (*g())() noexcept;
  virtual void h(void (*)()) throw();
};
struct B : A {
  void a(void (*)() noexcept);
  void b(void (&)() throw());
  void c(void (*)() throw(long, int, int));
  void d(void (*)(void (*)()));
  void e(int, void (*)());
  void f(void (*)() noexcept, void (*)());
  void (*g())() throw();
  void h(void (*)()) noexcept;
};

void s(void (*)() throw(int)) throw(int);
void (*ps)(void (*)() throw(int)) throw(int) = s;
