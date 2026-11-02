// [over.match.list]/1, [dcl.init.list]/3.7, [expr.call]/7: a braced
// argument copy-list-initializes a non-aggregate class parameter; when the
// constructor chosen is explicit (or deleted) the initialization is
// ill-formed, and the error is followed by the note naming the parameter
// (clang PrintInitLocationNote), for free and member functions alike.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:14 error "chosen constructor is explicit in copy-initialization"
//@ diag: 18:21 note "explicit constructor declared here"
//@ diag: 19:10 note "passing argument to parameter 'e' here"
//@ diag: 25:17 error "chosen constructor is explicit in copy-initialization"
//@ diag: 18:21 note "explicit constructor declared here"
//@ diag: 24:13 note "passing argument to parameter 'z' here"
//@ diag: 29:17 error "call to deleted constructor of 'C'"
//@ diag: 27:12 note "'C' has been explicitly marked deleted here"
//@ diag: 28:14 note "passing argument to parameter here"

struct E { explicit E(int); E(double); };
void f(E e);
void f2(E e);
void g() { f({1}); f2({1.0}); }

struct K {
  void mf(E z);
  void o() { mf({4}); }
};
struct C { C(int) = delete; C(double); };
void p(int, C);
void q() { p(0, {1}); }
