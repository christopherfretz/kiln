// [dcl.init.aggr], [temp.inst]/12, [dcl.typedef]: a header's designated
// initializers in templates, its vectors of a dependent element count,
// its member typedef named in a member alias template, and its uses of a
// function template specialization's default arguments behave the same
// through a precompiled header; the unit uses the same specializations'
// defaults again. With `--pch-diff`, the header's image carries them
// (WP-1350).
//@ expect: run
#include "Inputs/wp1350_forms.h"

using namespace wf;

int main() {
  Out o = mk(3), p = dep(o), s = abbrev(8);
  vec<int, 4> v = {1, 2, 3, 4};
  Dd d;
  up<Dd> x(&d);
  up<B> y(static_cast<up<Dd>&&>(x));
  const char* q = "ab";
  Str t = mks("abcd"), u(q, q + 2);
  int r = o.a + o.b.c + o.b.d + p.a + p.e + s.b.c + s.b.d + sum<int, 4>(v) + sum<int, 4>(splat(2)) +
          int(widen<int, 4>(v)) + y.p->v + (x.p ? 1 : 0) + t.n + t.a + u.n + u.a + use_g() + g(3.0) +
          g<char>(1);
  return r == 3 + 1 + 4 + 4 + 9 + 8 + 4 + 10 + 8 + 5 + 4 + 0 + 3 + 7 + 2 + 7 + 12 + 11 + 2 ? 0 : 1;
}
