// [expr.ref]/7: `E1.E2` has a meaning only when `E2` names a data
// member, a member function, an enumerator or a nested type (diagnosed
// as a type member); a member class template or member alias template
// is none of these. clang's catch-all "cannot refer to member" error
// with the note at the member (WP-1745).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:5 error "cannot refer to member 'CT' in 'S' with '.'"
//@ diag: 14:28 note "member 'CT' declared here"
//@ diag: 23:6 error "cannot refer to member 'AT' in 'S' with '->'"
//@ diag: 15:21 note "member 'AT' declared here"

struct S {
  template<class T> struct CT {};
  template<class T> using AT = T;
  int x;
};

void f() {
  S s;
  S* p = &s;
  s.CT;
  p->AT;
  (void)s.x;
}
