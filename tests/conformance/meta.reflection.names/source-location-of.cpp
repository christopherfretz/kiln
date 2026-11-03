// [meta.reflection.names]/8 (P2996R13): source_location_of gives the
// location of the declaration a reflection designates. kiln gives the
// line and column of the declared name and an empty function_name; a
// reflection with no declaration (a fundamental or compound type) gives
// a default-constructed source_location.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

constexpr unsigned s_line = __LINE__ + 1;
struct S {
  int mem;
};
int var;

static_assert(source_location_of(^^S).line() == s_line);
static_assert(source_location_of(^^S::mem).line() == s_line + 1);
static_assert(source_location_of(^^var).line() == s_line + 3);
static_assert(source_location_of(^^var).column() == 5);
consteval bool ends_with(const char* s, std::string_view tail) {
  decltype(sizeof 0) n = 0;
  while (s[n]) ++n;
  if (n < tail.size()) return false;
  for (decltype(sizeof 0) i = 0; i < tail.size(); ++i)
    if (s[n - tail.size() + i] != tail.data()[i]) return false;
  return true;
}
static_assert(ends_with(source_location_of(^^S).file_name(), "source-location-of.cpp"));
static_assert(std::string_view(source_location_of(^^S).function_name()).empty());
static_assert(source_location_of(^^int).line() == 0);

int main() {}
