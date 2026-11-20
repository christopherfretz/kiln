// [expr.type.conv]/2, [dcl.init.general]/9, [temp.inst]: `T()` of a
// pointer to a variable length array (a Clang extension) whose bound names
// a function parameter of a template is formed again in each
// specialization: its type is instantiation-dependent though not
// dependent, so the pattern's value-initialization is not shared. Before
// WP-1740 the instantiation stopped with "not yet implemented: substitution
// into CXXScalarValueInitExpr".
//@ expect: run
//@ strict-diags
//@ args: -Wno-vla-cxx-extension
//@ stdout: 3 1 4
#include <cstdio>
template <class T> int value_init(int n) {
  using PA = int (*)[n];
  PA p = PA();
  return p == nullptr ? n : 0;
}
template <class T> int value_init_auto(int n) {
  using PA = int (*)[n];
  auto q = PA();
  bool b = q == nullptr;
  return b;
}
template <class T> T value_init_ret(int n) {
  using PA = int (*)[n];
  return PA() == nullptr ? T(n) : T();
}
int main() {
  int r = value_init<int>(3);
  std::printf("%d ", r);
  r = value_init_auto<int>(2);
  std::printf("%d ", r);
  r = value_init_ret<int>(4);
  std::printf("%d\n", r);
}
