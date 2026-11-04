// [namespace.udecl]/14, [over.match.best]: a derived member function
// with a different parameter-type-list does not hide the using-declared
// base one; with neither conversion better the call is ambiguous. WP-612.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:28 error "call to member function 'f' is ambiguous"
//@ diag: 10:7 note "candidate function"
//@ diag: 14:7 note "candidate function"
struct B {
  int f(long) { return 1; }
};
struct D : B {
  using B::f;
  int f(unsigned) { return 2; }
};
int main() { D d; return d.f(1); }
