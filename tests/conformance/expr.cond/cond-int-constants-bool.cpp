// [expr.cond]/7, [conv.bool]/1: a conditional whose arms are both integer
// literals (a literal under a unary minus counts), not both 0 or 1, and
// both non-zero, converted to `bool` is always true
// (-Wtautological-constant-compare, on by default), reported at the
// condition; a conditional arm converted to `bool` is checked in turn.
// Character, parenthesized, enumerator, `bool` or converted literal arms
// do not count (WP-410).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 19:7 warning "converting the result of '?:' with integer constants to a boolean always evaluates to 'true'"
//@ diag: 20:7 warning "converting the result of '?:' with integer constants to a boolean always evaluates to 'true'"
//@ diag: 21:7 warning "converting the result of '?:' with integer constants to a boolean always evaluates to 'true'"
//@ diag: 22:9 warning "converting the result of '?:' with integer constants to a boolean always evaluates to 'true'"
//@ diag: 23:17 warning "converting the result of '?:' with integer constants to a boolean always evaluates to 'true'"
//@ diag: 24:13 warning "converting the result of '?:' with integer constants to a boolean always evaluates to 'true'"
//@ diag: 26:12 warning "converting the result of '?:' with integer constants to a boolean always evaluates to 'true'"
enum E { EK = 3 };
bool f(bool c, bool d, int k) {
  if (c ? 1 : 2) return 1;
  if (c ? -1 : -2) return 1;
  if (c ? 2L : 3L) return 1;
  if (!(c ? 3 : 1)) return 1;
  bool x = c ? (d ? 3 : 2) : k;
  bool y = (c ? 2 : 3) && d;
  bool z = c ? 4 : (5);
  int i = (c ? 2 : 3) ? 1 : 2;
  if (c ? 2 : 0) return 1;
  if (c ? 0 : 3) return 1;
  if (c ? 1 : 0) return 1;
  if (c ? 1 : 1) return 1;
  if (c ? 2 : 3u) return 1;
  if (c ? 'a' : 3) return 1;
  if (c ? (2) : 3) return 1;
  if (c ? EK : 3) return 1;
  if (c ? true : 2) return 1;
  if (c ? k : 3) return 1;
  return x && y && z && i && (bool)(c ? 2 : 3);
}
