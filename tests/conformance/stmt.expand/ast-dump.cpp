//@ expect: ast
//@ ast-filter: test
// [stmt.expand]: the dump of an expansion statement (spec/ast-dump.md):
// `CXXExpansionStmt <form> [has_init] [expanded]` with the
// init-statement, the for-range-declaration and the expansion-initializer
// of its pattern, then the expansions, each a compound statement holding
// its element's declaration and its copy of the body. The golden is
// kiln's own: clang 19 has no expansion statements, and the clang-ast
// normalizer does not map clang 23's CXXExpansionStmt nodes yet.
//@ requires: clang>=23
void g(int);
void test_enum() {
  template for (auto x : {1, 2}) { g(x); }
}
template <class T> void test_dep(T t) {
  template for (int i = 0; auto x : t) { g(x + i); }
}
