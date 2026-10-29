// [over.match.viable]/2: a candidate needs a parameter for every argument
// (or an ellipsis) and an argument or default argument for every
// parameter; the notes give the arity each candidate expects.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:3 error "no matching function for call to 'f'"
//@ diag: 20:6 note "candidate function not viable: requires 1 argument, but 0 were provided"
//@ diag: 21:6 note "candidate function not viable: requires 2 arguments, but 0 were provided"
//@ diag: 22:6 note "candidate function not viable: requires at least 3 arguments, but 0 were provided"
//@ diag: 29:3 error "no matching function for call to 'f'"
//@ diag: 22:6 note "candidate function not viable: requires at most 4 arguments, but 5 were provided"
//@ diag: 21:6 note "candidate function not viable: requires 2 arguments, but 5 were provided"
//@ diag: 20:6 note "candidate function not viable: requires 1 argument, but 5 were provided"
//@ diag: 30:3 error "no matching function for call to 'g'"
//@ diag: 23:6 note "candidate function not viable: requires at least 1 argument, but 0 were provided"
//@ diag: 24:6 note "candidate function not viable: requires 3 arguments, but 0 were provided"
//@ diag: 31:3 error "no matching function for call to 'h'"
//@ diag: 26:6 note "candidate function not viable: requires at most 3 arguments, but 4 were provided"
//@ diag: 25:6 note "candidate function not viable: requires 0 arguments, but 4 were provided"
void f(int);
void f(int, int);
void f(int, int, int, int = 0);
void g(int, ...);
void g(int, int, int);
void h();
void h(int, int = 1, int = 2);
void t() {
  f();
  f(1, 2, 3, 4, 5);
  g();
  h(1, 2, 3, 4);
}
