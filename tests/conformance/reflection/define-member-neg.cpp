// [meta.reflection.define.aggregate] P2996R13 (spike vehicle
// `__kiln_define_member`): outside the member-specification of the class
// being defined, or with an operand it cannot use, the call is not a
// constant expression. A duplicate member ([class.mem]/5) points at the call,
// the previous declaration (and where it was injected), and the class. A
// second identical `add_x(^^S)` is evaluated again: a defining call is never memoized.
//@ expect: compile-fail
//@ args: -Wno-kiln-extension -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 52:17 error "static assertion expression is not an integral constant expression"
//@ diag: 48:39 note "duplicate member 'x'"
//@ diag: 52:17 note "in call to 'add_x(^^S)'"
//@ diag: 48:39 note "previous declaration is here"
//@ diag: 51:3 note "member injected here"
//@ diag: 49:8 note "in evaluation of '__kiln_define_member' for class 'S' being defined here"
//@ diag: 53:17 error "static assertion expression is not an integral constant expression"
//@ diag: 53:17 note "duplicate member 'a'"
//@ diag: 50:7 note "previous declaration is here"
//@ diag: 49:8 note "in evaluation of '__kiln_define_member' for class 'S' being defined here"
//@ diag: 55:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:15 note "'__kiln_define_member' can only add a member to the class whose member-specification is being evaluated ('S' is not)"
//@ diag: 58:19 error "static assertion expression is not an integral constant expression"
//@ diag: 58:19 note "'__kiln_define_member' can only add a member to the class whose member-specification is being evaluated ('O' is not)"
//@ diag: 57:10 note "class being defined is 'I'"
//@ diag: 63:17 error "static assertion expression is not an integral constant expression"
//@ diag: 63:17 note "'^^int' does not reflect a class type"
//@ diag: 64:17 error "static assertion expression is not an integral constant expression"
//@ diag: 64:17 note "'^^void' is not a complete object type for a data member"
//@ diag: 65:17 error "static assertion expression is not an integral constant expression"
//@ diag: 65:17 note "'^^int &' is not a complete object type for a data member"
//@ diag: 66:17 error "static assertion expression is not an integral constant expression"
//@ diag: 66:17 note "'^^B' is not a complete object type for a data member"
//@ diag: 67:17 error "static assertion expression is not an integral constant expression"
//@ diag: 67:17 note "'^^Inc' is not a complete object type for a data member"
//@ diag: 68:17 error "static assertion expression is not an integral constant expression"
//@ diag: 68:17 note "'^^S::a' does not reflect a type"
//@ diag: 69:17 error "static assertion expression is not an integral constant expression"
//@ diag: 69:17 note "'1z' is not an identifier"
//@ diag: 70:17 error "static assertion expression is not an integral constant expression"
//@ diag: 70:17 note "'' is not an identifier"
//@ diag: 72:17 error "static assertion expression is not an integral constant expression"
//@ diag: 72:17 note "duplicate member 'f'"
//@ diag: 71:7 note "previous declaration is here"
//@ diag: 62:8 note "in evaluation of '__kiln_define_member' for class 'B' being defined here"
#include "kiln_meta.h"
using namespace std::meta;
consteval info add_x(info c) { return __kiln_define_member(c, ^^int, "x"); }
struct S {
  int a;
  static_assert(add_x(^^S) != info{});
  static_assert(add_x(^^S) != info{});
  static_assert(__kiln_define_member(^^S, ^^long, "a") != info{});
};
static_assert(__kiln_define_member(^^S, ^^int, "z") != info{});
struct O {
  struct I {
    static_assert(__kiln_define_member(^^O, ^^int, "z") != info{});
  };
};
struct Inc;
struct B {
  static_assert(__kiln_define_member(^^int, ^^int, "z") != info{});
  static_assert(__kiln_define_member(^^B, ^^void, "z") != info{});
  static_assert(__kiln_define_member(^^B, ^^int&, "z") != info{});
  static_assert(__kiln_define_member(^^B, ^^B, "z") != info{});
  static_assert(__kiln_define_member(^^B, ^^Inc, "z") != info{});
  static_assert(__kiln_define_member(^^B, ^^S::a, "z") != info{});
  static_assert(__kiln_define_member(^^B, ^^int, "1z") != info{});
  static_assert(__kiln_define_member(^^B, ^^int, "") != info{});
  int f(int);
  static_assert(__kiln_define_member(^^B, ^^int, "f") != info{});
};
