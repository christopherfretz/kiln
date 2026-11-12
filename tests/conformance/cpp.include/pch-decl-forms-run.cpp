// [namespace.udecl], [class.inhctor.init], [class.union.anon],
// [temp.mem], [temp.spec.partial.general], [dcl.fct], [enum.udecl],
// [dcl.fct.spec], [over.literal]: a header's dependent using-declarations
// and inheriting constructors, anonymous unions, out-of-line members of
// nested classes, explicit object member functions, a class's
// using-enum-declaration, explicit(expr) constructors of a specialization
// the header completed, an abbreviated function template and a literal
// operator behave the same through a precompiled header. With
// `--pch-diff`, the header's image carries them (WP-1345).
//@ expect: run
#include "Inputs/decl_forms.h"

using namespace df;

pr<bool, long> two() { return {1, 2L}; }

int main() {
  D<int> d(1);
  E<long> e(2L, 3L);
  d.f();
  U u;
  os<char> o;
  os<char>::sentry s(o);
  os<char>::in in;
  S x;
  x.ref() = 6;
  te t;
  fb::res<int&> rr;
  int r = 0;
  r += d.v + e.v == 2 ? 0 : 1;
  r += Opt<int>(3).value_or(1) == 3 && Opt<int>().value_or(4) == 4 ? 0 : 1;
  r += u.i == 5 ? 0 : 1;
  r += bool(s) && in.f() == 4 && os<char>::in::s == 5 ? 0 : 1;
  r += x.get() == 6 ? 0 : 1;
  r += t.f() == 2 && int(te::c) == 3 ? 0 : 1;
  r += need().z == 3 && two().z == 2 ? 0 : 1;
  r += twice<int>(2.5) == 4 && 3_k == 3000 ? 0 : 1;
  r += rr.g() + rr.k == 3 ? 0 : 1;
  return r;
}
