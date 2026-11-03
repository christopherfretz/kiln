// [expr.reflect]/5 P2996R13: a namespace-alias names the alias itself,
// not the namespace it designates; each alias is a distinct entity.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
namespace N {}
namespace M = N;
namespace M2 = M;
namespace M3 = N;
static_assert(^^M != ^^N);
static_assert(^^M2 != ^^M);
static_assert(^^M3 != ^^M);
static_assert(^^M == ^^M);
static_assert(^^::M == ^^M);
static_assert(name_is(^^M, "M"));
static_assert(name_is(^^M2, "M2"));
