// [expr.reflect]/6, [temp.alias]/2, [temp.type]/1 P2996R13: `^^Arr<char, 3>`
// reflects the alias template specialization, and two template-ids
// naming it with the same template arguments (template argument 3 both
// times, however written) are the same specialization, so the
// reflections compare equal. kiln keeps a non-type argument of an alias
// specialization as the written expression, so each spelling is its own
// type; a kind-2 module file does not carry such a reflection either
// (spec/modules.md §11.11).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ xfail(kiln): WP-1160 follow-up, an alias specialization keeps written non-type arguments, not converted values
//@ strict-diags
#include "kiln_meta.h"
template <class T, int N> using Arr = T[N];
constexpr std::meta::info r = ^^Arr<char, 3>;
static_assert(r == (^^Arr<char, 3>));
static_assert((^^Arr<char, 1 + 2>) == (^^Arr<char, 3>));
static_assert((^^Arr<char, 4>) != (^^Arr<char, 3>));
