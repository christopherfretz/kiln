// [dcl.attr.grammar]/5 [utility.syn] In a function with the wildcard
// `no_builtin`, `std::move` still folds to its argument's address in
// kiln; clang 19 emits a call of an uninstantiated `std::move` there
// (an undefined reference at link time). spec/codegen.md "no-builtin",
// WP-808.
//@ expect: run
//@ strict-diags
//@ oracle: none
//@ xfail(clang): clang 19 calls the uninstantiated std::move (undefined reference)
#include <utility>
[[clang::no_builtin]] int all_move(int& x) {
  int&& r = std::move(x);
  return r + 1;
}
int main() {
  int v = 41;
  return all_move(v) == 42 ? 0 : 1;
}
