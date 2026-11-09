// [class.union.general]/4: a union shall not have virtual member functions.
// Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, union member restrictions are not checked
//@ expect: compile-fail
//@ diag: 8:3 error "unions cannot have virtual functions"
union U {
  virtual void f();
};
