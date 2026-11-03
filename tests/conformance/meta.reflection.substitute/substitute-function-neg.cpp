// [meta.reflection.substitute] (P2996R13): for a function template,
// substitute throws unless the arguments name a unique specialization: too
// few arguments to deduce from, or a substitution failure in its type. A
// specialization whose return type is an undeduced placeholder is not one
// either: substitution does not instantiate the body to deduce it (GCC 16.1
// agrees).
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 34:20 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 34:20 note "'^^h' cannot be substituted with these template arguments: the arguments do not name a unique function template specialization"
//@ diag: 34:16 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 34:20 note "'^^h' cannot be substituted with these template arguments: the arguments do not name a unique function template specialization"
//@ diag: 35:20 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 35:20 note "'^^k' cannot be substituted with these template arguments: the arguments do not name a unique function template specialization"
//@ diag: 35:16 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 35:20 note "'^^k' cannot be substituted with these template arguments: the arguments do not name a unique function template specialization"
//@ diag: 36:20 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 36:20 note "'^^ret' cannot be substituted with these template arguments: the specialization's type contains an undeduced placeholder type"
//@ diag: 36:16 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 36:20 note "'^^ret' cannot be substituted with these template arguments: the specialization's type contains an undeduced placeholder type"
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using namespace std::meta;
template <class T, class U> void h(T, U) {}
template <class T> void k(T, typename T::type) {}
template <class T> auto ret(T t) { return t; }
static_assert(!can_substitute(^^h, {^^int}));
constexpr info a = substitute(^^h, {^^int});
constexpr info b = substitute(^^k, {^^int});
constexpr info c = substitute(^^ret, {^^int});
int main() {}
