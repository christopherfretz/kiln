// [temp.deduct.call]: a member template whose deduction fails is not viable.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:17 error "no matching member function for call to 'f'"
//@ diag: 7:27 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'double"
struct S {
  template <class T> void f(T, T);
};
void g(S s) { s.f(1, 2.0); }
