// [basic.scope.scope]/6 (P2169): two declarations potentially conflict
// "unless B is name-independent", whatever A is. So a name-independent
// `_` (a non-static data member, a block-scope automatic variable) may
// follow a member function, a parameter or a block-scope function named
// `_`; [basic.scope.block]/2 exempts it from the parameter rule too.
// GCC 16.1 accepts all of these; clang 22 reports each as a duplicate or
// a redefinition. Found running GCC's C++ testsuite.
//@ xfail(clang): clang 22 treats a name-independent declaration after a non-variable `_` as a redefinition
//@ oracle: gcc
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
struct S {
  void _();
  int _;
  int _;
};

int bar(int _ = 0) {
  int _ = 1;
  return 0;
}

void h() {
  void _();
  int _ = 3;
}

int main() { return bar(); }
