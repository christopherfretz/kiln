// [expr.reflect]/5 P2996R13: an enumerator represents that enumerator,
// however it is named (unscoped through the enclosing scope or the
// enumeration, scoped through the enumeration).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
enum E { A, B = A };
enum class F { A };
struct S { enum G { X }; };
static_assert(^^A == ^^E::A);
static_assert(^^A != ^^B);
static_assert(^^F::A != ^^E::A);
static_assert(^^S::X == ^^S::G::X);
static_assert(^^A != ^^E);
static_assert(name_is(^^B, "B"));
static_assert(name_is(^^F::A, "A"));
