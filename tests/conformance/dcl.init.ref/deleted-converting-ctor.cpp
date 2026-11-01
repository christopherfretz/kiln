// [dcl.init.ref]/5.4.1, [dcl.fct.def.delete]/2: a reference temporary or argument created by a deleted
// converting constructor uses a deleted function
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:10 error "attempt to use a deleted function"
//@ diag: 10:12 note "'G' has been explicitly marked deleted here"
//@ diag: 13:14 error "attempt to use a deleted function"
//@ diag: 10:12 note "'G' has been explicitly marked deleted here"
struct X {};
struct G { G(int) = delete; G(X) = delete; };
G&& g2 = 1;
void f(const G&);
void t() { f(1); }
