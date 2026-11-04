// [meta.reflection.annotation] (P3394R4): admitting namespace aliases
// leaves the other unsuitable reflections unsuitable: annotations_of and
// annotations_of_with_type of a template or a value still throw
// std::meta::exception.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 23:15 error "static assertion expression is not an integral constant expression"
//@ diag: 23:15 note "reflection does not represent a type, type alias, variable, function, function parameter, namespace, enumerator, direct base class relationship, or non-static data member"
//@ diag: 24:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:15 note "reflection does not represent a type, type alias, variable, function, function parameter, namespace, enumerator, direct base class relationship, or non-static data member"
//@ diag: 25:15 error "static assertion expression is not an integral constant expression"
//@ diag: 25:15 note "reflection does not represent a type, type alias, variable, function, function parameter, namespace, enumerator, direct base class relationship, or non-static data member"
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;
template <class T> struct Tm {};
static_assert(annotations_of(reflect_constant(1)).size() == 0);
static_assert(annotations_of_with_type(^^Tm, ^^int).size() == 0);
static_assert(annotations_of_with_type(reflect_constant(1), ^^int).size() == 0);
namespace inner {}
namespace alias = inner;
static_assert(annotations_of(^^alias).size() == 0);
