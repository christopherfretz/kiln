// [expr.reflect]/7, [over.over]/3, [temp.arg.explicit]/4 (P2996R13): a
// function template-id whose remaining template parameters all have
// default arguments (or are deduced from nothing) names exactly one
// specialization, which `^^` reflects, for a namespace-scope and a member
// function template alike (no object is needed). With several viable
// templates it is an overload set (expr.reflect/rejected-names.cpp).
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta_queries.h"
#endif
using std::meta::info;
template <class T, class U = long> void tf(T, U = U()) {}
struct S { template <class T, int N = 3> int mt(T) const { return N; } };

constexpr info a = ^^tf<int>;
static_assert(a == ^^tf<int, long>);
static_assert(a != ^^tf<int, int>);
static_assert(std::meta::is_function(a) && !std::meta::is_function_template(a));
static_assert(std::meta::template_of(a) == ^^tf);
static_assert(std::meta::type_of(a) == ^^void(int, long));

constexpr info m = ^^S::mt<char>;
static_assert(m == ^^S::mt<char, 3>);
static_assert(std::meta::parent_of(m) == ^^S);
static_assert(std::meta::template_of(m) == ^^S::mt);
