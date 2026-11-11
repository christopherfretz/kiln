// [dcl.enum]/2, [class.pre]/1, [cpp.include]: an unnamed unscoped
// enumeration introduces only its enumerators, and an unnamed class
// declares only the variables of its type. With `--pch-diff`, the
// header's precompiled image carries both (WP-1293: `<cfenv>` replayed
// with an internal image inconsistency).
//@ expect: run
#include "Inputs/unnamed_tags.h"

int main() {
  ut_g.a = 4;
  ut::v[1].c = 1;
  decltype(ut_g) copy = ut_g;
  auto e = UT_B;
  int k = UtS{}.v[UtS::K - 1];
  return ut_sum() == 1 + 4 + 5 + 3 + 5 + 1 && ut::f() == 15 && copy.f() == 5 && e == 4 && k == 0
             ? 0
             : 1;
}
