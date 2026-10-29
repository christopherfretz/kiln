// [class.base.init]/8, [basic.stc.auto]: a reference member bound to a
// by-value parameter, or a pointer member initialized with its address,
// dangles once the constructor returns (clang's -Wdangling-field); a
// reference member bound to a temporary is an error ([class.base.init]/8).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:39 warning "binding reference member 'r' to stack allocated parameter 'x'"
//@ diag: 18:8 note "reference member declared here"
//@ diag: 22:46 warning "initializing pointer member 'p' with the stack address of parameter 'y'"
//@ diag: 19:8 note "pointer member declared here"
//@ diag: 25:46 error "reference member 'r' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 25:23 note "reference member declared here"
//@ diag: 26:34 error "reference member 'r' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 26:23 note "reference member declared here"
//@ diag: 26:52 error "reference member 'r' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 26:23 note "reference member declared here"
struct S {
  int &r;
  int *p;
  const int &cr;
  int *q;
  S(int x, int y, int &z, int *w) : r(x), p(&y), cr(z), q(w) {}
  S(int &x, int *y) : r(x), p(y), cr(*y), q(&x) {}
};
struct T { const int &r; int k; T(int a) : r(a + 1), k(r) {} };
struct U { const int &r; U() : r(1) {} U(long) : r((long)2) {} };
