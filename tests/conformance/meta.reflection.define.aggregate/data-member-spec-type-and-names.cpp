// [meta.reflection.queries] type_of, [meta.reflection.define.aggregate]:
// a reflection of a data member description has the type it describes;
// a name spelled as a built-in type trait (a keyword to clang and GCC)
// is not an identifier, while another reserved identifier is.
// Found running GCC's C++ testsuite.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

static_assert(type_of(data_member_spec(^^int, {.name = "x"})) == ^^int);
static_assert(type_of(data_member_spec(^^const char*, {.name = "p"})) == ^^const char*);

consteval bool valid(data_member_options o) {
  try {
    data_member_spec(^^int, o);
  } catch (const std::meta::exception&) {
    return false;
  }
  return true;
}
static_assert(!valid({.name = "__is_convertible"}));
static_assert(!valid({.name = "__builtin_is_implicit_lifetime"}));
static_assert(valid({.name = "__reserved_but_no_keyword"}));
