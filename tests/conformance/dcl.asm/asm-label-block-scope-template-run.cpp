// [dcl.asm] (GNU asm labels), [temp.inst], [basic.link]/7: a block-scope
// `extern` function or variable declaration in a template keeps its asm
// label in every instantiation (clang `InstantiateAttrs` copies the
// `AsmLabelAttr`). Reduced from real code: before WP-1797 the label was
// dropped and the instantiation referenced `_Z1qi` and `v`, which do not
// exist, so the program did not link.
//@ expect: run
//@ stdout: 42 43
#include <cstdio>
int impl_q(int x) asm("kiln_block_q");
int impl_q(int x) { return x + 1; }
int impl_v asm("kiln_block_v") = 40;
template <class T> int f(T t) {
  extern int q(int) asm("kiln_block_q");
  extern int v asm("kiln_block_v");
  return q(int(t)) + v;
}
int main() {
  std::printf("%d ", f(1));
  std::printf("%d\n", f(2L));
}
