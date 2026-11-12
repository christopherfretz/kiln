// [dcl.pre]/10-11, [temp.inst]/4, [temp.arg.explicit]/1: a header's
// block-scope static assertions are checked again by each instantiation
// of their function, and a call through explicit template arguments
// (`pick<int>(3)`) names the same specialization. With `--pch-diff`, the
// header's precompiled image carries the assertions as local
// declarations and the names with their written arguments (WP-1314).
//@ expect: run
#include "Inputs/local_static_assert.h"

int main() {
  if (lsa::f(3) != 4 || lsa::f('a') != 98)
    return 1;
  if (lsa::g(2) != 4 || lsa::S<int>::h() != 7)
    return 2;
  if (lsa::callpick() != 11)
    return 3;
  return 0;
}
