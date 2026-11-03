// [expr.reflect]/5,6 P2996R13: a template-id naming a class or alias
// specialization is a type-id (the alias specialization is kept); one
// naming a function or variable specialization represents that
// specialization, not its template.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
template <class T> struct TC {};
template <class T> using TA = TC<T>;
template <class T> int tv = 0;
template <class T> void tf() {}
static_assert(^^TC<int> == ^^TC<int>);
static_assert(^^TC<int> != ^^TC<long>);
static_assert(^^TC<int> != ^^TC);
static_assert(^^TA<int> != ^^TC<int>);
static_assert(^^TA<int> == ^^TA<int>);
static_assert(^^tv<int> != ^^tv);
static_assert(^^tv<int> != ^^tv<long>);
static_assert(^^tf<int> != ^^tf);
static_assert(^^tf<int> == ^^tf<int>);
static_assert(^^tf<int> != ^^tf<long>);
using TCI = TC<int>;
static_assert(^^TCI != ^^TC<int>);
