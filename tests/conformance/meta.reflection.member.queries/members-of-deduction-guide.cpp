// [meta.reflection.member.queries]/3 (C++26 working draft), [temp.deduct.guide]:
// a deduction guide is none of the kinds of member that are
// members-of-representable (it is not a function), so members_of a
// namespace or a class does not list it. Deviation: GCC 16.1 lists it
// (spec/reflection-queries.md §5), so this test has no GCC oracle.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#ifdef __KILN__
#include "kiln_meta_queries.h"
#else
#include <meta>
#endif
using namespace std::meta;

constexpr auto all = access_context::unchecked();

namespace ns {
template <class T> struct box { box(T) {} };
box(const char *) -> box<int>;
struct after {};
}  // namespace ns
static_assert(members_of(^^ns, all).size() == 2);
static_assert(members_of(^^ns, all)[0] == ^^ns::box);
static_assert(members_of(^^ns, all)[1] == ^^ns::after);

struct S {
  template <class T> struct in { in(T) {} };
  in(const char *) -> in<int>;
};
// in, then the six implicit special members.
static_assert(members_of(^^S, all).size() == 7);
static_assert(members_of(^^S, all)[0] == ^^S::in);
