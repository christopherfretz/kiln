// [expr.reflect]/1, [temp.names]/1 (P2996R13): `^^W<int>::In` reflects
// the member class template `In` of the specialization `W<int>`; a
// nested-name-specifier naming a template specialization is followed
// like any other class qualifier.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ xfail(kiln): WP-1243 follow-up, reflect_lookahead stops at `W<` so `^^W<int>::In` parses as a type-id needing template arguments
//@ strict-diags
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif

namespace demo {
template <class T> struct W {
  template <class U> struct In {};
};
}  // namespace demo

constexpr auto r = ^^demo::W<int>::In;
static_assert(std::meta::is_class_template(r));
static_assert(std::meta::parent_of(r) == ^^demo::W<int>);

int main() {}
