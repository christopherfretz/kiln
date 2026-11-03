// [expr.reflect]/5 P2996R13: a qualified name of a non-static data member
// (a bit-field included) represents the member; an anonymous union's
// member is named through the enclosing class.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
struct S {
  int m;
  int b : 3;
  union { int u; long v; };
};
struct T : S { int m; };
static_assert(^^S::m != ^^S::b);
static_assert(^^S::m == ^^S::m);
static_assert(^^T::m != ^^S::m);
static_assert(^^S::u != ^^S::v);
static_assert(name_is(^^S::b, "b"));
static_assert(type_of(^^S::m) == ^^int);
