// [meta.reflection.queries] (P2996R13) type_of, [basic.types.general]/6:
// type_of of a variable whose type is still an array of unknown bound can
// change once a later declaration completes it, so the evaluation that
// asked is not memoized (spec/reflection.md §3.3): the same consteval call
// answers `const int[]` before the definition and `const int[2]` after.
// A complete type never changes, so that answer stays memoizable.
// Answers checked against GCC 16.1 (-freflection). WP-795.
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

struct M {
  static const int v[];
};
consteval std::meta::info type_of_v() { return std::meta::type_of(^^M::v); }
consteval std::meta::info type_of_r(std::meta::info r) { return std::meta::type_of(r); }

static_assert(type_of_v() == ^^const int[]);
static_assert(type_of_r(^^M::v) == ^^const int[]);
constexpr int M::v[] = {4, 5};
static_assert(type_of_v() == ^^const int[2]);
static_assert(type_of_r(^^M::v) == ^^const int[2]);
static_assert(std::meta::size_of(^^M::v) == 2 * sizeof(int));

int main() {}
