// [meta.reflection.substitute] (P2996R13): for a function template, substitute
// throws unless the arguments name a unique specialization: too few arguments
// to deduce from, or a substitution failure in its type. A specialization
// whose return type is an undeduced placeholder is not one either: with no
// reachable definition to instantiate, its return type cannot be deduced
// (the standard's fn1; GCC 16.1 agrees). With a definition, substitute
// instantiates it and deduces (is-substitute-fn2.cpp, WP-1674).
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 36:20 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 36:20 note "'^^h' cannot be substituted with these template arguments: the arguments do not name a unique function template specialization"
//@ diag: 36:16 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 36:20 note "'^^h' cannot be substituted with these template arguments: the arguments do not name a unique function template specialization"
//@ diag: 37:20 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 37:20 note "'^^k' cannot be substituted with these template arguments: the arguments do not name a unique function template specialization"
//@ diag: 37:16 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 37:20 note "'^^k' cannot be substituted with these template arguments: the arguments do not name a unique function template specialization"
//@ diag: 38:20 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 38:20 note "'^^ret' cannot be substituted with these template arguments: the specialization's type contains an undeduced placeholder type"
//@ diag: 38:16 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 38:20 note "'^^ret' cannot be substituted with these template arguments: the specialization's type contains an undeduced placeholder type"
//@ meta-header: skip the note spells the real header's default template argument initializer_list<info>
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using namespace std::meta;
template <class T, class U> void h(T, U) {}
template <class T> void k(T, typename T::type) {}
template <class T> auto ret(T t);
static_assert(!can_substitute(^^h, {^^int}));
constexpr info a = substitute(^^h, {^^int});
constexpr info b = substitute(^^k, {^^int});
constexpr info c = substitute(^^ret, {^^int});
int main() {}
