// [class.base.init]/1, [temp.variadic]/5.4: a mem-initializer followed
// by `...` is a pack expansion of base-initializers, one per element,
// in a constructor template (`Bs(u)...`), with a braced initializer, and
// when the expanded pack is the member class template's own while the
// enclosing class template's pack is substituted first (`Us(...)...`
// naming `sizeof...(Bs)`). An empty expansion initializes nothing.
//@ expect: run
//@ std: c++26
//@ stdout: 7 70 1 20 3 30 0
#include <cstdio>
struct A { int v; A(unsigned long n) : v(int(n)) {} };
struct B { int w; B(unsigned long n) : w(int(n) * 10) {} };
template <class... Bs> struct H {
  template <class... Us> struct I : Us... { I() : Us(sizeof...(Bs) + sizeof(Us))... {} };
};
template <class... Bs> struct G : Bs... {
  template <class... Us> G(Us... u) : Bs(u)... {}
};
template <class... Bs> struct L : Bs... { L() : Bs{3ul}... {} int n = sizeof...(Bs); };
int main() {
  H<int, char, long>::I<A, B> i;
  G<A, B> g(1ul, 2ul);
  L<A, B> l;
  L<> e;
  std::printf("%d %d %d %d %d %d %d\n", i.v, i.w, g.v, g.w, l.v, l.w, e.n);
}
