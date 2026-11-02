// [over.best.ics.general]/4.5, [over.ics.list]/7: in `h({{1}})` the
// braced argument initializes `H`; its single element `{1}` is itself a
// braced-init-list, so when `H(E)` and the copy/move constructors of H
// are considered, the latter (first parameter a reference to H) do not
// allow user-defined conversions and are not viable. `H(E)` is chosen
// and `{1}` copy-list-initializes `E` with the explicit `E(int)`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:15 error "chosen constructor is explicit in copy-initialization"
//@ diag: 13:12 note "explicit constructor declared here"
//@ diag: 17:6 note "passing argument to parameter here"
struct E {
  explicit E(int);
  E(double);
};
struct H {
  H(E);
};
void h(H);
void b() { h({{1}}); }
