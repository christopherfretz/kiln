// [class.mfct.non.static]/4, [dcl.stc]/10: in a const member function the
// object is const unless the member is mutable.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:7 error "cannot assign to non-static data member within const member function 'g'"
//@ diag: 13:8 note "member function 'S::g' is declared const here"
//@ diag: 20:3 error "'this' argument to member function 'h' has type 'const S', but function is not marked const"
//@ diag: 12:8 note "'h' declared here"
struct S {
  int x;
  mutable int m;
  void h();
  void g() const {
    m = 1;
    x = 2;
  }
};
void k(const S &s) {
  s.m = 3;
  s.h();
}
