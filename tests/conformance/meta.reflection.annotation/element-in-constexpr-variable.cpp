// [meta.reflection.annotation], [expr.const]/22 (P3032R2): the vector
// annotations_of returns is a temporary destroyed within the
// manifestly constant-evaluated initializer of a constexpr variable, so
// an element of it (an `info`) may initialize the variable.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
//@ strict-diags
#include <meta>
struct [[=1, =2]] S {};
constexpr auto first = annotations_of(^^S)[0];
constexpr auto second = annotations_of(^^S)[1];
static_assert([:constant_of(first):] == 1);
static_assert([:constant_of(second):] == 2);
