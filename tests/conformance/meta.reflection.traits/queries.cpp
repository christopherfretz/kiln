// [meta.reflection.traits] P2996R13, [meta.unary.prop.query]: rank and
// extent(type, i = 0) are the builtin trait engine's __array_rank and
// __array_extent; extent of a dimension past the rank, or of an unknown
// bound, is 0.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ xfail(gcc): compares with clang's __array_extent and other trait builtins, which GCC 16.1 lacks
#include "kiln_meta_traits.h"
using namespace std::meta;

using A = int[2][3][4];
static_assert(rank(^^int) == 0 && rank(^^int[]) == 1 && rank(^^A) == 3);
static_assert(extent(^^A) == 2 && extent(^^A, 1) == 3 && extent(^^A, 2) == 4);
static_assert(extent(^^A, 3) == 0 && extent(^^int[][5]) == 0 && extent(^^int[][5], 1) == 5);
static_assert(extent(^^int) == 0 && extent(^^A, ~0u) == 0);
static_assert(rank(^^A) == __array_rank(A) && extent(^^A, 1) == __array_extent(A, 1));
