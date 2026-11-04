// [meta.reflection.substitute] (P2996R13), the template_of /
// template_arguments_of example: a partial specialization's template is the
// primary, an alias template specialization keeps the alias, and constant
// template arguments are value (int) or object (int&, S) reflections.
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
#ifdef __KILN__
#include "kiln_meta_queries.h"
#else
#include <meta>
#endif
using namespace std::meta;

template<class T, class U = T> struct Pair { };
template<class T> struct Pair<char, T> { };
template<class T> using PairPtr = Pair<T*>;

static_assert(template_of(^^Pair<int>) == ^^Pair);
static_assert(template_of(^^Pair<char, char>) == ^^Pair);
static_assert(template_arguments_of(^^Pair<int>).size() == 2);
static_assert(template_arguments_of(^^Pair<int>)[0] == ^^int);

static_assert(template_of(^^PairPtr<int>) == ^^PairPtr);
static_assert(template_arguments_of(^^PairPtr<int>).size() == 1);

struct S { };
int i;
template<int, int&, S, template<class> class>
  struct X { };
constexpr auto T = ^^X<1, i, S{}, PairPtr>;
static_assert(is_value(template_arguments_of(T)[0]));
static_assert(is_object(template_arguments_of(T)[1]));
static_assert(is_object(template_arguments_of(T)[2]));
static_assert(template_arguments_of(T)[3] == ^^PairPtr);

int main() {}
