// [class.virtual], [class.dtor], Itanium ABI §5.2.3 (WP-1198): the
// non-module counterpart of module.interface/vtable-alias-O1-run.cpp.
// Another translation unit defines A's key destructor; at -O1 this one
// copies A's vtable `available_externally` (A::f is emitted here), its
// destructor slots naming the D1/D0 declarations.
//@ expect: run
//@ args: -O1
//@ unit: Inputs/key-dtor-other-tu.cpp
//@ exit: 4
#include "Inputs/key-dtor-other-tu.h"

int main() {
  A a;
  return a.f();
}
