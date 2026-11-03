// [stmt.dcl], [temp.inst] (clang `TransformDeclRefExpr` /
// `MarkDeclRefReferenced`): a non-dependent reference that an
// instantiated generic lambda (or explicit-object lambda) body shares
// with its pattern still counts as a reference for
// -Wunused-but-set-variable, so a captured variable assigned in the body
// is not diagnosed; the lambda's own and the instantiated function's
// assigned-only locals are.
//@ args: -Wunused-but-set-variable -Wunused-variable
//@ std: c++23
//@ strict-diags
//@ diag: 27:9 warning "variable 'q' set but not used"
//@ diag: 34:7 warning "variable 'q' set but not used"
int f1() {
  int r = 0;
  auto l = [&r](auto, int v) { r += v; };
  l(1, 2);
  return 0;
}
int f2() {
  int r = 0;
  auto l = [&](this auto const& self, int v) { r = v; };
  l(2);
  return 0;
}
int f3() {
  auto l = [](auto) {
    int q = 0;
    q = 1;
  };
  l(1);
  return 0;
}
template <class T> int f4() {
  int q = 0;
  q = 1;
  return 0;
}
int f5() { return f4<int>(); }
