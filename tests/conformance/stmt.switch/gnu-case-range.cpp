// GNU case ranges (`case lo ... hi`, clang `Sema::ActOnFinishSwitchStmt`):
// an empty range is warned about and dropped; a range overlapping an
// earlier case value or range is a duplicate (the reported value is the
// largest overlapping case value, or the previous range's upper bound).
// The extension is diagnosed under -Wgnu-case-range (here via -pedantic).
//@ expect: compile-fail
//@ strict-diags
//@ args: -pedantic
//@ diag: 29:10 warning "use of GNU case range extension"
//@ diag: 30:10 warning "use of GNU case range extension"
//@ diag: 31:11 warning "use of GNU case range extension"
//@ diag: 33:11 warning "use of GNU case range extension"
//@ diag: 35:11 warning "use of GNU case range extension"
//@ diag: 30:8 warning "empty case range specified"
//@ diag: 29:8 error "duplicate case value '3'"
//@ diag: 34:8 note "previous case defined here"
//@ diag: 31:8 error "duplicate case value '15'"
//@ diag: 32:8 note "previous case defined here"
//@ diag: 33:8 error "duplicate case value '20'"
//@ diag: 31:8 note "previous case defined here"
//@ diag: 40:23 warning "use of GNU case range extension"
//@ diag: 40:21 warning "empty case range specified"
//@ diag: 43:31 note "in instantiation of function template specialization 'g<9, 8>' requested here"
//@ diag: 40:21 error "duplicate case value '7'"
//@ diag: 43:44 note "in instantiation of function template specialization 'g<5, 9>' requested here"
//@ diag: 40:45 note "previous case defined here"
int f(int x) {
  switch (x) {
  case 1 ... 3: return 1;
  case 5 ... 4: return 2;
  case 10 ... 20: return 3;
  case 15: return 4;
  case 18 ... 30: return 5;
  case 3: return 6;
  case 40 ... 40: return 7;
  }
  return 0;
}
template <int L, int H> int g(int x) {
  switch (x) { case L ... H: return 1; case 7: return 2; }
  return 0;
}
int h() { return g<1, 5>(2) + g<9, 8>(3) + g<5, 9>(1); }
