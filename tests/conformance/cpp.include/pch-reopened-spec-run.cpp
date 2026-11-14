// [temp.expl.spec]/1, [namespace.def]/2: a header declares an explicit
// specialization in one namespace block and defines it in a later one
// (libc++'s `char_traits<char>` in `<__fwd/string.h>` and
// `<__string/char_traits.h>`). With `--pch-diff`, the header's
// snapshot carries the definitions and their members across the
// reopened namespace (WP-1314).
//@ expect: run
#include "Inputs/reopened_spec.h"
static_assert(rs::traits<char>::size() == 1);
int main() {
  rs::traits<char>::type c = 2;
  return c - 2;
}
