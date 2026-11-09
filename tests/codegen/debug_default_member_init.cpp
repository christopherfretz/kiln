// Debug info (WP-941): a default member initializer used by a constructor
// ([class.base.init]/9) is located at the member, not at the constructor
// (clang `CXXCtorInitializer::getSourceLocation`); gcov line counts and
// debugger stepping depend on it.
//@ args: -g
struct S {
  int n = 7;
  int m{n + 1};
  S() {}
  S(int k) : n(k) {}
};
int use() {
  S a, b(2);
  return a.m + b.m;
}
