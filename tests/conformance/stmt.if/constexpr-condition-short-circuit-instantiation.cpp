// [stmt.if]/2, [expr.const]: the condition of an `if constexpr` is a
// constant expression evaluated as a whole, also in an instantiation, so
// `&&` short-circuits a consteval call whose evaluation would fail rather
// than evaluating it on its own as an immediate invocation. Found running
// GCC's C++ testsuite.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#include "kiln_meta_traits.h"
#else
#include <meta>
#endif
using namespace std::meta;

template <class T>
constexpr bool has_members() {
  if constexpr (is_class_type(^^T) &&
                !nonstatic_data_members_of(^^T, access_context::unchecked()).empty())
    return true;
  else
    return false;
}
struct A { int x; };
static_assert(has_members<A>());
static_assert(!has_members<int>());
