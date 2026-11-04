// [meta.reflection.member.queries]/2 (P2996R13): a function is
// members-of-representable only if its constraints (if any) are
// satisfied, so members_of a class template specialization omits a member
// whose trailing requires-clause fails for its arguments (GCC 16.1 agrees).
// An iterator template constraining base() to its reverse directions is
// then bound without base() for the forward ones.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

template <bool Forward> struct It {
  int base()
    requires(!Forward)
  {
    return 0;
  }
  int pos()
    requires(Forward)
  {
    return 1;
  }
  int both() { return 2; }
};

consteval bool has(info cls, const char* name) {
  for (info m : members_of(cls, access_context::unchecked())) {
    if (!has_identifier(m)) continue;
    std::string_view s = identifier_of(m);
    decltype(sizeof 0) n = 0;
    while (name[n] && n < s.size() && s.data()[n] == name[n]) ++n;
    if (!name[n] && n == s.size()) return true;
  }
  return false;
}
static_assert(!has(^^It<true>, "base") && has(^^It<true>, "pos") && has(^^It<true>, "both"));
static_assert(has(^^It<false>, "base") && !has(^^It<false>, "pos") && has(^^It<false>, "both"));

int main() {}
