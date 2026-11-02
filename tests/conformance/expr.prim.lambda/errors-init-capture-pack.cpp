// [expr.prim.lambda.capture]/17, [temp.variadic]/5: the initializer of an
// init-capture pack must name an unexpanded pack, and the pack's name is
// not reused by another capture.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:13 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 12:14 error "pack expansion does not contain any unexpanded parameter packs"
//@ diag: 15:25 error "'xs' can appear only once in a capture list"
void f() {
  auto a = [...xs = 1] { return 0; };
  int n = 0;
  auto b = [&...ys = n] {};
}
template <class... T> void g(T... ts) {
  auto d = [...xs = ts, xs = 0] {};
}
