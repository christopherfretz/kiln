// [namespace.udecl]/1, [temp.res.general]/1: a name in a template found
// through a using-declaration names the declarations it introduces; a
// later overload added to the nominated namespace is not among them
// ([namespace.udecl]/11). With `--pch-diff`, the header's precompiled
// snapshot carries the using-declarations' shadows (WP-1342).
//@ expect: run
#include "Inputs/using_shadow_calls.h"

namespace usc_lib {
constexpr short mag(short) { return 100; }
} // namespace usc_lib

int main() {
  if (usc::alt(-2) != -2 || usc::holder<long>::go(-3) != 6)
    return 1;
  return usc::alt(short(-1)) == -1 ? 0 : 2;
}
