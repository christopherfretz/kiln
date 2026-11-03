// [basic.link], [basic.def]/1, [class.mfct]/1, [class.friend]/1 and
// [expr.reflect] P2996R13: a reflection designates an entity, not a
// declaration. A reflection taken before a redeclaration, a friend
// declaration or an out-of-line member definition equals one taken after
// it, and members_of yields the same reflection as ^^C::m
// (`Sema::same_entity`, spec/sema.md §4 "Entity identity"; WP-471).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
#include "kiln_meta.h"
using namespace std::meta;

void g();
constexpr info g_before = ^^g;
struct F {
  friend void g();
};
void g();
static_assert(g_before == ^^g);

namespace N { int v; }
constexpr info n_before = ^^N;
namespace N { extern int v; }
namespace A = N;
// A namespace alias is its own entity ([expr.reflect]/5; dealias unwraps it).
static_assert(n_before == ^^N && n_before != ^^A);
static_assert(^^N::v == ^^A::v);

struct C {
  int x;
  void m();
};
constexpr info m_before = ^^C::m;
void C::m() {}
static_assert(m_before == ^^C::m);
static_assert(__kiln_member_at(^^C, 1) == ^^C::m);
static_assert(__kiln_member_at(^^C, 0) == ^^C::x);
static_assert(^^C::m != ^^g && ^^C::x != ^^N::v);
