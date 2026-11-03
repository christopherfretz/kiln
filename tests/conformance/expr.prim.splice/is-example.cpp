// [expr.prim.splice] Example 1, P2996R13: a splice-expression of a
// reflection of a static data member's class names it as a scope; a
// splice of a template parameter of type info is an expression; `typename`
// makes a splice a type in an expression. The example's
// `template [:...:]` and unparenthesized-template-argument lines are
// WP-482's (template splices).
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
struct S { static constexpr int a = 1; };

constexpr int c = [:^^S:]::a;  // [:^^S:] is not an expression

template <auto V> constexpr int e = [:V:];  // splice-expression

constexpr auto g = typename [:^^int:](42);  // [:^^int:] is a type

constexpr auto h = ^^g;
static_assert(c == 1);
static_assert(g == 42);
static_assert(e<h> == 42);
static_assert(e<([:^^h:])> == 42);  // OK: parenthesized
