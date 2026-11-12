// [expr.const.const]/5.1, [expr.const.imm]/2.1 (P4101R1): a class member
// access naming a static data member that holds a non-null reflection is
// an immediate constant expression even when the object expression is not
// an immediate object, so outside an immediate function context it is
// rejected like `G::s`. GCC 16.1 rejects
// `G::s` but accepts `g.s`; kiln treats both alike, so this is not
// GCC-compared (spec/reflection-repr.md §6).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 19:19 error "consteval-only value outside an immediate function context"
//@ diag: 20:19 error "consteval-only value outside an immediate function context"
#include "kiln_meta.h"
using std::meta::info;
struct G { static constexpr info s = ^^int; int x; };
G g;
static_assert(g.s == ^^int);
void f1() { (void)G::s; }
void f2() { (void)g.s; }
consteval info f3() { return g.s; }
