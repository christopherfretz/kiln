// [over.best.ics.general]/4.5, [over.ics.list]/7: in `H hh({1})` the
// single element `1` of the braced argument is not itself a
// braced-init-list, so the copy and move constructors of H are not
// restricted to standard conversions: `{1}` converts to `H` through
// `H(E)` as well as to `E`, each a user-defined conversion sequence,
// and the three constructors are ambiguous.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:14 error "call to constructor of 'H' is ambiguous"
//@ diag: 17:8 note "candidate constructor (the implicit copy constructor)"
//@ diag: 17:8 note "candidate constructor (the implicit move constructor)"
//@ diag: 18:3 note "candidate constructor"
struct E {
  explicit E(int);
  E(double);
};
struct H {
  H(E);
};
void a() { H hh({1}); }
