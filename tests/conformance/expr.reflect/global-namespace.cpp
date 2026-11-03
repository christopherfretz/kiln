// [expr.reflect]/4 P2996R13: `^^::` represents the global namespace; it
// is one entity however it is written.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
namespace N {}
static_assert(^^:: == ^^::);
static_assert(^^:: != ^^N);
static_assert(^^:: != info{});
consteval info global() { return ^^::; }
static_assert(global() == ^^::);
namespace N { constexpr info from_inside = ^^::; }
static_assert(N::from_inside == ^^::);
