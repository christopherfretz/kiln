// [meta.reflection.queries] P2996R13: a query whose operand does not
// designate what it requires is not a constant expression (the spike's
// notes name the operand).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 18:15 error "static assertion expression is not an integral constant expression"
//@ diag: 18:15 note "neither complete class type nor namespace"
//@ diag: 19:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:15 note "'^^S' has no type"
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 20:15 note "'std::meta::info()' has no name"
#include "kiln_meta.h"
using namespace std::meta;
struct S { int a; };
struct Inc;
static_assert(members_of(^^Inc, access_context::unchecked()).size() == 0);
static_assert(type_of(^^S) == ^^int);
static_assert(name_of(info{}) != nullptr);
