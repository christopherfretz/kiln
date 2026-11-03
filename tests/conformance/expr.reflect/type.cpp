// [expr.reflect]/6 P2996R13: `^^type-id` represents the type it denotes;
// types that are the same type are one reflection.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
struct S {};
enum E { e0 };
static_assert(^^int == ^^int);
static_assert(^^int != ^^long);
static_assert(^^signed int == ^^int);
static_assert(^^unsigned != ^^int);
static_assert(^^const int != ^^int);
static_assert(^^const int == ^^int const);
static_assert(^^int* == ^^int*);
static_assert(^^int* != ^^const int*);
static_assert(^^int& != ^^int);
static_assert(^^int[3] != ^^int[4]);
static_assert(^^int(int) == ^^int(int));
static_assert(^^S == ^^struct S);
static_assert(^^E == ^^enum E);
static_assert(^^decltype(e0) == ^^E);
static_assert(^^decltype(^^int) != ^^info);  // info is an alias, kept
