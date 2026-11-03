// [meta.reflection.annotation] (P3394R4): annotations_of needs a type,
// type alias, variable, function, function parameter, namespace,
// enumerator, direct base class relationship or non-static data member;
// annotations_of_with_type also needs a complete type or type alias as
// its second argument. Each failure throws std::meta::exception.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 38:15 error "static assertion expression is not an integral constant expression"
//@ diag: 38:15 note "reflection does not represent a type, type alias, variable, function, function parameter, namespace, enumerator, direct base class relationship, or non-static data member"
//@ diag: 39:15 error "static assertion expression is not an integral constant expression"
//@ diag: 39:15 note "reflection does not represent a type, type alias, variable, function, function parameter, namespace, enumerator, direct base class relationship, or non-static data member"
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 40:15 note "reflection does not represent a type, type alias, variable, function, function parameter, namespace, enumerator, direct base class relationship, or non-static data member"
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 41:15 note "reflection does not represent a type, type alias, variable, function, function parameter, namespace, enumerator, direct base class relationship, or non-static data member"
//@ diag: 42:15 error "static assertion expression is not an integral constant expression"
//@ diag: 42:15 note "reflection does not represent a complete type or type alias"
//@ diag: 43:15 error "static assertion expression is not an integral constant expression"
//@ diag: 43:15 note "reflection does not represent a complete type or type alias"
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:15 note "reflection does not represent a complete type or type alias"
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:15 note "reflection does not represent a type, type alias, variable, function, function parameter, namespace, enumerator, direct base class relationship, or non-static data member"
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;
template <class T> struct Tm {};
template <class T> concept Co = true;
struct [[=1]] S {};
struct Inc;
int v;
static_assert(annotations_of(info()).size() == 0);
static_assert(annotations_of(^^Tm).size() == 0);
static_assert(annotations_of(^^Co).size() == 0);
static_assert(annotations_of(annotations_of(^^S)[0]).size() == 0);
static_assert(annotations_of_with_type(^^S, ^^v).size() == 0);
static_assert(annotations_of_with_type(^^S, ^^Inc).size() == 0);
static_assert(annotations_of_with_type(^^S, info()).size() == 0);
static_assert(annotations_of_with_type(^^Tm, ^^int).size() == 0);
