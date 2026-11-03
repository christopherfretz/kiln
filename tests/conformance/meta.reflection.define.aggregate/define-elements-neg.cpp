// [meta.reflection.define.aggregate]/7.4-7.5 (P2996R13; spec/reflection.md
// §6.2): every element of the range is a data member description whose
// type is complete (a reference may refer to an incomplete type; an array
// of unknown bound and the class itself are incomplete), and no two
// descriptions share a name (`_` and unnamed bit-fields may repeat). No
// member is of abstract class type ([class.mem.general]).
//@ expect: compile-fail
//@ args: -ferror-limit=0 -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 42:1 error "consteval block is not a constant expression"
//@ diag: 42:13 note "element 0 ('^^int') is not a data member description"
//@ diag: 42:1 note "in call to"
//@ diag: 43:1 error "consteval block is not a constant expression"
//@ diag: 43:13 note "element 1 ('std::meta::info()') is not a data member description"
//@ diag: 43:1 note "in call to"
//@ diag: 44:1 error "consteval block is not a constant expression"
//@ diag: 44:13 note "type 'Inc' of data member description 0 is incomplete"
//@ diag: 44:1 note "in call to"
//@ diag: 45:1 error "consteval block is not a constant expression"
//@ diag: 45:13 note "type 'int[]' of data member description 0 is incomplete"
//@ diag: 45:1 note "in call to"
//@ diag: 46:1 error "consteval block is not a constant expression"
//@ diag: 46:13 note "type 'A' of data member description 0 is incomplete"
//@ diag: 46:1 note "in call to"
//@ diag: 47:1 error "consteval block is not a constant expression"
//@ diag: 48:3 note "name 'a' used in multiple data member descriptions"
//@ diag: 47:1 note "in call to"
//@ diag: 51:1 error "consteval block is not a constant expression"
//@ diag: 52:3 note "name 'a' used in multiple data member descriptions"
//@ diag: 51:1 note "in call to"
//@ diag: 58:1 error "consteval block is not a constant expression"
//@ diag: 58:13 note "type 'Abs' of data member description 0 is an abstract class"
//@ diag: 58:1 note "in call to"
//@ diag: 60:1 error "consteval block is not a constant expression"
//@ diag: 61:3 note "name 'b' used in multiple data member descriptions"
//@ diag: 60:1 note "in call to"
#include "kiln_meta.h"
using namespace std::meta;
struct Inc;
struct A;
consteval { define_aggregate(^^A, {^^int}); }
consteval { define_aggregate(^^A, {data_member_spec(^^int, {.name = "a"}), info{}}); }
consteval { define_aggregate(^^A, {data_member_spec(^^Inc, {.name = "a"})}); }
consteval { define_aggregate(^^A, {data_member_spec(^^int[], {.name = "a"})}); }
consteval { define_aggregate(^^A, {data_member_spec(^^A, {.name = "a"})}); }
consteval {
  define_aggregate(^^A, {data_member_spec(^^int, {.name = "a"}),
                         data_member_spec(^^long, {.name = "a"})});
}
consteval {
  define_aggregate(^^A, {data_member_spec(^^int, {.name = "a"}),
                         data_member_spec(^^int, {.name = u8"a"})});
}
struct Abs {
  virtual void f() = 0;
};
consteval { define_aggregate(^^A, {data_member_spec(^^Abs, {.name = "a"})}); }
// The first failing element is reported.
consteval {
  define_aggregate(^^A, {data_member_spec(^^int, {.name = "b"}), data_member_spec(^^int, {.name = "b"}),
                         ^^int});
}
// None of these defined A; this one does.
consteval {
  define_aggregate(^^A, {data_member_spec(^^int, {.name = "_"}), data_member_spec(^^int, {.name = "_"}),
                         data_member_spec(^^int, {.bit_width = 0}), data_member_spec(^^int, {.bit_width = 0}),
                         data_member_spec(^^Inc&, {.name = "r"})});
}
static_assert(sizeof(A) == 2 * sizeof(int) + sizeof(void*));
