// [temp.spec.partial.general]/7: a member template's partial
// specializations are declared in its class; access to them is the
// class's. With `--pch-diff`, the header's precompiled image carries
// them (WP-1342).
//@ expect: run
#include "Inputs/member_partial_specs.h"

int mps::base::f() {
  return list<char, int>::size + one<int *>::v + vt<int &>;
}

template <class X> struct bx { X x; };

int main() {
  static_assert(mps::tmpl<char>::ptr<int>::v == 0);
  static_assert(mps::tmpl<short>::ptr<int *>::v == 2);
  static_assert(mps::tmpl<short>::ptr<long>::v == 8);
  static_assert(mps::tmpl<long>::vt<int &> == 9 && mps::tmpl<long>::vt<int> == 1);
  static_assert(sizeof(mps::tmpl<long>::st<char, false>) == 8);
  static_assert(sizeof(mps::tmpl<long>::st<char>) == 1);
  mps::rebind<long>::p<bx, int>::type r{3};
  if (r.x != 3)
    return 2;
  static_assert(mps::outer::inner::arr<int, 3>::n == 3);
  static_assert(mps::outer::inner::arr<int, 2>::n == 0);
  return mps::base::f() == 1 + 2 + 4 ? 0 : 1;
}
