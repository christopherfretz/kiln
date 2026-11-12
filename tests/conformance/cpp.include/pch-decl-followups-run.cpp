// [temp.names]/3, [temp.res.general], [new.delete]/1, [temp.explicit],
// [namespace.udecl]/3, [temp.expl.spec]/2, [dcl.fct.def.default]/5: a
// class template named through a qualifier in a dependent type, a
// header's std::align_val_t after the implicit one, a deprecated
// explicit instantiation declaration, a using-declaration of a base's
// member template, an explicit specialization of a member class
// template defined outside its class, special members defaulted
// outside their class. With `--pch-diff`, the header's
// precompiled image carries them (WP-1353).
//@ expect: run
#include "Inputs/decl_followups.h"

int main() {
  int *p = f1::make();
  int r = *p - 7;
  delete p;
  if (r != 0 || f1::align_of(std::align_val_t{2}) != 2)
    return 1;
  static_assert(f2::ptr_or_int<int *>::value && f2::ptr_or_int<int>::value);
  static_assert(!f2::ptr_or_int<long>::value && f2::pi<int> && !f2::pi<char>);
  f2::wrap<int> w{{3}};
  f3::old<int> o{4};
  static_assert(f4::dr(2) == 3);
  f4::outer::res<void> rv;
  f4::outer::res<int> ri;
  f4::M m;
  f4::M m2(m);
  if (f4::dr(1, 2) + rv.k() + ri.k() + m2.x != 9)
    return 3;
  return w.b.v + o.get() == 7 ? 0 : 2;
}
