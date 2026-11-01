// [dcl.asm], GNU asm goto: the statement has an edge to each of its labels
// as well as to the next statement, so code after a label it names is
// reachable for -Wreturn-type and -Wexceptions (clang CFG GCCAsmStmt
// successors; WP-230)
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wall
//@ diag: 27:3 warning "'h' has a non-throwing exception specification but can still throw"
//@ diag: 23:6 note "function declared non-throwing here"
//@ diag: 34:1 warning "non-void function does not return a value in all control paths"
int f(int x) {
  asm goto("" :::: out);
  return 1;
out:
  return 0;
}
int g(int x) {
  asm goto("" :::: out);
  throw 1;
out:
  return 0;
}
void h() noexcept {
  asm goto("" :::: out);
  return;
out:
  throw 1;
}
int k(int x) {
  asm goto("" :::: out);
  return 1;
out:
  ;
}
