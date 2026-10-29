// [over.ics.list]: braced-init-list arguments with no conversion.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:3 error "no matching function for call to 'f'"
//@ diag: 12:6 note "candidate function not viable: cannot convert initializer list argument to 'int'"
//@ diag: 13:6 note "candidate function not viable: cannot convert initializer list argument to 'char *'"
//@ diag: 21:3 error "no matching function for call to 'g'"
//@ diag: 17:6 note "candidate function not viable: cannot convert initializer list argument to 'S'"
//@ diag: 23:6 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 23:6 note "insert an explicit cast to silence this issue"
//@ diag: 23:5 warning "braces around scalar initializer"
void f(int);
void f(char *);
struct S {
  S(int, int);
};
void g(S);
void h(int);
void t() {
  f({1, 2});
  g({1});
  double d = 1.5;
  h({d});
}
