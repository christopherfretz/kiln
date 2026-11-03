// [class.bit]/1 (P2996R13 [meta.reflection.define.aggregate]/8): a
// described bit-field wider than its type has padding bits, laid out as a
// declared one; kiln warns as for a declared one (-Wbitfield-width, not
// for `bool`), at the define_aggregate call.
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 16:3 warning "width of bit-field 'c' (9 bits) exceeds the width of its type; value will be truncated to 8 bits"
//@ diag: 21:8 warning "width of bit-field 'c' (9 bits) exceeds the width of its type; value will be truncated to 8 bits"
#include "kiln_meta.h"
using namespace std::meta;
enum class E : unsigned char { a, b };
struct A;
consteval {
  define_aggregate(^^A, {data_member_spec(^^char, {.name = "c", .bit_width = 9}),
                         data_member_spec(^^E, {.name = "e", .bit_width = 2}),
                         data_member_spec(^^bool, {.name = "b", .bit_width = 3})});
}
struct A2 {
  char c : 9;  // (the same layout)
  E e : 2;
  bool b : 3;
};
static_assert(sizeof(A) == sizeof(A2) && alignof(A) == alignof(A2));
constexpr A a{'x', E::b, true};
static_assert(a.c == 'x' && a.e == E::b && a.b);
