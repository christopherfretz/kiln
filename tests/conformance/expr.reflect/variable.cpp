// [expr.reflect]/5 P2996R13: an id-expression naming a variable
// represents the variable, not its value, even when it is constexpr;
// a static data member is a variable too.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
int g;
constexpr int cv = 3;
constexpr int cw = 3;
struct S { static int s; static constexpr int t = 1; };
namespace N { int g; }
static_assert(^^g == ^^g);
static_assert(^^g == ^^::g);
static_assert(^^g != ^^N::g);
static_assert(^^cv != ^^cw);  // equal values, distinct variables
static_assert(^^S::s != ^^S::t);
static_assert(name_is(^^cv, "cv"));
static_assert(type_of(^^cv) == ^^const int);
static_assert(type_of(^^g) == ^^int);
extern int g;
static_assert(^^g == ^^::g);  // a redeclaration is the same variable
