//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:18 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 13:18 note "pointer to 'y' is not a constant expression"
//@ diag: 12:7 note "declared here"
//@ diag: 14:18 error "constexpr variable 'q' must be initialized by a constant expression"
//@ diag: 14:18 note "pointer to subobject of 'a' is not a constant expression"
//@ diag: 12:14 note "declared here"
// WP-165: the address of a variable with automatic storage duration is not
// a constant expression ([expr.const]/14), from an enclosing function too.
void f() {
  int y = 0, a[2] = {};
  constexpr int *p = &y;
  constexpr int *q = &a[1];
}
