// [meta.reflection.define.aggregate]/8 (P2996R13), [stmt.expand]: the
// members of a defined class are found by members_of in declaration
// order, with their names and types, and an expansion statement iterates
// them.
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

struct S;
consteval {
  define_aggregate(^^S, {data_member_spec(^^int, {.name = "a"}),
                         data_member_spec(^^long, {.name = "bb"}),
                         data_member_spec(^^char, {.name = "ccc", .bit_width = 3})});
}
constexpr auto members = members_of<^^S>();
static_assert(members.size() == 3);
static_assert(name_is(members[0], "a") && name_is(members[2], "ccc"));
static_assert(type_of(members[1]) == ^^long);
consteval unsigned long total() {
  unsigned long t = 0;
  template for (constexpr info m : members.items) {
    t = t * 10 + identifier_of(m).size();
  }
  return t;
}
static_assert(total() == 123);
