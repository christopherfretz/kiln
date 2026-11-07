// [class.abstract]/2, [temp.res.general]/6: in a class template whose
// base is dependent, `void w(int) override = 0;` may redeclare a virtual
// function of that base, so whether it is virtual (and may be pure) is
// known only in an instantiation (clang `CheckPureMethod` skips a
// dependent class; the instantiation is declared pure again and checked
// then). The shape of folly's `WriteChainAsyncTransportWrapper<T>`. WP-726.
//@ std: c++20
//@ expect: run
//@ stdout: 5
#include <cstdio>
struct B {
  virtual void w(int) = 0;
  virtual ~B() = default;
};
template <class T> struct D : T {};
template <class T> struct W : D<T> {
  void w(int) override = 0;
};
template <class T> struct V {
  void g() = 0; // never instantiated: not diagnosed
};
static_assert(__is_abstract(W<B>));
struct Impl : W<B> {
  void w(int x) override { std::printf("%d\n", x); }
};
int main() {
  Impl i;
  B& b = i;
  b.w(5);
}
