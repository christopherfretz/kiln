// [lex.nullptr], [dcl.type.decltype]: `decltype(nullptr)` is
// `std::nullptr_t`, as `<stddef.h>` and libstdc++'s `<bits/c++config.h>`
// declare it. With `--pch-diff`, the header's image carries the type
// (WP-1300, spec/modules.md "Expressions in types").
//@ expect: run
#include "Inputs/expr_nullptr.h"

static_assert(sizeof(nullptr_t) == sizeof(void *));

int main() {
  NpHolder h{np_make()};
  return np_is_null(h.p) && h.get() == nullptr ? 0 : 1;
}
