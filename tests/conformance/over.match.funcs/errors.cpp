// [over.match.funcs]/4-5: a candidate whose implicit object parameter
// cannot bind the object is not viable; the note names the object's
// qualifiers or value category.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:6 error "no matching member function for call to 'f'"
//@ diag: 22:8 note "candidate function not viable: 'this' argument has type 'const S', but method is not marked const"
//@ diag: 23:8 note "candidate function not viable: requires 1 argument, but 0 were provided"
//@ diag: 33:6 error "no matching member function for call to 'f'"
//@ diag: 23:8 note "candidate function not viable: 'this' argument has type 'const S', but method is not marked const"
//@ diag: 22:8 note "candidate function not viable: requires 0 arguments, but 1 was provided"
//@ diag: 34:7 error "no matching member function for call to 'g'"
//@ diag: 24:8 note "candidate function not viable: expects an lvalue for object argument"
//@ diag: 25:8 note "candidate function not viable: requires 1 argument, but 0 were provided"
//@ diag: 35:5 error "no matching member function for call to 'h'"
//@ diag: 26:8 note "candidate function not viable: expects an rvalue for object argument"
//@ diag: 27:8 note "candidate function not viable: requires 1 argument, but 0 were provided"
//@ diag: 36:6 error "no matching member function for call to 'm'"
//@ diag: 28:8 note "candidate function not viable: 'this' argument has type 'const S', but method is not marked const"
//@ diag: 29:8 note "candidate function not viable: 'this' argument has type 'const S', but method is not marked const"
struct S {
  void f();
  void f(int);
  void g() &;
  void g(int) &;
  void h() &&;
  void h(int) &&;
  void m(int) volatile;
  void m(long) volatile;
};
void t(S &s, const S &cs, S &&rs) {
  cs.f();
  cs.f(1);
  S().g();
  s.h();
  cs.m(1);
}
