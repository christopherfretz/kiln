// [basic.types.general]/12, [expr.const] (P2996R13): a class member access
// naming a static data member of consteval-only type is a potentially
// evaluated expression of consteval-only type even when the object
// expression's class is not consteval-only, so outside a
// constant-evaluated context it is rejected like `G::s`. GCC 16.1 rejects
// `G::s` but accepts `g.s`; kiln treats both alike, so this is not
// GCC-compared (spec/reflection-repr.md §6).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 19:19 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 20:19 error "consteval-only expressions are only allowed in a constant-evaluated context"
#include "kiln_meta.h"
using std::meta::info;
struct G { static constexpr info s = ^^int; int x; };
G g;
static_assert(g.s == ^^int);
void f1() { (void)G::s; }
void f2() { (void)g.s; }
consteval info f3() { return g.s; }
