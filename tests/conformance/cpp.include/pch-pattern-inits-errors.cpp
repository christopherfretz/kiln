// [class.base.init]/7, [temp.inst]/4: a mem-initializer of a header's
// class template constructor that does not instantiate is diagnosed at
// the instantiation, in the header, with the point of instantiation in
// the unit; overload resolution sees the header's constructors. With
// `--pch-diff`, the image's carried mem-initializers give the same
// diagnostics, byte for byte (WP-1310).
//@ expect: compile-fail
//@ diag: 16:29 error "no matching constructor for initialization of 'pi::Multi<pi::B1, pi::B2>'"
//@ diag: 17:17 error "no matching constructor for initialization of 'pi::Iter<int>'"
#include "Inputs/pattern_inits.h"

struct NC {};
void f() {
  pi::Base<NC> q(NC{});
  pi::Out<NC> o(NC{});
  pi::Multi<pi::B1, pi::B2> m;
  pi::Iter<int> i(1, 2);
  pi::Derived<NC> d;
}
