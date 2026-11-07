// [stmt.return], [stmt.pre]: control reaches the closing brace of a body
// only through its last statement, so a body whose last statement is a
// return cannot flow off its end and -Wreturn-type asks for no
// control-flow graph (spec/sema.md §7.5, WP-798). Shown through
// -fkiln-cfg-max-elems: 'ends' would exceed it, yet no remark says its
// analysis was skipped, because nothing built its graph; 'falls' does not
// end in a return, so -Wreturn-type asks for its graph and the remark
// fires.
//@ expect: compile-ok
//@ strict-diags
//@ args: -fkiln-cfg-max-elems=10 -Rkiln-analysis-skipped
//@ skip(clang): kiln-only flags
//@ diag: 20:5 remark "flow-sensitive warnings for 'falls' skipped: its control-flow graph exceeds -fkiln-cfg-max-elems=10"
//@ diag: 26:1 warning "non-void function does not return a value in all control paths"
int ends(int a) {
  int b = a + 1, c = b + 2, d = c + 3;
  if (a) return b;
  return b + c + d;
}
int falls(int a) {
  int b = a + 1, c = b + 2, d = c + 3;
  if (a) return b + c + d;
}
int small(int a) {
  if (a) return 1;
}
