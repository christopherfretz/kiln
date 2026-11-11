// [class.base.init]/2-9, [temp.inst]/4, [temp.variadic]/5: the
// mem-initializers of constructors of class templates declared in an
// included header are substituted by each instantiation. With
// `--pch-diff`, the header's precompiled image carries them as parsed
// (WP-1310, spec/modules.md "Pattern mem-initializers").
//@ expect: run
//@ stdout: 4 1 5 3 2 7 10 11 6
#include "Inputs/pattern_inits.h"
extern "C" int printf(const char *, ...);

int main() {
  int x = 4;
  pi::Iter<int> a(&x);
  pi::Iter<const int> b(a);
  pi::Iter<int> z;
  pi::Derived<int> d(5);
  pi::Derived<long> e;
  pi::Multi<pi::B1, pi::B2> m(10);
  pi::Out<int> o(3);
  printf("%d %d %d %d %d %d %d %d %d\n", *b.p, z.p == nullptr, d.b, d.k, d.arr[1], (int)e.b, m.a,
         m.c, o.v);
  return 0;
}
