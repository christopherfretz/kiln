//@ expect: compile-fail
//@ strict-diags
//@ args: -std=gnu++23
//@ diag: 19:15 error "static assertion expression is not an integral constant expression"
//@ diag: 16:15 note "subexpression not valid in a constant expression"
//@ diag: 19:15 note "in call to 'range(2)'"
//@ diag: 24:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:10 note "subexpression not valid in a constant expression"
//@ diag: 24:15 note "in call to 'lam(2)'"
// A variable length array bound (GNU extension) is never a constant: the
// end of a range-based for over one and a lambda capturing its bound
// fail constant evaluation where clang does (at the `:`, at the lambda).
constexpr int range(int n) {
  int a[n];
  int c = 0;
  for (int &x : a) { x = 1; ++c; }
  return c;
}
static_assert(range(2) == 2);
constexpr int lam(int n) {
  int a[n];
  return [&] { return (int)sizeof a; }();
}
static_assert(lam(2) == 8);
