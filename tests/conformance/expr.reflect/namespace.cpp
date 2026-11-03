// [expr.reflect]/5 P2996R13: a namespace-name represents the namespace,
// whichever of its definitions names it; an inline namespace is its own
// namespace, though lookup sees through it.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
namespace N { int w; }
namespace N { inline namespace In { int x; } namespace Q {} }
static_assert(^^N == ^^N);
static_assert(^^N == ^^::N);
static_assert(^^N::In != ^^N);
static_assert(^^N::Q != ^^N);
static_assert(^^N::x == ^^N::In::x);
static_assert(name_is(^^N, "N"));
static_assert(name_is(^^N::Q, "Q"));
namespace N { constexpr info self = ^^N; }
static_assert(N::self == ^^N);
