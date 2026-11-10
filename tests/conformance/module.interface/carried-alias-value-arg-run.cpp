// [temp.alias]/2, [temp.type]/1, [expr.reflect]/6 P2996R13,
// [module.interface], spec/modules.md §11.11 (WP-1168): an importer uses
// `std::meta::info` constants the module evaluated that reflect alias
// template specializations with non-type arguments written as
// expressions (`1 + 2`, `(short)3`, an enumerator and a `bool`). The
// specialization's identity is its converted arguments, so each compares
// equal to the importer's own reflection however it spells them, with a
// kind-1 or a kind-2 module file (`--bmi-kind 1|2`).
//@ expect: run
//@ std: c++26
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ oracle: none
//@ module: cav=Inputs/cav.cppm
//@ stdout: 3 4
#include "kiln_meta.h"
import cav;
extern "C" int printf(const char *, ...);
static_assert(rarr == (^^Arr<char, 3>) && rarr == rarr_s && rarr != (^^Arr<char, 4>));
static_assert(rarr != (^^char[3]));
static_assert(reb == (^^EB<E::b, true>) && reb != (^^EB<E::a, true>));
int main() {
  printf("%d %d\n", int(sizeof(typename [:rarr:])), int(sizeof(typename [:reb:]) / sizeof(int)));
}
