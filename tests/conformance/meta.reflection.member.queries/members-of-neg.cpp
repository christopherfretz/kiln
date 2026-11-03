// [meta.reflection.member.queries] (P2996R13): the member queries' failure
// conditions. members_of needs a complete class type or a namespace;
// bases_of, static_data_members_of, nonstatic_data_members_of and
// subobjects_of a complete class type; enumerators_of an enumeration type
// whose enumerators are known (not an opaque declaration); parameters_of
// a function or function type. Each failure throws std::meta::exception
// (P3560R2), so the static_assert's condition is not constant; the note
// is the exception's message (GCC 16.1's what()).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 52:15 error "static assertion expression is not an integral constant expression"
//@ diag: 52:15 note "neither complete class type nor namespace"
//@ diag: 53:15 error "static assertion expression is not an integral constant expression"
//@ diag: 53:15 note "neither complete class type nor namespace"
//@ diag: 54:15 error "static assertion expression is not an integral constant expression"
//@ diag: 54:15 note "neither complete class type nor namespace"
//@ diag: 55:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:15 note "neither complete class type nor namespace"
//@ diag: 56:15 error "static assertion expression is not an integral constant expression"
//@ diag: 56:15 note "not a complete class type"
//@ diag: 57:15 error "static assertion expression is not an integral constant expression"
//@ diag: 57:15 note "not a complete class type"
//@ diag: 58:15 error "static assertion expression is not an integral constant expression"
//@ diag: 58:15 note "not a complete class type"
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 59:15 note "not a complete class type"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 60:15 note "not a complete class type"
//@ diag: 61:15 error "static assertion expression is not an integral constant expression"
//@ diag: 61:15 note "reflection does not represent an enumerable enumeration type"
//@ diag: 62:15 error "static assertion expression is not an integral constant expression"
//@ diag: 62:15 note "reflection does not represent an enumerable enumeration type"
//@ diag: 63:15 error "static assertion expression is not an integral constant expression"
//@ diag: 63:15 note "reflection does not represent a function or function type"
//@ diag: 64:15 error "static assertion expression is not an integral constant expression"
//@ diag: 64:15 note "reflection does not represent a function or function type"
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;
constexpr auto all = access_context::unchecked();
struct Inc;
enum Opaque : int;
struct S { int a; };
void f(int);
int v;
static_assert(members_of(^^Inc, all).size() == 0);
static_assert(members_of(^^int, all).size() == 0);
static_assert(members_of(^^f, all).size() == 0);
static_assert(members_of(info(), all).size() == 0);
static_assert(bases_of(^^Inc, all).size() == 0);
static_assert(bases_of(^^std, all).size() == 0);
static_assert(static_data_members_of(^^int, all).size() == 0);
static_assert(nonstatic_data_members_of(^^Inc, all).size() == 0);
static_assert(subobjects_of(^^v, all).size() == 0);
static_assert(enumerators_of(^^Opaque).size() == 0);
static_assert(enumerators_of(^^S).size() == 0);
static_assert(parameters_of(^^v).size() == 0);
static_assert(parameters_of(^^int).size() == 0);
