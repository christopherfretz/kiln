// [expr.new]/7, [dcl.init.general]/16.5 (P0960): the parenthesized
// aggregate initialization of an array `new` (`new int[2](1, 2)`, an
// omitted bound `new int[](1, 2, 3)`, a nested array `new R[2][2]({R(4)})`)
// is a `CXXParenListInitExpr` whose written extent (spec/ast.md §21) is
// its `(` through its `)`, as clang. `cargo xtask extents` compares it
// with clang (WP-2463).
//@ std: c++20
//@ expect: compile-ok

struct R {
  int v;
};

int *two() { return new int[2](1, 2); }
int *three() { return new int[](1, 2, 3); }
R (*rows())[2] { return new R[2][2]({R(4)}); }
constexpr int sum() {
  int *p = new int[3](4, 5);
  int s = p[0] + p[1] + p[2];
  delete[] p;
  return s;
}
static_assert(sum() == 9);
