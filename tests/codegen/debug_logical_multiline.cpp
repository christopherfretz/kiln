// Debug info (WP-941): a `&&`/`||` split across lines. clang
// `VisitBinLAnd`/`VisitBinLOr` emit the right-hand side's branch into the
// merge block with no location and give the phi an artificial line-0
// location, so gcov does not count the operator's line again for every
// right-hand evaluation.
//@ args: -g
bool land(int e) {
  return e >= 2 &&
         e <= 3;
}
bool lor(int e) {
  return e < 0 ||
         e > 9;
}
bool mixed(int a, int b) {
  return a < b ||
         (a == b &&
          a < 3);
}
int use(int x) { return land(x) + lor(x) + mixed(x, x); }
