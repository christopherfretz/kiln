// The std::meta queries answer in the reflectee set `^^` builds
// (spec/reflection-repr.md §1): members_of keys each member as `^^`
// keys it (a member alias is its alias type, a nested class or enum its
// type, a member template its template, others their declaration),
// type_of yields the canonical type (an alias is not kept; GCC 16.1
// agrees), and name_of names namespace, namespace-alias and template
// reflections.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
#include "kiln_meta.h"
using namespace std::meta;

using I = int;
I v;
const int cv = 0;

namespace ns {
int x;
}
namespace na = ns;
template <class T>
struct Tmpl {};

struct S {
  int a;
  using T = int;
  struct N {};
  enum E { e0 };
  template <class U>
  void tm(U) {}
  void f();
  I m;
};

// members_of: alias, nested class, enum, member template, member function
// (an unscoped enumerator is found in S but is not a member of it; GCC
// 16.1 lists the same seven).
static_assert(__kiln_members_count(^^S) == 7);
static_assert(__kiln_member_at(^^S, 0) == ^^S::a);
static_assert(__kiln_member_at(^^S, 1) == ^^S::T);
static_assert(__kiln_member_at(^^S, 1) != ^^int);
static_assert(__kiln_member_at(^^S, 2) == ^^S::N);
static_assert(__kiln_member_at(^^S, 3) == ^^S::E);
static_assert(__kiln_member_at(^^S, 4) == ^^S::tm);
static_assert(__kiln_member_at(^^S, 5) == ^^S::f);
static_assert(__kiln_member_at(^^S, 6) == ^^S::m);

// type_of: canonical, cv kept, enumerators give their enumeration.
static_assert(type_of(^^v) == ^^int);
static_assert(type_of(^^S::m) == ^^int);
static_assert(type_of(^^S::m) != ^^I);
static_assert(type_of(^^cv) == ^^const int);
static_assert(type_of(^^S::e0) == ^^S::E);

// name_of on the non-Decl kinds.
static_assert(name_is(^^ns, "ns"));
static_assert(name_is(^^na, "na"));
static_assert(name_is(^^Tmpl, "Tmpl"));
static_assert(name_is(^^S::tm, "tm"));
static_assert(name_is(^^S::T, "T"));
static_assert(name_is(^^S::N, "N"));
static_assert(name_is(^^ns::x, "x"));

int main() {}
