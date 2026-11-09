// clang's -Wmain warnings on `main` itself (WP-1040): one parameter, and a
// `bool` literal returned (at `return`; not a parenthesized one, not from
// another function). Both clang 19 and 22.
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
//@ diag: 10:5 warning "only one parameter on 'main' declaration"
//@ diag: 11:17 warning "bool literal returned from 'main'"
bool g() { return true; }
int main(int argc) {
  if (argc > 5) return false;
  if (argc > 4) return (true);
  if (argc > 3) return 1 == 1;
  return g() ? 0 : 1;
}
