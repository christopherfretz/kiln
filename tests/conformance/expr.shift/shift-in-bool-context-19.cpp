// [expr.shift]/2, [conv.bool]/1: at -fclang-compat=19 the
// -Wint-in-bool-context `<<` warning prints the shift as source text in
// its suggested comparison (clang 22: shift-in-bool-context-22.cpp)
// (WP-410).
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang<22
//@ args: -Wall
//@ diag: 13:9 warning "converting the result of '<<' to a boolean; did you mean '(a << 1) != 0'?"
//@ diag: 14:13 warning "converting the result of '<<' to a boolean; did you mean '(a << 2) != 0'?"
//@ diag: 15:13 warning "converting the result of '<<' to a boolean; did you mean '(-a << (3)) != 0'?"
bool f(int a, bool c) {
  if (a << 1) return 1;
  while ((a << 2)) {}
  return -a << (3) || c;
}
