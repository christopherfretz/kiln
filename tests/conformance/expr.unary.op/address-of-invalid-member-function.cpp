// [expr.unary.op]/3, [conv.func]: the address of a member function (or function)
// whose declaration is already invalid is erroneous without a second error
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:3 error "unknown type name 'undeclared_t'"
//@ diag: 15:9 error "unknown type name 'undeclared2_t'"
//@ diag: 16:10 error "unknown type name 'undeclared_t'"
//@ diag: 18:8 error "unknown type name 'undeclared3_t'"
//@ diag: 29:3 error "no matching function for call to 'k'"
//@ diag: 18:6 note "candidate function not viable: requires 1 argument, but 0 were provided"
// An invalid parameter keeps the function callable (clang resolves the call
// through an overload set), so the call is still checked.
struct A {
  undeclared_t f();
  int g(undeclared2_t);
  static undeclared_t sf();
};
void k(undeclared3_t);
void (A::*p)() = &A::f;
int (A::*q)() = &A::g;
void (*sp)() = &A::sf;
void (*kp)() = k;
void (*kp2)() = &k;
auto ap = &A::f;
int i = ap;
void take(void (*)());
void t() {
  take(k);
  k();
}
