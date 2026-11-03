// [expr.comma] (WP-458): clang `isUnusedResultAWarning` does not warn for
// a discarded comma expression whose right operand is the integer literal
// 0, possibly parenthesized (`(x = y, 0)`, the idiom for hiding an
// assignment's result in a macro); any other right operand (`1`, `'\0'`)
// is "expression result unused" as before, the left operand is still
// checked on its own ("left operand of comma operator has no effect"), and
// a nodiscard call on either side is reported. `(void)(r = 1, 0)` is
// silent.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 24:10 warning "expression result unused"
//@ diag: 24:20 warning "expression result unused"
//@ diag: 25:10 warning "left operand of comma operator has no effect"
//@ diag: 25:23 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 26:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 26:8 warning "ignoring return value of function declared with 'nodiscard' attribute"
[[nodiscard]] int f();
[[nodiscard]] int g();
#define SET(a, b) ((a) = (b), 0)
void h() {
  int r;
  r = 1, 0; r = 1, (0); r = 1, 0L; r = 1, 0x0; (r = 1, 0);
  (void)(r = 1, 0); SET(r, 2);
  r = 1, 1; r = 1, '\0';
  r = 1, 2, 0; r = 2, f(), 0;
  f(), g();
}
