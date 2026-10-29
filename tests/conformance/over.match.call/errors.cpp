// [over.match.viable], [over.match.best]/3: no viable function and an
// ambiguous call are errors, with a note per candidate.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:3 error "no matching function for call to 'f'"
//@ diag: 25:6 note "candidate function not viable: requires single argument 'x', but no arguments were provided"
//@ diag: 24:6 note "candidate function not viable: requires 1 argument, but 0 were provided"
//@ diag: 23:6 note "candidate function not viable: requires 2 arguments, but 0 were provided"
//@ diag: 34:3 error "no matching function for call to 'f'"
//@ diag: 23:6 note "candidate function not viable: requires 2 arguments, but 3 were provided"
//@ diag: 25:6 note "candidate function not viable: requires single argument 'x', but 3 arguments were provided"
//@ diag: 24:6 note "candidate function not viable: requires 1 argument, but 3 were provided"
//@ diag: 35:3 error "no matching function for call to 'f'"
//@ diag: 23:6 note "candidate function not viable: no known conversion from 'std::nullptr_t' to 'int' for 1st argument"
//@ diag: 25:6 note "candidate function not viable: requires single argument 'x', but 2 arguments were provided"
//@ diag: 24:6 note "candidate function not viable: requires 1 argument, but 2 were provided"
//@ diag: 36:3 error "call to 'g' is ambiguous"
//@ diag: 26:6 note "candidate function"
//@ diag: 27:6 note "candidate function"
//@ diag: 37:6 error "no matching member function for call to 'm'"
//@ diag: 29:8 note "candidate function not viable: 'this' argument has type 'const S', but method is not marked const"
//@ diag: 30:8 note "candidate function not viable: requires 2 arguments, but 1 was provided"
void f(int, int);
void f(const char *);
void f(double x);
void g(long);
void g(unsigned);
struct S {
  void m(int);
  void m(int, int) const;
};
void h(const S &cs) {
  f();
  f(1, 2, 3);
  f(nullptr, 1);
  g(1);
  cs.m(1);
}
