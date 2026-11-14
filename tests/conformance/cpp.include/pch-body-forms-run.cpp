// [expr.delete], [expr.typeid], [expr.dynamic.cast], [except.pre],
// [namespace.udecl], [enum.udecl], [dcl.struct.bind], [dcl.init.aggr],
// [over.match.oper] (rewritten candidates), [dcl.spec.auto.general]: a
// header's function bodies with these forms, and pointer and class
// constants, behave the same through a precompiled header. With
// `--pch-diff`, the header's snapshot carries them (WP-1343).
//@ expect: run
#include "Inputs/body_forms.h"

int main() {
  bf::D d;
  int v = 4;
  bf::del(new int(1), new int[2]);
  int r = 0;
  r += bf::ne(bf::P{1}, bf::P{2}) ? 0 : 1;
  r += bf::dc(&d) == 2 ? 0 : 1;
  r += bf::same(d) ? 0 : 1;
  r += bf::bc(0x3f800000u) == 1.0f ? 0 : 1;
  r += bf::at(&v) == 8 ? 0 : 1;
  r += bf::Der{}.m() == 10 ? 0 : 1;
  r += bf::lu() == 9 ? 0 : 1;
  r += bf::tr(3) == 3 && bf::tr(0) == 0 ? 0 : 1;
  r += bf::ue(bf::Col::green) == 1 ? 0 : 1;
  r += bf::ext() == 6 ? 0 : 1;
  r += bf::un() == 5 ? 0 : 1;
  r += bf::sb() == 23 ? 0 : 1;
  r += bf::usb() == 7 ? 0 : 1;
  r += bf::ag() == 3 ? 0 : 1;
  r += bf::pc() == 4 ? 0 : 1;
  r += bf::cr() == 3 ? 0 : 1;
  return r;
}
