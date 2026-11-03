// [meta.reflection.exception] (P3560R2): a member, access, layout or
// annotation query that is not a constant subexpression throws a
// std::meta::exception, which a consteval function can catch; from() is
// the query that threw.
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
struct Inc;
consteval bool caught(info r) {
  try {
    (void)members_of(r, access_context::unchecked());
  } catch (const std::meta::exception& e) {
    return e.from() == ^^members_of;
  }
  return false;
}
static_assert(caught(^^Inc));
static_assert(caught(^^int));
static_assert(!caught(^^::));

consteval bool layout() {
  try {
    (void)offset_of(^^int);
  } catch (const std::meta::exception& e) {
    return e.from() == ^^offset_of;
  }
  return false;
}
static_assert(layout());

consteval int count_throws() {
  int n = 0;
  info bad[] = {^^int, ^^Inc, info()};
  for (info r : bad) {
    try {
      (void)has_inaccessible_bases(r, access_context::current());
    } catch (const std::meta::exception&) {
      ++n;
    }
    try {
      (void)access_context::current().via(r);
    } catch (const std::meta::exception&) {
      ++n;
    }
    try {
      (void)annotations_of(r);
    } catch (const std::meta::exception&) {
      ++n;
    }
  }
  return n;
}
// via(info()) and annotations_of of a type do not throw.
static_assert(count_throws() == 6);

int main() {}
