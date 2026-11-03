//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 1 x 5 9 16 32
//@ stdout: 7 -3 2
//@ stdout: 11 12 1
//@ stdout: 4 8
// [meta.reflection.define.aggregate]/8-10 (P2996R13): code generation lays
// out, initializes and names a defined class like a declared one: its
// members (aligned, bit-fields) are read and written at run time, it is
// aggregate-initialized statically and dynamically, passed by value and
// by reference, and a function taking it is mangled with its name (a
// class template specialization's too).
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
using namespace std::meta;

struct A;
consteval {
  define_aggregate(^^A, {data_member_spec(^^char, {.name = "c"}),
                         data_member_spec(^^int, {.name = "i", .alignment = 16}),
                         data_member_spec(^^unsigned, {.bit_width = 0}),
                         data_member_spec(^^unsigned, {.name = "b", .bit_width = 3}),
                         data_member_spec(^^unsigned, {.name = "d", .bit_width = 4})});
}
A global{'x', 1, 5, 9};

template <class T> struct P;
consteval {
  define_aggregate(^^P<int>, {data_member_spec(^^int, {.name = "x"}),
                              data_member_spec(^^int, {.name = "y"}),
                              data_member_spec(^^int, {.name = "_"}),
                              data_member_spec(^^int, {.name = "_"})});
}
P<int> make(int a, int b) { return {a, b, 0, 0}; }
int sum(const P<int>& p) { return p.x + p.y; }

union U;
consteval {
  define_aggregate(^^U, {data_member_spec(^^int, {.name = "i"}),
                         data_member_spec(^^unsigned char, {.name = "c"})});
}
U pun(int v) { return {.i = v}; }

int main(int argc, char**) {
  printf("%d %c %u %u %zu %zu\n", global.i, global.c, global.b, global.d,
         __builtin_offsetof(A, i), sizeof(A));
  A local{'y', argc + 6, 0, 0};
  local.b = 7;
  local.d = 13;
  local.i -= 10;
  printf("%d %d %u\n", local.i + local.b - local.b + 10, local.i, local.d - 11);
  P<int> p = make(argc + 10, 12);
  printf("%d %d %d\n", p.x, p.y, sum(p) == 23);
  U u = pun(argc + 3);
  printf("%d %zu\n", u.i, sizeof(P<int>) / 2);
  return 0;
}
