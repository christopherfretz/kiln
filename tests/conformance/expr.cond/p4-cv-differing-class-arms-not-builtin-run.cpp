// [expr.cond]/4.3.1, /7: a `const B` lvalue and a `B` prvalue of a class
// with `operator bool` unify by /4 (the prvalue converts to `const B`), so
// the result is a `B` prvalue copied from the chosen arm, not a `bool`
// from the [expr.cond]/6 built-in candidates. Reduced from clang's
// Parse/ParseDecl.cpp: `(c ? Diag(L, X) << A : Diag(L, Y)) << FixItHint(...)`,
// where `DiagnosticBuilder::operator<<` returns `const DiagnosticBuilder &`.
//@ expect: run
//@ stdout: 12 2 1
#include <cstdio>

struct B {
  int id;
  int n = 0;
  explicit B(int i) : id(i) {}
  B(const B &o) : id(o.id), n(o.n + 1) {}
  operator bool() const { return id != 0; }
  const B &operator<<(int) const { return *this; }
};
const B &operator<<(const B &b, const char *) { return b; }

int main() {
  B a = (true ? B(12) << 1 : B(13)) << "x";
  B c = false ? B(1) << 1 : B(2);
  std::printf("%d %d %d\n", a.id, c.id, a.n >= 1);
}
