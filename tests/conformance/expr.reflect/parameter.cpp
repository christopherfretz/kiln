// [expr.reflect]/5 P2996R13: in a consteval function, a function
// parameter can be reflected; it represents the parameter.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
consteval bool params(int p, int q) { return ^^p != ^^q && ^^p == ^^p; }
static_assert(params(1, 2));
consteval info first(int p, int) { return ^^p; }
static_assert(first(1, 2) == first(3, 4));
static_assert(name_is(first(0, 0), "p"));
