// [dcl.spec.auto.general]/11: a static member function whose return type
// is not yet deduced cannot be called.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:12 error "function 'f' with deduced return type cannot be used before it is defined"
//@ diag: 8:15 note "'f' declared here"
struct S {
  static auto f();
};
int x = S::f();
auto S::f() { return 1; }
int y = S::f();
