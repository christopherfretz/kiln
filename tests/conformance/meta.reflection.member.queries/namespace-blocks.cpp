// [meta.reflection.member.queries]/1 (P2996R13): members_of a namespace is
// in declaration order across linkage specifications (nested ones too):
// the members of an `extern "C"` block come where the block
// is, not after the rest of the enclosing definition (GCC 16.1 agrees). A
// query from inside an open block sees the block so far, and a later query
// sees members added to it afterwards in place. A variable being declared
// is already a member of its own initializer's query.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

constexpr auto all = access_context::unchecked();
consteval unsigned count(info ns) { return members_of(ns, all).size(); }

namespace N {
int a;
extern "C" {
int b;
extern "C++" {
int c;
}
int d;
}
int e;
extern "C" int f;
constexpr auto mid = count(^^N);
extern "C" {
int g;
constexpr auto in_block = count(^^N);
int h;
}
int i;
} // namespace N

namespace N {
extern "C" int j;
int k;
} // namespace N

static_assert(N::mid == 7);
static_assert(N::in_block == 9);

consteval bool order() {
  auto ms = members_of(^^N, all);
  info want[] = {^^N::a, ^^N::b, ^^N::c, ^^N::d, ^^N::e, ^^N::f, ^^N::mid, ^^N::g,
                 ^^N::in_block, ^^N::h, ^^N::i, ^^N::j, ^^N::k};
  if (ms.size() != sizeof want / sizeof want[0])
    return false;
  for (unsigned n = 0; n < ms.size(); ++n)
    if (ms[n] != want[n])
      return false;
  return true;
}
static_assert(order());
