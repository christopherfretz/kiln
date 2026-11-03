// [meta.reflection.define.aggregate]/5 (P2996R13), [meta.reflection.exception]:
// a data_member_spec precondition failure throws a std::meta::exception
// (from() is ^^data_member_spec), which a consteval function catches;
// define_aggregate's failures are not exceptions (define-plainly-neg.cpp).
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

consteval bool throws(info type, data_member_options o) {
  try {
    (void)data_member_spec(type, o);
  } catch (const std::meta::exception& e) {
    return e.from() == ^^data_member_spec;
  }
  return false;
}
static_assert(throws(^^void, {.name = "a"}));
static_assert(throws(^^int, {.name = "while"}));
static_assert(throws(^^int, {}));
static_assert(throws(^^int, {.name = "a", .alignment = 3}));
static_assert(throws(^^int, {.name = "a", .bit_width = 0}));
static_assert(!throws(^^int, {.name = "a"}));
static_assert(!throws(^^int, {.bit_width = 0}));
