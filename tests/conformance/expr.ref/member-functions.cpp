// [expr.ref]/7, [expr.prim.id.general]/4: a non-static member function named
// through an object must be called; a member function cannot be called
// without an object.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:21 error "call to non-static member function without an object argument"
//@ diag: 22:5 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 23:14 error "reference to non-static member function must be called"
//@ diag: 24:6 error "call to non-static member function without an object argument"
//@ diag: 26:13 error "cannot refer to type member 'T' in 'S' with '.'"
//@ diag: 18:9 note "member 'T' declared here"
//@ diag: 27:7 error "too few arguments to function call, expected 1, have 0"
//@ diag: 16:7 note "'g' declared here"
struct S {
  int f();
  int g(int);
  static int h();
  using T = int;
};
int S::h() { return f(); }
void use(S s) {
  s.f;
  auto p = s.g;
  S::f();
  s.h();
  int n = s.T;
  s.g();
}
