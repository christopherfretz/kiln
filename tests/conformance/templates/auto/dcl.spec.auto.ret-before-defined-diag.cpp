// [dcl.spec.auto.general]/11: a function with an undeduced return type
// cannot be used in an expression before its return type is deduced.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:9 error "function 'f' with deduced return type cannot be used before it is defined"
//@ diag: 9:6 note "'f' declared here"
//@ diag: 13:12 error "function 'g' with deduced return type cannot be used before it is defined"
//@ diag: 11:6 note "'g' declared here"
auto f();
int x = f();
auto g(int n) {
  if (n > 0)
    return g(n - 1);
  return 0;
}
auto f() { return 1; }
int y = f();
