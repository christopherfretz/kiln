// CWG2496 in C++23, clang 19: the ref-qualifier mix with a base virtual
// function is not diagnosed from C++23 on (clang 22 diagnoses it; see the
// sibling without -19). WP-1041.
//@ std: c++23
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang<22
struct B {
  virtual void f();
  virtual void g() &;
  virtual void h() const;
};
struct D : B {
  void f() &;
  void g();
  void h() &&;
};
