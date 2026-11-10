// [dcl.spec.auto.general]/11, [expr.reflect] (P2996R13): `^^v` in the
// initializer of a variable `v` declared with a placeholder type names a
// variable whose type is not yet deduced, which is ill-formed as for
// `auto v = v;` (GCC 16.1: "use of 'v' before deduction of 'auto'"), so
// no query sees a variable of undeduced type. WP-1243.
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 23:37 error "variable 'self' declared with deduced type 'const auto' cannot appear in its own initializer"
//@ diag: 24:27 error "variable 'glob' declared with deduced type 'auto' cannot appear in its own initializer"
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

consteval bool throws_type(info r) {
  try { (void)type_of(r); return false; } catch (exception&) { return true; }
}
constexpr auto self = throws_type(^^self);
auto glob = throws_type(^^glob);
