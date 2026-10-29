// [basic.lookup.argdep]: hidden friends are found only by ADL;
// ambiguities between ordinary and ADL candidates; ADL suppressed by
// parentheses and qualification.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:3 error "use of undeclared identifier 'hf'"
//@ diag: 21:4 error "use of undeclared identifier 'hf'"
//@ diag: 22:15 error "no member named 'hf' in namespace 'N'"
//@ diag: 24:3 error "call to 'f' is ambiguous"
//@ diag: 16:6 note "candidate function"
//@ diag: 18:6 note "candidate function"
namespace N {
struct A {
  friend void hf(A);
};
void f(A, long);
} // namespace N
void f(N::A, int);
void t(N::A a) {
  hf(1);
  (hf)(a);
  int i = (N::hf(a), 0);
  f(a, 'c');
  f(a, 1.0);
}
