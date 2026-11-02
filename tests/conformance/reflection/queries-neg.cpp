// [meta.reflection.queries] P2996R13: a query whose operand does not
// designate what it requires is not a constant expression (the spike's
// notes name the operand).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 19:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:15 note "member index 3 is out of range for a class with 1 members"
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 20:15 note "'^^S' has no type"
//@ diag: 21:15 error "static assertion expression is not an integral constant expression"
//@ diag: 21:15 note "'std::meta::info()' has no name"
//@ diag: 23:15 error "static assertion expression is not an integral constant expression"
//@ diag: 23:15 note "'^^Inc' reflects an incomplete class"
#include "kiln_meta.h"
using namespace std::meta;
struct S { int a; };
static_assert(__kiln_member_at(^^S, 3) == ^^S::a);
static_assert(type_of(^^S) == ^^int);
static_assert(name_of(info{}) != nullptr);
struct Inc;
static_assert(__kiln_members_count(^^Inc) == 0);
