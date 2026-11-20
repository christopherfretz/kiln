//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:22 note "cannot allocate array; evaluated array bound 3 is too small to hold 4 explicitly initialized elements"
//@ diag: 16:15 note "in call to 'too_short(3)'"
// [expr.new]/9: a run-time bound smaller than the string literal's length
// (its terminating '\0' included) makes the new-expression throw
// std::bad_array_new_length, which a constant evaluation does not reach.
constexpr int too_short(int n) {
  char *p = new char[n]{"abc"};
  int r = p[0];
  delete[] p;
  return r;
}
static_assert(too_short(4) == 'a');
static_assert(too_short(3) == 'a');
