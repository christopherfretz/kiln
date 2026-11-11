// [temp.arg.nontype], [temp.alias]/2: non-type template arguments written
// as expressions, value-dependent (`!is_ptr<T>::value`, `sizeof(T)`,
// `N * 2`) and not (`1 + 2`), substituted through alias templates as
// libstdc++'s `integral_constant.h` does. With `--pch-diff`, the
// header's image carries each argument's expression (WP-1300).
//@ expect: run
#include "Inputs/expr_nttp.h"

static_assert(not_ptr<int>::value && !not_ptr<int *>::value);
static_assert(size_of<int>::value == sizeof(int));
static_assert(twice<21>::value == 42 && three::value == 3);

int main() {
  holder<char> h{};
  int r = twice<3>() + three() + int(size_of<short>()) + decltype(h.extra)::value;
  return r == 6 + 3 + int(sizeof(short)) + 2 ? 0 : 1;
}
