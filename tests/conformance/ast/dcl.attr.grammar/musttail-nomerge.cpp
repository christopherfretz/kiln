//@ expect: ast
//@ ast-filter: f
// [dcl.attr.grammar] (WP-1816): clang's `[[clang::musttail]]` and `[[clang::nomerge]]`
// statement attributes are kept in an AttributedStmt and dump with clang's class names
// (MustTailAttr, NoMergeAttr).
int g(int);
int f(int x) {
  [[clang::nomerge]] g(x);
  [[clang::musttail]] return g(x + 1);
}
