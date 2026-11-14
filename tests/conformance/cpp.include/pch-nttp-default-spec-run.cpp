// [temp.arg.general]/1, [temp.param]/12, [temp.expl.spec]/1: explicit
// specializations of a class template that take its non-type default
// argument (the shape of <ext/type_traits.h> `__promote`). With
// `--pch-diff`, the header's snapshot carries the default as an
// expression and the specializations as written (WP-1320).
//@ expect: run
#include "Inputs/nttp_default_spec.h"

template <typename T> using P = typename nds::promote<T>::type;

int main() {
  P<int> d = 1.5;
  P<float> f = 2.5f;
  P<long double> l = 0.5L;
  if (sizeof(P<double>) != sizeof(double) || sizeof(l) != sizeof(long double))
    return 1;
  if (nds::twice<short>::value != 4 || nds::twice<char, 7>::value != 7)
    return 2;
  return int(d + f) - 4 + nds::is_int<long>::value;
}
