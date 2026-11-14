// [temp.spec.partial.match]/2, [temp.spec.partial.general]/9,
// [temp.mem]/1, [over.literal]/5, [temp.mem.func]/1: specializations of
// constrained and declared-before-defined partial specializations, a
// dependent partial specialization argument, variable template partial
// specializations, member function template specializations, literal
// operator template arguments, a trailing requires-clause of a member
// and a constrained member template. With `--pch-diff`, the header's
// snapshot carries them (WP-1358).
//@ expect: run
//@ std: c++20
#include "Inputs/spec_forms.h"

using namespace sf;
static_assert(A<double *>::k == 2 && A<char *>::k == 3 && A<int>::k == 1);
static_assert(has<int>::value && sizeof(cp) == 1 && traits<int *>::k == 2);
static_assert(ks == 239 && k_v<double *> == 3);

int main() {
  if (a_sum() != 3210 || in_mins(hours(2)) != 7200)
    return 1;
  if (secs() != 4321 || (12_s).n != 12)
    return 2;
  if (cm_use() != 0 || CM<char>(1, 0).k != 2)
    return 4;
  return w_use() == 3 && W<long>{}.g() == 2 ? 0 : 3;
}
