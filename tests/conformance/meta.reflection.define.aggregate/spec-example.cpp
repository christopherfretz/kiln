// [meta.reflection.define.aggregate]/11 (P2996R13): the IS's example. A
// union and a class template specialization are defined from consteval
// blocks; a name may be UTF-8 (u8 and ordinary names give the same
// description, /4).
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

union U;
consteval {
  define_aggregate(^^U, {
    data_member_spec(^^int, {.name="i"}),
    data_member_spec(^^char, {.name="c"}),
  });
}

// U is now defined to the equivalent of
// union U {
//   int i;
//   char c;
// };

template<typename T> struct S;
consteval {
  define_aggregate(^^S<int>, {
    data_member_spec(^^int, {.name="i", .alignment=64}),
    data_member_spec(^^int, {.name=u8"こんにち"}),
  });
}

// S<int> is now defined to the equivalent of
// template<> struct S<int> {
//   alignas(64) int i;
//               int こんにち;
// };

static_assert(sizeof(U) == sizeof(int));
static_assert(__builtin_offsetof(U, c) == 0);
static_assert(alignof(S<int>) == 64 && sizeof(S<int>) == 64);
static_assert(__builtin_offsetof(S<int>, こんにち) == sizeof(int));
constexpr U u{.i = 7};
static_assert(u.i == 7);
constexpr S<int> s{1, 2};
static_assert(s.i == 1 && s.こんにち == 2);
