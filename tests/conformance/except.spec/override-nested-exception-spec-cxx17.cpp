// [except.spec]/5 (C++14), kept by clang in later modes: from C++17 the
// exception specification is part of the function type, so an overrider
// whose parameter type differs only in it overrides nothing (j hides A::j).
// A return type that differs only in it is still the same function, so the
// overrider gets both clang 22's "different return type" error and
// "exception specifications of return types differ". Clang 19 words the
// first error "not covariant" (kiln under -fclang-compat=19,
// class.virtual/covariant-return-clang22.cpp). WP-1078 (WP-1079).
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 26:10 error "virtual function 'h' has a different return type ('void (*)()') than the function it overrides (which has return type 'void (*)() noexcept')"
//@ diag: 21:18 note "overridden virtual function is here"
//@ diag: 26:10 error "exception specifications of return types differ"
//@ diag: 27:10 error "virtual function 'i' has a different return type ('void (*)() noexcept') than the function it overrides (which has return type 'void (*)()')"
//@ diag: 22:18 note "overridden virtual function is here"
//@ diag: 27:10 error "exception specifications of return types differ"

struct A {
  virtual void (*h())() noexcept;
  virtual void (*i())();
  virtual void j(void (*)() noexcept);
};
struct B : A {
  void (*h())();
  void (*i())() noexcept;
  void j(void (*)());
};
