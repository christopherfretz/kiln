// [expr.reflect]/5 P2996R13: a template-name represents the template:
// class, alias, variable and function templates and concepts; a
// template template parameter represents its argument.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;
template <class T> struct TC {};
template <class T> using TA = TC<T>;
template <class T> int tv = 0;
template <class T> void tf() {}
template <class T> concept C = true;
static_assert(^^TC == ^^TC);
static_assert(^^TC != ^^TA);
static_assert(^^TA != ^^tv);
static_assert(^^tv != ^^tf);
static_assert(^^tf != ^^C);
static_assert(^^C == ^^C);
static_assert(^^::TC == ^^TC);
static_assert(name_is(^^TC, "TC"));
static_assert(name_is(^^C, "C"));
template <template <class> class TT> consteval info arg() { return ^^TT; }
static_assert(arg<TC>() == ^^TC);
static_assert(arg<TA>() == ^^TA);
namespace N { template <class T> struct X {}; }
static_assert(^^N::X != ^^TC);
