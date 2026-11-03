// [expr.log.and]/1, [expr.log.or]/1, [conv.bool]/1: an operand of `&&` or
// `||` naming an enumerator whose value is not 0 or 1 is reported
// (-Wint-in-bool-context, in -Wall) at the operator, and then
// -Wconstant-logical-operand is not; a parenthesized enumerator or one
// with value 0 or 1 is not. The enumerator still silences
// -Wconstant-logical-operand when -Wint-in-bool-context is off
// (enum-constant-in-bool-off.cpp) (WP-410).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wall
//@ diag: 18:15 warning "converting the enum constant to a boolean"
//@ diag: 19:10 warning "converting the enum constant to a boolean"
//@ diag: 20:9 warning "converting the enum constant to a boolean"
//@ diag: 21:10 warning "converting the enum constant to a boolean"
enum E { EA, EB, EK = 4 };
enum class S { K = 2 };
bool f(E e, bool c, int i) {
  if (e == EA || EK) return 1;
  if (EK && c) return 1;
  if (i && EK) return 1;
  if (EK || EB) return 1;
  if (EB && c) return 1;
  if ((EK) && c) return 1;
  return !EK || c;
}
