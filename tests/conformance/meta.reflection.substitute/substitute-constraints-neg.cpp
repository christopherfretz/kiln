// [meta.reflection.substitute] (P2996R13): substitute with arguments that do
// not satisfy the template's constraints throws; can_substitute is false for
// them.
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 27:20 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 27:20 note "'^^R' cannot be substituted with these template arguments: constraints not satisfied (constraints not satisfied for class template 'R' [with T = char])"
//@ diag: 27:16 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 27:20 note "'^^R' cannot be substituted with these template arguments: constraints not satisfied (constraints not satisfied for class template 'R' [with T = char])"
//@ diag: 28:20 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 28:20 note "'^^v' cannot be substituted with these template arguments: constraints not satisfied (constraints not satisfied for variable template 'v' [with T = char])"
//@ diag: 28:16 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 28:20 note "'^^v' cannot be substituted with these template arguments: constraints not satisfied (constraints not satisfied for variable template 'v' [with T = char])"
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using namespace std::meta;
template <class T> requires(sizeof(T) > 1) struct R {};
template <class T> concept Big = sizeof(T) > 1;
template <Big T> constexpr int v = 0;
static_assert(!can_substitute(^^R, {^^char}));
constexpr info a = substitute(^^R, {^^char});
constexpr info b = substitute(^^v, {^^char});
int main() {}
