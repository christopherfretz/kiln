// [meta.reflection.define.aggregate] P2996R13 at `-g -gdwarf-4` (WP-910):
// classes completed by define_aggregate, a plain one and a class template
// specialization, get the debug information of a class written out
// (members, offsets, bit-fields, the template's parameters), so the TU
// compiles and runs as without `-g` (spec/codegen.md §5.1).
//@ expect: run
//@ args: -freflection -I tests/support -g -gdwarf-4
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ oracle: none
//@ stdout: 1 2 3 4
#include "kiln_meta.h"
#include <cstdio>
using namespace std::meta;

struct S;
template <class T> struct Box;
consteval {
  define_aggregate(^^S, {data_member_spec(^^int, {.name = "a"}),
                         data_member_spec(^^unsigned, {.name = "b", .bit_width = 3}),
                         data_member_spec(^^double, {.name = "c"})});
  define_aggregate(^^Box<long>, {data_member_spec(^^long, {.name = "value"})});
}

int main() {
  S s{1, 2, 3.0};
  Box<long> b{4};
  std::printf("%d %u %g %ld\n", s.a, s.b, s.c, b.value);
}
