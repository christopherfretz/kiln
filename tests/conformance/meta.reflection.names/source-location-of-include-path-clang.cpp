// [meta.reflection.names]/8 (P2996R13): source_location_of's file name
// is implementation-defined; by default (-fkiln-source-path-style=clang,
// spec/reflection.md §3.9) kiln gives the presumed name as written, as
// clang's __builtin_FILE() does: `-I /p//Inputs/../Inputs/./` names the
// header `/p//Inputs/../Inputs/./slo-header.h`.
//@ args: -freflection -I tests/support -I %{test_dir}//Inputs/../Inputs/./
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include "kiln_meta.h"
#include <slo-header.h>
using namespace std::meta;

consteval decltype(sizeof 0) len(const char* s) {
  decltype(sizeof 0) n = 0;
  while (s[n]) ++n;
  return n;
}
consteval bool at(const char* s, decltype(sizeof 0) i, const char* part) {
  for (decltype(sizeof 0) k = 0; part[k]; ++k)
    if (s[i + k] != part[k]) return false;
  return true;
}
consteval bool ends_with(const char* s, const char* tail) {
  return len(s) >= len(tail) && at(s, len(s) - len(tail), tail);
}
constexpr const char* hf = source_location_of(^^InHeader).file_name();
static_assert(hf[0] == '/');
static_assert(ends_with(hf, "/meta.reflection.names//Inputs/../Inputs/./slo-header.h"));
static_assert(source_location_of(^^InHeader).line() == 3);
// One translation unit is self-consistent: __builtin_FILE() agrees.
constexpr const char* bf = InHeaderFile;
static_assert(ends_with(bf, "/meta.reflection.names//Inputs/../Inputs/./slo-header.h"));

int main() {}
