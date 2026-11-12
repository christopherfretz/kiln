// [basic.fundamental] (GNU extension: complex types): `_Complex` and
// `__complex__` types in a header's declarations and bodies. With
// `--pch-diff`, the header's precompiled image carries them (WP-1320).
//@ expect: run
#include "Inputs/complex_types.h"

int main() {
  _Complex double d = 3.0;
  __imag__ d = 1.0;
  cxt::box<int> b{cxt::twice(d)};
  cxt::cfloat z = cxt::make(3, 4);
  if (b.real() != 6 || b.imag() != 2)
    return 1;
  return cxt::norm(z) == 25.0f ? 0 : 2;
}
