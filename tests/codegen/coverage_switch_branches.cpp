// Coverage branch regions of a switch without `default` (WP-1082,
// spec/codegen.md §5.4 "Instrumentation"): its implicit-default branch is
// (sum of the cases, parent - sum) under clang 20+, and
// (parent - sum, sum) under clang 19 (`VisitSwitchStmt`).
//@ args: -fprofile-instr-generate -fcoverage-mapping -fcoverage-compilation-dir=/cov -Xclang -disable-llvm-passes
int f(int x) {
  int r = 0;
  switch (x) {
  case 1:
    r = 10;
    break;
  case 2:
  case 3:
    r = 20;
    break;
  }
  switch (x) {
  case 4:
    return 4;
  default:
    break;
  }
  return r;
}
