// [dcl.spec.auto.general]/12: a virtual function cannot have a deduced
// return type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:3 error "function with deduced return type cannot be virtual"
struct S {
  virtual auto f() { return 1; }
};
