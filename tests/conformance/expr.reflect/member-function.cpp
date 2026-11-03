// [expr.reflect]/5 P2996R13: a name of a function that is not an overload
// set represents the function: a member function, a static member
// function, a free function; a redeclaration is the same function.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
struct S { void f(); static int g(); int h() const; };
void free_fn();
void free_fn();
static_assert(^^S::f == ^^S::f);
static_assert(^^S::f != ^^S::g);
static_assert(^^S::g != ^^S::h);
static_assert(^^free_fn == ^^::free_fn);
static_assert(name_is(^^S::h, "h"));
static_assert(name_is(^^free_fn, "free_fn"));
