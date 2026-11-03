// [meta.reflection.define.aggregate]/1-6 (P2996R13): data_member_spec
// reflects a data member description (T, N, A, W, NUA); two calls with
// the same description give the same reflection (/4), whether the name is
// an ordinary or a UTF-8 string; is_data_member_spec tests the kind.
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

constexpr info a = data_member_spec(^^int, {.name = "a"});
static_assert(is_data_member_spec(a));
static_assert(!is_data_member_spec(^^int));
static_assert(!is_data_member_spec(info{}));
static_assert(a == data_member_spec(^^int, {.name = "a"}));
static_assert(a == data_member_spec(^^int, {.name = u8"a"}));
static_assert(a == data_member_spec(^^int, {.name = std::string_view("ab", 1)}));
static_assert(a != data_member_spec(^^int, {.name = "b"}));
static_assert(a != data_member_spec(^^long, {.name = "a"}));
static_assert(a != data_member_spec(^^const int, {.name = "a"}));
static_assert(a != data_member_spec(^^int, {.name = "a", .alignment = 4}));
static_assert(a != data_member_spec(^^int, {.name = "a", .bit_width = 4}));
static_assert(a != data_member_spec(^^int, {.name = "a", .no_unique_address = true}));
static_assert(a != ^^int);
// Unnamed bit-fields, of width 0 too.
static_assert(data_member_spec(^^int, {.bit_width = 0}) ==
              data_member_spec(^^int, {.bit_width = 0}));
static_assert(data_member_spec(^^int, {.bit_width = 0}) !=
              data_member_spec(^^int, {.bit_width = 1}));
// Identifiers ([lex.name]): `_`, reserved ones, contextual keywords, UTF-8.
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "_"})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "__x"})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = "final"})));
static_assert(is_data_member_spec(data_member_spec(^^int, {.name = u8"ñ"})));
// Reference, const, array and class types; an incomplete type (the
// alignment is checked against alignof only for a complete type).
struct Inc;
static_assert(is_data_member_spec(data_member_spec(^^int&, {.name = "r"})));
static_assert(is_data_member_spec(data_member_spec(^^int[3], {.name = "v"})));
static_assert(is_data_member_spec(data_member_spec(^^Inc, {.name = "i", .alignment = 1})));
static_assert(is_data_member_spec(data_member_spec(^^char&, {.name = "r", .alignment = 1})));
// Usable in a constexpr variable's initializer and a consteval function.
consteval bool same() {
  info x = data_member_spec(^^int, {.name = "a"});
  return x == a;
}
static_assert(same());
