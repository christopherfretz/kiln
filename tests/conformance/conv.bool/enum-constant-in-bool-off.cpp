// [expr.log.and]/1, [conv.bool]/1: with -Wint-in-bool-context off (its
// default), an enumerator operand of `&&` whose value is not 0 or 1 still
// keeps -Wconstant-logical-operand quiet; another constant right operand
// draws it (WP-410).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 13:12 warning "use of logical '&&' with constant operand"
//@ diag: 13:12 note "use '&' for a bitwise operation"
//@ diag: 13:12 note "remove constant to silence this warning"
enum E { EK = 4 };
bool f(int i) {
  if (i && EK) return 1;
  return i && 4;
}
