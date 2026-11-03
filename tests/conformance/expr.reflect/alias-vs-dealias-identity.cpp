// [expr.reflect]/6 P2996R13: `^^` of a type-id that names a type alias
// represents the alias (a typedef-name or an alias template
// specialization), not the type it denotes; anything else written with
// an alias (cv-qualified, a pointer to it) is the underlying type. Each
// alias declaration is its own entity, also when it aliases an alias
// (GCC 16.1 dealiases that second level; kiln keeps it, as
// identifier_of(^^J) == "J" requires).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
using I = int;
typedef int TI;
using J = I;
template <class T> struct TC {};
template <class T> using TA = TC<T>;
static_assert(^^I != ^^int);
static_assert(^^TI != ^^int);
static_assert(^^TI != ^^I);
static_assert(^^J != ^^I);
static_assert(^^J != ^^int);
static_assert(^^I == ^^I);
static_assert(^^::I == ^^I);
static_assert(^^const I == ^^const int);
static_assert(^^I* == ^^int*);
static_assert(^^I& == ^^int&);
static_assert(^^TA<int> != ^^TC<int>);
static_assert(^^TA<I> == ^^TA<int>);
static_assert(name_is(^^I, "I"));
static_assert(name_is(^^J, "J"));
template <class T> struct Holder { using type = T; static constexpr info r = ^^type; };
static_assert(Holder<int>::r != ^^int);
static_assert(Holder<int>::r != Holder<long>::r);
