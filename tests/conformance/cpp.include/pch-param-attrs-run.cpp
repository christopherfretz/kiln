// [dcl.attr.grammar]/5, [dcl.fct]/3: a header's parameters with
// attributes (`[[clang::lifetimebound]]`, `[[maybe_unused]]`; libc++'s
// `std::move`, `std::forward`, `std::min`). With `--pch-diff`, the
// header's snapshot carries the parameters' attributes
// (WP-1314).
//@ expect: run
#include "Inputs/param_attrs.h"
int main() {
  int x = 3;
  const int &r = pa::pick(x, 0);
  pa::S s{0};
  return r + pa::fwd(x) + s.get(x) - 9;
}
