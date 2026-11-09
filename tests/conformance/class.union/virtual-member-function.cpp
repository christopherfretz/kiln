// [class.union.general]/4: a union shall not have virtual member functions.
// Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 7:3 error "unions cannot have virtual functions"
union U {
  virtual void f();
};
