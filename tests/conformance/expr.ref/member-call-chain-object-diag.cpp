// [expr.ref], [over.match.funcs]/5: the object argument of the last call of
// a long member-call chain that drops a qualifier or binds an lvalue to `&&`
// is diagnosed at the chain's first token (WP-2201: the location is found
// only for the diagnostic).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:3 error "'this' argument to member function 'g' has type 'const X', but function is not marked const"
//@ diag: 15:8 note "'g' declared here"
//@ diag: 22:3 error "'this' argument to member function 'h' is an lvalue, but function has rvalue ref-qualifier"
//@ diag: 16:8 note "'h' declared here"
//@ diag: 23:3 error "'this' argument to member function 'g' has type 'const X', but function is not marked const"
//@ diag: 15:8 note "'g' declared here"
struct X {
  const X& f() const;
  void g();
  void h() &&;
  X& m();
  X* p() const;
};
void t(const X& x, X& y, const X* q) {
  x.f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().g();
  y.m().m().m().m().m().m().m().m().m().m().m().m().m().m().m().m().m().m().m().m().h();
  q->f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().f().g();
}
