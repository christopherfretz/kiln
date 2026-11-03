// [expr.shift]/2, [conv.bool]/1: a signed `<<` implicitly converted to
// `bool` probably meant a comparison with 0 (-Wint-in-bool-context, in
// -Wall); in every boolean context, through parentheses and the arms of
// a conditional, but not under an explicit cast nor for an unsigned
// shift. clang 22 wording (clang 19: shift-in-bool-context-19.cpp)
// (WP-410).
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
//@ args: -Wint-in-bool-context
//@ diag: 21:9 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 22:13 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 23:11 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 24:9 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 25:18 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 26:7 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 28:18 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
//@ diag: 30:12 warning "converting the result of '<<' to a boolean; did you mean to compare with '0'?"
void g(bool);
bool f(int a, unsigned u, bool c) {
  if (a << 1) return 1;
  while ((a << 2)) {}
  if (!(a << 3)) return 1;
  if (a << 1 && c) return 1;
  bool x = c ? a << 1 : false;
  g(a << 1);
  bool y = static_cast<bool>(a << 1) || (bool)(a << 1);
  if (c ? 1 : -a << 1) return 1;
  if (u << 1) return 1;
  return a << 4 || x || y;
}
