//@ expect: ast
//@ ast-filter: test
//@ std: c++23
//@ requires: clang>=22
// Pinned to C++23: at C++26 the reference e is constant-initialized (P2686R5,
// [expr.const.init]/2) and names of it are non_odr_use_constant; clang 22 lacks that.
// [dcl.struct.bind]/3 (C++26 [dcl.struct.bind]/6): binding an array. The
// unnamed holding variable `e` (DecompositionDecl) copies the array (an
// ArrayInitLoopExpr) for `auto`, or refers to it for the reference forms;
// each binding names `e[i]`, an lvalue of the element type.
int arr[2] = {1, 2};
const long carr[3] = {4, 5, 6};
int test_value() {
  auto [a, b] = arr;
  return a + b;
}
int test_refs() {
  auto &[a, b] = arr;
  const auto &[c, d, e] = carr;
  auto &&[f, g] = arr;
  return a + b + int(c + d + e) + f + g;
}
int test_nested() {
  int m[2][2] = {{1, 2}, {3, 4}};
  auto &[r0, r1] = m;
  return r0[1] + r1[0];
}
