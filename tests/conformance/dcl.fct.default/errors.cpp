// [dcl.fct.default]: too few arguments, ambiguity introduced by a
// default argument, and a redefined default.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:19 error "redefinition of default argument"
//@ diag: 15:19 note "previous definition is here"
//@ diag: 20:3 error "call to 'f' is ambiguous"
//@ diag: 13:6 note "candidate function"
//@ diag: 14:6 note "candidate function"
//@ diag: 21:3 error "no matching function for call to 'h'"
//@ diag: 17:6 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 18:6 note "candidate function not viable: requires at least 2 arguments, but 1 was provided"
void f(int, int = 0);
void f(int);
void g(int a, int b = 1);
void g(int a, int b = 2);
void h(int, int);
void h(double, int, int = 0);
void t() {
  f(1);
  h(1);
}
