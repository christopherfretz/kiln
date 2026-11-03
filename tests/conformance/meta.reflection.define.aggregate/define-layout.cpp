// [meta.reflection.define.aggregate]/8-10 (P2996R13): the defined class
// has one non-static data member per description, in order, each public,
// with the alignment, bit width and [[no_unique_address]] it describes;
// it is complete after the call and an aggregate.
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

struct A;
consteval {
  define_aggregate(^^A, {data_member_spec(^^char, {.name = "c"}),
                         data_member_spec(^^int, {.name = "i", .alignment = 16}),
                         data_member_spec(^^unsigned, {.bit_width = 0}),
                         data_member_spec(^^unsigned, {.bit_width = 0}),
                         data_member_spec(^^unsigned, {.name = "b", .bit_width = 3}),
                         data_member_spec(^^unsigned, {.bit_width = 5}),
                         data_member_spec(^^unsigned, {.name = "d", .bit_width = 4})});
}
static_assert(alignof(A) == 16 && sizeof(A) == 32);
static_assert(__builtin_offsetof(A, i) == 16);
constexpr A a{'x', 7, 5, 9};
static_assert(a.c == 'x' && a.i == 7 && a.b == 5 && a.d == 9);
static_assert(__is_aggregate(A) && __is_standard_layout(A));

// An empty member with [[no_unique_address]] takes no storage.
struct Empty {};
struct N;
consteval {
  define_aggregate(^^N, {data_member_spec(^^int, {.name = "x"}),
                         data_member_spec(^^Empty, {.name = "e", .no_unique_address = true})});
}
static_assert(sizeof(N) == sizeof(int));

// Reference and const members; a member named like the class.
int g = 3;
struct R;
consteval {
  define_aggregate(^^R, {data_member_spec(^^int&, {.name = "r"}),
                         data_member_spec(^^const long, {.name = "k"}),
                         data_member_spec(^^int, {.name = "R"})});
}
constexpr R r{g, 4, 5};
static_assert(&r.r == &g && r.k == 4 && r.R == 5);

// No members: an empty aggregate.
struct E;
consteval { define_aggregate(^^E, {}); }
static_assert(sizeof(E) == 1 && __is_empty(E));

// A built-in array and an initializer_list variable as the range; a
// reference to an incomplete type and a member of an earlier defined class.
struct Inc;
struct C;
consteval {
  info specs[] = {data_member_spec(^^A, {.name = "a"}), data_member_spec(^^Inc&, {.name = "p"})};
  define_aggregate(^^C, specs);
}
static_assert(sizeof(C) == 48);
struct D;
consteval {
  std::initializer_list<info> l = {data_member_spec(^^C*, {.name = "c"})};
  define_aggregate(^^D, l);
}
static_assert(sizeof(D) == sizeof(void*));

// Class template specializations: explicit, implicit (not yet
// instantiated), and a member class of a class template defined from a
// consteval block in the template.
template <class T> struct Y;
template <> struct Y<int>;
consteval { define_aggregate(^^Y<int>, {data_member_spec(^^int, {.name = "v"})}); }
consteval { define_aggregate(^^Y<char>, {data_member_spec(^^char, {.name = "w"})}); }
static_assert(Y<int>{3}.v == 3 && Y<char>{'q'}.w == 'q');
template <class T> struct Outer {
  struct Inner;
  consteval { define_aggregate(^^Inner, {data_member_spec(^^T, {.name = "t"})}); }
};
static_assert(sizeof(Outer<double>::Inner) == sizeof(double));
static_assert(Outer<short>::Inner{9}.t == 9);

// A local class defined from a consteval block in the same block scope.
consteval int local() {
  struct L;
  consteval { define_aggregate(^^L, {data_member_spec(^^int, {.name = "n"})}); }
  return L{4}.n;
}
static_assert(local() == 4);
