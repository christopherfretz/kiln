// [temp.expl.spec], [temp.spec.partial], [temp.inst], [temp.explicit],
// [temp.arg.template]: an explicit specialization declared before its
// definition, a partial specialization declared before its definition,
// implicit variable template specializations, a template argument naming
// a class template inside a body, explicit specializations and explicit
// instantiation declarations of function templates. With `--pch-diff`,
// the header's snapshot carries them (WP-1352).
//@ expect: run
#include "Inputs/declared_specializations.h"

template double d5::dbl<double>(double);

int main() {
  if (d1::ch<char>::size + d1::other{}.k != 2 || d2::pr<int, int>::v + d2::pr<int, char>::v != 2)
    return 1;
  if (d3::e != 2.5 || d3::total() != 5 || d3::e_v<float> != 2.5f)
    return 2;
  if (d4::use() != int(sizeof(int)) || d5::pick() != 2)
    return 3;
  return d5::inc(5) + d5::inc(1L) + d5::dbl(2.0) == 4 + 2 + 4 ? 0 : 4;
}
