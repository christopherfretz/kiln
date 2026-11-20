// [expr.cond]/7, [class.temporary]/4: when one arm of a class-typed
// conditional is a glvalue and the other a prvalue, the glvalue is
// copied into a prvalue temporary, which is destroyed at the end of the
// full-expression like any other (kiln left the copy unbound and never
// destroyed it). Also for GNU `x ?: y`. WP-1738.
//@ expect: run
//@ stdout: 2 1|3 2|5 4 2|
#include <cstdio>
static int c, d;
struct S {
  int i;
  S(int i) : i(i) { ++c; }
  S(const S &o) : i(o.i) { ++c; }
  ~S() { ++d; }
  explicit operator bool() const { return i; }
};
S make(int i) { return S(i); }
struct K {
  int n = 0;
  constexpr K() = default;
  constexpr K(const K &) : n(1) {}
  constexpr ~K() {}
};
constexpr int copies(bool b) {
  K k;
  int r = (b ? k : K()).n;
  return r;
}
static_assert(copies(true) == 1);
static_assert(copies(false) == 0);
int main() {
  S a(1);
  bool b = true;
  (void)(b ? a : make(9));
  std::printf("%d %d|", c, d);
  (void)(a ?: make(9));
  std::printf("%d %d|", c, d);
  int n = (a ?: make(9)).i + (b ? a : make(2)).i;
  std::printf("%d %d %d|\n", c, d, n);
}
