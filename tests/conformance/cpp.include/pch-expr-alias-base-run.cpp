// [class.derived.general], [temp.alias]/2, [temp.dep.type]: a class
// template whose base is a dependent alias template specialization with
// an expression argument (`bc<!is_ptr<T>::value>`, as libc++'s
// `__bool_constant` bases do). With `--pch-diff`, the header's image
// carries the base; the image's validator once rejected it as "a bad
// base class", so the header was replayed (WP-1300).
//@ expect: run
#include "Inputs/expr_alias_base.h"

static_assert(not_ptr<int>::value && !not_ptr<int *>::value);
static_assert(wide<short>::value == 2 * sizeof(short));

int main() { return not_ptr<char>::value && wide<int>::value == 2 * sizeof(int) ? 0 : 1; }
