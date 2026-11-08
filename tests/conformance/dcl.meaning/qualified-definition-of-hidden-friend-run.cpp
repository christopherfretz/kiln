// [dcl.meaning.general]/2.3, [namespace.memdef]/3: a function first
// declared by a friend declaration is a member of the innermost enclosing
// namespace. Qualified lookup does not find it, but it is nominable
// there, so a qualified definition `void N::f(C &) {...}` outside the
// namespace redeclares the friend: it is not an error, and it has the
// friend's access. Reduced from clang's unittests/Basic/DiagnosticTest.cpp:
// `void clang::DiagnosticsTestHelper(DiagnosticsEngine &diag) {...}`.
//@ expect: run
//@ stdout: 7 7 8
#include <cstdio>

namespace N {
class C {
  int hidden = 7;
  friend void helper(C &);
  friend int twice(const C &, int);
};
int twice(const C &c, int k);
} // namespace N

using namespace N;

void N::helper(C &c) {
  std::printf("%d ", c.hidden);
  ++c.hidden;
}

int N::twice(const C &c, int k) { return c.hidden * k; }

int main() {
  C c;
  N::helper(c);
  std::printf("%d %d\n", N::twice(c, 1) - 1, twice(c, 1));
}
