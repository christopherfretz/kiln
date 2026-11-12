// [dcl.typedef]/1-2, [temp.inst]/4: a header's block-scope typedef and
// alias declarations name their types in each instantiation of their
// function. With `--pch-diff`, the header's precompiled image carries
// them as local declarations (WP-1314).
//@ expect: run
#include "Inputs/local_typedef.h"
struct Q {
  using type = short;
};
int main() {
  if (ltd::f(3) != 7 || ltd::f('a') != 98)
    return 1;
  if (ltd::g(2) != 4 || ltd::S<Q>::h() != 2)
    return 2;
  return 0;
}
