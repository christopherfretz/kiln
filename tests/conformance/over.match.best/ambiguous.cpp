// [over.match.best]/2-3: no function is better than all others; the
// ambiguity diagnostic notes every tied candidate.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:3 error "call to 'f' is ambiguous"
//@ diag: 17:6 note "candidate function"
//@ diag: 18:6 note "candidate function"
//@ diag: 31:3 error "call to 'g' is ambiguous"
//@ diag: 19:6 note "candidate function"
//@ diag: 20:6 note "candidate function"
//@ diag: 32:3 error "call to 'h' is ambiguous"
//@ diag: 25:6 note "candidate function"
//@ diag: 26:6 note "candidate function"
//@ diag: 33:3 error "call to 'k' is ambiguous"
//@ diag: 27:6 note "candidate function"
//@ diag: 28:6 note "candidate function"
void f(int, long);
void f(long, int);
void g(long);
void g(double);
struct A {};
struct B : A {};
struct C : A {};
struct D : B, C {};
void h(B *);
void h(C *);
void k(const int &);
void k(int);
void t(D *d) {
  f(1, 2);
  g(1);
  h(d);
  k(1);
}
