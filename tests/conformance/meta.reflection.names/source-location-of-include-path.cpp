// [meta.reflection.names]/8 (P2996R13): source_location_of's file name
// is implementation-defined; under -fkiln-source-path-style=gcc kiln gives
// GCC 16.1's (spec/reflection.md §3.9). GCC simplifies the absolute path of a file found through an
// include search path: empty and `.` components are dropped and `dir/..`
// is folded when `dir` is a real directory, so `-I /p//Inputs/../Inputs/./`
// names the header `/p/Inputs/slo-header.h`. Build systems pass such
// paths (a `//opt/inc` from an empty prefix), and a generator that derives
// `#include` lines from source_location_of needs GCC's spelling.
//@ args: -freflection -fkiln-source-path-style=gcc -I tests/support -I %{test_dir}//Inputs/../Inputs/./
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ xfail(gcc): GCC 16.1 keeps the -I spelling `//Inputs/../Inputs/./` in the file name
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
consteval bool contains(const char* s, const char* part) {
  for (decltype(sizeof 0) i = 0; i + len(part) <= len(s); ++i)
    if (at(s, i, part)) return true;
  return false;
}
consteval bool ends_with(const char* s, const char* tail) {
  return len(s) >= len(tail) && at(s, len(s) - len(tail), tail);
}
constexpr const char* hf = source_location_of(^^InHeader).file_name();
static_assert(hf[0] == '/' && hf[1] != '/');
static_assert(!contains(hf, "//"));
static_assert(!contains(hf, "/./"));
static_assert(!contains(hf, "/../"));
static_assert(ends_with(hf, "/meta.reflection.names/Inputs/slo-header.h"));
static_assert(source_location_of(^^InHeader).line() == 3);
// One translation unit is self-consistent: __builtin_FILE() agrees.
static_assert(ends_with(InHeaderFile, "/meta.reflection.names/Inputs/slo-header.h"));

int main() {}
