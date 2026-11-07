// [over.match.oper]/3.4: rewritten (`!(a == b)`) and reversed candidates
// are C++20's. Before it, `a != b` considers only `operator!=`, so a base's
// member `operator!=` beside a more-derived base's friend `operator==` is
// not ambiguous (LLVM's `iterator_facade_base` / `iterator_adaptor_base`;
// clang `CreateOverloadedBinOp`), and `a == b` with only the reversed
// order declared does not find it.
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 1 0 1
#include <cstdio>
template <class D> struct Facade {
  bool operator!=(const D &r) const { return !(static_cast<const D &>(*this) == r); }
};
template <class D, class W> struct Adaptor : Facade<D> {
  W I;
  Adaptor(W u) : I(u) {}
  friend bool operator==(const Adaptor &l, const Adaptor &r) { return l.I == r.I; }
};
template <class It> struct Mapped : Adaptor<Mapped<It>, It> {
  Mapped(It i) : Adaptor<Mapped<It>, It>(i) {}
};
template <class I> bool ne(I a, I b) { return a != b; }
struct A {};
struct B { int v; };
bool operator==(const B &b, const A &) { return b.v == 0; }
template <class T> auto eq(int) -> decltype(T() == B{0}, true) { return true; }
template <class T> bool eq(...) { return false; }
int main() {
  const char *s = "ab";
  Mapped<const char *> a(s), b(s + 2), c(s);
  std::printf("%d %d %d\n", ne(a, b), ne(a, c), !eq<A>(0));
}
