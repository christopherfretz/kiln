// [expr.new], [dcl.init]/7, [basic.indet]/2: a new-expression without an
// initializer default-initializes, so a scalar has an indeterminate value and
// reading it is not a constant expression; list-initialization
// value-initializes the elements past the initializers.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:15 error "static assertion expression is not an integral constant expression"
//@ diag: 15:11 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 19:15 note "in call to 'scalar()'"
//@ diag: 28:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:11 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 28:15 note "in call to 'array(3)'"
constexpr int scalar() {
  int *p = new int;
  int r = *p;
  delete p;
  return r;
}
static_assert(scalar() == 0);

constexpr int array(int n) {
  int *p = new int[n];
  p[0] = 1;
  int r = p[n - 1];
  delete[] p;
  return r;
}
static_assert(array(3) == 0);

// List-initialization value-initializes the elements without an
// initializer.
constexpr int filled(int n) {
  char *s = new char[n]{'a', 'b'};
  int r = s[n - 1];
  delete[] s;
  return r;
}
static_assert(filled(4) == 0);
