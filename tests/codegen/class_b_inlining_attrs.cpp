// WP-813: the inlining and call-site attributes: flatten
// (`flatten` on the definition), leaf (`nocallback`), nomerge,
// not_tail_called (`notail` calls), noduplicate, convergent, nouwtable
// (no `uwtable`), and the statement forms `[[clang::nomerge]]`,
// `[[clang::noinline]]`, `[[clang::always_inline]]` on calls
// (CGCall.cpp ConstructAttributeList, CGStmt.cpp attributed statements).
// spec/landed/codegen/WP-813.md.
void g(); int h(int);
__attribute__((flatten)) void f1() { g(); h(1); }
__attribute__((leaf)) void f2();
__attribute__((nomerge)) void f3();
__attribute__((not_tail_called)) void f4();
__attribute__((noduplicate)) void f5();
__attribute__((convergent)) void f6();
__attribute__((nouwtable)) void f7() {}
[[clang::always_inline]] inline void f8() {}
[[clang::noinline]] void f9() {}
void use() { f2(); f3(); f4(); f5(); f6(); f8(); f9(); [[clang::nomerge]] g(); [[clang::noinline]] g(); [[clang::always_inline]] f9(); }
