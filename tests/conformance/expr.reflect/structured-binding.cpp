// [expr.reflect]/5 P2996R13: an identifier naming a structured binding
// represents the binding; each binding is its own entity.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
struct P { int x; int y; };
auto [a, b] = P{1, 2};
static_assert(^^a != ^^b);
static_assert(^^a == ^^a);
static_assert(^^a != ^^P::x);
static_assert(name_is(^^a, "a"));
static_assert(name_is(^^b, "b"));
